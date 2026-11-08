use crate::{
  RttEstimator,
  math::{clamp, mul_div},
};

/// 带宽滤波窗口轮数
const BW_ROUNDS: usize = 4;
/// 增益定点单位：1000 = 1.0×
const GAIN_UNIT: u32 = 1000;
/// 统计轮长下限（20 ms）：低时延链路也至少按 20 ms 统计交付速率，压低采样噪声
const MIN_ROUND_US: u64 = 20_000;
/// 首轮提前采样：确认 ≥ 16 包且满 2 ms 即给出种子带宽，免去高时延首轮突发与盲发
const EARLY_ACKED: u64 = 16;
const EARLY_MIN_US: u64 = 2_000;
/// 高丢包：本轮丢包率 > 1/12
const HIGH_LOSS_DIV: u64 = 12;
/// 探测：本轮带宽 ≥ 估计值 9/10 且无丢包、无排队
const PROBE_NUM: u64 = 9;
const PROBE_DEN: u64 = 10;
/// 队列膨胀：srtt > 1.5 × 轮长
const INFLATE_NUM: u64 = 3;
const INFLATE_DEN: u64 = 2;
/// 1 秒的微秒数（B/s ↔ B/µs 换算）
const US_PER_S: u64 = 1_000_000;

/// 突发：回环 / 高丢包 / 无样本（RTT > `BURST_FAR_RTT_US` 取 `BURST_INIT_FAR`）
const BURST_LOOP: usize = 32;
const BURST_LOSS: usize = 4;
const BURST_INIT: usize = 8;
const BURST_INIT_FAR: usize = 12;
const BURST_FAR_RTT_US: u64 = 80_000;
/// 突发调度粒度 250 µs：每粒度字节数 = bw / 4000
const BURST_TICK_DIV: u64 = US_PER_S / 250;
const BURST_MIN: u64 = 2;
const BURST_MAX: u64 = 32;

/// 排队时延梯度增益曲线（定点 `GAIN_UNIT`）
struct Ramp {
  /// 浅队列且处于探测期
  probe: u32,
  /// 浅队列
  top: u32,
  /// 深队列（排空）
  low: u32,
  /// 高丢包
  loss: u32,
}

const PACING: Ramp = Ramp {
  probe: 1450,
  top: 1250,
  low: 950,
  loss: 1050,
};
const CWND: Ramp = Ramp {
  probe: 1625,
  top: 1375,
  low: 1100,
  loss: 1125,
};

// 插值 `low + (hi−dq)·(top−low)/(hi−lo)` 要求 low ≤ top ≤ probe，否则下溢
const _: () = assert!(PACING.low <= PACING.top && PACING.top <= PACING.probe);
const _: () = assert!(CWND.low <= CWND.top && CWND.top <= CWND.probe);

/// 浅队列阈值 `d_lo = clamp(rtt_min/8, 2ms, 15ms)`
const DQ_LO_MIN_US: u64 = 2_000;
const DQ_LO_MAX_US: u64 = 15_000;
/// 深队列阈值 `d_hi = clamp(rtt_min/2, 8ms, 60ms)`
const DQ_HI_MIN_US: u64 = 8_000;
const DQ_HI_MAX_US: u64 = 60_000;

/// FASP / Vegas 风格增益：排队时延 `d_q = srtt − rtt_min`
///
/// - `d_q ≤ d_lo`：浅队列，取 `top`（探测期 `probe`）
/// - `d_q ≥ d_hi`：深队列，取 `low` 排空
/// - 其间从起点值线性降到 `low`，处处连续、单调不增；
///   `d_lo < d_hi` 恒成立（`d_lo > 8ms ⇒ rtt_min > 64ms ⇒ d_hi > 32ms`）
const fn ramp(r: &Ramp, srtt_us: u64, rtt_min_us: u64, high_loss: bool, probe_up: bool) -> u32 {
  if high_loss {
    return r.loss;
  }
  let top = if probe_up { r.probe } else { r.top };
  if rtt_min_us == 0 || srtt_us <= rtt_min_us {
    return top;
  }
  let dq = srtt_us - rtt_min_us;
  let lo = clamp(rtt_min_us / 8, DQ_LO_MIN_US, DQ_LO_MAX_US);
  let hi = clamp(rtt_min_us / 2, DQ_HI_MIN_US, DQ_HI_MAX_US);
  if dq <= lo {
    top
  } else if dq >= hi {
    r.low
  } else {
    // (hi − dq) / (hi − lo) ∈ (0, 1)，结果 ∈ (low, top)，u32 不截断
    r.low + ((hi - dq) * (top - r.low) as u64 / (hi - lo)) as u32
  }
}

/// 发包速率增益：浅队列 1.25×（探测 1.45×）→ 深队列 0.95×，高丢包 1.05×
pub const fn delay_pacing_gain(
  srtt_us: u64,
  rtt_min_us: u64,
  high_loss: bool,
  probe_up: bool,
) -> u32 {
  ramp(&PACING, srtt_us, rtt_min_us, high_loss, probe_up)
}

/// 在途窗口增益（× BDP）：浅队列 1.375×（探测 1.625×）→ 深队列 1.1×，高丢包 1.125×
pub const fn delay_cwnd_gain(
  srtt_us: u64,
  rtt_min_us: u64,
  high_loss: bool,
  probe_up: bool,
) -> u32 {
  ramp(&CWND, srtt_us, rtt_min_us, high_loss, probe_up)
}

/// 时延梯度拥塞控制（sans-IO，全项目唯一一套）。单位：字节、微秒。
///
/// - 带宽：每轮交付速率；新高立即采纳，否则向近 `BW_ROUNDS` 轮最大值 EWMA 回落
///   （高丢包或队列膨胀时权重 1/4，平时 1/8）
/// - 速率 = 带宽 × `delay_pacing_gain`；窗口 = 带宽 × `rtt_min` × `delay_cwnd_gain`
/// - 随机丢包只影响「高丢包」档位（增益收窄到 1.05×），不按丢包降速：拥塞信号是排队时延
#[derive(Clone, Copy, Debug, Default)]
pub struct Cc {
  started: bool,
  round_start_us: u64,
  round_delivered: u64,
  round_acked: u64,
  round_lost: u64,
  bw_win: [u64; BW_ROUNDS],
  bw_idx: usize,
  max_bw: u64,
  high_loss: bool,
  probe_up: bool,
}

impl Cc {
  pub const fn new() -> Self {
    Self {
      started: false,
      round_start_us: 0,
      round_delivered: 0,
      round_acked: 0,
      round_lost: 0,
      bw_win: [0; BW_ROUNDS],
      bw_idx: 0,
      max_bw: 0,
      high_loss: false,
      probe_up: false,
    }
  }

  /// 带宽估计（B/s，0 = 尚无样本）
  pub const fn max_bw(&self) -> u64 {
    self.max_bw
  }

  pub const fn high_loss(&self) -> bool {
    self.high_loss
  }

  /// 每个 ACK 调用：`delivered` 为累计交付字节（`Sender::delivered`），`acked` / `lost` 为本次确认 / 判丢包数；
  /// 返回本次是否更新了带宽估计
  pub fn on_ack(
    &mut self,
    now_us: u64,
    delivered: u64,
    acked: u32,
    lost: u32,
    rtt: &RttEstimator,
  ) -> bool {
    if !self.started {
      if acked == 0 {
        return false;
      }
      // 首个有效数据 ACK 只开轮：之前的确认属于握手 / 首包，不计入速率
      self.started = true;
      self.new_round(now_us, delivered);
      return false;
    }
    self.round_acked += u64::from(acked);
    self.round_lost += u64::from(lost);
    let round = rtt.rtt_min_us().max(MIN_ROUND_US);
    let dt = now_us.saturating_sub(self.round_start_us);
    let early_us = (rtt.rtt_min_us() / 4).max(EARLY_MIN_US);
    let early = self.max_bw == 0 && self.round_acked >= EARLY_ACKED && dt >= early_us;
    if dt < round && !early {
      return false;
    }

    let bw = mul_div(
      delivered.saturating_sub(self.round_delivered),
      US_PER_S,
      dt.max(1),
    );
    self.high_loss =
      self.round_lost.saturating_mul(HIGH_LOSS_DIV) > self.round_acked + self.round_lost;
    let inflated = rtt.srtt_us() > round * INFLATE_NUM / INFLATE_DEN;
    self.probe_up = false;
    let updated = bw > 0;
    if updated {
      // bw_win[0] == 0 当且仅当尚无整轮样本：提前采样与首个整轮样本均无条件覆盖 max_bw
      let reset = self.bw_win[0] == 0;
      // 提前采样不进窗口：相对误差 ≈ 单个 ACK 覆盖量 / (bw·dt)，2 ms 窗口下可达数倍
      if !early {
        self.bw_win[self.bw_idx] = bw;
        self.bw_idx = (self.bw_idx + 1) % BW_ROUNDS;
      }
      self.probe_up = !reset
        && self.round_lost == 0
        && !inflated
        && bw >= mul_div(self.max_bw, PROBE_NUM, PROBE_DEN);
      self.max_bw = if reset || bw >= self.max_bw {
        bw
      } else {
        let win_max = self.bw_win.into_iter().max().unwrap_or(bw);
        if self.high_loss || inflated {
          // 链路降级或队列膨胀：1/4 权重快速回落
          (self.max_bw.saturating_mul(3) + win_max) / 4
        } else {
          (self.max_bw.saturating_mul(7) + win_max) / 8
        }
      };
    }
    if dt >= round {
      self.new_round(now_us, delivered);
    }
    updated
  }

  const fn new_round(&mut self, now_us: u64, delivered: u64) {
    self.round_start_us = now_us;
    self.round_delivered = delivered;
    self.round_acked = 0;
    self.round_lost = 0;
  }

  /// 发送速率（B/s）；尚无带宽样本返回 `None`
  pub const fn pacing_rate(&self, rtt: &RttEstimator) -> Option<u64> {
    if self.max_bw == 0 {
      return None;
    }
    let gain = delay_pacing_gain(
      rtt.srtt_us(),
      rtt.rtt_min_us(),
      self.high_loss,
      self.probe_up,
    );
    Some(mul_div(self.max_bw, gain as u64, GAIN_UNIT as u64))
  }

  /// 在途窗口（字节）；尚无带宽或 RTT 样本返回 `None`
  pub const fn cwnd_bytes(&self, rtt: &RttEstimator) -> Option<u64> {
    let rtt_min = rtt.rtt_min_us();
    if self.max_bw == 0 || rtt_min == 0 {
      return None;
    }
    let bdp = mul_div(self.max_bw, rtt_min, US_PER_S);
    let gain = delay_cwnd_gain(rtt.srtt_us(), rtt_min, self.high_loss, self.probe_up);
    Some(mul_div(bdp, gain as u64, GAIN_UNIT as u64))
  }

  /// 单次突发包数：按 250 µs 调度粒度 `clamp(bw/4000/pkt, 2, 32)`；高丢包 4；回环 32；无样本 8（RTT > 80 ms 取 12）
  pub const fn burst(&self, pkt: u64, rtt: &RttEstimator, loopback: bool) -> usize {
    if loopback {
      return BURST_LOOP;
    }
    if self.high_loss {
      return BURST_LOSS;
    }
    if self.max_bw == 0 || pkt == 0 {
      return if rtt.srtt_us() > BURST_FAR_RTT_US {
        BURST_INIT_FAR
      } else {
        BURST_INIT
      };
    }
    // 结果 ≤ BURST_MAX，usize 不截断
    clamp(self.max_bw / BURST_TICK_DIV / pkt, BURST_MIN, BURST_MAX) as usize
  }
}
