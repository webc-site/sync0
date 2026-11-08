use std::{
  io,
  net::{SocketAddr, UdpSocket},
  time::{Duration, Instant},
};

use s0_cc::AntiAmp;
use s0_cdc::get_bit;

use super::{
  MAX_NACK_RESEND_BATCH, Pacer, RangeSpec, SUPER_MAX_BYTES, SUPER_MAX_EXTRA, SUPER_SMALL_LIMIT,
  TX_CACHE_SLOTS, TxCacheSlot, UdpStats, WAN_DEFAULT_RATE_BPS, seg_payload_for_addr,
};
use crate::{
  BlockSource, UdpError,
  bench::{handle_probe_observed, now_us, send_udp},
  codec::{Codec, Detector},
  consts::{
    DATA_FLAG_ZSTD, DATA_HDR_LEN, MAX_UDP_PKT, PKT_ACK, PKT_NACK, PKT_PUNCH, ZSTD_DEFAULT_LEVEL,
  },
  wire::{AckPkt, DataHeader, NackPkt, decode_punch, encode_punch},
};

struct SendCtx<'a> {
  sock: &'a UdpSocket,
  peer_addr: SocketAddr,
  seg_payload: usize,
  session_id: u32,
  blk_lo: u32,
  last_entry_idx: u32,
  init_cooldown_us: u64,
  retrans_cooldown_us: u64,
  source: BlockSource<'a>,
  codec: Codec,
  detector: Detector,
  block_buf: Vec<u8>,
  pkt_buf: [u8; MAX_UDP_PKT],
  pacer: Pacer,
  cache: Vec<TxCacheSlot>,
  last_sent_us: Vec<u64>,
  last_retrans_us: Vec<u64>,
}

impl<'a> SendCtx<'a> {
  #[inline]
  fn update_peer_addr(&mut self, from: SocketAddr) {
    if self.peer_addr != from {
      self.peer_addr = from;
      self.seg_payload = seg_payload_for_addr(from);
    }
  }

  /// 发送连续小文件聚合块（SuperBlock）或常规单块，返回 `(实际发送原始字节数, 覆盖的块数, 末尾块索引)`
  fn send_next_coalesced(
    &mut self,
    start_blk: u32,
    hi: u32,
    have_bits: &[u8],
    dup_tail: bool,
  ) -> Result<(u64, u32, u32), UdpError> {
    let Some(first_loc) = self.source.block_loc(start_blk) else {
      return Ok((0, 0, start_blk));
    };
    if first_loc.entry_idx != self.last_entry_idx {
      self.last_entry_idx = first_loc.entry_idx;
      self.detector.reset_file();
    }
    let coalesce_limit = if self.detector.is_compressible() {
      SUPER_SMALL_LIMIT
    } else {
      16_000
    };
    if first_loc.len > coalesce_limit {
      let tail = dup_tail && start_blk + 1 >= hi;
      let bytes = self.prepare_and_send(start_blk, 0, first_loc.len as usize, 0, tail)?;
      return Ok((bytes, u32::from(bytes > 0), start_blk));
    }

    // 尝试将连续缺失的小文件或可压块聚合成单个 <= 64KiB 的 SuperBlock（跨块联合 Zstd 压缩，零头部膨胀）
    let mut total_raw = first_loc.len as usize;
    let mut extra = 0u8;
    let mut cur_blk = start_blk + 1;
    while cur_blk < hi && extra < SUPER_MAX_EXTRA {
      if get_bit(have_bits, cur_blk) || !self.source.has_block(cur_blk) {
        break;
      }
      let Some(loc) = self.source.block_loc(cur_blk) else {
        break;
      };
      if loc.len > coalesce_limit || total_raw + (loc.len as usize) > SUPER_MAX_BYTES {
        break;
      }
      total_raw += loc.len as usize;
      extra += 1;
      cur_blk += 1;
    }

    if extra == 0 {
      let tail = dup_tail && cur_blk >= hi;
      let bytes = self.prepare_and_send(start_blk, 0, first_loc.len as usize, 0, tail)?;
      return Ok((bytes, u32::from(bytes > 0), start_blk));
    }

    self.block_buf.resize(total_raw, 0);
    let mut offset = 0usize;
    for b in start_blk..=start_blk + u32::from(extra) {
      let loc = self.source.block_loc(b).ok_or(UdpError::Corrupt)?;
      let len = loc.len as usize;
      self
        .source
        .read_loc_into(b, loc, &mut self.block_buf[offset..offset + len])?;
      offset += len;
    }

    let tail = dup_tail && cur_blk >= hi;
    let bytes = self.compress_and_transmit(start_blk, extra, total_raw, 0, tail)?;
    let count = 1 + u32::from(extra);
    Ok((bytes, count, start_blk + u32::from(extra)))
  }

  /// 发送单个块或已缓存 SuperBlock（支持 64-bit 分片掩码过滤与 2048 槽位环缓存命中，返回 `(发送字节数, 覆盖块数)`）
  fn send_block_masked(
    &mut self,
    block_idx: u32,
    have_seg_mask: u64,
  ) -> Result<(u64, u32), UdpError> {
    let slot_idx = (block_idx as usize) & (TX_CACHE_SLOTS - 1);
    if self.cache[slot_idx].block_idx == block_idx {
      let raw_len = self.cache[slot_idx].raw_len as u64;
      let count = 1 + u32::from(self.cache[slot_idx].extra_blocks);
      self.transmit_cached_slot(slot_idx, have_seg_mask, false)?;
      return Ok((raw_len, count));
    }
    if !self.source.has_block(block_idx) {
      return Ok((0, 0));
    }
    let Some(loc) = self.source.block_loc(block_idx) else {
      return Ok((0, 0));
    };
    let mask = if loc.len > SUPER_SMALL_LIMIT {
      have_seg_mask
    } else {
      0
    };
    let sent = self.prepare_and_send(block_idx, 0, loc.len as usize, mask, false)?;
    Ok((sent, u32::from(sent > 0)))
  }

  fn prepare_and_send(
    &mut self,
    block_idx: u32,
    extra_blocks: u8,
    raw_len: usize,
    have_seg_mask: u64,
    dup_tail: bool,
  ) -> Result<u64, UdpError> {
    if !self.source.has_block(block_idx) {
      return Ok(0);
    }
    self.block_buf.resize(raw_len, 0);
    let loc = self.source.block_loc(block_idx).ok_or(UdpError::Corrupt)?;
    self
      .source
      .read_loc_into(block_idx, loc, &mut self.block_buf)?;
    self.compress_and_transmit(block_idx, extra_blocks, raw_len, have_seg_mask, dup_tail)
  }

  fn compress_and_transmit(
    &mut self,
    block_idx: u32,
    extra_blocks: u8,
    raw_len: usize,
    have_seg_mask: u64,
    dup_tail: bool,
  ) -> Result<u64, UdpError> {
    let slot_idx = (block_idx as usize) & (TX_CACHE_SLOTS - 1);
    let slot = &mut self.cache[slot_idx];
    slot.block_idx = block_idx;
    slot.extra_blocks = extra_blocks;
    slot.raw_len = raw_len as u32;
    slot.wire_data.clear();

    if raw_len >= 256
      && let Ok(Some(comp)) = self.detector.compress(&self.block_buf, &mut self.codec)
    {
      slot.wire_data.extend_from_slice(comp);
      slot.zstd = true;
    } else {
      slot.wire_data.extend_from_slice(&self.block_buf);
      slot.zstd = false;
    }

    self.transmit_cached_slot(slot_idx, have_seg_mask, dup_tail)?;
    Ok(raw_len as u64)
  }

  fn transmit_cached_slot(
    &mut self,
    slot_idx: usize,
    have_seg_mask: u64,
    dup_tail: bool,
  ) -> Result<(), UdpError> {
    let sid = self.session_id & 0x00FF_FFFF;
    let seg_payload = self.seg_payload;
    let slot = &self.cache[slot_idx];
    let block_idx = slot.block_idx;
    let extra_prefix = u32::from(slot.extra_blocks) << 24;
    let total_len = slot.wire_data.len();
    let seg_cnt = slot.seg_cnt(seg_payload);
    let flagged_cnt = if slot.zstd {
      seg_cnt | DATA_FLAG_ZSTD
    } else {
      seg_cnt
    };

    for seg_idx in 0..seg_cnt {
      if seg_idx < 64 && (have_seg_mask & (1u64 << seg_idx)) != 0 {
        continue;
      }
      let off = (seg_idx as usize) * seg_payload;
      let end = (off + seg_payload).min(total_len);
      let payload = &self.cache[slot_idx].wire_data[off..end];
      let hdr = DataHeader {
        session_id: sid,
        block_idx,
        seg_idx,
        seg_cnt: flagged_cnt,
        offset_in_block: extra_prefix | (off as u32),
        payload_len: payload.len() as u16,
      };
      let hdr_slice: &mut [u8; DATA_HDR_LEN] =
        (&mut self.pkt_buf[..DATA_HDR_LEN]).try_into().unwrap();
      hdr.encode(hdr_slice);
      self.pkt_buf[DATA_HDR_LEN..DATA_HDR_LEN + payload.len()].copy_from_slice(payload);
      let pkt_len = DATA_HDR_LEN + payload.len();
      send_udp(self.sock, &self.pkt_buf[..pkt_len], self.peer_addr)?;
      self.pacer.on_send(pkt_len);
      if dup_tail && seg_idx + 2 >= seg_cnt {
        send_udp(self.sock, &self.pkt_buf[..pkt_len], self.peer_addr)?;
      }
    }
    Ok(())
  }

  fn handle_nack(&mut self, nack: &NackPkt, hi_bound: u32) -> Result<u64, UdpError> {
    let now = now_us();
    let mut resent = 0u32;
    let mut bytes = 0u64;
    for &(s_lo, s_hi) in &nack.spans {
      let mut r_blk = s_lo.max(self.blk_lo);
      let r_hi = s_hi.min(hi_bound);
      while r_blk < r_hi {
        let idx = (r_blk - self.blk_lo) as usize;
        let cooldown_ok = if self.last_retrans_us[idx] != 0 {
          now.saturating_sub(self.last_retrans_us[idx]) >= self.retrans_cooldown_us
        } else {
          now.saturating_sub(self.last_sent_us[idx]) >= self.init_cooldown_us
        };
        if !cooldown_ok {
          r_blk += 1;
          continue;
        }
        let mask = nack
          .partials
          .iter()
          .find(|&&(b, _)| b == r_blk)
          .map_or(0, |&(_, m)| m);
        let (sent, count) = self.send_block_masked(r_blk, mask)?;
        let step = count.max(1);
        if sent > 0 {
          let stamp = now_us();
          for b in r_blk..(r_blk + step).min(hi_bound) {
            self.last_retrans_us[(b - self.blk_lo) as usize] = stamp;
          }
          bytes += sent;
          resent += step;
          if resent >= MAX_NACK_RESEND_BATCH {
            return Ok(bytes);
          }
        }
        r_blk += step;
      }
    }
    Ok(bytes)
  }

  fn process_ctrl_pkt(
    &mut self,
    pkt: &[u8],
    from: SocketAddr,
    sid24: u32,
    hi_bound: u32,
  ) -> Result<(bool, Option<u64>), UdpError> {
    if pkt.is_empty() {
      return Ok((false, None));
    }
    match pkt[0] {
      PKT_PUNCH => {
        if decode_punch(pkt) == Ok(sid24) {
          self.update_peer_addr(from);
        }
        Ok((false, None))
      }
      PKT_ACK => {
        if let Ok(ack) = AckPkt::decode(pkt)
          && ack.session_id == sid24
        {
          self.update_peer_addr(from);
          return Ok((true, None));
        }
        Ok((false, None))
      }
      PKT_NACK => {
        if let Ok(nack) = NackPkt::decode(pkt)
          && nack.session_id == sid24
        {
          self.update_peer_addr(from);
          let resent = self.handle_nack(&nack, hi_bound)?;
          return Ok((false, Some(resent)));
        }
        Ok((false, None))
      }
      _ => Ok((false, None)),
    }
  }
}

/// 原生 UDP 区间发送引擎（支持 `BlockSource`、SuperBlock 小文件跨块压缩、`Detector` 不可压流跳过、2048 槽位环缓存与分片级掩码重传）
pub fn send_range_udp<'a>(
  sock: &UdpSocket,
  peer_addrs: &[SocketAddr],
  source: impl Into<BlockSource<'a>>,
  have_bits: &[u8],
  spec: RangeSpec,
) -> Result<UdpStats, UdpError> {
  let source = source.into();
  let start = Instant::now();
  let peer_addr = peer_addrs.first().copied().ok_or(UdpError::NoPeerAddr)?;
  let hi = spec.blk_hi.min(source.total_blocks());
  if spec.blk_lo >= hi {
    return Ok(UdpStats::default());
  }

  let is_loopback = peer_addr.ip().is_loopback();
  let eff_rate = if spec.eff_rate == 0 && !is_loopback {
    WAN_DEFAULT_RATE_BPS
  } else {
    spec.eff_rate
  };
  let span_len = (hi - spec.blk_lo) as usize;
  let mut ctx = SendCtx {
    sock,
    peer_addr,
    seg_payload: seg_payload_for_addr(peer_addr),
    session_id: spec.session_id,
    blk_lo: spec.blk_lo,
    last_entry_idx: u32::MAX,
    init_cooldown_us: if is_loopback { 10_000 } else { 85_000 },
    retrans_cooldown_us: if is_loopback { 10_000 } else { 110_000 },
    source,
    codec: Codec::new(ZSTD_DEFAULT_LEVEL)?,
    detector: Detector::with_flags(false, true),
    block_buf: Vec::with_capacity(64 * 1024),
    pkt_buf: [0u8; MAX_UDP_PKT],
    pacer: Pacer::new(eff_rate),
    cache: (0..TX_CACHE_SLOTS)
      .map(|_| TxCacheSlot::default())
      .collect(),
    last_sent_us: vec![0u64; span_len],
    last_retrans_us: vec![0u64; span_len],
  };
  let mut ctrl_buf = [0u8; MAX_UDP_PKT];
  let mut bytes_transferred = 0u64;
  let mut blocks_transferred = 0u32;
  let sid24 = spec.session_id & 0x00FF_FFFF;
  let mut amp: AntiAmp<SocketAddr> = AntiAmp::default();
  let timeout_us = spec.timeout.as_micros() as u64;
  let mut deadline_us = now_us().saturating_add(timeout_us);
  let punch_pkt = encode_punch(sid24);

  // 协议层抗反射放大与真实 NAT 端口锁定：在收到对端携带匹配 24-bit session_id 的 PKT_PUNCH / PKT_NACK / PKT_ACK 前绝不发送 64KiB 数据块
  sock.set_nonblocking(false)?;
  sock.set_read_timeout(Some(Duration::from_millis(6)))?;
  while !amp.is_validated() {
    if now_us() >= deadline_us {
      return Err(UdpError::Timeout(spec.blk_lo, hi));
    }
    let targets = if start.elapsed() < Duration::from_millis(15) {
      &peer_addrs[..1]
    } else {
      peer_addrs
    };
    for addr in targets {
      let _ = sock.send_to(&punch_pkt, addr);
    }
    match sock.recv_from(&mut ctrl_buf) {
      Ok((n, from)) if n > 0 => {
        if let Some(ack) = handle_probe_observed(&ctrl_buf[..n], from) {
          let _ = sock.send_to(&ack, from);
          continue;
        }
        let valid_sid = match ctrl_buf[0] {
          PKT_PUNCH => decode_punch(&ctrl_buf[..n]) == Ok(sid24),
          PKT_NACK => NackPkt::decode(&ctrl_buf[..n]).is_ok_and(|p| p.session_id == sid24),
          PKT_ACK => AckPkt::decode(&ctrl_buf[..n]).is_ok_and(|p| p.session_id == sid24),
          _ => false,
        };
        if valid_sid {
          let _ = amp.on_recv(from, n);
          amp.mark_validated();
          ctx.update_peer_addr(from);
          if ctrl_buf[0] == PKT_ACK {
            return Ok(UdpStats {
              bytes_transferred: 0,
              blocks_transferred: 0,
              elapsed_us: start.elapsed().as_micros() as u64,
            });
          }
        }
      }
      Ok(_) => {}
      Err(e) if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut => {}
      Err(e) => return Err(UdpError::Io(e)),
    }
  }

  ctx.pacer = Pacer::new(eff_rate);
  sock.set_nonblocking(true)?;
  let mut blk = spec.blk_lo;
  while blk < hi {
    if now_us() >= deadline_us {
      sock.set_nonblocking(false)?;
      return Err(UdpError::Timeout(spec.blk_lo, hi));
    }
    if get_bit(have_bits, blk) || !ctx.source.has_block(blk) {
      blk += 1;
      continue;
    }
    let (sent, count, end_blk) = ctx.send_next_coalesced(blk, hi, have_bits, !is_loopback)?;
    if sent > 0 {
      let stamp = now_us();
      deadline_us = stamp.saturating_add(timeout_us);
      for b in blk..=end_blk {
        ctx.last_sent_us[(b - spec.blk_lo) as usize] = stamp;
      }
      bytes_transferred += sent;
      blocks_transferred += count;
    }
    blk = end_blk + 1;

    while let Ok((n, from)) = sock.recv_from(&mut ctrl_buf) {
      let (acked, resent_opt) = ctx.process_ctrl_pkt(&ctrl_buf[..n], from, sid24, blk)?;
      if acked {
        sock.set_nonblocking(false)?;
        return Ok(UdpStats {
          bytes_transferred,
          blocks_transferred,
          elapsed_us: start.elapsed().as_micros() as u64,
        });
      }
      if let Some(resent) = resent_opt {
        bytes_transferred += resent;
      }
    }
  }
  sock.set_nonblocking(false)?;
  sock.set_read_timeout(Some(Duration::from_millis(6)))?;
  deadline_us = now_us().saturating_add(timeout_us);

  while now_us() < deadline_us {
    match sock.recv_from(&mut ctrl_buf) {
      Ok((n, from)) => {
        let (acked, resent_opt) = ctx.process_ctrl_pkt(&ctrl_buf[..n], from, sid24, hi)?;
        if acked {
          return Ok(UdpStats {
            bytes_transferred,
            blocks_transferred,
            elapsed_us: start.elapsed().as_micros() as u64,
          });
        }
        if let Some(resent) = resent_opt {
          deadline_us = now_us().saturating_add(timeout_us);
          bytes_transferred += resent;
        }
      }
      Err(e) if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut => {
        continue;
      }
      Err(e) => return Err(UdpError::Io(e)),
    }
  }

  Err(UdpError::Timeout(spec.blk_lo, hi))
}
