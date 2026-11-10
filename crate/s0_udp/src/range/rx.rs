use std::{
  io,
  net::{SocketAddr, UdpSocket},
  slice::from_ref,
  time::{Duration, Instant},
};

use gxhash::HashMap;
use s0_cc::AntiAmp;
use s0_codec::{Codec, SLACK};
use s0_const::{
  DATA_FLAG_ZSTD, DATA_HDR_LEN, DEFAULT_PKT, DEFAULT_SEG_PAYLOAD, MAX_UDP_PKT, PKT_PUNCH,
  PUNCH_PRIMARY_WINDOW_MS, SID24_MASK, SOCK_POLL_TIMEOUT_MS, SUPER_EXTRA_SHIFT, SUPER_OFFSET_MASK,
  TUNNEL_SEG_PAYLOAD, VARINT_CONT_BIT, ZSTD_DEFAULT_LEVEL,
};
use s0_fs::{FsError, SharedSink};
use s0_wire::{DataHeader, decode_punch, encode_punch, encode_punch_padded};

use crate::{
  RangeSpec, UdpError, UdpStats, is_tunnel_addr, now_us,
  range::{
    PartialSlot, SUPER_MAX_BYTES, nack_timers_for_rtt, send_ack_burst, send_ack_pkts,
    send_nack_for_range, unpack_and_verify,
  },
  send_punch_burst,
};

const MAX_POOL_BUFS: usize = 64;
const MAX_SUPER_SEGS: u16 = 64;
const PROG_ACK_INIT_INTERVAL_US: u64 = 15_000;
const INIT_NACK_DELAY_US: u64 = 300_000;
const SOCK_POLL_TIMEOUT: Duration = Duration::from_millis(SOCK_POLL_TIMEOUT_MS);
const PUNCH_PRIMARY_WINDOW: Duration = Duration::from_millis(PUNCH_PRIMARY_WINDOW_MS);
const COMPLETE_WAIT_WINDOW: Duration = Duration::from_millis(250);

/// 原生 UDP 区间接收引擎（支持 SuperBlock 跨小文件批量解包 + $O(1)$ 内存缓冲池）
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
  let sid24 = spec.session_id & SID24_MASK;

  if sink.is_range_complete(spec.blk_lo, hi) {
    send_ack_burst(sock, peer_addrs, sid24, spec.blk_lo, hi);
    return Ok(UdpStats::default());
  }

  let is_loopback = peer_addrs.first().is_some_and(|a| a.ip().is_loopback());
  let (mut nack_interval_us, mut idle_nack_us, mut gap_nack_interval_us) =
    nack_timers_for_rtt(is_loopback, 0);
  let mut rtt_measured = is_loopback;
  let mut prog_ack_interval_us: u64 = PROG_ACK_INIT_INTERVAL_US;

  sock.set_read_timeout(Some(SOCK_POLL_TIMEOUT))?;
  let mut partials_map: HashMap<u32, PartialSlot> = HashMap::default();
  let mut buf_pool: Vec<Vec<u8>> = Vec::with_capacity(MAX_POOL_BUFS);
  let mut codec = Codec::new(ZSTD_DEFAULT_LEVEL)?;
  let mut dec_buf = vec![0u8; SUPER_MAX_BYTES];
  let mut pkt_buf = [0u8; MAX_UDP_PKT];
  let mut nack_buf = Vec::with_capacity(256);
  let (mut bytes_transferred, mut blocks_transferred) = (0u64, 0u32);
  let mut eff_wire_bytes = 0u64;
  let mut corrupt_strikes = 0u32;
  let (mut expected_blk, mut highest_seen_blk) = (spec.blk_lo, spec.blk_lo);
  let (mut has_unacked_gap, mut idle_nacked) = (false, false);
  let punch_start_us = now_us();
  let (mut last_nack_us, mut last_rx_us, mut last_prog_ack_us) =
    (punch_start_us, punch_start_us, punch_start_us);
  let mut last_ack_blks = 0u32;
  let mut amp: AntiAmp<SocketAddr> = AntiAmp::default();

  send_punch_burst(sock, &peer_addrs[..1], sid24);
  let padded_punch = encode_punch_padded(sid24);
  let punch_pkt = encode_punch(sid24);

  let mut timeout_us = spec.timeout.as_micros() as u64;
  let mut deadline_us = now_us().saturating_add(timeout_us);
  while now_us() < deadline_us {
    match sock.recv_from(&mut pkt_buf) {
      Ok((n, from)) => {
        if n > 0 && pkt_buf[0] == PKT_PUNCH {
          if decode_punch(&pkt_buf[..n]) == Ok(sid24) && amp.on_recv(from, n) {
            if n >= DEFAULT_PKT && !is_tunnel_addr(from) && amp.can_send(padded_punch.len()) {
              let _ = sock.send_to(&padded_punch, from);
              amp.on_send(padded_punch.len());
            }
            if amp.can_send(punch_pkt.len()) {
              let _ = sock.send_to(&punch_pkt, from);
              amp.on_send(punch_pkt.len());
            }
            locked_peer = Some(from);
          }
          continue;
        }
        if n < DATA_HDR_LEN || (pkt_buf[0] & VARINT_CONT_BIT) != 0 {
          continue;
        }
        let Ok(hdr) = DataHeader::decode(&pkt_buf[..n]) else {
          continue;
        };
        let pkt_sid24 = hdr.session_id & SID24_MASK;
        let zstd = (hdr.seg_cnt & DATA_FLAG_ZSTD) != 0;
        let seg_cnt = hdr.seg_cnt & !DATA_FLAG_ZSTD;
        let extra_blocks = (hdr.offset_in_block >> SUPER_EXTRA_SHIFT) as u8;
        let seg_off = (hdr.offset_in_block & SUPER_OFFSET_MASK) as usize;
        let idx_usize = hdr.seg_idx as usize;
        let valid_off =
          seg_off == idx_usize * DEFAULT_SEG_PAYLOAD || seg_off == idx_usize * TUNNEL_SEG_PAYLOAD;
        let req_end = seg_off + (hdr.payload_len as usize);
        if pkt_sid24 != sid24
          || hdr.block_idx < spec.blk_lo
          || hdr.block_idx + u32::from(extra_blocks) >= hi
          || seg_cnt == 0
          || hdr.seg_idx >= seg_cnt
          || seg_cnt > MAX_SUPER_SEGS
          || hdr.payload_len == 0
          || !valid_off
          || req_end > SUPER_MAX_BYTES + SLACK
        {
          continue;
        }
        let payload_end = DATA_HDR_LEN + (hdr.payload_len as usize);
        if payload_end > n || !amp.on_recv(from, n) {
          continue;
        }
        eff_wire_bytes += n.max(1024) as u64;
        locked_peer = Some(from);
        last_rx_us = now_us();
        if !rtt_measured {
          let rtt_us = last_rx_us.saturating_sub(punch_start_us);
          (nack_interval_us, idle_nack_us, gap_nack_interval_us) =
            nack_timers_for_rtt(is_loopback, rtt_us);
          prog_ack_interval_us = (rtt_us / 4).clamp(4_000, 250_000);
          timeout_us = timeout_us.max(rtt_us.saturating_mul(8));
          rtt_measured = true;
        }
        deadline_us = last_rx_us.saturating_add(timeout_us);
        idle_nacked = false;
        expected_blk = sink.next_missing(expected_blk, hi);
        if expected_blk >= hi || sink.is_range_complete(spec.blk_lo, hi) {
          send_ack_burst(sock, from_ref(&from), sid24, spec.blk_lo, hi);
          return Ok(UdpStats {
            bytes_transferred,
            blocks_transferred,
          });
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
              (sock, from_ref(&from), sid24),
              sink,
              (expected_blk, highest_seen_blk),
              &partials_map,
              &mut nack_buf,
              Some(&mut amp),
            )?;
          }
        } else {
          has_unacked_gap = false;
        }
        if (extra_blocks == 0 && sink.has_block(hdr.block_idx))
          || (extra_blocks > 0
            && sink.is_range_complete(hdr.block_idx, hdr.block_idx + u32::from(extra_blocks) + 1))
        {
          continue;
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
              corrupt_strikes = 0;
              amp.mark_validated();
              bytes_transferred += raw_len as u64;
              blocks_transferred += blks_cnt;
              highest_seen_blk = highest_seen_blk.max(hdr.block_idx + blks_cnt);
              if hdr.block_idx <= expected_blk {
                expected_blk = sink.next_missing(expected_blk, hi);
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
                });
              }
              if !is_loopback
                && (blocks_transferred.saturating_sub(last_ack_blks) >= 8
                  || last_rx_us >= last_prog_ack_us.saturating_add(prog_ack_interval_us))
              {
                last_ack_blks = blocks_transferred;
                last_prog_ack_us = last_rx_us;
                let prog_hi = (spec.blk_lo + blocks_transferred).min(hi.saturating_sub(1));
                if prog_hi > spec.blk_lo {
                  send_ack_pkts(
                    sock,
                    from_ref(&from),
                    sid24,
                    (eff_wire_bytes >> 10) as u32,
                    prog_hi,
                    1,
                  );
                }
              }
            }
            Err(FsError::BlockCorrupted(bad_blk)) => {
              corrupt_strikes += 1;
              if corrupt_strikes >= 2 {
                return Err(UdpError::Fs(FsError::BlockCorrupted(bad_blk)));
              }
              send_nack_for_range(
                (sock, from_ref(&from), sid24),
                sink,
                (bad_blk, bad_blk + u32::from(eb) + 1),
                &partials_map,
                &mut nack_buf,
                Some(&mut amp),
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
        } else if start.elapsed() < PUNCH_PRIMARY_WINDOW {
          &peer_addrs[..1]
        } else {
          peer_addrs
        };
        if sink.is_range_complete(spec.blk_lo, hi)
          && (locked_peer.is_some() || start.elapsed() >= COMPLETE_WAIT_WINDOW)
        {
          send_ack_burst(sock, targets, sid24, spec.blk_lo, hi);
          return Ok(UdpStats {
            bytes_transferred,
            blocks_transferred,
          });
        }
        let now = now_us();
        let nack_range = if locked_peer.is_none() || !rtt_measured {
          for addr in targets {
            let _ = sock.send_to(&punch_pkt, addr);
          }
          (now >= punch_start_us.saturating_add(INIT_NACK_DELAY_US)
            && now >= last_nack_us.saturating_add(nack_interval_us))
          .then_some((spec.blk_lo, hi))
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
          let amp_guard = locked_peer.is_some().then_some(&mut amp);
          send_nack_for_range(
            (sock, targets, sid24),
            sink,
            range,
            &partials_map,
            &mut nack_buf,
            amp_guard,
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
