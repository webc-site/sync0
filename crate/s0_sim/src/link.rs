/// 离散事件广域网链路仿真参数
#[derive(Clone, Copy, Debug)]
pub struct SimLink {
  /// 瓶颈链路物理带宽（bit/s，例如 `100_000_000` = 100 Mbps）
  pub bw_bps: u64,
  /// 基础往返传播时延（µs，例如 `40_000` = 40 ms）
  pub rtt_us: u64,
  /// 随机丢包率（百万分比 ppm，`1_000` = 0.1%，`10_000` = 1.0%）
  pub loss_ppm: u32,
  /// 瓶颈路由器队列深度上限（包数，超过触发尾丢弃 Tail-Drop）
  pub queue_pkts: usize,
  /// 确定性伪随机种子
  pub seed: u64,
}

impl Default for SimLink {
  fn default() -> Self {
    Self {
      bw_bps: 100_000_000,
      rtt_us: 40_000,
      loss_ppm: 1_000,
      queue_pkts: 512,
      seed: 42,
    }
  }
}

impl SimLink {
  const fn profile(bw_bps: u64, rtt_us: u64, loss_ppm: u32, queue_pkts: usize, seed: u64) -> Self {
    Self {
      bw_bps,
      rtt_us,
      loss_ppm,
      queue_pkts,
      seed,
    }
  }

  /// 数据中心 / 内网千兆低时延链路（1 Gbps, 1 ms RTT, 0% 丢包, 1024 包缓冲）
  #[must_use]
  pub const fn lan_1g() -> Self {
    Self::profile(1_000_000_000, 1_000, 0, 1024, 11)
  }

  /// 同大洲城域广域网（500 Mbps, 20 ms RTT, 0.05% 丢包, 1024 包缓冲）
  #[must_use]
  pub const fn regional_500m() -> Self {
    Self::profile(500_000_000, 20_000, 500, 1024, 22)
  }

  /// 跨洋云骨干高 BDP 专线（300 Mbps, 90 ms RTT, 0.1% 丢包, 1024 包缓冲）
  #[must_use]
  pub const fn trans_oceanic_300m() -> Self {
    Self::profile(300_000_000, 90_000, 1_000, 1024, 33)
  }

  /// 跨太平洋家庭/办公长肥浅队列链路（如本机 <-> c1：45 Mbps, 180 ms RTT, 0.2% 丢包, 192 包浅队列）
  #[must_use]
  pub const fn trans_pacific_45m() -> Self {
    Self::profile(45_000_000, 180_000, 2_000, 192, 44)
  }

  /// 弱网受限边缘移动链路（15 Mbps, 220 ms RTT, 1.0% 随机丢包, 128 包浅队列）
  #[must_use]
  pub const fn lossy_edge_15m() -> Self {
    Self::profile(15_000_000, 220_000, 10_000, 128, 55)
  }

  /// 高抖动乱序 + 突发连丢跨洋恶劣弱网（25 Mbps, 200 ms RTT, 0.4% 触发突发连丢, 160 包浅队列）
  #[must_use]
  pub const fn harsh_burst_jitter_25m() -> Self {
    Self::profile(25_000_000, 200_000, 4_000, 160, 66)
  }

  /// 高轨卫星 / 多跳跨洲代理超高时延链路（20 Mbps, 500 ms RTT, 0.2% 丢包, 256 包队列）
  #[must_use]
  pub const fn satellite_geo_20m() -> Self {
    Self::profile(20_000_000, 500_000, 2_000, 256, 88)
  }

  /// 多跳地球同步卫星 / 海事 BGAN / 2G 无线重传恶劣高时延链路（15 Mbps, 3000 ms = 3s RTT, 0.2% 丢包, 384 包队列）
  #[must_use]
  pub const fn extreme_delay_3s() -> Self {
    Self::profile(15_000_000, 3_000_000, 2_000, 384, 99)
  }

  /// 窄带卫星 Iridium / 多跳卫星叠加深度 Bufferbloat 极限时延链路（8 Mbps, 10000 ms = 10s RTT, 0.2% 丢包, 512 包队列）
  #[must_use]
  pub const fn ultra_extreme_10s() -> Self {
    Self::profile(8_000_000, 10_000_000, 2_000, 512, 108)
  }
}

/// 离散事件仿真统计报告（对照论文 Shannon / BDP 理论极限）
#[derive(Clone, Copy, Debug)]
pub struct SimReport {
  /// 实际有效应用层吞吐（bit/s）
  pub goodput_bps: u64,
  /// 扣除协议头与信道丢包后的理论容量上界 $\eta_{\text{hdr}} \cdot C \cdot (1 - p)$（bit/s）
  pub theoretical_max_bps: u64,
  /// 达到理论极限的效率百分比（`0.0..=100.0`）
  pub efficiency_pct: f64,
  /// 仿真虚拟耗时（µs）
  pub sim_elapsed_us: u64,
  /// 总发送数据包数
  pub total_packets: u64,
  /// 重传数据包数
  pub retx_packets: u64,
  /// 收敛平滑往返时延（µs）
  pub srtt_us: u64,
  /// 最小往返时延（µs）
  pub rtt_min_us: u64,
}
