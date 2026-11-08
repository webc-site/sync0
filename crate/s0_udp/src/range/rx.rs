use std::{
  io,
  net::{SocketAddr, UdpSocket},
  slice::from_ref,
  time::{Duration, Instant},
};

use gxhash::HashMap;
use s0_cc::MIN_TACK_US;
use s0_cdc::for_each_missing_span;

use super::{MAX_POISON_STRIKES, RangeSpec, SUPER_MAX_BYTES, TUNNEL_SEG_PAYLOAD, UdpStats};
use crate::{
  UdpError,
  codec::{Codec, SLACK},
  consts::{
    DATA_FLAG_ZSTD, DATA_HDR_LEN, DEFAULT_SEG_PAYLOAD, MAX_UDP_PKT, PKT_PUNCH, ZSTD_DEFAULT_LEVEL,
  },
  fs::{FsError, SharedSink},
  is_tunnel_addr, now_us, send_punch_burst,
  wire::{AckPkt, DataHeader, NackPkt, decode_punch, encode_punch, encode_punch_padded},
};

const MAX_POOL_BUFS: usize = 64;

/// 活跃分片重组槽位（仅在块尚未收齐分片期间驻留，收齐后立即回收 `buf` 到缓冲池，空间复杂度 $O(\text{乱序活跃块})$）
struct PartialSlot {
  extra_blocks: u8,
  seg_mask: u64,
  seg_cnt: u16,
  received_segs: u16,
  zstd: bool,
  buf: Vec<u8>,
}

/// 原生 UDP 区间接收引擎（支持 SuperBlock 跨小文件批量解包 + 逐文件 `gxhash128` 摘要防投毒校验 + $O(1)$ 内存缓冲池）
pub fn recv_range_udp(
  sock: &UdpSocket,
  peer_addrs: &[SocketAddr],
  sink: &SharedSink,
  spec: RangeSpec,
) -> Result<UdpStats, UdpError> {
  let start = Instant::now();
  if peer_addrs.is_empty() {
    return Err(UdpError::NoPeerAddr);
  }
  let mut locked_peer: Option<SocketAddr> = None;
  let hi = spec.blk_hi.min(sink.total_blocks());
  let sid24 = spec.session_id & 0x00FF_FFFF;

  if sink.is_range_complete(spec.blk_lo, hi) {
    send_ack_burst(sock, peer_addrs, sid24, spec.blk_lo, hi);
    return Ok(UdpStats::default());
  }

  let is_loopback = peer_addrs.first().is_some_and(|a| a.ip().is_loopback());
  let nack_interval_us: u64 = if is_loopback { 15_000 } else { 140_000 };
  let idle_nack_us: u64 = if is_loopback { 15_000 } else { 36_000 };
  let gap_nack_interval_us: u64 = if is_loopback { MIN_TACK_US } else { 20_000 };

  sock.set_read_timeout(Some(Duration::from_millis(6)))?;
  let mut partials_map: HashMap<u32, PartialSlot> = HashMap::default();
  let mut buf_pool: Vec<Vec<u8>> = Vec::with_capacity(MAX_POOL_BUFS);
  let mut codec = Codec::new(ZSTD_DEFAULT_LEVEL)?;
  let mut dec_buf = vec![0u8; SUPER_MAX_BYTES];
  let mut pkt_buf = [0u8; MAX_UDP_PKT];
  let mut nack_buf = Vec::with_capacity(256);
  let (mut bytes_transferred, mut blocks_transferred) = (0u64, 0u32);
  let (mut expected_blk, mut highest_seen_blk) = (spec.blk_lo, spec.blk_lo);
  let (mut has_unacked_gap, mut idle_nacked, mut poison_strikes) = (false, false, 0u8);
  let (mut last_nack_us, mut last_rx_us) = (now_us(), now_us());

  send_punch_burst(sock, peer_addrs, sid24);
  let padded_punch = encode_punch_padded(sid24);
  let punch_pkt = encode_punch(sid24);

  let timeout_us = spec.timeout.as_micros() as u64;
  let mut deadline_us = now_us().saturating_add(timeout_us);
  while now_us() < deadline_us {
    match sock.recv_from(&mut pkt_buf) {
      Ok((n, from)) => {
        if n > 0 && pkt_buf[0] == PKT_PUNCH {
          if decode_punch(&pkt_buf[..n]) == Ok(sid24) {
            if !is_tunnel_addr(from) {
              let _ = sock.send_to(&padded_punch, from);
            }
            let _ = sock.send_to(&punch_pkt, from);
            locked_peer = Some(from);
          }
          continue;
        }
        if n < DATA_HDR_LEN || (pkt_buf[0] & 0x80) != 0 {
          continue;
        }
        let Ok(hdr) = DataHeader::decode(&pkt_buf[..n]) else {
          continue;
        };
        let pkt_sid24 = hdr.session_id & 0x00FF_FFFF;
        let zstd = (hdr.seg_cnt & DATA_FLAG_ZSTD) != 0;
        let seg_cnt = hdr.seg_cnt & !DATA_FLAG_ZSTD;
        let extra_blocks = (hdr.offset_in_block >> 24) as u8;
        let seg_off = (hdr.offset_in_block & 0x00FF_FFFF) as usize;
        let idx_usize = hdr.seg_idx as usize;
        let valid_off =
          seg_off == idx_usize * DEFAULT_SEG_PAYLOAD || seg_off == idx_usize * TUNNEL_SEG_PAYLOAD;
        let req_end = seg_off + (hdr.payload_len as usize);
        if pkt_sid24 != sid24
          || hdr.block_idx < spec.blk_lo
          || hdr.block_idx + u32::from(extra_blocks) >= hi
          || seg_cnt == 0
          || hdr.seg_idx >= seg_cnt
          || seg_cnt > 64
          || hdr.payload_len == 0
          || !valid_off
          || req_end > SUPER_MAX_BYTES + SLACK
        {
          continue;
        }
        locked_peer = Some(from);
        last_rx_us = now_us();
        deadline_us = last_rx_us.saturating_add(timeout_us);
        idle_nacked = false;
        let payload_end = DATA_HDR_LEN + (hdr.payload_len as usize);
        if payload_end > n {
          continue;
        }
        while expected_blk < hi && sink.has_block(expected_blk) {
          expected_blk += 1;
        }
        if expected_blk >= hi {
          send_ack_burst(sock, from_ref(&from), sid24, spec.blk_lo, hi);
          return Ok(UdpStats {
            bytes_transferred,
            blocks_transferred,
            elapsed_us: start.elapsed().as_micros() as u64,
          });
        }
        if (extra_blocks == 0 && sink.has_block(hdr.block_idx))
          || (extra_blocks > 0
            && sink.is_range_complete(hdr.block_idx, hdr.block_idx + u32::from(extra_blocks) + 1))
        {
          continue;
        }
        if hdr.block_idx > highest_seen_blk {
          let prev_high = highest_seen_blk;
          highest_seen_blk = hdr.block_idx;
          if !sink.is_range_complete(prev_high, hdr.block_idx) {
            has_unacked_gap = true;
          }
        }
        if expected_blk < highest_seen_blk {
          let wait_us = if has_unacked_gap {
            gap_nack_interval_us
          } else {
            nack_interval_us
          };
          if last_rx_us >= last_nack_us.saturating_add(wait_us) {
            last_nack_us = last_rx_us;
            has_unacked_gap = false;
            send_nack_for_range(
              sock,
              from_ref(&from),
              sid24,
              sink,
              (expected_blk, highest_seen_blk),
              &partials_map,
              &mut nack_buf,
            )?;
          }
        } else {
          has_unacked_gap = false;
        }

        let completed_slot = if seg_cnt == 1 {
          Some((
            extra_blocks,
            zstd,
            &pkt_buf[DATA_HDR_LEN..payload_end],
            None,
          ))
        } else {
          let slot = partials_map.entry(hdr.block_idx).or_insert_with(|| {
            let mut b = buf_pool
              .pop()
              .unwrap_or_else(|| Vec::with_capacity(SUPER_MAX_BYTES));
            b.clear();
            PartialSlot {
              extra_blocks,
              seg_mask: 0,
              seg_cnt,
              received_segs: 0,
              zstd,
              buf: b,
            }
          });
          if slot.seg_cnt != seg_cnt || slot.zstd != zstd || slot.extra_blocks != extra_blocks {
            slot.extra_blocks = extra_blocks;
            slot.seg_cnt = seg_cnt;
            slot.zstd = zstd;
            slot.seg_mask = 0;
            slot.received_segs = 0;
            slot.buf.clear();
          }

          let bit = 1u64 << hdr.seg_idx;
          if (slot.seg_mask & bit) == 0 {
            if slot.buf.len() < req_end {
              slot.buf.resize(req_end, 0);
            }
            slot.buf[seg_off..req_end].copy_from_slice(&pkt_buf[DATA_HDR_LEN..payload_end]);
            slot.seg_mask |= bit;
            slot.received_segs += 1;
          }

          if slot.received_segs == slot.seg_cnt {
            let done = partials_map.remove(&hdr.block_idx).unwrap();
            Some((done.extra_blocks, done.zstd, &[][..], Some(done.buf)))
          } else {
            None
          }
        };

        if let Some((eb, is_zstd, inline_slice, owned_buf)) = completed_slot {
          let wire_bytes = owned_buf.as_deref().unwrap_or(inline_slice);
          let unpack_res = unpack_and_verify(
            sink,
            &mut codec,
            &mut dec_buf,
            hdr.block_idx,
            eb,
            is_zstd,
            wire_bytes,
          );
          if let Some(b) = owned_buf {
            recycle_buf(&mut buf_pool, b);
          }

          match unpack_res {
            Ok((raw_len, blks_cnt)) => {
              bytes_transferred += raw_len as u64;
              blocks_transferred += blks_cnt;
              highest_seen_blk = highest_seen_blk.max(hdr.block_idx + blks_cnt);
              if hdr.block_idx <= expected_blk {
                while expected_blk < hi && sink.has_block(expected_blk) {
                  expected_blk += 1;
                }
              }
              if expected_blk >= hi || sink.is_range_complete(spec.blk_lo, hi) {
                let targets = if let Some(ref p) = locked_peer {
                  from_ref(p)
                } else {
                  peer_addrs
                };
                send_ack_burst(sock, targets, sid24, spec.blk_lo, hi);
                return Ok(UdpStats {
                  bytes_transferred,
                  blocks_transferred,
                  elapsed_us: start.elapsed().as_micros() as u64,
                });
              }
            }
            Err(FsError::BlockCorrupted(bad_blk)) => {
              poison_strikes = poison_strikes.saturating_add(1);
              if poison_strikes >= MAX_POISON_STRIKES {
                return Err(UdpError::PoisonedPeer(bad_blk));
              }
              send_nack_for_range(
                sock,
                from_ref(&from),
                sid24,
                sink,
                (bad_blk, bad_blk + u32::from(eb) + 1),
                &partials_map,
                &mut nack_buf,
              )?;
            }
            Err(e) => return Err(UdpError::Fs(e)),
          }
        }
      }
      Err(e)
        if matches!(
          e.kind(),
          io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
        ) =>
      {
        let targets = if let Some(ref p) = locked_peer {
          from_ref(p)
        } else if start.elapsed() < Duration::from_millis(15) {
          &peer_addrs[..1]
        } else {
          peer_addrs
        };
        if sink.is_range_complete(spec.blk_lo, hi) {
          send_ack_burst(sock, targets, sid24, spec.blk_lo, hi);
          return Ok(UdpStats {
            bytes_transferred,
            blocks_transferred,
            elapsed_us: start.elapsed().as_micros() as u64,
          });
        }
        let now = now_us();
        let nack_range = if locked_peer.is_none() {
          for addr in targets {
            let _ = sock.send_to(&punch_pkt, addr);
          }
          (now >= last_nack_us.saturating_add(nack_interval_us)).then_some((spec.blk_lo, hi))
        } else if now >= last_rx_us.saturating_add(idle_nack_us)
          && ((!idle_nacked && now >= last_nack_us.saturating_add(idle_nack_us))
            || now >= last_nack_us.saturating_add(nack_interval_us))
        {
          idle_nacked = true;
          Some((expected_blk, hi))
        } else if expected_blk < highest_seen_blk
          && now >= last_nack_us.saturating_add(nack_interval_us)
        {
          Some((expected_blk, highest_seen_blk))
        } else {
          None
        };
        if let Some(range) = nack_range {
          last_nack_us = now;
          send_nack_for_range(
            sock,
            targets,
            sid24,
            sink,
            range,
            &partials_map,
            &mut nack_buf,
          )?;
        }
      }
      Err(e) => return Err(UdpError::Io(e)),
    }
  }

  Err(UdpError::Timeout(spec.blk_lo, hi))
}

#[inline]
fn recycle_buf(pool: &mut Vec<Vec<u8>>, mut b: Vec<u8>) {
  if pool.len() < MAX_POOL_BUFS {
    b.clear();
    pool.push(b);
  }
}

/// 解压并逐块校验 `gxhash128` 摘要（支持单块与 SuperBlock 跨小文件批量解包，任意子块篡改立即返回 `BlockCorrupted`）
fn unpack_and_verify(
  sink: &SharedSink,
  codec: &mut Codec,
  dec_buf: &mut [u8],
  start_blk: u32,
  extra_blocks: u8,
  is_zstd: bool,
  wire_bytes: &[u8],
) -> Result<(usize, u32), FsError> {
  let raw_slice = if is_zstd {
    let raw_len = codec
      .decompress_block(wire_bytes, dec_buf)
      .map_err(|_| FsError::BlockCorrupted(start_blk))?;
    &dec_buf[..raw_len]
  } else {
    wire_bytes
  };

  if extra_blocks == 0 {
    let newly_written = sink.write_verified_block(start_blk, raw_slice)?;
    return Ok(if newly_written {
      (raw_slice.len(), 1)
    } else {
      (0, 0)
    });
  }

  let mut cursor = 0usize;
  let mut written_bytes = 0usize;
  let mut written_blks = 0u32;
  let count = 1 + u32::from(extra_blocks);
  for b in start_blk..start_blk + count {
    let loc = sink.block_loc(b).ok_or(FsError::BlockCorrupted(b))?;
    let len = loc.len as usize;
    if cursor + len > raw_slice.len() {
      return Err(FsError::BlockCorrupted(b));
    }
    if sink.write_verified_block(b, &raw_slice[cursor..cursor + len])? {
      written_bytes += len;
      written_blks += 1;
    }
    cursor += len;
  }
  if cursor != raw_slice.len() {
    return Err(FsError::BlockCorrupted(start_blk));
  }
  Ok((written_bytes, written_blks))
}

fn send_nack_for_range(
  sock: &UdpSocket,
  targets: &[SocketAddr],
  session_id: u32,
  sink: &SharedSink,
  range: (u32, u32),
  partials_map: &HashMap<u32, PartialSlot>,
  nack_buf: &mut Vec<u8>,
) -> Result<(), UdpError> {
  let have = sink.snapshot_have_bits();
  let mut spans = Vec::new();
  for_each_missing_span(&have, range.0, range.1, |lo, hi| {
    spans.push((lo, hi));
  });
  let mut partials = Vec::with_capacity(partials_map.len().min(64));
  for chunk in spans.chunks(256) {
    partials.clear();
    if !partials_map.is_empty() {
      'outer: for &(lo, hi) in chunk {
        for blk in lo..hi {
          if let Some(slot) = partials_map.get(&blk)
            && slot.seg_mask != 0
          {
            partials.push((blk, slot.seg_mask));
            if partials.len() >= 64 {
              break 'outer;
            }
          }
        }
      }
    }
    nack_buf.clear();
    NackPkt::encode_parts(session_id, chunk, &partials, nack_buf)?;
    for addr in targets {
      let _ = sock.send_to(nack_buf, addr);
    }
  }
  Ok(())
}

fn send_ack_burst(
  sock: &UdpSocket,
  targets: &[SocketAddr],
  session_id: u32,
  blk_lo: u32,
  blk_hi: u32,
) {
  let mut raw = [0u8; 12];
  AckPkt {
    session_id,
    blk_lo,
    blk_hi,
  }
  .encode(&mut raw);
  for _ in 0..3 {
    for addr in targets {
      let _ = sock.send_to(&raw, addr);
    }
  }
}
