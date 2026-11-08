use std::{
  collections::VecDeque,
  io::ErrorKind,
  mem,
  net::{SocketAddr, UdpSocket},
};

use s0_cc::Cc;
use s0_cdc::SliceWr as Wr;

use super::{
  MAX_DATAGRAM, TUNNEL_PKT, now_us, send_udp,
  source::{Src, cnt_mask},
};
use crate::{
  AckGen, Sender, UdpError,
  consts::{RETX, ZSTD},
  is_emsgsize,
  wire::{Ack, Kind, SessionHeader, StreamDataHeader as DataHeader},
};

/// 在途环容量（包）
pub(super) const RING: usize = 1 << 16;
/// 在途窗口下限
const MIN_CWND: u64 = 4 * AckGen::MAX_PACKETS as u64;
/// 在途窗口上限：留一半在途环给重传
const MAX_CWND: u64 = (RING / 2) as u64;
const _: () = assert!(MIN_CWND <= INIT_CWND && INIT_CWND <= MAX_CWND);
/// 包间隔上下限（µs）
const MIN_GAP_US: u64 = 6;
const MAX_GAP_US: u64 = 100_000;
/// 首轮带宽采样前的慢启动初始在途窗口下限（包）与平滑发包间隔（µs）
const INIT_CWND: u64 = 512;
const MAX_INIT_CWND: u64 = 4096;
const INIT_GAP_US: u64 = 75;

pub(super) struct Tx<'a> {
  pub(super) sock: &'a UdpSocket,
  pub(super) sender: Sender,
  pub(super) peer: SocketAddr,
  pub(super) src: Src,
  pub(super) pkt_size: usize,
  pub(super) pkt_buf: [u8; MAX_DATAGRAM],
  pub(super) done_lo: u32,
  pub(super) credit: u32,
  pub(super) first_send_us: u64,
  pub(super) last_ack: u64,
  pub(super) got_ack: bool,
  pub(super) backoff: u32,
  pub(super) lost: VecDeque<(u32, u8)>,
  pub(super) lost_pns: Vec<u64>,
  pub(super) inflight: u64,
  pub(super) packets: u64,
  pub(super) retx: u64,
  pub(super) last_tx_us: u64,
  pub(super) last_pkt: Option<(u32, u8)>,
  pub(super) cc: Cc,
}

impl Tx<'_> {
  pub(super) fn downgrade_mtu(&mut self) {
    if self.pkt_size > TUNNEL_PKT {
      self.pkt_size = TUNNEL_PKT;
      self.src.downgrade_pkt_size(TUNNEL_PKT);
      self.lost.clear();
      self.inflight = 0;
    }
  }

  pub(super) const fn est_rtt_us(&self) -> u64 {
    let srtt = self.sender.rtt().srtt_us();
    if srtt > 0 {
      srtt
    } else {
      250_000u64 << self.backoff
    }
  }

  pub(super) const fn init_gap_us(&self) -> u64 {
    let rtt = self.est_rtt_us();
    if rtt <= 2_000 {
      15
    } else if rtt <= 30_000 {
      35
    } else {
      INIT_GAP_US
    }
  }

  pub(super) fn cwnd(&self) -> u64 {
    let init = (self.est_rtt_us() / self.init_gap_us()).clamp(INIT_CWND, MAX_INIT_CWND);
    self.cc.cwnd_bytes(self.sender.rtt()).map_or(init, |b| {
      (b / self.pkt_size as u64).clamp(MIN_CWND, MAX_CWND)
    })
  }

  pub(super) fn pkt_gap_us(&self) -> u64 {
    let pkt_sz = self.pkt_size as u64;
    self.cc.pacing_rate(self.sender.rtt()).map_or_else(
      || self.init_gap_us(),
      |r| (pkt_sz * 1_000_000 / r.max(1)).clamp(MIN_GAP_US, MAX_GAP_US),
    )
  }

  pub(super) fn probe_interval_us(&self, tail: bool) -> u64 {
    let rtt = self.est_rtt_us();
    let var = self.sender.rtt().rttvar_us();
    if tail {
      (rtt + (rtt >> 2) + var).clamp(20_000, 2_000_000)
    } else {
      (rtt.saturating_mul(2) + var.saturating_mul(2)).clamp(150_000, 2_000_000)
    }
  }

  pub(super) fn stall_us(&self) -> u64 {
    let rtt = self.est_rtt_us();
    let var = self.sender.rtt().rttvar_us();
    (rtt.saturating_mul(5) / 2 + var.saturating_mul(2)).clamp(150_000, 3_000_000)
  }

  pub(super) fn idle_timeout_us(&self) -> u64 {
    (self.est_rtt_us().saturating_mul(24)).clamp(10_000_000, 60_000_000)
  }

  pub(super) fn drain_acks(&mut self, buf: &mut [u8], no_rack: bool) -> Result<(), UdpError> {
    loop {
      match self.sock.recv_from(buf) {
        Ok((n, _)) => {
          let Ok((h, body)) = SessionHeader::parse(&buf[..n]) else {
            continue;
          };
          if h.kind != Kind::Ack {
            continue;
          }
          let Ok(ack) = Ack::parse(&h, body) else {
            continue;
          };
          let first_ack = !self.got_ack;
          self.got_ack = true;
          self.backoff = 0;
          self.done_lo = self.done_lo.max(ack.done_lo);
          self.credit = self.credit.max(ack.credit);
          let rx_now = now_us();
          self.last_ack = rx_now;
          let mut lost_pns = mem::take(&mut self.lost_pns);
          lost_pns.clear();
          let src = &mut self.src;
          let st = self.sender.on_ack(
            &ack,
            rx_now,
            |blk, idx| {
              if idx < 128
                && let Some(slot) = src.slot_mut(blk)
              {
                slot.have |= 1u128 << idx;
              }
            },
            |pn| {
              if !no_rack {
                lost_pns.push(pn);
              }
            },
          )?;
          if first_ack && self.sender.rtt().srtt_us() == 0 {
            self
              .sender
              .seed_rtt(rx_now.saturating_sub(self.first_send_us).max(1));
          }
          if self.cc.on_ack(
            rx_now,
            self.sender.delivered(),
            st.acked,
            lost_pns.len() as u32,
            self.sender.rtt(),
          ) {
            self.src.set_net_state(self.cc.max_bw(), ack.dec);
          }
          let mut max_b = self.done_lo;
          for (b, bits) in ack.blocks() {
            max_b = max_b.max(b);
            if let Some(slot) = self.src.slot_mut(b) {
              slot.have |= bits;
            }
          }
          if !no_rack {
            for (b, bits) in ack.blocks() {
              if b < self.done_lo || bits == 0 {
                continue;
              }
              let Some(slot) = self.src.slot(b) else {
                continue;
              };
              let hi_idx = (127 - bits.leading_zeros()) as u8;
              let limit_idx = if b < max_b {
                slot.cnt
              } else {
                hi_idx.saturating_sub(7)
              };
              let mut missing = cnt_mask(limit_idx) & !slot.have;
              while missing != 0 {
                let idx = missing.trailing_zeros() as u8;
                missing &= missing - 1;
                if self.queue_retx(b, idx, rx_now) {
                  self.lost.push_back((b, idx));
                }
              }
            }
          }
          for &pn in &lost_pns {
            if let Some(i) = self.sender.sent(pn)
              && self.queue_retx(i.blk, i.idx, rx_now)
            {
              self.lost.push_back((i.blk, i.idx));
            }
          }
          self.lost_pns = lost_pns;
          self.inflight = self
            .inflight
            .saturating_sub(u64::from(st.acked) + u64::from(st.lost));
        }
        Err(e) if e.kind() == ErrorKind::WouldBlock => return Ok(()),
        Err(e) => return Err(e.into()),
      }
    }
  }

  pub(super) fn is_acked(&self, blk: u32, idx: u8) -> bool {
    self.src.slot(blk).is_some_and(|s| (s.have >> idx) & 1 != 0)
  }

  pub(super) fn should_retx(&self, blk: u32, idx: u8, now: u64) -> bool {
    if blk < self.done_lo {
      return false;
    }
    let Some(s) = self.src.slot(blk) else {
      return true;
    };
    if (s.have >> idx) & 1 != 0 {
      return false;
    }
    let last = s.last_retx.get(usize::from(idx)).copied().unwrap_or(0);
    if last == 1 {
      return false;
    }
    let rtt = self.est_rtt_us();
    let cooldown = (rtt + (rtt >> 2) + self.sender.rtt().rttvar_us())
      .max(self.sender.rtt().reo_wnd_us())
      .max(20_000);
    last == 0 || now.saturating_sub(last) >= cooldown
  }

  fn queue_retx(&mut self, blk: u32, idx: u8, now: u64) -> bool {
    if !self.should_retx(blk, idx, now) {
      return false;
    }
    if let Some(s) = self.src.slot_mut(blk)
      && let Some(t) = s.last_retx.get_mut(usize::from(idx))
    {
      *t = 1;
    }
    true
  }

  pub(super) fn due_mask(&self, blk: u32, now: u64) -> u128 {
    let mut mask = self.missing_mask(blk);
    let mut m = mask;
    while m != 0 {
      let idx = m.trailing_zeros() as u8;
      m &= m - 1;
      if !self.should_retx(blk, idx, now) {
        mask &= !(1u128 << idx);
      }
    }
    mask
  }

  pub(super) fn missing_mask(&self, blk: u32) -> u128 {
    self.src.slot(blk).map_or(0, |s| cnt_mask(s.cnt) & !s.have)
  }

  pub(super) fn send(
    &mut self,
    blk: u32,
    idx: u8,
    now: u64,
    retx: bool,
  ) -> Result<usize, UdpError> {
    let pkt_sz = self.pkt_size;
    let b = self.src.get(blk)?;
    let start = usize::from(idx) * pkt_sz;
    if start >= b.data.len() {
      return Ok(0);
    }
    let end = (start + pkt_sz).min(b.data.len());
    let payload_len = end - start;
    let flag = if retx { RETX } else { 0 } | if b.zstd { ZSTD } else { 0 };
    let cnt = b.cnt;
    let pn = self
      .sender
      .on_sent(blk, idx, payload_len as u16, flag, now)?;
    let hdr = DataHeader {
      flag,
      idx,
      cnt,
      sid: 1,
      pn,
      blk,
      ts: (now as u32).max(1),
    };
    let mut w = Wr::new(&mut self.pkt_buf);
    hdr.write(&mut w, &b.data[start..end])?;
    let n = w.pos();
    if retx && let Some(t) = b.last_retx.get_mut(usize::from(idx)) {
      *t = now.max(2);
    }
    if let Err(e) = send_udp(self.sock, &self.pkt_buf[..n], self.peer) {
      if let UdpError::Io(ref io_err) = e
        && is_emsgsize(io_err)
        && self.pkt_size > TUNNEL_PKT
      {
        self.downgrade_mtu();
        return Ok(0);
      }
      return Err(e);
    }
    self.packets += 1;
    self.inflight += 1;
    self.retx += u64::from(retx);
    self.last_tx_us = now;
    self.last_pkt = Some((blk, idx));
    Ok(payload_len)
  }

  pub(super) fn resend_missing(&mut self, blk: u32, mask: u128, now: u64) -> Result<u64, UdpError> {
    self.resend_missing_limited(blk, mask, now, u64::MAX)
  }

  pub(super) fn resend_missing_limited(
    &mut self,
    blk: u32,
    mut mask: u128,
    now: u64,
    limit: u64,
  ) -> Result<u64, UdpError> {
    let mut n = 0u64;
    while mask != 0 && n < limit {
      let idx = mask.trailing_zeros() as u8;
      mask &= mask - 1;
      self.send(blk, idx, now, true)?;
      n += 1;
    }
    Ok(n)
  }

  pub(super) fn resend_all(&mut self, blk: u32, now: u64) -> Result<u64, UdpError> {
    let cnt = self.src.get(blk)?.cnt;
    for idx in 0..cnt {
      self.send(blk, idx, now, true)?;
    }
    Ok(u64::from(cnt))
  }
}
