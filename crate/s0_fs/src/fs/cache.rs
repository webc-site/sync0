use std::{
  fs::File,
  io,
  path::{Path, PathBuf},
  sync::Arc,
  thread,
};

use parking_lot::Mutex;
use s0_cdc::{
  CdcConfig, ChunkSig, FileAt, ManifestEntry, chunk_file_with_buf, chunk_slice, hash128,
};

use super::{RootBuf, ScannedFile, open_nonempty_file, par_chunk_plan};

/// 发送端扫描期小文件内存池上限（2 GiB）
const MAX_SMALL_ARENA: usize = 2048 << 20;

static LAST_SMALL_CACHE: Mutex<Option<(PathBuf, usize, Arc<SmallCache>)>> = Mutex::new(None);

/// 发送端小文件统一连续内存缓存：将扫描期前 128 MiB 小文件驻留连续内存 `arena`，
/// 消除高频小文件重复 `File::open` 系统调用，同时避免扫描期向磁盘回写数 GiB 临时包阻塞控制面握手。
pub struct SmallCache {
  offsets: Vec<u64>,
  arena: Vec<u8>,
}

impl SmallCache {
  pub fn empty() -> Self {
    Self {
      offsets: Vec::new(),
      arena: Vec::new(),
    }
  }

  #[inline]
  pub fn offset_of(&self, idx: usize) -> Option<u64> {
    self.offsets.get(idx).copied().filter(|&o| o != u64::MAX)
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

/// 查询最近一次对 `root` 扫描生成的 `SmallCache`
pub fn take_or_clone_small_cache(root: &Path, entry_count: usize) -> Option<Arc<SmallCache>> {
  let guard = LAST_SMALL_CACHE.lock();
  guard
    .as_ref()
    .filter(|(r, n, _)| *n == entry_count && r == root)
    .map(|(_, _, c)| Arc::clone(c))
}

/// 并发预检接收端目录中已存在的单块小文件是否与清单完全一致，并将命中文件同步驻留 `SmallCache` 供后续零系统调用做种
pub(super) fn precheck_manifest_single_chunks(root: &Path, entries: &[ManifestEntry]) -> Vec<bool> {
  let max_small = CdcConfig::DEFAULT.max_size as u64;
  let mut offsets = Vec::with_capacity(entries.len());
  let mut arena_len = 0usize;
  for e in entries {
    if !e.deleted
      && e.size > 0
      && e.chunks.len() == 1
      && e.size <= max_small
      && arena_len + (e.size as usize) <= MAX_SMALL_ARENA
    {
      offsets.push(arena_len as u64);
      arena_len += e.size as usize;
    } else {
      offsets.push(u64::MAX);
    }
  }

  let mut arena = vec![0u8; arena_len];
  let (n_threads, chunk_len) = par_chunk_plan(entries.len());
  let mut slices = Vec::with_capacity(n_threads);
  let mut rem_arena = arena.as_mut_slice();
  let mut prev_end = 0usize;
  for (ents, offs) in entries.chunks(chunk_len).zip(offsets.chunks(chunk_len)) {
    let mut slice_end = prev_end;
    for (e, &off) in ents.iter().zip(offs.iter()) {
      if off != u64::MAX {
        slice_end = (off as usize) + (e.size as usize);
      }
    }
    let take = slice_end - prev_end;
    let (head, tail) = rem_arena.split_at_mut(take);
    rem_arena = tail;
    slices.push((ents, offs, prev_end, head));
    prev_end = slice_end;
  }

  let parts: Vec<Vec<bool>> = thread::scope(|s| {
    let mut handles = Vec::with_capacity(slices.len());
    for (slice, offs, base_off, buf_slice) in slices {
      handles.push(s.spawn(move || {
        let mut root_buf = RootBuf::new(root);
        let mut io_buf = Vec::new();
        let mut out = Vec::with_capacity(slice.len());
        for (e, &off) in slice.iter().zip(offs.iter()) {
          if e.deleted || e.size == 0 || e.chunks.len() != 1 {
            out.push(false);
            continue;
          }
          let Some((bf, bs)) = open_nonempty_file(root_buf.join(&e.path)) else {
            out.push(false);
            continue;
          };
          let n = e.size as usize;
          if bs == e.size {
            if off != u64::MAX {
              let rel = (off as usize) - base_off;
              let dst = &mut buf_slice[rel..rel + n];
              let same =
                bf.read_exact_at_pos(dst, 0).is_ok() && hash128(dst) == e.chunks[0].hash;
              out.push(same);
            } else {
              if io_buf.len() < n {
                io_buf.resize(n, 0);
              }
              let same = bf.read_exact_at_pos(&mut io_buf[..n], 0).is_ok()
                && hash128(&io_buf[..n]) == e.chunks[0].hash;
              out.push(same);
            }
          } else {
            out.push(false);
          }
        }
        out
      }));
    }
    handles
      .into_iter()
      .map(|h| h.join().unwrap_or_default())
      .collect()
  });
  let mut res = Vec::with_capacity(entries.len());
  for p in parts {
    res.extend(p);
  }
  let mut any_cached = false;
  for (off, &ok) in offsets.iter_mut().zip(res.iter()) {
    if !ok {
      *off = u64::MAX;
    } else if *off != u64::MAX {
      any_cached = true;
    }
  }
  if any_cached {
    let cache = Arc::new(SmallCache { offsets, arena });
    *LAST_SMALL_CACHE.lock() = Some((root.to_path_buf(), entries.len(), cache));
  }
  res
}

#[inline]
const fn scanned_small_cap(first_size: u64) -> usize {
  let est = (first_size as usize).saturating_mul(256);
  if est < 256 * 1024 { 256 * 1024 } else { est }
}

/// 扫描并计算所有文件的 FastCDC 分块签名，同时构建小文件连续缓存 `SmallCache`
pub fn scan_and_cache_files(
  root: &Path,
  scanned: &[ScannedFile],
) -> io::Result<(Vec<Vec<ChunkSig>>, Arc<SmallCache>)> {
  let max_small = CdcConfig::DEFAULT.max_size as u64;
  if scanned.len() < 16 {
    let mut root_buf = RootBuf::new(root);
    let mut offsets = Vec::with_capacity(scanned.len());
    let mut out_chunks = Vec::with_capacity(scanned.len());
    let mut arena = Vec::new();
    let mut io_buf = Vec::new();
    for f in scanned {
      if f.size == 0 {
        offsets.push(u64::MAX);
        out_chunks.push(Vec::new());
      } else if f.size <= max_small && arena.len() + (f.size as usize) <= MAX_SMALL_ARENA {
        let n = f.size as usize;
        let file = File::open(root_buf.join(&f.path))?;
        if arena.is_empty() {
          arena.reserve(MAX_SMALL_ARENA.min(scanned_small_cap(f.size)));
        }
        let base = arena.len();
        arena.resize(base + n, 0);
        if let Err(e) = file.read_exact_at_pos(&mut arena[base..base + n], 0) {
          arena.truncate(base);
          return Err(e);
        }
        out_chunks.push(chunk_slice(&arena[base..base + n], CdcConfig::DEFAULT));
        offsets.push(base as u64);
      } else {
        let file = File::open(root_buf.join(&f.path))?;
        out_chunks.push(
          chunk_file_with_buf(&file, f.size, CdcConfig::DEFAULT, &mut io_buf)
            .map_err(io::Error::other)?,
        );
        offsets.push(u64::MAX);
      }
    }
    let cache = Arc::new(SmallCache { offsets, arena });
    *LAST_SMALL_CACHE.lock() = Some((root.to_path_buf(), scanned.len(), Arc::clone(&cache)));
    return Ok((out_chunks, cache));
  }

  let mut offsets = Vec::with_capacity(scanned.len());
  let mut arena_len = 0usize;
  for f in scanned {
    if f.size > 0 && f.size <= max_small && arena_len + (f.size as usize) <= MAX_SMALL_ARENA {
      offsets.push(arena_len as u64);
      arena_len += f.size as usize;
    } else {
      offsets.push(u64::MAX);
    }
  }

  let mut arena = vec![0u8; arena_len];
  let (n_threads, chunk_len) = par_chunk_plan(scanned.len());
  let mut slices = Vec::with_capacity(n_threads);
  let mut rem_arena = arena.as_mut_slice();
  let mut prev_end = 0usize;
  for (files, offs) in scanned.chunks(chunk_len).zip(offsets.chunks(chunk_len)) {
    let mut slice_end = prev_end;
    for (f, &off) in files.iter().zip(offs.iter()) {
      if off != u64::MAX {
        slice_end = (off as usize) + (f.size as usize);
      }
    }
    let take = slice_end - prev_end;
    let (head, tail) = rem_arena.split_at_mut(take);
    rem_arena = tail;
    slices.push((files, offs, prev_end, head));
    prev_end = slice_end;
  }

  let worker_res: Vec<io::Result<Vec<Vec<ChunkSig>>>> = thread::scope(|s| {
    let mut handles = Vec::with_capacity(slices.len());
    for (files, offs, base_off, buf_slice) in slices {
      handles.push(s.spawn(move || {
        let mut root_buf = RootBuf::new(root);
        let mut io_buf = Vec::new();
        let mut out = Vec::with_capacity(files.len());
        for (f, &off) in files.iter().zip(offs.iter()) {
          if f.size == 0 {
            out.push(Vec::new());
          } else if off != u64::MAX {
            let rel = (off as usize) - base_off;
            let n = f.size as usize;
            let dst = &mut buf_slice[rel..rel + n];
            let file = File::open(root_buf.join(&f.path))?;
            file.read_exact_at_pos(dst, 0)?;
            out.push(chunk_slice(dst, CdcConfig::DEFAULT));
          } else {
            let file = File::open(root_buf.join(&f.path))?;
            out.push(
              chunk_file_with_buf(&file, f.size, CdcConfig::DEFAULT, &mut io_buf)
                .map_err(io::Error::other)?,
            );
          }
        }
        Ok(out)
      }));
    }
    handles
      .into_iter()
      .map(|h| {
        h.join()
          .unwrap_or_else(|_| Err(io::Error::other("worker panic")))
      })
      .collect()
  });

  let mut out_chunks = Vec::with_capacity(scanned.len());
  for res in worker_res {
    out_chunks.extend(res?);
  }
  let cache = Arc::new(SmallCache { offsets, arena });
  *LAST_SMALL_CACHE.lock() = Some((root.to_path_buf(), scanned.len(), Arc::clone(&cache)));
  Ok((out_chunks, cache))
}
