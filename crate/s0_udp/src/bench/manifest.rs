use std::{path::Path, sync::Arc};

use s0_cdc::{CdcConfig, ChunkSig, Rd, Wr as VecWr};

pub(super) use super::manifest_net::{ManifestRx, send_manifest, try_build_final_dir_ack};
use super::{MANIFEST_CHUNK, MANIFEST_ZSTD, MAX_BLOCK, MAX_DATAGRAM, ZSTD_DEFAULT_LEVEL};
use crate::{
  UdpError,
  codec::{Codec, SLACK},
  consts::{
    DIR_ACK_HDR_LEN, HASH_LEN, HAVE_ALL_ONES_BIT, MANIFEST_DELETE, MANIFEST_PARTIAL,
    MIN_ZSTD_BYTES, U15_MAX,
  },
  fs::{ScannedFile, SmallCache, rel_path, scan_and_cache_files, scan_files},
};

/// 单包 `DIR_ACK` 可容纳的最大位图字节数
const MAX_MANIFEST_BITMAP: usize = MAX_DATAGRAM - DIR_ACK_HDR_LEN - 4;
/// 清单编码后最大字节数（受限于单包 `DIR_ACK` 位图容量）
pub(super) const MAX_MANIFEST_ENCODED: usize = MAX_MANIFEST_BITMAP * 8 * MANIFEST_CHUNK;
/// 清单解压后最大字节数（64 MiB）
const MAX_MANIFEST_RAW: usize = 64 << 20;
/// 清单最大文件条目数
const MAX_MANIFEST_ENTRIES: usize = 1_000_000;
const MANIFEST_FLAGS: u32 = MANIFEST_DELETE | MANIFEST_PARTIAL;
pub(super) const HAVE_FLAGS: u32 = MANIFEST_ZSTD | HAVE_ALL_ONES_BIT;
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
  pub(super) cached: Option<u64>,
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

/// 将扫描列表与可选删除墓碑列表转为带 FastCDC 签名的条目、小文件连续缓存池与逻辑总字节数
pub(super) fn entries_from_changes(
  root: &Path,
  mut scanned: Vec<ScannedFile>,
  mut removed: Vec<String>,
) -> Result<(Vec<Entry>, Arc<SmallCache>, u64), UdpError> {
  if !scanned.is_sorted_by(|a, b| a.path < b.path) {
    scanned.sort_unstable_by(|a, b| a.path.cmp(&b.path));
    scanned.dedup_by(|b, a| a.path == b.path);
  }
  if !removed.is_sorted_by(|a, b| a < b) {
    removed.sort_unstable();
    removed.dedup();
  }
  let (all_chunks, cache) = scan_and_cache_files(root, &scanned)?;
  let mut entries = Vec::with_capacity(scanned.len() + removed.len());
  let mut rem_it = removed.into_iter().peekable();
  let mut total = 0u64;
  for (i, (f, chunks)) in scanned.into_iter().zip(all_chunks).enumerate() {
    while let Some(path) = rem_it.next_if(|p| *p < f.path) {
      entries.push(Entry::tombstone(path));
    }
    let _ = rem_it.next_if(|p| *p == f.path);
    total = total.saturating_add(f.size);
    entries.push(Entry {
      path: f.path,
      size: f.size,
      chunks,
      del: false,
      cached: cache.offset_of(i),
    });
  }
  for path in rem_it {
    entries.push(Entry::tombstone(path));
  }
  Ok((entries, cache, total))
}

pub(super) fn scan_dir(root: &Path) -> Result<(Vec<Entry>, Arc<SmallCache>, u64), UdpError> {
  entries_from_changes(root, scan_files(root)?, Vec::new())
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
    if p.len() > U15_MAX {
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
      VecWr::new(&mut raw).uvar(0);
      continue;
    }
    let nchunks = u32::try_from(e.chunks.len()).map_err(|_| UdpError::Arg("文件分块过多"))?;
    let mut w = VecWr::new(&mut raw);
    w.uvar(u64::from(nchunks) + 1);
    for c in &e.chunks {
      if c.len == 0 || c.len > CdcConfig::DEFAULT.max_size as u32 {
        return Err(UdpError::Arg("CDC 块长度越界"));
      }
      w.uvar(u64::from(c.len));
      w.bytes(&c.hash);
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
  let mut codec = Codec::new(ZSTD_DEFAULT_LEVEL)?;
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
        None => codec.insert(Codec::new(ZSTD_DEFAULT_LEVEL)?),
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
      if suf_len == 0 || pre > path_buf.len() || pre + suf_len > U15_MAX {
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
    let state_or_chunks = r.uvar()?;
    if state_or_chunks == 0 {
      if path.is_empty() {
        return Err(UdpError::Corrupt);
      }
      entries.push(Entry::tombstone(path));
      continue;
    }
    let nchunks =
      usize::try_from(state_or_chunks - 1).map_err(|_| UdpError::Corrupt)?;
    if nchunks > r.rest().len() / (1 + HASH_LEN) {
      return Err(UdpError::Corrupt);
    }
    let mut chunks = Vec::with_capacity(nchunks);
    let mut size = 0u64;
    for _ in 0..nchunks {
      let len = r.uvar_u32()?;
      if len == 0 || len > CdcConfig::DEFAULT.max_size as u32 {
        return Err(UdpError::Corrupt);
      }
      let hash: [u8; HASH_LEN] = r.array()?;
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
