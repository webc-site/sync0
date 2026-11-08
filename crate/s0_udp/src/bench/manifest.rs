use std::{fs::File, path::Path, thread};

use s0_cdc::{CdcConfig, ChunkSig, FileAt, Rd, Wr as VecWr, chunk_file_with_buf, chunk_slice};

pub(super) use super::manifest_net::{ManifestRx, send_manifest, try_build_final_dir_ack};
use super::{DGRAM, MANIFEST_CHUNK, MANIFEST_ZSTD, RootBuf, ZSTD_LEVEL, par_chunk_plan};
use crate::{
  UdpError,
  codec::{Codec, SLACK},
  consts::{
    DIR_ACK_HDR_LEN, HAVE_ALL_ONES_BIT as HAVE_ALL_ONES, MANIFEST_DELETE, MANIFEST_PARTIAL,
    MAX_BLOCK, TOMBSTONE_NCHUNKS,
  },
  fs::{ScannedFile, rel_path, scan_files},
};

/// 单包 `DIR_ACK` 可容纳的最大位图字节数
const MAX_MANIFEST_BITMAP: usize = DGRAM - DIR_ACK_HDR_LEN - 4;
/// 清单编码后最大字节数（受限于单包 `DIR_ACK` 位图容量）
pub(super) const MAX_MANIFEST_ENCODED: usize = MAX_MANIFEST_BITMAP * 8 * MANIFEST_CHUNK;
/// 清单解压后最大字节数（64 MiB）
const MAX_MANIFEST_RAW: usize = 64 << 20;
/// 清单最大文件条目数
const MAX_MANIFEST_ENTRIES: usize = 1_000_000;
/// 发送端扫描期小文件内存池上限（16 MiB）：避免 `DIR_ACK` 后二次 `open`/`pread` 海量小文件
const MAX_SMALL_ARENA: usize = 16 << 20;
/// 启用 Zstd 压缩的最小清单/位图字节阈值
pub(super) const MIN_ZSTD_BYTES: usize = 256;
/// 路径长度上限
const MAX_PATH_LEN: usize = 0x7FFF;
const MANIFEST_FLAGS: u32 = MANIFEST_DELETE | MANIFEST_PARTIAL;
pub(super) const HAVE_FLAGS: u32 = MANIFEST_ZSTD | HAVE_ALL_ONES;
const _: () = assert!(CdcConfig::DEFAULT.max_size <= (u16::MAX as usize) + 1);
const _: () = assert!((MAX_MANIFEST_ENTRIES as u32) & MANIFEST_FLAGS == 0);

/// 清单同步模式标记
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ManifestMode {
  pub(super) delete: bool,
  pub(super) partial: bool,
}

/// 清单文件条目：`path == ""` 表示单文件模式，否则为目录内相对路径；`del == true` 表示删除墓碑
#[derive(Clone, Debug)]
pub(super) struct Entry {
  pub(super) path: String,
  pub(super) size: u64,
  pub(super) chunks: Vec<ChunkSig>,
  pub(super) del: bool,
  pub(super) cached: Option<u32>,
}

impl Entry {
  const fn tombstone(path: String) -> Self {
    Self {
      path,
      size: 0,
      chunks: Vec::new(),
      del: true,
      cached: None,
    }
  }
}

/// 将扫描列表与可选删除墓碑列表转为带 FastCDC 签名的条目、小文件内存池与逻辑总字节数
pub(super) fn entries_from_changes(
  root: &Path,
  mut scanned: Vec<ScannedFile>,
  mut removed: Vec<String>,
) -> Result<(Vec<Entry>, Vec<u8>, u64), UdpError> {
  if !scanned.is_sorted_by(|a, b| a.path < b.path) {
    scanned.sort_unstable_by(|a, b| a.path.cmp(&b.path));
    scanned.dedup_by(|b, a| a.path == b.path);
  }
  if !removed.is_sorted_by(|a, b| a < b) {
    removed.sort_unstable();
    removed.dedup();
  }
  let max_small = CdcConfig::DEFAULT.max_size as u64;
  if scanned.len() >= 16 {
    return entries_from_changes_par(root, scanned, removed, max_small);
  }
  let mut root_buf = RootBuf::new(root);
  let mut entries = Vec::with_capacity(scanned.len() + removed.len());
  let mut arena = Vec::new();
  let mut rem_it = removed.into_iter().peekable();
  let mut io_buf = Vec::new();
  let mut total = 0u64;
  for f in scanned {
    while let Some(path) = rem_it.next_if(|p| *p < f.path) {
      entries.push(Entry::tombstone(path));
    }
    let _ = rem_it.next_if(|p| *p == f.path);
    let (chunks, cached) = if f.size == 0 {
      (Vec::new(), None)
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
        return Err(e.into());
      }
      let chunks = chunk_slice(&arena[base..base + n], CdcConfig::DEFAULT);
      (chunks, Some(base as u32))
    } else {
      let file = File::open(root_buf.join(&f.path))?;
      (
        chunk_file_with_buf(&file, f.size, CdcConfig::DEFAULT, &mut io_buf)?,
        None,
      )
    };
    total = total.saturating_add(f.size);
    entries.push(Entry {
      path: f.path,
      size: f.size,
      chunks,
      del: false,
      cached,
    });
  }
  for path in rem_it {
    entries.push(Entry::tombstone(path));
  }
  Ok((entries, arena, total))
}

/// 多线程并行扫描 + 小文件无锁预分配内存池：按已排序路径分片并行执行 `open + pread + FastCDC/BLAKE3`
fn entries_from_changes_par(
  root: &Path,
  scanned: Vec<ScannedFile>,
  removed: Vec<String>,
  max_small: u64,
) -> Result<(Vec<Entry>, Vec<u8>, u64), UdpError> {
  let mut offsets = Vec::with_capacity(scanned.len());
  let mut arena_len = 0usize;
  let mut total = 0u64;
  for f in &scanned {
    total = total.saturating_add(f.size);
    if f.size > 0 && f.size <= max_small && arena_len + (f.size as usize) <= MAX_SMALL_ARENA {
      offsets.push(Some(arena_len as u32));
      arena_len += f.size as usize;
    } else {
      offsets.push(None);
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
      if let Some(base) = off {
        slice_end = base as usize + f.size as usize;
      }
    }
    let take = slice_end - prev_end;
    let (head, tail) = rem_arena.split_at_mut(take);
    rem_arena = tail;
    slices.push((files, offs, prev_end, head));
    prev_end = slice_end;
  }

  let worker_res: Vec<Result<Vec<Vec<ChunkSig>>, UdpError>> = thread::scope(|s| {
    let mut handles = Vec::with_capacity(slices.len());
    for (files, offs, base_off, buf_slice) in slices {
      handles.push(s.spawn(move || {
        let mut root_buf = RootBuf::new(root);
        let mut io_buf = Vec::new();
        let mut out = Vec::with_capacity(files.len());
        for (f, &off) in files.iter().zip(offs.iter()) {
          if f.size == 0 {
            out.push(Vec::new());
          } else if let Some(abs_off) = off {
            let rel = abs_off as usize - base_off;
            let n = f.size as usize;
            let dst = &mut buf_slice[rel..rel + n];
            let file = File::open(root_buf.join(&f.path))?;
            file.read_exact_at_pos(dst, 0)?;
            out.push(chunk_slice(dst, CdcConfig::DEFAULT));
          } else {
            let file = File::open(root_buf.join(&f.path))?;
            out.push(chunk_file_with_buf(
              &file,
              f.size,
              CdcConfig::DEFAULT,
              &mut io_buf,
            )?);
          }
        }
        Ok(out)
      }));
    }
    handles
      .into_iter()
      .map(|h| h.join().unwrap_or(Err(UdpError::Corrupt)))
      .collect()
  });

  let mut entries = Vec::with_capacity(scanned.len() + removed.len());
  let mut rem_it = removed.into_iter().peekable();
  let mut scanned_it = scanned.into_iter().zip(offsets);
  for res in worker_res {
    for chunks in res? {
      let Some((f, cached)) = scanned_it.next() else {
        break;
      };
      while let Some(path) = rem_it.next_if(|p| *p < f.path) {
        entries.push(Entry::tombstone(path));
      }
      let _ = rem_it.next_if(|p| *p == f.path);
      entries.push(Entry {
        path: f.path,
        size: f.size,
        chunks,
        del: false,
        cached,
      });
    }
  }
  for path in rem_it {
    entries.push(Entry::tombstone(path));
  }
  Ok((entries, arena, total))
}

#[inline]
const fn scanned_small_cap(first_size: u64) -> usize {
  let est = (first_size as usize).saturating_mul(256);
  if est < 256 * 1024 { 256 * 1024 } else { est }
}

pub(super) fn entries_from_scanned(
  root: &Path,
  scanned: Vec<ScannedFile>,
) -> Result<(Vec<Entry>, Vec<u8>, u64), UdpError> {
  entries_from_changes(root, scanned, Vec::new())
}

pub(super) fn scan_dir(root: &Path) -> Result<(Vec<Entry>, Vec<u8>, u64), UdpError> {
  entries_from_scanned(root, scan_files(root)?)
}

pub(super) fn encode_manifest(entries: &[Entry], mode: ManifestMode) -> Result<Vec<u8>, UdpError> {
  if entries.len() > MAX_MANIFEST_ENTRIES {
    return Err(UdpError::Arg("清单文件数过多"));
  }
  let est_chunks: usize = entries.iter().map(|e| e.chunks.len()).sum();
  let mut raw = Vec::with_capacity(4 + entries.len() * 12 + est_chunks * 18);
  let count_hdr = (entries.len() as u32)
    | if mode.delete { MANIFEST_DELETE } else { 0 }
    | if mode.partial { MANIFEST_PARTIAL } else { 0 };
  raw.extend_from_slice(&count_hdr.to_le_bytes());
  let mut prev_path: &[u8] = &[];
  for (i, e) in entries.iter().enumerate() {
    let p = e.path.as_bytes();
    if p.len() > MAX_PATH_LEN {
      return Err(UdpError::Arg("清单路径过长"));
    }
    if i == 0 {
      VecWr::new(&mut raw).slice_u15(p)?;
    } else {
      if prev_path >= p {
        return Err(UdpError::Arg("清单路径重复或未排序"));
      }
      let pre = prev_path
        .iter()
        .zip(p.iter())
        .take_while(|(a, b)| a == b)
        .count();
      let suf = &p[pre..];
      let mut w = VecWr::new(&mut raw);
      w.u15(pre)?;
      w.slice_u15(suf)?;
    }
    prev_path = p;
    if e.del {
      raw.extend_from_slice(&TOMBSTONE_NCHUNKS.to_le_bytes());
      continue;
    }
    let nchunks = u32::try_from(e.chunks.len()).map_err(|_| UdpError::Arg("文件分块过多"))?;
    if nchunks < u32::from(TOMBSTONE_NCHUNKS) {
      raw.extend_from_slice(&(nchunks as u16).to_le_bytes());
    } else {
      raw.extend_from_slice(&u16::MAX.to_le_bytes());
      raw.extend_from_slice(&nchunks.to_le_bytes());
    }
    for c in &e.chunks {
      if c.len == 0 || c.len > CdcConfig::DEFAULT.max_size as u32 {
        return Err(UdpError::Arg("CDC 块长度越界"));
      }
      let len_m1 = (c.len - 1) as u16;
      raw.extend_from_slice(&len_m1.to_le_bytes());
      raw.extend_from_slice(&c.hash);
    }
  }
  if raw.len() > MAX_MANIFEST_RAW {
    return Err(UdpError::Arg("清单过大"));
  }
  let raw_len = raw.len() as u32;
  if raw.len() <= MIN_ZSTD_BYTES {
    let mut out = Vec::with_capacity(8 + raw.len());
    out.extend_from_slice(&raw_len.to_le_bytes());
    out.extend_from_slice(&raw_len.to_le_bytes());
    out.extend_from_slice(&raw);
    return Ok(out);
  }
  let mut codec = Codec::new(ZSTD_LEVEL)?;
  let mut dst = vec![0u8; raw.len().min(MAX_BLOCK) + SLACK];
  let mut out = Vec::with_capacity(4 + raw.len() / 2);
  out.extend_from_slice(&raw_len.to_le_bytes());
  for chunk in raw.chunks(MAX_BLOCK) {
    if let Some(clen) = codec.try_compress_block(chunk, &mut dst)? {
      let hdr = (clen as u32) | MANIFEST_ZSTD;
      out.extend_from_slice(&hdr.to_le_bytes());
      out.extend_from_slice(&dst[..clen]);
    } else {
      let hdr = chunk.len() as u32;
      out.extend_from_slice(&hdr.to_le_bytes());
      out.extend_from_slice(chunk);
    }
  }
  if out.len() > MAX_MANIFEST_ENCODED {
    return Err(UdpError::Arg("清单过大"));
  }
  Ok(out)
}

pub(super) fn parse_manifest(b: &[u8]) -> Result<(Vec<Entry>, u64, ManifestMode), UdpError> {
  let mut rd = Rd::new(b);
  let raw_len = rd.u32()? as usize;
  if !(4..=MAX_MANIFEST_RAW).contains(&raw_len) {
    return Err(UdpError::Arg("清单长度非法"));
  }
  let mut raw = Vec::with_capacity(raw_len.min(MAX_BLOCK));
  let mut codec: Option<Codec> = None;
  while !rd.is_empty() {
    let hdr = rd.u32()?;
    let zstd = hdr & MANIFEST_ZSTD != 0;
    let clen = (hdr & !MANIFEST_ZSTD) as usize;
    let chunk = rd.take(clen)?;
    let rem = raw_len.checked_sub(raw.len()).ok_or(UdpError::Corrupt)?;
    if rem == 0 {
      return Err(UdpError::Corrupt);
    }
    if zstd {
      let c = match &mut codec {
        Some(c) => c,
        None => codec.insert(Codec::new(ZSTD_LEVEL)?),
      };
      let want = rem.min(MAX_BLOCK);
      let cur = raw.len();
      raw.resize(cur + want, 0);
      if c.decompress_block(chunk, &mut raw[cur..])? != want {
        return Err(UdpError::Corrupt);
      }
    } else {
      if chunk.len() > rem || chunk.len() > MAX_BLOCK {
        return Err(UdpError::Corrupt);
      }
      raw.extend_from_slice(chunk);
    }
  }
  if raw.len() != raw_len {
    return Err(UdpError::Arg("清单长度不符"));
  }

  let mut r = Rd::new(&raw);
  let count_hdr = r.u32()?;
  let mode = ManifestMode {
    delete: count_hdr & MANIFEST_DELETE != 0,
    partial: count_hdr & MANIFEST_PARTIAL != 0,
  };
  let n = (count_hdr & !MANIFEST_FLAGS) as usize;
  if n > (raw.len() - 4) / 3 || n > MAX_MANIFEST_ENTRIES {
    return Err(UdpError::Corrupt);
  }
  let mut entries: Vec<Entry> = Vec::with_capacity(n);
  let mut path_buf: Vec<u8> = Vec::new();
  let mut total = 0u64;
  for i in 0..n {
    let (pre, suf_len) = if i == 0 {
      (0usize, usize::from(r.u15()?))
    } else {
      (usize::from(r.u15()?), usize::from(r.u15()?))
    };
    let path = if i == 0 && suf_len == 0 {
      if n != 1 {
        return Err(UdpError::Corrupt);
      }
      String::new()
    } else {
      if suf_len == 0 || pre > path_buf.len() || pre + suf_len > MAX_PATH_LEN {
        return Err(UdpError::Corrupt);
      }
      let suf = r.take(suf_len)?;
      if i > 0 && pre < path_buf.len() && suf[0] <= path_buf[pre] {
        return Err(UdpError::Corrupt);
      }
      path_buf.truncate(pre);
      path_buf.extend_from_slice(suf);
      rel_path(&path_buf)
        .map_err(|_| UdpError::Arg("清单路径非法"))?
        .as_os_str()
        .to_str()
        .ok_or(UdpError::Arg("清单路径"))?
        .to_owned()
    };
    let nchunks_lo = r.u16()?;
    if nchunks_lo == TOMBSTONE_NCHUNKS {
      if path.is_empty() {
        return Err(UdpError::Corrupt);
      }
      entries.push(Entry::tombstone(path));
      continue;
    }
    let nchunks = if nchunks_lo < TOMBSTONE_NCHUNKS {
      usize::from(nchunks_lo)
    } else {
      let n32 = r.u32()?;
      if n32 < u32::from(TOMBSTONE_NCHUNKS) {
        return Err(UdpError::Corrupt);
      }
      n32 as usize
    };
    if nchunks > r.rest().len() / 18 {
      return Err(UdpError::Corrupt);
    }
    let mut chunks = Vec::with_capacity(nchunks);
    let mut size = 0u64;
    for _ in 0..nchunks {
      let len = u32::from(r.u16()?) + 1;
      let hash: [u8; 16] = r.array()?;
      chunks.push(ChunkSig {
        offset: size,
        len,
        hash,
      });
      size = size.saturating_add(u64::from(len));
    }
    entries.push(Entry {
      path,
      size,
      chunks,
      del: false,
      cached: None,
    });
    total = total.saturating_add(size);
  }
  r.expect_end()?;
  Ok((entries, total, mode))
}
