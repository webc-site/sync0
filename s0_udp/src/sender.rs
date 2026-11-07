use s0_cc::{MAX_RTO_US, MAX_TACK_US, RttEstimator};
use s0_wire::{Ack, expand_pn};

use crate::UdpError;

/// 在途环最小容量
pub const MIN_CAP: usize = 256;
/// 在途环最大容量（约 24 MiB）
pub const MAX_CAP: usize = 1 << 20;
/// 单次 ACK 最多扫描的失序候选
const LOSS_SCAN: usize = 512;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum State {
  #[default]
  Free,
  Inflight,
  Done,
}

#[derive(Clone, Copy, Debug, Default)]
struct Entry {
  pn: u64,
  blk: u32,
  sent_us: u64,
  len: u16,
  idx: u8,
  lost: bool,
  flag: u8,
  state: State,
}

/// 在途包的只读视图
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SentInfo {
  pub pn: u64,
  pub blk: u32,
  pub idx: u8,
  pub flag: u8,
  pub len: u16,
  pub sent_us: u64,
  pub inflight: bool,
}

/// 一次 ACK 的处理结果
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AckStats {
  pub acked: u32,
  pub lost: u32,
  pub spurious: u32,
  pub rtt_sample_us: Option<u64>,
  pub largest: Option<u32>,
  pub duplicate: bool,
}

/// 发送端状态：在途环 + 自适应 RTT (`s0_cc::RttEstimator`) + RACK 检丢（sans-IO）
pub struct Sender {
  mask: usize,
  ring: Box<[Entry]>,
  next_pn: u64,
  last_sent_us: u64,
  scan_pn: u64,
  rtt: RttEstimator,
  largest_acked: Option<u64>,
  done_lo: u32,
  delivered: u64,
}

impl Sender {
  pub fn new(capacity: usize, start_pn: u64) -> Result<Self, UdpError> {
    if !(MIN_CAP..=MAX_CAP).contains(&capacity) {
      return Err(UdpError::Arg("在途环容量"));
    }
    let cap = capacity.next_power_of_two().min(MAX_CAP);
    Ok(Self {
      mask: cap - 1,
      ring: vec![Entry::default(); cap].into_boxed_slice(),
      next_pn: start_pn,
      last_sent_us: 0,
      scan_pn: start_pn,
      rtt: RttEstimator::new(),
      largest_acked: None,
      done_lo: 0,
      delivered: 0,
    })
  }

  const fn capacity(&self) -> usize {
    self.mask + 1
  }

  pub const fn rtt(&self) -> &RttEstimator {
    &self.rtt
  }

  pub fn seed_rtt(&mut self, sample_us: u64) {
    self.rtt.on_sample(sample_us);
  }

  pub const fn delivered(&self) -> u64 {
    self.delivered
  }

  pub fn on_sent(
    &mut self,
    blk: u32,
    idx: u8,
    len: u16,
    flag: u8,
    now_us: u64,
  ) -> Result<u32, UdpError> {
    let pn = self.next_pn;
    if pn - self.scan_pn >= self.capacity() as u64 {
      return Err(UdpError::Arg("在途环已满"));
    }
    self.last_sent_us = self.last_sent_us.max(now_us);
    self.ring[pn as usize & self.mask] = Entry {
      pn,
      blk,
      sent_us: self.last_sent_us,
      len,
      idx,
      lost: false,
      flag,
      state: State::Inflight,
    };
    self.next_pn += 1;
    Ok(pn as u32)
  }

  pub fn sent(&self, pn: u64) -> Option<SentInfo> {
    let e = self.ring[pn as usize & self.mask];
    (e.state != State::Free && e.pn == pn).then_some(SentInfo {
      pn: e.pn,
      blk: e.blk,
      idx: e.idx,
      flag: e.flag,
      len: e.len,
      sent_us: e.sent_us,
      inflight: e.state == State::Inflight,
    })
  }

  /// 处理一个 ACK：
  /// - `on_acked(blk, idx)` 通知每个经 `ack.ranges()` 精确确认的分片（含乱序已收齐但高于 `done_lo` 的块分片）
  /// - `on_lost(pn)` 收到 RACK 判定丢失的 pn
  pub fn on_ack(
    &mut self,
    ack: &Ack<'_>,
    now_us: u64,
    mut on_acked: impl FnMut(u32, u8),
    mut on_lost: impl FnMut(u64),
  ) -> Result<AckStats, UdpError> {
    let mut stats = AckStats {
      duplicate: ack.flag & Ack::DUP != 0,
      ..AckStats::default()
    };
    let delay_us = (u64::from(ack.delay) * Ack::DELAY_US).min(MAX_TACK_US);
    let wire_largest = ack.largest;
    let base = self
      .largest_acked
      .unwrap_or_else(|| self.next_pn.saturating_sub(1));
    let largest = expand_pn(base, wire_largest);
    if ack.nr > 0 && (largest >= self.next_pn || u64::from(ack.reach) > largest + 1) {
      return Err(UdpError::Corrupt);
    }
    self.done_lo = self.done_lo.max(ack.done_lo);
    let fresh = self.largest_acked.is_none_or(|prev| largest > prev);
    let mut latest_ack_sent = 0u64;

    let mut cursor = largest + 1;
    let floor = cursor.saturating_sub(self.capacity() as u64);
    let mut saw_ooo = false;
    for (gap, len) in ack.ranges() {
      let Some((start, end)) = ack.span(gap, len, &mut cursor) else {
        break;
      };
      if end <= floor {
        break;
      }
      for pn in start.max(floor)..end {
        let (sent_us, len, blk, idx) = {
          let slot = &mut self.ring[pn as usize & self.mask];
          if slot.state != State::Inflight || slot.pn != pn {
            if slot.state == State::Done && slot.pn == pn && slot.lost {
              stats.spurious += 1;
              slot.lost = false;
              on_acked(slot.blk, slot.idx);
            }
            continue;
          }
          slot.state = State::Done;
          (slot.sent_us, slot.len, slot.blk, slot.idx)
        };
        on_acked(blk, idx);
        stats.acked += 1;
        self.delivered = self.delivered.saturating_add(u64::from(len));
        latest_ack_sent = latest_ack_sent.max(sent_us);

        if pn == largest && stats.rtt_sample_us.is_none() {
          stats.rtt_sample_us = self.sample_rtt(now_us.saturating_sub(sent_us), delay_us);
        } else if self.largest_acked.is_some_and(|prev| pn < prev) && self.rtt.has_rtt() {
          saw_ooo = true;
          self.rtt.on_out_of_order(now_us.saturating_sub(sent_us));
        }
      }
    }

    let range_acked = stats.acked;
    stats.acked += self.advance_scan();

    if range_acked > 0 {
      if fresh && stats.rtt_sample_us.is_none() && ack.ts_echo != 0 {
        let raw = (now_us as u32).wrapping_sub(ack.ts_echo) as u64;
        if raw <= MAX_RTO_US * 4 {
          stats.rtt_sample_us = self.sample_rtt(raw, delay_us);
        }
      }
      self.largest_acked = Some(match self.largest_acked {
        Some(prev) if prev >= largest => prev,
        _ => largest,
      });
      stats.largest = Some(wire_largest);
    }

    if stats.spurious > 0 {
      self.rtt.on_spurious();
    } else if range_acked == 0 && stats.duplicate {
      self.rtt.on_duplicate_ack();
    } else if !saw_ooo && range_acked > 0 {
      self.rtt.on_clean_ack();
    }

    if range_acked > 0 {
      self.scan_loss(latest_ack_sent, &mut on_lost, &mut stats);
    }

    Ok(stats)
  }

  fn sample_rtt(&mut self, raw: u64, delay_us: u64) -> Option<u64> {
    let adj = raw.saturating_sub(delay_us);
    let sample = if self.rtt.has_rtt() {
      adj.max(self.rtt.rtt_min_us().min(raw))
    } else if adj > 0 {
      adj
    } else {
      raw
    };
    (sample > 0).then(|| {
      self.rtt.on_sample(sample);
      sample
    })
  }

  fn advance_scan(&mut self) -> u32 {
    let mut acked = 0;
    while self.scan_pn < self.next_pn {
      let slot = &mut self.ring[self.scan_pn as usize & self.mask];
      if slot.state == State::Inflight {
        if slot.blk >= self.done_lo {
          break;
        }
        slot.state = State::Done;
        acked += 1;
        self.delivered = self.delivered.saturating_add(u64::from(slot.len));
      }
      self.scan_pn += 1;
    }
    acked
  }

  fn scan_loss(
    &mut self,
    latest_ack_sent: u64,
    on_lost: &mut impl FnMut(u64),
    stats: &mut AckStats,
  ) {
    if latest_ack_sent == 0 || !self.rtt.has_rtt() {
      return;
    }
    let reo_us = self.rtt.reo_wnd_us();
    for _ in 0..LOSS_SCAN {
      let pn = self.scan_pn;
      if pn >= self.next_pn {
        break;
      }
      let slot = &mut self.ring[pn as usize & self.mask];
      if slot.sent_us.saturating_add(reo_us) >= latest_ack_sent {
        break;
      }
      slot.state = State::Done;
      slot.lost = true;
      stats.lost += 1;
      on_lost(pn);
      self.scan_pn = pn + 1;
      stats.acked += self.advance_scan();
    }
  }
}
