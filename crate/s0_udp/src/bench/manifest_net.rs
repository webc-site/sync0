use std::{
  io::ErrorKind,
  net::{SocketAddr, UdpSocket},
  thread::sleep,
  time::Duration,
};

use s0_cdc::{Rd, SliceWr as Wr};

use super::{
  DGRAM, DIR_ACK, DIR_MAGIC, MANIFEST_CHUNK, MANIFEST_ZSTD, ZSTD_LEVEL,
  manifest::{HAVE_FLAGS, MAX_MANIFEST_ENCODED, MIN_ZSTD_BYTES},
  now_us, send_udp, wait_until,
};
use crate::{
  UdpError,
  codec::{Codec, SLACK},
  consts::{DIR_ACK_HDR_LEN, DIR_HDR_LEN, HAVE_ALL_ONES_BIT as HAVE_ALL_ONES, MAX_BLOCK},
};

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
    let passes = if missing <= 16 { 2 } else { 1 };
    let last_missing = ok.iter().rposition(|&b| !b);
    let mut sent_in_round = 0usize;
    for pass in 0..passes {
      for (i, chunk) in data.chunks(MANIFEST_CHUNK).enumerate() {
        if ok[i] {
          continue;
        }
        if !is_loop && sent_in_round > 0 {
          wait_until(now_us() + 25);
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
            wait_until(now_us() + 25);
          }
          send_udp(sock, &pkt[..len], peer)?;
          sent_in_round += 1;
        }
      }
    }
    let until = now_us() + wait_us;
    let mut partial_at: Option<u64> = None;
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
          if partial_at.is_none() {
            partial_at = Some(now);
          }
        }
        Ok(_) => {}
        Err(e) if e.kind() == ErrorKind::WouldBlock => {
          if let Some(p_at) = partial_at
            && now_us().saturating_sub(p_at) >= 20_000
          {
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
