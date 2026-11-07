use s0_cc::ack_interval_us;

use crate::{
  UdpError,
  s0_wire::{Ack, MAX_RANGES, check_ranges},
};

/// 位图字数：8192 个包号，覆盖 300 Mbps × 66 ms 高 RTT 链路的历史
const WORDS: usize = 128;
/// 启动期：largest 小于此值时每 2 包确认
const START_PN: u32 = 64;
/// 包号回绕比较的半窗口（RFC 1982 序号算术）
const HALF: u32 = 1 << 31;

/// TACK 式 ACK 生成器（sans-IO）
#[derive(Debug)]
pub struct AckGen {
  largest: Option<u32>,
  /// bit 0 = largest，bit k = largest - k，共 WINDOW 位
  seen: [u64; WORDS],
  since_ack: u32,
  immediate: bool,
  dup: bool,
  last_send_us: u64,
  ranges: [(u16, u16); MAX_RANGES],
}

impl Default for AckGen {
  fn default() -> Self {
    Self::new()
  }
}

impl AckGen {
  pub const WINDOW: u32 = (WORDS as u32) * 64;
  pub const MAX_PACKETS: u32 = 64;

  pub const fn new() -> Self {
    Self {
      largest: None,
      seen: [0; WORDS],
      since_ack: 0,
      immediate: false,
      dup: false,
      last_send_us: 0,
      ranges: [(0, 0); MAX_RANGES],
    }
  }

  pub const fn largest(&self) -> Option<u32> {
    self.largest
  }

  /// 记录一个需确认（ack-eliciting）的包号：DATA / CTRL / PROBE
  pub fn on_packet(&mut self, pn: u32) {
    match self.record(pn) {
      Rec::Old => return,
      Rec::Dup => self.dup = true,
      Rec::New { hole } => self.immediate |= hole,
    }
    self.since_ack += 1;
  }

  /// 记录无需确认的包号（ACK / CLOSE）：只置位、不触发 ACK
  pub fn on_quiet(&mut self, pn: u32) {
    self.record(pn);
  }

  fn record(&mut self, pn: u32) -> Rec {
    let Some(largest) = self.largest else {
      self.largest = Some(pn);
      self.set(0);
      return Rec::New { hole: false };
    };
    let shift = pn.wrapping_sub(largest);
    if shift != 0 && shift < HALF {
      self.shift_left(shift);
      self.set(0);
      self.largest = Some(pn);
      return Rec::New { hole: shift > 1 };
    }
    let diff = largest.wrapping_sub(pn);
    if diff >= Self::WINDOW {
      return Rec::Old;
    }
    if self.bit(diff) {
      return Rec::Dup;
    }
    self.set(diff);
    Rec::New { hole: true }
  }

  /// `rtt_us` 为接收端的 RTT 估计（0 = 未知）；周期间隔见 `s0_cc::ack_interval_us`
  pub fn should_send(&self, now_us: u64, rtt_us: u64) -> bool {
    let Some(largest) = self.largest else {
      return false;
    };
    if self.since_ack == 0 {
      return false;
    }
    if self.since_ack >= 2 && largest < START_PN {
      return true;
    }
    if self.immediate || self.since_ack >= Self::MAX_PACKETS {
      return true;
    }
    now_us.saturating_sub(self.last_send_us) >= ack_interval_us(rtt_us)
  }

  /// 生成 ACK 区间；不改变计数，需在发送后调用 `mark_sent`
  pub fn build(&mut self) -> Option<AckView<'_>> {
    let largest = self.largest?;
    let mut nr = 0usize;
    let mut idx = 0u32;
    while idx < Self::WINDOW && nr < MAX_RANGES {
      let gap = if idx == 0 {
        0
      } else {
        let zeros = self.run_len(idx, false);
        idx += zeros;
        if idx >= Self::WINDOW {
          break;
        }
        zeros as u16
      };
      let ones = self.run_len(idx, true);
      idx += ones;
      self.ranges[nr] = (gap, ones as u16);
      nr += 1;
    }

    let flag = if self.dup { Ack::DUP } else { 0 };
    Some(AckView {
      largest,
      flag,
      ranges: &self.ranges[..nr],
    })
  }

  /// 发送完成后清零触发状态
  pub fn mark_sent(&mut self, now_us: u64) {
    self.since_ack = 0;
    self.immediate = false;
    self.dup = false;
    self.last_send_us = now_us;
  }

  const fn bit(&self, idx: u32) -> bool {
    self.seen[(idx >> 6) as usize] >> (idx & 63) & 1 != 0
  }

  fn run_len(&self, mut idx: u32, one: bool) -> u32 {
    let start = idx;
    while idx < Self::WINDOW {
      let w = self.seen[(idx >> 6) as usize];
      let bit_off = idx & 63;
      let shifted = (if one { w } else { !w }) >> bit_off;
      let rem_in_word = 64 - bit_off;
      let n = shifted.trailing_ones();
      idx += n;
      if n < rem_in_word {
        break;
      }
    }
    idx - start
  }

  fn set(&mut self, idx: u32) {
    self.seen[(idx >> 6) as usize] |= 1u64 << (idx & 63);
  }

  fn shift_left(&mut self, shift: u32) {
    let ws = (shift >> 6) as usize;
    let bs = shift & 63;
    if ws >= WORDS {
      self.seen.fill(0);
      return;
    }
    if bs == 0 {
      self.seen.copy_within(..WORDS - ws, ws);
    } else {
      for i in (ws..WORDS).rev() {
        let lo = if i > ws {
          self.seen[i - ws - 1] >> (64 - bs)
        } else {
          0
        };
        self.seen[i] = (self.seen[i - ws] << bs) | lo;
      }
    }
    self.seen[..ws].fill(0);
  }
}

enum Rec {
  Old,
  Dup,
  New { hole: bool },
}

/// 一次 ACK 的只读视图
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AckView<'a> {
  pub largest: u32,
  pub flag: u8,
  pub ranges: &'a [(u16, u16)],
}

impl AckView<'_> {
  pub fn check(&self) -> Result<u32, UdpError> {
    Ok(check_ranges(self.ranges.iter().copied())?)
  }
}
