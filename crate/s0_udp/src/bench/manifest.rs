use std::{path::Path, sync::Arc};

use s0_cdc::{CdcConfig, ChunkSig, Rd, WireError, Wr as VecWr};
use s0_codec::{Codec, SLACK};
use s0_const::{
  DIR_ACK_HDR_LEN, HAVE_ALL_ONES_BIT, MANIFEST_CHUNK, MANIFEST_DELETE, MANIFEST_ZSTD, MAX_BLOCK,
  MAX_DATAGRAM, MIN_ZSTD_BYTES, U15_MAX, ZSTD_DEFAULT_LEVEL,
};
use s0_fs::{SmallCache, rel_path, scan_and_cache_files, scan_files};

use crate::{ArgError, UdpError};

/// 单包 `DIR_ACK` 可容纳的最大位图字节数
const MAX_MANIFEST_BITMAP: usize = MAX_DATAGRAM - DIR_ACK_HDR_LEN - 4;
/// 清单编码后最大字节数（受限于单包 `DIR_ACK` 位图容量）
pub(crate) const MAX_MANIFEST_ENCODED: usize = MAX_MANIFEST_BITMAP * 8 * MANIFEST_CHUNK;
/// 清单解压后最大字节数（64 MiB）
const MAX_MANIFEST_RAW: usize = 64 << 20;
pub(crate) const HAVE_FLAGS: u32 = MANIFEST_ZSTD | HAVE_ALL_ONES_BIT;
const _: () = assert!(CdcConfig::DEFAULT.max_size <= (u16::MAX as usize) + 1);

/// 清单文件条目：`path == ""` 表示单文件模式，否则为目录内相对路径
#[derive(Clone, Debug)]
pub(crate) struct Entry {
  pub(crate) path: String,
  pub(crate) size: u64,
  pub(crate) chunks: Vec<ChunkSig>,
  pub(crate) cached: Option<u64>,
}

/// 扫描目录并转为带 FastCDC 签名的条目、小文件连续缓存池与逻辑总字节数
pub(crate) fn scan_dir(root: &Path) -> Result<(Vec<Entry>, Arc<SmallCache>, u64), UdpError> {
  let scanned = scan_files(root)?;
  let (all_chunks, cache) = scan_and_cache_files(root, &scanned)?;
  let mut entries = Vec::with_capacity(scanned.len());
  let mut total = 0u64;
  for (i, (f, chunks)) in scanned.into_iter().zip(all_chunks).enumerate() {
    total = total.saturating_add(f.size);
    entries.push(Entry {
      path: f.path,
      size: f.size,
      chunks,
      cached: cache.offset_of(i),
    });
  }
  Ok((entries, cache, total))
}

pub(crate) fn encode_manifest(entries: &[Entry], delete: bool) -> Result<Vec<u8>, UdpError> {
  let Ok(cnt) = u32::try_from(entries.len()) else {
    return Err(WireError::Overflow.into());
  };
  if cnt & MANIFEST_DELETE != 0 {
    return Err(WireError::Overflow.into());
  }
  let est_chunks: usize = entries.iter().map(|e| e.chunks.len()).sum();
  let mut raw = Vec::with_capacity(4 + entries.len() * 12 + est_chunks * 18);
  let count_hdr = cnt | if delete { MANIFEST_DELETE } else { 0 };
  raw.extend_from_slice(&count_hdr.to_le_bytes());
  let mut prev_path: &[u8] = &[];
  for (i, e) in entries.iter().enumerate() {
    let p = e.path.as_bytes();
    if p.len() > U15_MAX {
      return Err(WireError::Overflow.into());
    }
    if i > 0 && prev_path >= p {
      return Err(ArgError::UnsortedPath.into());
    }
    VecWr::new(&mut raw).front_str(i == 0, &mut prev_path, p)?;
    let nchunks = u32::try_from(e.chunks.len()).map_err(|_| WireError::Overflow)?;
    let mut w = VecWr::new(&mut raw);
    w.uvar(u64::from(nchunks) + 1);
    ChunkSig::encode_slice(&e.chunks, &mut w)?;
  }
  if raw.len() > MAX_MANIFEST_RAW {
    return Err(WireError::Overflow.into());
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
    return Err(WireError::Overflow.into());
  }
  Ok(out)
}

pub(crate) fn parse_manifest(b: &[u8]) -> Result<(Vec<Entry>, u64, bool), UdpError> {
  let mut rd = Rd::new(b);
  let raw_len = rd.u32()? as usize;
  if !(4..=MAX_MANIFEST_RAW).contains(&raw_len) {
    return Err(UdpError::Corrupt);
  }
  let mut raw = Vec::with_capacity(raw_len.min(MAX_BLOCK));
  let mut codec: Option<Codec> = None;
  while !rd.is_empty() {
    let hdr = rd.u32()?;
    let zstd = hdr & MANIFEST_ZSTD != 0;
    let clen = (hdr & !MANIFEST_ZSTD) as usize;
    let chunk = rd.bytes(clen)?;
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
    return Err(UdpError::Corrupt);
  }

  let mut r = Rd::new(&raw);
  let count_hdr = r.u32()?;
  let delete = count_hdr & MANIFEST_DELETE != 0;
  let n = (count_hdr & !MANIFEST_DELETE) as usize;
  if n > (raw.len() - 4) / 2 {
    return Err(UdpError::Corrupt);
  }
  let mut entries: Vec<Entry> = Vec::with_capacity(n.min(65536));
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
      let suf = r.bytes(suf_len)?;
      if i > 0 && pre < path_buf.len() && suf[0] <= path_buf[pre] {
        return Err(UdpError::Corrupt);
      }
      path_buf.truncate(pre);
      path_buf.extend_from_slice(suf);
      rel_path(&path_buf)?.to_owned()
    };
    let state_or_chunks = r.uvar()?;
    if state_or_chunks == 0 {
      return Err(UdpError::Corrupt);
    }
    let nchunks = usize::try_from(state_or_chunks - 1).map_err(|_| UdpError::Corrupt)?;
    let (chunks, size) = ChunkSig::decode_vec(&mut r, nchunks).map_err(|_| UdpError::Corrupt)?;
    entries.push(Entry {
      path,
      size,
      chunks,
      cached: None,
    });
    total = total.saturating_add(size);
  }
  r.expect_end()?;
  Ok((entries, total, delete))
}
