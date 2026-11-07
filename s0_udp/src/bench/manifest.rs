use std::{
  fs::File,
  io::ErrorKind,
  net::{SocketAddr, UdpSocket},
  path::Path,
  thread::sleep,
  time::Duration,
};

use s0_cdc::{ChunkSig, Config, FileAt, chunk_file_with_buf, chunk_slice};
use s0_codec::{Codec, MAX_BLOCK, SLACK};
pub(super) use s0_const::{
  DIR_ACK_HDR_LEN, DIR_HDR_LEN, HAVE_ALL_ONES_BIT as HAVE_ALL_ONES, MANIFEST_DELETE,
  MANIFEST_PARTIAL, TOMBSTONE_NCHUNKS,
};
use s0_fs::{ScannedFile, rel_path, scan_files};
use s0_wire::{Rd, SliceWr as Wr};

use super::{
  DGRAM, DIR_ACK, DIR_MAGIC, MANIFEST_CHUNK, MANIFEST_ZSTD, RootBuf, ZSTD_LEVEL, now_us, send_udp,
  wait_until,
};
use crate::UdpError;

/// 单包 `DIR_ACK` 可容纳的最大位图字节数
const MAX_MANIFEST_BITMAP: usize = DGRAM - DIR_ACK_HDR_LEN - 4;
/// 清单编码后最大字节数（受限于单包 `DIR_ACK` 位图容量）
const MAX_MANIFEST_ENCODED: usize = MAX_MANIFEST_BITMAP * 8 * MANIFEST_CHUNK;
/// 清单解压后最大字节数（64 MiB）
const MAX_MANIFEST_RAW: usize = 64 << 20;
/// 清单最大文件条目数
const MAX_MANIFEST_ENTRIES: usize = 1_000_000;
/// 发送端扫描期小文件内存池上限（16 MiB）：避免 `DIR_ACK` 后二次 `open`/`pread` 海量小文件
const MAX_SMALL_ARENA: usize = 16 << 20;
/// 启用 Zstd 压缩的最小清单/位图字节阈值
const MIN_ZSTD_BYTES: usize = 256;
/// 路径长度上限
const MAX_PATH_LEN: usize = 0x7FFF;
const MANIFEST_FLAGS: u32 = MANIFEST_DELETE | MANIFEST_PARTIAL;
const HAVE_FLAGS: u32 = MANIFEST_ZSTD | HAVE_ALL_ONES;
const _: () = assert!(Config::DEFAULT.max_size <= (u16::MAX as usize) + 1);
const _: () = assert!((MAX_MANIFEST_ENTRIES as u32) & MANIFEST_FLAGS == 0);

#[inline]
fn push_u15(out: &mut Vec<u8>, v: usize) {
  if v < 0x80 {
    out.push(v as u8);
  } else {
    out.push(0x80 | ((v >> 8) as u8));
    out.push(v as u8);
  }
}

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
  let mut root_buf = RootBuf::new(root);
  let mut entries = Vec::with_capacity(scanned.len() + removed.len());
  let mut arena = Vec::new();
  let mut rem_it = removed.into_iter().peekable();
  let mut io_buf = Vec::new();
  let mut total = 0u64;
  let max_small = Config::DEFAULT.max_size as u64;
  for f in scanned {
    while let Some(path) = rem_it.next_if(|p| *p < f.path) {
      entries.push(Entry {
        path,
        size: 0,
        chunks: Vec::new(),
        del: true,
        cached: None,
      });
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
      let chunks = chunk_slice(&arena[base..base + n], Config::DEFAULT);
      (chunks, Some(base as u32))
    } else {
      let file = File::open(root_buf.join(&f.path))?;
      (
        chunk_file_with_buf(&file, f.size, Config::DEFAULT, &mut io_buf)?,
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
    entries.push(Entry {
      path,
      size: 0,
      chunks: Vec::new(),
      del: true,
      cached: None,
    });
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
      push_u15(&mut raw, p.len());
      raw.extend_from_slice(p);
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
      push_u15(&mut raw, pre);
      push_u15(&mut raw, suf.len());
      raw.extend_from_slice(suf);
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
      if c.len == 0 || c.len > Config::DEFAULT.max_size as u32 {
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
      entries.push(Entry {
        path,
        size: 0,
        chunks: Vec::new(),
        del: true,
        cached: None,
      });
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

pub(super) fn try_build_final_dir_ack(
  base_ack: &[u8],
  mut have_bits: &[u8],
  all_hit: bool,
) -> Result<Option<Vec<u8>>, UdpError> {
  while have_bits.last() == Some(&0) {
    have_bits = &have_bits[..have_bits.len() - 1];
  }
  let max_payload = DGRAM.saturating_sub(base_ack.len() + 4);
  if have_bits.is_empty() {
    let mut out = Vec::with_capacity(base_ack.len() + 4);
    out.extend_from_slice(base_ack);
    out.extend_from_slice(&0u32.to_le_bytes());
    return Ok(Some(out));
  }
  if all_hit && have_bits.len() <= MAX_BLOCK {
    let mut out = Vec::with_capacity(base_ack.len() + 4);
    out.extend_from_slice(base_ack);
    let hdr = (have_bits.len() as u32) | HAVE_ALL_ONES;
    out.extend_from_slice(&hdr.to_le_bytes());
    return Ok(Some(out));
  }
  if max_payload == 0 {
    return Ok(None);
  }
  let mut out = Vec::with_capacity(base_ack.len() + 4 + have_bits.len().min(max_payload));
  out.extend_from_slice(base_ack);
  if have_bits.len() > MIN_ZSTD_BYTES && have_bits.len() <= MAX_BLOCK {
    let mut codec = Codec::new(ZSTD_LEVEL)?;
    let mut dst = vec![0u8; have_bits.len() + SLACK];
    if let Some(clen) = codec.try_compress_block(have_bits, &mut dst)?
      && clen <= max_payload
    {
      let hdr = (have_bits.len() as u32) | MANIFEST_ZSTD;
      out.extend_from_slice(&hdr.to_le_bytes());
      out.extend_from_slice(&dst[..clen]);
      return Ok(Some(out));
    }
  }
  if have_bits.len() > max_payload {
    return Ok(None);
  }
  out.extend_from_slice(&(have_bits.len() as u32).to_le_bytes());
  out.extend_from_slice(have_bits);
  Ok(Some(out))
}

fn decode_have_tail(tail: &[u8]) -> Result<Vec<u8>, UdpError> {
  let mut rd = Rd::new(tail);
  let hdr = rd.u32()?;
  if hdr == 0 {
    return Ok(Vec::new());
  }
  let zstd = hdr & MANIFEST_ZSTD != 0;
  let all_ones = hdr & HAVE_ALL_ONES != 0;
  let raw_len = (hdr & !HAVE_FLAGS) as usize;
  if raw_len > MAX_BLOCK {
    return Err(UdpError::Corrupt);
  }
  if all_ones {
    return Ok(vec![0xFFu8; raw_len]);
  }
  let body = rd.rest();
  if zstd {
    let mut codec = Codec::new(ZSTD_LEVEL)?;
    let mut out = vec![0u8; raw_len];
    if codec.decompress_block(body, &mut out)? != raw_len {
      return Err(UdpError::Corrupt);
    }
    Ok(out)
  } else {
    if body.len() < raw_len {
      return Err(UdpError::Corrupt);
    }
    Ok(body[..raw_len].to_vec())
  }
}

/// 发送清单并等待完成 `DIR_ACK`，在等待期间调用 `on_idle()` 预读/预压缩首轮块，隐藏跨洋 RTT 延迟
pub(super) fn send_manifest(
  sock: &UdpSocket,
  peer: SocketAddr,
  data: &[u8],
  mut on_idle: impl FnMut() -> bool,
) -> Result<(u64, Vec<u8>), UdpError> {
  if data.len() > MAX_MANIFEST_ENCODED {
    return Err(UdpError::Arg("清单过大"));
  }
  let n = data.len().div_ceil(MANIFEST_CHUNK);
  let dir_ack_le = DIR_ACK.to_le_bytes();
  let chunks_le = (n as u32).to_le_bytes();
  let mut ok = vec![false; n];
  let total = data.len() as u32;
  let bitmap_len = n.div_ceil(8);
  let full_ack_min = DIR_ACK_HDR_LEN + bitmap_len + 4;
  let mut pkt = [0u8; DIR_HDR_LEN + MANIFEST_CHUNK];
  let mut buf = [0u8; DGRAM];
  let mut wait_us = 450_000u64;
  let mut rtt_us = 0u64;
  let mut last_progress = now_us();
  let is_loop = peer.ip().is_loopback();
  loop {
    let round_start = now_us();
    let missing = ok.iter().filter(|&&b| !b).count();
    let passes = if missing <= 8 { 2 } else { 1 };
    let last_missing = ok.iter().rposition(|&b| !b);
    let mut sent_in_round = 0usize;
    for pass in 0..passes {
      for (i, chunk) in data.chunks(MANIFEST_CHUNK).enumerate() {
        if ok[i] {
          continue;
        }
        if !is_loop && sent_in_round > 0 {
          wait_until(now_us() + 50);
        }
        let mut w = Wr::new(&mut pkt);
        let _ = w.u32(DIR_MAGIC);
        let _ = w.u32(total);
        let _ = w.u32(i as u32);
        let _ = w.bytes(chunk);
        let len = w.pos();
        send_udp(sock, &pkt[..len], peer)?;
        sent_in_round += 1;
        if passes == 1 && pass == 0 && Some(i) == last_missing {
          if !is_loop {
            wait_until(now_us() + 50);
          }
          send_udp(sock, &pkt[..len], peer)?;
          sent_in_round += 1;
        }
      }
    }
    let until = now_us() + wait_us;
    let mut got_partial = false;
    while now_us() < until {
      match sock.recv_from(&mut buf) {
        Ok((len, _))
          if len >= DIR_ACK_HDR_LEN + bitmap_len
            && buf[..4] == dir_ack_le
            && buf[4..DIR_ACK_HDR_LEN] == chunks_le =>
        {
          let now = now_us();
          if rtt_us == 0 {
            rtt_us = now.saturating_sub(round_start).max(1);
            wait_us = (rtt_us * 3 / 2).clamp(25_000, 2_000_000);
          }
          let seen = &buf[DIR_ACK_HDR_LEN..DIR_ACK_HDR_LEN + bitmap_len];
          for (i, done) in ok.iter_mut().enumerate() {
            if !*done && (seen[i / 8] >> (i % 8) & 1 == 1) {
              *done = true;
              last_progress = now;
            }
          }
          if len >= full_ack_min {
            let have = decode_have_tail(&buf[DIR_ACK_HDR_LEN + bitmap_len..len])?;
            return Ok((rtt_us, have));
          }
          got_partial = true;
        }
        Ok(_) => {}
        Err(e) if e.kind() == ErrorKind::WouldBlock => {
          if got_partial {
            break;
          }
          if on_idle() {
            continue;
          }
          if is_loop {
            wait_until(now_us() + 20);
          } else {
            sleep(Duration::from_micros(200));
          }
        }
        Err(e) => return Err(e.into()),
      }
    }
    if rtt_us == 0 {
      wait_us = (wait_us * 3 / 2).min(2_000_000);
    }
    if now_us().saturating_sub(last_progress) > 15_000_000 {
      return Err(UdpError::StreamTimeout);
    }
  }
}

/// 清单接收缓冲：单块连续内存 + 原位维护 ACK 位图，零分片堆分配
pub(super) struct ManifestRx {
  pub(super) data: Vec<u8>,
  pub(super) ack: Vec<u8>,
  pub(super) chunks: usize,
  pub(super) rem: usize,
  pub(super) rx: usize,
}

impl ManifestRx {
  pub(super) const fn valid_chunk(total_len: usize, idx: usize, payload_len: usize) -> bool {
    if total_len < 4 || total_len > MAX_MANIFEST_ENCODED {
      return false;
    }
    let chunks = total_len.div_ceil(MANIFEST_CHUNK);
    if idx >= chunks {
      return false;
    }
    let off = idx * MANIFEST_CHUNK;
    let rem = total_len - off;
    let want = if rem < MANIFEST_CHUNK {
      rem
    } else {
      MANIFEST_CHUNK
    };
    payload_len >= want
  }

  pub(super) fn new(total_len: usize) -> Option<Self> {
    if !(4..=MAX_MANIFEST_ENCODED).contains(&total_len) {
      return None;
    }
    let chunks = total_len.div_ceil(MANIFEST_CHUNK);
    let mut ack = vec![0u8; DIR_ACK_HDR_LEN + chunks.div_ceil(8)];
    ack[..4].copy_from_slice(&DIR_ACK.to_le_bytes());
    ack[4..DIR_ACK_HDR_LEN].copy_from_slice(&(chunks as u32).to_le_bytes());
    Some(Self {
      data: vec![0u8; total_len],
      ack,
      chunks,
      rem: chunks,
      rx: 0,
    })
  }

  pub(super) fn on_chunk(&mut self, idx: usize, payload: &[u8]) {
    if idx >= self.chunks {
      return;
    }
    let off = idx * MANIFEST_CHUNK;
    let want = (self.data.len() - off).min(MANIFEST_CHUNK);
    if payload.len() < want {
      return;
    }
    let byte = &mut self.ack[DIR_ACK_HDR_LEN + idx / 8];
    let bit = 1u8 << (idx % 8);
    if *byte & bit == 0 {
      *byte |= bit;
      self.data[off..off + want].copy_from_slice(&payload[..want]);
      self.rem -= 1;
      self.rx += 1;
    }
  }
}
