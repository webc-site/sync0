use std::{
  fs::{File, read_to_string},
  io,
  path::{Path, PathBuf},
  sync::Arc,
  thread,
};

use parking_lot::Mutex;
use s0_cdc::{
  BasisIndex, CdcConfig, ChunkSig, FileAt, Hash128, ManifestEntry, chunk_file_range_with_buf,
  chunk_slice, hash128, join_io,
};

use crate::{
  RootBuf, ScannedFile,
  compio_io::{probe_batch_files, read_batch_files},
  pack::UNPACKED_OFFSET,
  par_chunk_plan, scan_files,
};

/// 发送端扫描期小文件内存池默认下限（128 MiB）、默认回退值（512 MiB）与自适应上限（2 GiB）
const MIN_SMALL_ARENA: usize = 128 << 20;
const DEFAULT_SMALL_ARENA: usize = 512 << 20;
const MAX_SMALL_ARENA: usize = 2 << 30;
/// 目录内跨文件去重扫描的基准总字节上限（128 MiB）
const MAX_DIR_BASIS_BYTES: u64 = 128 << 20;
/// 启用多线程并发预检/扫描的最小文件条目阈值
pub const PAR_SCAN_MIN_FILES: usize = 16;

/// 单块文件预检结果
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SinglePrecheck {
  Fallback,
  SameHash,
  NeedFull,
}

#[inline]
fn verify_file_hash(
  file: &File,
  size: usize,
  expected_hash: Hash128,
  io_buf: &mut Vec<u8>,
) -> io::Result<bool> {
  if io_buf.len() < size {
    io_buf.resize(size, 0);
  }
  file.read_exact_at_pos(&mut io_buf[..size], 0)?;
  Ok(hash128(&io_buf[..size]) == expected_hash)
}

/// 校验本地已存在文件与单块目标条目是否一致、需全量重传或需回退 CDC 细粒度匹配
pub fn check_single_chunk(
  basis_file: &File,
  basis_size: u64,
  target_size: u64,
  chunk_hash: Hash128,
  min_sz: u64,
  io_buf: &mut Vec<u8>,
) -> io::Result<SinglePrecheck> {
  if basis_size == target_size {
    Ok(
      if verify_file_hash(basis_file, target_size as usize, chunk_hash, io_buf)? {
        SinglePrecheck::SameHash
      } else {
        SinglePrecheck::NeedFull
      },
    )
  } else if basis_size < target_size || basis_size <= min_sz {
    Ok(SinglePrecheck::NeedFull)
  } else {
    Ok(SinglePrecheck::Fallback)
  }
}

/// 并发预检接收端目录中已存在的单块文件（利用 `compio` 异步批量探测与读取）
pub fn precheck_single_chunks<T, F>(
  out: &Path,
  entries: &[T],
  min_sz: u64,
  entry_info: F,
) -> io::Result<Vec<SinglePrecheck>>
where
  T: Sync,
  F: Fn(&T) -> (&str, u64, &[ChunkSig]) + Sync,
{
  let (n_threads, chunk_len) = par_chunk_plan(entries.len());
  let info_ref = &entry_info;
  Ok(thread::scope(|s| {
    let mut handles = Vec::with_capacity(n_threads);
    for slice in entries.chunks(chunk_len) {
      handles.push(s.spawn(move || {
        let mut v = vec![SinglePrecheck::Fallback; slice.len()];
        probe_batch_files(
          out,
          slice.len(),
          |i| {
            let (path, size, chunks) = info_ref(&slice[i]);
            (size > 0 && chunks.len() == 1).then_some((path, size as usize))
          },
          |i, probed| {
            let (_, size, chunks) = info_ref(&slice[i]);
            v[i] = match probed {
              Some((_, Some(data))) => {
                if hash128(data) == chunks[0].hash {
                  SinglePrecheck::SameHash
                } else {
                  SinglePrecheck::NeedFull
                }
              }
              Some((bs, None)) if bs < size || bs <= min_sz => SinglePrecheck::NeedFull,
              _ => SinglePrecheck::Fallback,
            };
          },
        );
        v
      }));
    }
    handles
      .into_iter()
      .flat_map(|h| h.join().unwrap_or_default())
      .collect()
  }))
}

/// 扫描接收端目录中不在清单同名列表内的本地已有文件，构建跨文件去重 `BasisIndex`（上限 `MAX_DIR_BASIS_BYTES`）
pub fn build_dir_basis<T, F>(
  out: &Path,
  entries: &[T],
  io_buf: &mut Vec<u8>,
  path_of: F,
) -> (BasisIndex, Vec<PathBuf>)
where
  F: Fn(&T) -> &str,
{
  let Ok(existing) = scan_files(out) else {
    return (BasisIndex::new(), Vec::new());
  };
  let mut root = RootBuf::new(out);
  let mut index = BasisIndex::new();
  let mut paths = Vec::new();
  let mut total_indexed = 0u64;
  let mut entry_cur = 0usize;
  for sf in existing {
    if sf.size == 0 {
      continue;
    }
    while entry_cur < entries.len() && path_of(&entries[entry_cur]) < sf.path.as_str() {
      entry_cur += 1;
    }
    if entry_cur < entries.len() && path_of(&entries[entry_cur]) == sf.path {
      continue;
    }
    if total_indexed + sf.size > MAX_DIR_BASIS_BYTES {
      break;
    }
    let p = root.join(&sf.path);
    if let Ok(f) = File::open(p) {
      let file_idx = paths.len() as u32;
      if index
        .insert_file_with_buf(file_idx, &f, sf.size, CdcConfig::DEFAULT, io_buf)
        .is_ok()
      {
        paths.push(p.to_path_buf());
        total_indexed += sf.size;
      }
    }
  }
  (index, paths)
}

fn max_small_arena() -> usize {
  if let Ok(s) = read_to_string("/proc/meminfo") {
    for line in s.lines() {
      if let Some(rest) = line.strip_prefix("MemAvailable:")
        && let Some(kb_str) = rest.split_whitespace().next()
        && let Ok(kb) = kb_str.parse::<usize>()
      {
        return (kb.saturating_mul(1024) / 3).clamp(MIN_SMALL_ARENA, MAX_SMALL_ARENA);
      }
    }
  }
  DEFAULT_SMALL_ARENA
}

static LAST_SMALL_CACHE: Mutex<Option<(PathBuf, Hash128, Arc<SmallCache>)>> = Mutex::new(None);

fn entries_cache_key<'a>(
  len: usize,
  iter: impl Iterator<Item = (&'a str, u64, &'a [ChunkSig])>,
) -> Hash128 {
  let mut buf = Vec::with_capacity(len * 24);
  for (path, size, chunks) in iter {
    buf.extend_from_slice(path.as_bytes());
    buf.extend_from_slice(&size.to_le_bytes());
    if let Some(first) = chunks.first() {
      buf.extend_from_slice(&first.hash);
    }
  }
  hash128(&buf)
}

/// 发送端小文件统一连续内存缓存：将扫描期前 128 MiB 小文件驻留连续内存 `arena`，
/// 消除高频小文件重复 `File::open` 系统调用，同时避免扫描期向磁盘回写数 GiB 临时包阻塞控制面握手。
pub struct SmallCache {
  offsets: Vec<u64>,
  arena: Vec<u8>,
}

impl SmallCache {
  #[inline]
  pub fn offset_of(&self, idx: usize) -> Option<u64> {
    self
      .offsets
      .get(idx)
      .copied()
      .filter(|&o| o != UNPACKED_OFFSET)
  }

  #[inline]
  pub fn read_at(&self, off: u64, dst: &mut [u8]) -> io::Result<()> {
    let s = off as usize;
    let end = s.saturating_add(dst.len());
    if end <= self.arena.len() {
      dst.copy_from_slice(&self.arena[s..end]);
      Ok(())
    } else {
      Err(io::Error::from(io::ErrorKind::UnexpectedEof))
    }
  }
}

/// 查询最近一次对 `root` 扫描生成的 `SmallCache`（同时匹配全量清单与增量 `delta_mf` 内容摘要指纹）
pub(crate) fn take_or_clone_small_cache(
  root: &Path,
  entries: &[ManifestEntry],
) -> Option<Arc<SmallCache>> {
  if entries.is_empty() {
    return None;
  }
  let key = entries_cache_key(
    entries.len(),
    entries
      .iter()
      .map(|e| (e.path.as_str(), e.size, e.chunks.as_slice())),
  );
  let guard = LAST_SMALL_CACHE.lock();
  guard
    .as_ref()
    .filter(|(r, k, _)| *k == key && r == root)
    .map(|(_, _, c)| Arc::clone(c))
}

/// 当目录发生增量更新或落盘提交时，失效对应 `root` 的 `SmallCache`
pub(crate) fn invalidate_small_cache(root: &Path) {
  let mut guard = LAST_SMALL_CACHE.lock();
  if guard.as_ref().is_some_and(|(r, ..)| r == root) {
    *guard = None;
  }
}

type ArenaSlice<'a, T> = (&'a [T], &'a [u64], usize, &'a mut [u8]);

fn plan_arena<T>(
  items: &[T],
  max_small: u64,
  arena_limit: usize,
  eligible_size: impl Fn(&T) -> Option<u64>,
) -> (Vec<u64>, Vec<u8>) {
  let mut offsets = Vec::with_capacity(items.len());
  let mut arena_len = 0usize;
  for item in items {
    if let Some(sz) = eligible_size(item)
      && sz > 0
      && sz <= max_small
      && arena_len + (sz as usize) <= arena_limit
    {
      offsets.push(arena_len as u64);
      arena_len += sz as usize;
    } else {
      offsets.push(UNPACKED_OFFSET);
    }
  }
  (offsets, vec![0u8; arena_len])
}

/// 按线程分块计划切分 `items`、`offsets` 与连续可写 `arena` 切片
fn split_arena_slices<'a, T>(
  items: &'a [T],
  offsets: &'a [u64],
  mut rem_arena: &'a mut [u8],
  size_of: impl Fn(&T) -> u64,
) -> Vec<ArenaSlice<'a, T>> {
  let (n_threads, chunk_len) = par_chunk_plan(items.len());
  let mut slices = Vec::with_capacity(n_threads);
  let mut prev_end = 0usize;
  for (ents, offs) in items.chunks(chunk_len).zip(offsets.chunks(chunk_len)) {
    let mut slice_end = prev_end;
    for (e, &off) in ents.iter().zip(offs.iter()) {
      if off != UNPACKED_OFFSET {
        slice_end = (off as usize) + (size_of(e) as usize);
      }
    }
    let (head, tail) = rem_arena.split_at_mut(slice_end - prev_end);
    rem_arena = tail;
    slices.push((ents, offs, prev_end, head));
    prev_end = slice_end;
  }
  slices
}

/// 并发预检接收端目录中已存在的单块小文件是否与清单完全一致，并将命中文件同步驻留 `SmallCache` 供后续零系统调用做种
pub(crate) fn precheck_manifest_single_chunks(root: &Path, entries: &[ManifestEntry]) -> Vec<bool> {
  let max_small = CdcConfig::DEFAULT.max_size as u64;
  let arena_limit = max_small_arena();
  let (mut offsets, mut arena) = plan_arena(entries, max_small, arena_limit, |e| {
    (!e.deleted && e.chunks.len() == 1).then_some(e.size)
  });
  let slices = split_arena_slices(entries, &offsets, &mut arena, |e| e.size);

  let res: Vec<bool> = thread::scope(|s| {
    let mut handles = Vec::with_capacity(slices.len());
    for (slice, offs, base_off, buf_slice) in slices {
      handles.push(s.spawn(move || {
        let mut out = vec![false; slice.len()];
        probe_batch_files(
          root,
          slice.len(),
          |i| {
            let e = &slice[i];
            (!e.deleted && e.size > 0 && e.chunks.len() == 1)
              .then_some((e.path.as_str(), e.size as usize))
          },
          |i, probed| {
            if let Some((_, Some(data))) = probed {
              let e = &slice[i];
              let off = offs[i];
              if off != UNPACKED_OFFSET {
                let rel = (off as usize) - base_off;
                buf_slice[rel..rel + data.len()].copy_from_slice(data);
              }
              out[i] = hash128(data) == e.chunks[0].hash;
            }
          },
        );
        out
      }));
    }
    handles
      .into_iter()
      .flat_map(|h| h.join().unwrap_or_default())
      .collect()
  });
  let mut any_cached = false;
  for (off, &ok) in offsets.iter_mut().zip(res.iter()) {
    if !ok {
      *off = UNPACKED_OFFSET;
    } else if *off != UNPACKED_OFFSET {
      any_cached = true;
    }
  }
  if any_cached {
    let key = entries_cache_key(
      entries.len(),
      entries
        .iter()
        .map(|e| (e.path.as_str(), e.size, e.chunks.as_slice())),
    );
    let cache = Arc::new(SmallCache { offsets, arena });
    *LAST_SMALL_CACHE.lock() = Some((root.to_path_buf(), key, cache));
  }
  res
}

fn scan_arena_slice(
  root: &Path,
  (files, offs, base_off, buf_slice): ArenaSlice<'_, ScannedFile>,
) -> io::Result<Vec<Vec<ChunkSig>>> {
  let mut out = vec![Vec::new(); files.len()];
  read_batch_files(
    root,
    files.len(),
    |i| {
      (files[i].size > 0 && offs[i] != UNPACKED_OFFSET)
        .then_some((files[i].path.as_str(), files[i].size as usize))
    },
    |i, data| {
      let rel = (offs[i] as usize) - base_off;
      let dst = &mut buf_slice[rel..rel + data.len()];
      dst.copy_from_slice(data);
      out[i] = chunk_slice(dst, CdcConfig::DEFAULT);
    },
  )?;
  if files
    .iter()
    .zip(offs.iter())
    .any(|(f, &off)| f.size > 0 && off == UNPACKED_OFFSET)
  {
    let mut root_buf = RootBuf::new(root);
    let mut io_buf = Vec::new();
    for (i, (f, &off)) in files.iter().zip(offs.iter()).enumerate() {
      if f.size > 0 && off == UNPACKED_OFFSET {
        let file = File::open(root_buf.join(&f.path))?;
        out[i] = chunk_file_range_with_buf(&file, 0, f.size, CdcConfig::DEFAULT, &mut io_buf)?;
      }
    }
  }
  Ok(out)
}

/// 扫描并计算所有文件的 FastCDC 分块签名，同时构建小文件连续缓存 `SmallCache`
pub fn scan_and_cache_files(
  root: &Path,
  scanned: &[ScannedFile],
) -> io::Result<(Vec<Vec<ChunkSig>>, Arc<SmallCache>)> {
  let max_small = CdcConfig::DEFAULT.max_size as u64;
  let arena_limit = max_small_arena();
  let (offsets, mut arena) = plan_arena(scanned, max_small, arena_limit, |f| Some(f.size));
  let mut slices = split_arena_slices(scanned, &offsets, &mut arena, |f| f.size);
  let out_chunks = if slices.len() <= 1 {
    match slices.pop() {
      Some(s) => scan_arena_slice(root, s)?,
      None => Vec::new(),
    }
  } else {
    let worker_res: Vec<io::Result<Vec<Vec<ChunkSig>>>> = thread::scope(|s| {
      let handles: Vec<_> = slices
        .into_iter()
        .map(|slice| s.spawn(move || scan_arena_slice(root, slice)))
        .collect();
      handles.into_iter().map(|h| join_io(h.join())).collect()
    });
    let mut all = Vec::with_capacity(scanned.len());
    for res in worker_res {
      all.extend(res?);
    }
    all
  };
  let key = entries_cache_key(
    scanned.len(),
    scanned
      .iter()
      .zip(out_chunks.iter())
      .map(|(sf, ch)| (sf.path.as_str(), sf.size, ch.as_slice())),
  );
  let cache = Arc::new(SmallCache { offsets, arena });
  *LAST_SMALL_CACHE.lock() = Some((root.to_path_buf(), key, Arc::clone(&cache)));
  Ok((out_chunks, cache))
}
