use std::{
  io,
  net::{SocketAddr, UdpSocket},
  time::{Duration, Instant},
};

use s0_cc::AntiAmp;
use s0_cdc::{BlockLoc, get_bit};
use s0_codec::{Codec, Detector};
use s0_const::{
  DEFAULT_SEG_PAYLOAD, MAX_UDP_PKT, PKT_ACK, PKT_NACK, PKT_PUNCH, PUNCH_PRIMARY_WINDOW_MS,
  SID24_MASK, SOCK_POLL_TIMEOUT_MS, TUNNEL_SEG_PAYLOAD,
};
use s0_fs::BlockSource;
use s0_wire::{AckPkt, NackPkt, decode_punch, encode_punch};

use crate::{
  RangeSpec, UdpError, UdpStats,
  bench::handle_probe_observed,
  is_tunnel_addr, now_us,
  range::{
    MAX_NACK_RESEND_BATCH, Pacer, SUPER_INCOMP_LIMIT, SUPER_MAX_BYTES, SUPER_MAX_EXTRA,
    SUPER_SMALL_LIMIT, TX_CACHE_SLOTS, TxCacheSlot, WAN_DEFAULT_RATE_BPS,
    WAN_HIGH_RTT_INIT_RATE_BPS, cache_slot_idx, cooldowns_for_rtt, is_valid_ctrl_sid,
    seg_payload_for_sock,
  },
};

struct SendCtx<'a> {
  sock: &'a UdpSocket,
  peer_addr: SocketAddr,
  seg_payload: usize,
  pmtu_ok: bool,
  is_loopback: bool,
  adaptive_rate: bool,
  rtt_measured: bool,
  punch_start_us: u64,
  session_id: u32,
  blk_lo: u32,
  hi: u32,
  last_entry_idx: u32,
  rtt_us: u64,
  timeout_us: u64,
  deadline_us: u64,
  init_cooldown_us: u64,
  retrans_cooldown_us: u64,
  peer_done_blks: u32,
  last_backoff_us: u64,
  rate_win_start_us: u64,
  rate_win_bytes: u64,
  stats: UdpStats,
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
  fn is_slow_link(&self, addr: SocketAddr) -> bool {
    self.adaptive_rate && self.rtt_us >= 80_000 && (is_tunnel_addr(addr) || self.rtt_us >= 120_000)
  }

  #[inline]
  fn update_peer_addr(&mut self, from: SocketAddr, punch_len: usize) {
    if !self.rtt_measured {
      self.rtt_us = now_us()
        .saturating_sub(self.punch_start_us)
        .clamp(1_000, 30_000_000);
      self.timeout_us = self.timeout_us.max(self.rtt_us.saturating_mul(8));
      (self.init_cooldown_us, self.retrans_cooldown_us) =
        cooldowns_for_rtt(self.is_loopback, self.rtt_us);
      if self.is_slow_link(from) && self.peer_done_blks == 0 {
        self.pacer.set_rate(WAN_HIGH_RTT_INIT_RATE_BPS);
      }
      self.rtt_measured = true;
    }
    let port_changed = self.peer_addr != from;
    if is_tunnel_addr(from) {
      self.pmtu_ok = false;
      self.seg_payload = TUNNEL_SEG_PAYLOAD;
    } else {
      self.pmtu_ok |= punch_len >= DEFAULT_SEG_PAYLOAD;
      if port_changed && !self.pmtu_ok {
        self.seg_payload = seg_payload_for_sock(self.sock, from);
      }
    }
    self.peer_addr = from;
    if port_changed {
      self.last_sent_us.fill(0);
    }
  }

  #[inline]
  fn ready_loc(&self, blk: u32) -> Option<BlockLoc> {
    self
      .source
      .has_block(blk)
      .then(|| self.source.block_loc(blk))
      .flatten()
  }

  /// 发送连续小文件聚合块（SuperBlock）或常规单块，返回 `(实际发送原始字节数, 覆盖的块数, 末尾块索引)`
  fn send_next_coalesced(
    &mut self,
    start_blk: u32,
    hi: u32,
    have_bits: &[u8],
    dup_tail: bool,
  ) -> Result<(u64, u32, u32), UdpError> {
    let Some(first_loc) = self.ready_loc(start_blk) else {
      return Ok((0, 0, start_blk));
    };
    if first_loc.entry_idx != self.last_entry_idx {
      self.last_entry_idx = first_loc.entry_idx;
      self.detector.reset_file();
    }
    let lim = if self.detector.is_compressible() {
      SUPER_SMALL_LIMIT
    } else {
      SUPER_INCOMP_LIMIT
    };
    let (mut total_raw, mut extra, mut cur_blk) = (first_loc.len as usize, 0u8, start_blk + 1);
    while first_loc.len <= lim
      && cur_blk < hi
      && extra < SUPER_MAX_EXTRA
      && !get_bit(have_bits, cur_blk)
    {
      let Some(loc) = self
        .ready_loc(cur_blk)
        .filter(|l| l.len <= lim && total_raw + (l.len as usize) <= SUPER_MAX_BYTES)
      else {
        break;
      };
      total_raw += loc.len as usize;
      extra += 1;
      cur_blk += 1;
    }
    self.block_buf.resize(total_raw, 0);
    let mut off = 0usize;
    for b in start_blk..=start_blk + u32::from(extra) {
      let loc = self.source.block_loc(b).ok_or(UdpError::Corrupt)?;
      let len = loc.len as usize;
      self
        .source
        .read_loc_into(b, loc, &mut self.block_buf[off..off + len])?;
      off += len;
    }
    let bytes =
      self.compress_and_transmit(start_blk, extra, total_raw, 0, dup_tail && cur_blk >= hi)?;
    Ok((bytes, 1 + u32::from(extra), start_blk + u32::from(extra)))
  }

  #[inline]
  fn transmit_slot(&mut self, slot_idx: usize, mask: u64, dup_tail: bool) -> Result<(), UdpError> {
    self.cache[slot_idx].transmit(
      (self.sock, self.peer_addr, self.session_id),
      &mut self.seg_payload,
      mask,
      dup_tail,
      &mut self.pkt_buf,
      &mut self.pacer,
    )
  }

  /// 发送单个块或已缓存 SuperBlock（支持 64-bit 分片掩码过滤与 4096 槽位散列缓存命中，返回 `(发送字节数, 覆盖块数)`）
  fn send_block_masked(&mut self, block_idx: u32, mask: u64) -> Result<(u64, u32), UdpError> {
    let slot_idx = cache_slot_idx(block_idx);
    if self.cache[slot_idx].block_idx == block_idx {
      let (raw, cnt) = (
        self.cache[slot_idx].raw_len as u64,
        1 + u32::from(self.cache[slot_idx].extra_blocks),
      );
      self.transmit_slot(slot_idx, mask, false)?;
      return Ok((raw, cnt));
    }
    let Some(loc) = self.ready_loc(block_idx) else {
      return Ok((0, 0));
    };
    let eff_mask = if loc.len > SUPER_SMALL_LIMIT { mask } else { 0 };
    let raw_len = loc.len as usize;
    self.block_buf.resize(raw_len, 0);
    self
      .source
      .read_loc_into(block_idx, loc, &mut self.block_buf)?;
    let sent = self.compress_and_transmit(block_idx, 0, raw_len, eff_mask, false)?;
    Ok((sent, u32::from(sent > 0)))
  }

  fn compress_and_transmit(
    &mut self,
    block_idx: u32,
    extra_blocks: u8,
    raw_len: usize,
    have_seg_mask: u64,
    dup_tail: bool,
  ) -> Result<u64, UdpError> {
    let slot_idx = cache_slot_idx(block_idx);
    self.cache[slot_idx].store(
      block_idx,
      extra_blocks,
      &self.block_buf,
      &mut self.detector,
      &mut self.codec,
    );
    self.transmit_slot(slot_idx, have_seg_mask, dup_tail)?;
    Ok(raw_len as u64)
  }

  fn handle_nack(&mut self, nack: &NackPkt, hi_bound: u32) -> Result<u64, UdpError> {
    let now = now_us();
    let (mut resent, mut bytes) = (0u32, 0u64);
    for &(s_lo, s_hi) in &nack.spans {
      let (mut r_blk, r_hi) = (s_lo.max(self.blk_lo), s_hi.min(hi_bound));
      while r_blk < r_hi && resent < MAX_NACK_RESEND_BATCH {
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
        let mut mask = nack
          .partials
          .iter()
          .find(|&&(b, _)| b == r_blk)
          .map_or(0, |&(_, m)| m);
        let s_idx = cache_slot_idx(r_blk);
        if self.seg_payload > TUNNEL_SEG_PAYLOAD
          && !self.pmtu_ok
          && self.peer_done_blks == 0
          && nack.partials.is_empty()
          && mask == 0
          && self.cache[s_idx].block_idx == r_blk
          && self.cache[s_idx].wire_data.len() > TUNNEL_SEG_PAYLOAD
        {
          self.seg_payload = TUNNEL_SEG_PAYLOAD;
          mask = 0;
        }
        let (sent, count) = if self.cache[s_idx].block_idx != r_blk && mask == 0 && r_hi - r_blk > 1
        {
          let (s, c, _) = self.send_next_coalesced(r_blk, r_hi, &[], false)?;
          (s, c)
        } else {
          self.send_block_masked(r_blk, mask)?
        };
        let step = count.max(1);
        if sent > 0 {
          let end_idx = ((r_blk + step).min(hi_bound) - self.blk_lo) as usize;
          self.last_retrans_us[idx..end_idx].fill(now_us());
          bytes += sent;
          resent += step;
        }
        r_blk += step;
      }
    }
    if self.is_slow_link(self.peer_addr)
      && resent >= 4
      && self.pacer.rate() > 0
      && now.saturating_sub(self.last_backoff_us) >= self.rtt_us
    {
      self.last_backoff_us = now;
      self
        .pacer
        .set_rate((self.pacer.rate() * 3 / 4).max(250_000));
    }
    Ok(bytes)
  }

  fn handle_ack(&mut self, ack: AckPkt) -> Option<u64> {
    if ack.blk_hi >= self.hi {
      return Some(u64::MAX);
    }
    let done = ack.blk_hi.saturating_sub(self.blk_lo);
    let done_bytes = u64::from(ack.blk_lo) << 10;
    if done <= self.peer_done_blks && done_bytes <= self.rate_win_bytes {
      return Some(0);
    }
    self.peer_done_blks = self.peer_done_blks.max(done);
    let now = now_us();
    let elapsed = now.saturating_sub(self.rate_win_start_us);
    if self.is_slow_link(self.peer_addr)
      && elapsed >= (self.rtt_us / 2).clamp(20_000, 100_000)
      && done_bytes > self.rate_win_bytes
    {
      let rx_rate = (done_bytes - self.rate_win_bytes) * 1_000_000 / elapsed.max(1);
      let target = (rx_rate * 5 / 4).clamp(250_000, 120_000_000);
      let next_rate = if target >= self.pacer.rate() {
        target
      } else {
        (self.pacer.rate() + target * 2) / 3
      };
      self.pacer.set_rate(next_rate);
      self.rate_win_start_us = now;
      self.rate_win_bytes = done_bytes;
    }
    Some(0)
  }

  fn process_ctrl_pkt(
    &mut self,
    pkt: &[u8],
    from: SocketAddr,
    hi_bound: u32,
  ) -> Result<bool, UdpError> {
    let sid24 = self.session_id;
    let res = match pkt.first().copied() {
      Some(PKT_PUNCH) if decode_punch(pkt) == Ok(sid24) && from.ip() == self.peer_addr.ip() => {
        self.update_peer_addr(from, pkt.len());
        None
      }
      Some(PKT_ACK) => AckPkt::decode(pkt)
        .ok()
        .filter(|a| a.session_id == sid24)
        .and_then(|a| {
          self.update_peer_addr(from, 0);
          self.handle_ack(a)
        }),
      Some(PKT_NACK) => NackPkt::decode(pkt)
        .ok()
        .filter(|n| n.session_id == sid24)
        .map(|n| {
          self.update_peer_addr(from, 0);
          self.handle_nack(&n, hi_bound)
        })
        .transpose()?,
      _ => None,
    };
    if let Some(resent) = res {
      if resent == u64::MAX {
        return Ok(true);
      }
      self.deadline_us = now_us().saturating_add(self.timeout_us);
      self.stats.bytes_transferred += resent;
    }
    Ok(false)
  }
}

/// 原生 UDP 区间发送引擎（支持 `BlockSource`、SuperBlock 小文件跨块压缩、`Detector` 不可压流跳过、4096 槽位环缓存与分片级掩码重传）
pub fn send_range_udp<'a>(
  sock: &UdpSocket,
  peer_addrs: &[SocketAddr],
  source: impl Into<BlockSource<'a>>,
  have_bits: &[u8],
  spec: RangeSpec,
) -> Result<UdpStats, UdpError> {
  let (source, start) = (source.into(), Instant::now());
  let peer_addr = peer_addrs.first().copied().ok_or(UdpError::NoPeerAddr)?;
  let hi = spec.blk_hi.min(source.total_blocks());
  if spec.blk_lo >= hi {
    return Ok(UdpStats::default());
  }

  let is_loopback = peer_addr.ip().is_loopback();
  let adaptive_rate = spec.eff_rate == 0 && !is_loopback;
  let eff_rate = if adaptive_rate {
    WAN_DEFAULT_RATE_BPS
  } else {
    spec.eff_rate
  };
  let span_len = (hi - spec.blk_lo) as usize;
  let sid24 = spec.session_id & SID24_MASK;
  let punch_start_us = now_us();
  let timeout_us = spec.timeout.as_micros() as u64;
  let (init_cooldown_us, retrans_cooldown_us) = cooldowns_for_rtt(is_loopback, 0);
  let mut ctx = SendCtx {
    sock,
    peer_addr,
    seg_payload: seg_payload_for_sock(sock, peer_addr),
    pmtu_ok: is_loopback,
    is_loopback,
    adaptive_rate,
    rtt_measured: is_loopback,
    punch_start_us,
    session_id: sid24,
    blk_lo: spec.blk_lo,
    hi,
    last_entry_idx: u32::MAX,
    rtt_us: 40_000,
    timeout_us,
    deadline_us: punch_start_us.saturating_add(timeout_us),
    init_cooldown_us,
    retrans_cooldown_us,
    peer_done_blks: 0,
    last_backoff_us: 0,
    rate_win_start_us: punch_start_us,
    rate_win_bytes: 0,
    stats: UdpStats::default(),
    source,
    codec: Codec::new(1)?,
    detector: Detector::with_flags(false, true),
    block_buf: Vec::with_capacity(SUPER_MAX_BYTES),
    pkt_buf: [0u8; MAX_UDP_PKT],
    pacer: Pacer::new(eff_rate),
    cache: (0..TX_CACHE_SLOTS)
      .map(|_| TxCacheSlot::default())
      .collect(),
    last_sent_us: vec![0u64; span_len],
    last_retrans_us: vec![0u64; span_len],
  };
  let mut ctrl_buf = [0u8; MAX_UDP_PKT];
  let mut amp: AntiAmp<SocketAddr> = AntiAmp::default();
  let punch_pkt = encode_punch(sid24);
  let poll_dur = Some(Duration::from_millis(SOCK_POLL_TIMEOUT_MS));

  // 协议层抗反射放大与真实 NAT 端口锁定：必须收到对端至少 1 个携带本会话 sid24 的合法控制包（PKT_PUNCH / PKT_NACK / PKT_ACK）并测量真实 RTT 后，才向已验证源地址高速发流
  sock.set_nonblocking(false)?;
  sock.set_read_timeout(poll_dur)?;
  while !amp.is_validated() {
    if now_us() >= ctx.deadline_us {
      return Err(UdpError::Timeout(spec.blk_lo, hi));
    }
    let targets = if start.elapsed() < Duration::from_millis(PUNCH_PRIMARY_WINDOW_MS) {
      &peer_addrs[..1]
    } else {
      peer_addrs
    };
    for addr in targets {
      let _ = sock.send_to(&punch_pkt, addr);
    }
    if let Ok((n, from)) = sock.recv_from(&mut ctrl_buf)
      && n > 0
    {
      if let Some(ack) = handle_probe_observed(&ctrl_buf[..n], from) {
        let _ = sock.send_to(&ack, from);
        continue;
      }
      if is_valid_ctrl_sid(&ctrl_buf[..n], sid24) {
        let _ = amp.on_recv(from, n);
        amp.mark_validated();
        ctx.update_peer_addr(from, if ctrl_buf[0] == PKT_PUNCH { n } else { 0 });
        if ctrl_buf[0] == PKT_ACK && AckPkt::decode(&ctrl_buf[..n]).is_ok_and(|a| a.blk_hi >= hi) {
          return Ok(ctx.stats);
        }
      }
    }
  }

  ctx.pacer = Pacer::new(ctx.pacer.rate());
  ctx.rate_win_start_us = now_us();
  sock.set_nonblocking(true)?;
  let mut blk = spec.blk_lo;
  while blk < hi {
    if now_us() >= ctx.deadline_us {
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
      ctx.deadline_us = stamp.saturating_add(ctx.timeout_us);
      ctx.last_sent_us[(blk - spec.blk_lo) as usize..=(end_blk - spec.blk_lo) as usize].fill(stamp);
      ctx.stats.bytes_transferred += sent;
      ctx.stats.blocks_transferred += count;
    }
    blk = end_blk + 1;
    while let Ok((n, from)) = sock.recv_from(&mut ctrl_buf) {
      if ctx.process_ctrl_pkt(&ctrl_buf[..n], from, blk)? {
        sock.set_nonblocking(false)?;
        return Ok(ctx.stats);
      }
    }
  }
  sock.set_nonblocking(false)?;
  sock.set_read_timeout(poll_dur)?;
  ctx.deadline_us = now_us().saturating_add(ctx.timeout_us);

  while now_us() < ctx.deadline_us {
    match sock.recv_from(&mut ctrl_buf) {
      Ok((n, from)) if ctx.process_ctrl_pkt(&ctrl_buf[..n], from, hi)? => return Ok(ctx.stats),
      Ok(_) => {}
      Err(e)
        if matches!(
          e.kind(),
          io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
        ) => {}
      Err(e) => return Err(UdpError::Io(e)),
    }
  }
  Err(UdpError::Timeout(spec.blk_lo, hi))
}
