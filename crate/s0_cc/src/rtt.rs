use crate::math::clamp;

/// TACK 最小周期（2 ms = 2000 µs）
pub const MIN_TACK_US: u64 = 2_000;
/// TACK 最大周期（20 ms = 20000 µs）
pub const MAX_TACK_US: u64 = 20_000;
/// RTO 下限（5 ms）
pub const MIN_RTO_US: u64 = 5_000;
/// RTO 上限（2 s）
pub const MAX_RTO_US: u64 = 2_000_000;
/// RACK 乱序窗口上限（250 ms）
pub const MAX_REO_US: u64 = 250_000;

/// RACK 重排窗口下限（1 ms）
const MIN_REO_US: u64 = 1_000;
/// 前瞻乱序抬升上限（30 ms）：防止高 RTT 链路下 ACK 延迟虚抬 `reo_us`
const MAX_PROACTIVE_REO_US: u64 = 30_000;
/// 尚无 RTT 样本时的 TACK 间隔（10 ms）
const DEFAULT_TACK_US: u64 = 10_000;
/// 单个 RTT 样本上限（60 s）：截断异常时间戳，防 EWMA 乘法溢出
const MAX_SAMPLE_US: u64 = 60_000_000;
/// `rtt_min` 向上阶跃所需的连续高时延样本数
const STEP_SAMPLES: u16 = 16;
/// 高时延判定的最小绝对增量（10 ms）：`sample > rtt_min + max(rtt_min/2, 10ms)`
const STEP_MIN_US: u64 = 10_000;
/// 连续多少个正常 ACK 后衰减乱序下限
const DECAY_ACKS: u16 = 32;

/// TACK 周期 ACK 间隔（全协议唯一定义）：每 RTT 约 4 次，`clamp(rtt/4, 2ms, 20ms)`；`rtt_us = 0` 表示未知
pub const fn ack_interval_us(rtt_us: u64) -> u64 {
  if rtt_us == 0 {
    DEFAULT_TACK_US
  } else {
    clamp(rtt_us / 4, MIN_TACK_US, MAX_TACK_US)
  }
}

/// 自适应 RTT、RTO 与 RACK 乱序时间窗估计器（sans-IO，可独立复用于任意传输协议）
///
/// 特性：
/// - **非对称平滑 RTT（`srtt_us`）**：时延下降以 `1/4` 权重快速收敛（4G → Wi-Fi/5G），上升以 `1/8` 权重过滤瞬态排队抖动
/// - **双向阶跃 `rtt_min_us`**：遇更低时延立即下探；连续 16 个样本显著高于历史最小 RTT（Wi-Fi → 4G 切换或路由重收敛）自动向上刷新基准传播时延，防止 BDP 饥饿
/// - **自适应乱序窗（`reo_us`）**：遇乱序按偏离跨度前瞻抬升下限，遇伪重传翻倍，连续 32 个有序无误判 ACK 后按 `3/4` 平滑衰减下限
#[derive(Clone, Copy, Debug)]
pub struct RttEstimator {
  srtt_us: u64,
  rttvar_us: u64,
  rtt_min_us: u64,
  win_min_us: u64,
  high_rtt_cnt: u16,
  clean_acks: u16,
  has_rtt: bool,
  reo_us: u64,
  reo_floor_us: u64,
}

impl Default for RttEstimator {
  fn default() -> Self {
    Self::new()
  }
}

impl RttEstimator {
  pub const fn new() -> Self {
    Self {
      srtt_us: 0,
      rttvar_us: 0,
      rtt_min_us: u64::MAX,
      win_min_us: u64::MAX,
      high_rtt_cnt: 0,
      clean_acks: 0,
      has_rtt: false,
      reo_us: MIN_REO_US,
      reo_floor_us: MIN_REO_US,
    }
  }

  pub const fn has_rtt(&self) -> bool {
    self.has_rtt
  }

  pub const fn srtt_us(&self) -> u64 {
    self.srtt_us
  }

  pub const fn rttvar_us(&self) -> u64 {
    self.rttvar_us
  }

  pub const fn rtt_min_us(&self) -> u64 {
    if self.has_rtt { self.rtt_min_us } else { 0 }
  }

  pub const fn reo_wnd_us(&self) -> u64 {
    self.reo_us
  }

  const fn pto_us(&self) -> u64 {
    self
      .srtt_us
      .saturating_add(self.rttvar_us.saturating_mul(4))
  }

  pub const fn rto_us(&self) -> u64 {
    if !self.has_rtt {
      return MAX_RTO_US;
    }
    clamp(self.pto_us(), MIN_RTO_US, MAX_RTO_US)
  }

  pub const fn tlp_us(&self) -> u64 {
    if !self.has_rtt {
      return MIN_RTO_US;
    }
    clamp(self.pto_us(), MIN_RTO_US, u64::MAX)
  }

  /// TACK 周期性 ACK 间隔，见 [`ack_interval_us`]
  pub const fn tack_interval_us(&self) -> u64 {
    ack_interval_us(self.srtt_us)
  }

  /// 输入一个新的 RTT 测量样本（微秒，`0` 忽略，超过 60 s 截断）
  pub fn on_sample(&mut self, sample: u64) {
    self.on_sample_with_inflight(sample, true);
  }

  /// 输入一个新的 RTT 测量样本；`allow_step_up = false` 时（在途窗口较大、排队可能由自身突发引起）仅允许下探 `rtt_min` 并更新 `srtt`，防止自致排队污染基准传播时延
  pub fn on_sample_with_inflight(&mut self, sample: u64, allow_step_up: bool) {
    if sample == 0 {
      return;
    }
    let sample = sample.min(MAX_SAMPLE_US);
    if !self.has_rtt {
      self.srtt_us = sample;
      self.rttvar_us = sample / 2;
      self.rtt_min_us = sample;
      self.win_min_us = u64::MAX;
      self.high_rtt_cnt = 0;
      self.has_rtt = true;
    } else {
      let diff = self.srtt_us.abs_diff(sample);
      self.rttvar_us = (self.rttvar_us * 3 + diff) / 4;
      // 链路切换导致时延骤降（如 4G → Wi-Fi/5G）时以 1/4 权重快速收敛，上升时保持 1/8 抑制抖动
      self.srtt_us = if sample < self.srtt_us {
        (self.srtt_us * 3 + sample) / 4
      } else {
        (self.srtt_us * 7 + sample) / 8
      };
      if sample <= self.rtt_min_us {
        self.rtt_min_us = sample;
        self.win_min_us = u64::MAX;
        self.high_rtt_cnt = 0;
      } else if allow_step_up {
        let step_thresh = self.rtt_min_us + (self.rtt_min_us / 2).max(STEP_MIN_US);
        if sample > step_thresh {
          self.win_min_us = self.win_min_us.min(sample);
          self.high_rtt_cnt = self.high_rtt_cnt.saturating_add(1);
          // 连续 16 个样本显著高于历史最小 RTT（如 Wi-Fi 切换至移动蜂窝网或路由重收敛），更新基准传播时延
          if self.high_rtt_cnt >= STEP_SAMPLES {
            self.rtt_min_us = self.win_min_us;
            self.win_min_us = u64::MAX;
            self.high_rtt_cnt = 0;
          }
        } else {
          self.high_rtt_cnt = 0;
          self.win_min_us = u64::MAX;
        }
      }
    }
    self.reo_us = (self.rtt_min_us / 4).clamp(self.reo_floor_us, MAX_REO_US);
  }

  const fn proactive_reo_cap(&self) -> u64 {
    let half = self.rtt_min_us / 2;
    let quarter = self.rtt_min_us / 4;
    let capped = if half < MAX_PROACTIVE_REO_US {
      half
    } else {
      MAX_PROACTIVE_REO_US
    };
    if capped > quarter { capped } else { quarter }
  }

  /// 观察到乱序到达的原始包（`pkt_age_us = now_us - sent_us`）：前瞻抬升重排窗口（上限不超过 `max(rtt_min/4, min(rtt_min/2, 30ms))`，避免高 RTT 下 ACK 延迟/截断虚抬 `reo_us` 致 RACK 迟钝；真实超长乱序由 `on_spurious` 兜底）
  pub fn on_out_of_order(&mut self, pkt_age_us: u64) {
    if !self.has_rtt {
      return;
    }
    self.clean_acks = 0;
    let base_rtt = self.srtt_us.min(self.rtt_min_us.saturating_mul(2));
    let spread = pkt_age_us.saturating_sub(base_rtt);
    let target = (spread + self.rtt_min_us / 4).min(self.proactive_reo_cap());
    self.reo_floor_us = self.reo_floor_us.max(target);
    self.reo_us = self.reo_us.max(self.reo_floor_us);
  }

  /// 观察到伪重传（被 RACK 判丢的包随后被确认）：翻倍重排窗口并抬升下限
  pub fn on_spurious(&mut self) {
    self.clean_acks = 0;
    let grown = (self.reo_us * 2).max(self.rtt_min_us()).min(MAX_REO_US);
    self.reo_floor_us = self.reo_floor_us.max(grown);
    self.reo_us = grown;
  }

  /// 收到无新确认的重复包提示：小幅抬升重排窗口
  pub fn on_duplicate_ack(&mut self) {
    self.clean_acks = 0;
    self.reo_us = (self.reo_us * 2).min(self.proactive_reo_cap());
  }

  /// 收到有序且无伪重传的正常 ACK：累计 32 次后按 3/4 平滑衰减历史乱序下限
  pub fn on_clean_ack(&mut self) {
    if !self.has_rtt {
      return;
    }
    self.clean_acks = self.clean_acks.saturating_add(1);
    if self.clean_acks >= DECAY_ACKS {
      self.clean_acks = 0;
      let base_floor = (self.rtt_min_us / 8).max(MIN_REO_US);
      if self.reo_floor_us > base_floor {
        self.reo_floor_us = ((self.reo_floor_us * 3 + base_floor) / 4).max(MIN_REO_US);
        self.reo_us = (self.rtt_min_us / 4).clamp(self.reo_floor_us, MAX_REO_US);
      }
    }
  }
}
