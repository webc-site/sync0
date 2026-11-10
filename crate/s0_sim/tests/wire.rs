use aok::{OK, Void};
use ctor::ctor;
use log_init::init;
use s0_sim::{
  DeltaSimSpec, SimLink, run_delta_wan_sim, run_wan_sim, run_wan_sim_dynamic, run_wan_sim_harsh,
};

#[ctor(unsafe)]
fn _log_init() {
  init();
}

#[test]
fn test_delta_wan_sim_partial_modifications() -> Void {
  let link = SimLink {
    bw_bps: 100_000_000,
    rtt_us: 38_000,
    loss_ppm: 1_000,
    queue_pkts: 512,
    seed: 77,
  };

  // 场景 A：10 MiB 大文件原位局部覆写 16 KiB + 尾部追加 128 KiB（模拟 tar -rf 与原位补丁）
  let rep_in_place = run_delta_wan_sim(DeltaSimSpec {
    link,
    basis_size: 10 << 20,
    insert_at: None,
    patch_at: Some((4 << 20, 16 * 1024)),
    append_len: 128 * 1024,
  });
  assert!(
    rep_in_place.in_place,
    "append + patch must be in-place: {:?}",
    rep_in_place
  );
  assert!(
    rep_in_place.dedup_ratio_pct > 95.0,
    "in-place dedup ratio too low: {:?}",
    rep_in_place
  );
  assert!(
    rep_in_place.effective_goodput_bps > link.bw_bps * 5,
    "effective goodput must exceed 5x physical link bw: {:?}",
    rep_in_place
  );

  // 场景 B：10 MiB 大文件在 1 MiB 处插入 777 字节引发后续 9 MiB 全量偏移（Byte-Shift）
  let rep_shifted = run_delta_wan_sim(DeltaSimSpec {
    link,
    basis_size: 10 << 20,
    insert_at: Some((1 << 20, 777)),
    patch_at: None,
    append_len: 0,
  });
  assert!(
    !rep_shifted.in_place,
    "byte-shifted delta should require copy relocate: {:?}",
    rep_shifted
  );
  assert!(
    rep_shifted.dedup_ratio_pct > 95.0,
    "byte-shift FastCDC resync dedup ratio too low: {:?}",
    rep_shifted
  );
  assert!(
    rep_shifted.effective_goodput_bps > link.bw_bps * 5,
    "shifted effective goodput must exceed 5x physical link bw: {:?}",
    rep_shifted
  );
  OK
}

#[test]
fn test_adaptive_multi_network_profiles_sim() -> Void {
  // 1. 数据中心 / 内网千兆低时延链路（1 Gbps, 1 ms RTT, 0% 丢包）
  let rep_lan = run_wan_sim(SimLink::lan_1g(), 16 << 20);
  assert!(
    rep_lan.efficiency_pct >= 80.0,
    "lan_1g efficiency too low: {:?}",
    rep_lan
  );

  // 2. 同大洲城域广域网（500 Mbps, 20 ms RTT, 0.05% 丢包）
  let rep_reg = run_wan_sim(SimLink::regional_500m(), 16 << 20);
  assert!(
    rep_reg.efficiency_pct >= 78.0,
    "regional_500m efficiency too low: {:?}",
    rep_reg
  );

  // 3. 跨洋云骨干高 BDP 专线（300 Mbps, 90 ms RTT, 0.1% 丢包）
  let rep_ocean = run_wan_sim(SimLink::trans_oceanic_300m(), 32 << 20);
  assert!(
    rep_ocean.efficiency_pct >= 75.0 && rep_ocean.retx_packets <= 40,
    "trans_oceanic_300m efficiency too low: {:?}",
    rep_ocean
  );

  // 4. 跨太平洋长肥浅队列链路（本机 <-> c1 同款：45 Mbps, 180 ms RTT, 0.2% 丢包, 192 包浅队列）
  let rep_pac = run_wan_sim(SimLink::trans_pacific_45m(), 16 << 20);
  assert!(
    rep_pac.efficiency_pct >= 72.0,
    "trans_pacific_45m efficiency too low: {:?}",
    rep_pac
  );
  assert!(
    rep_pac.rtt_min_us >= 170_000,
    "rtt_min must not be clamped at 80ms: {:?}",
    rep_pac
  );

  // 5. 弱网受限边缘移动链路（15 Mbps, 220 ms RTT, 1.0% 随机丢包, 128 包浅队列）
  let rep_edge = run_wan_sim(SimLink::lossy_edge_15m(), 8 << 20);
  assert!(
    rep_edge.efficiency_pct >= 65.0,
    "lossy_edge_15m efficiency too low: {:?}",
    rep_edge
  );

  // 6. 动态带宽阶跃变化（120 Mbps -> 250 ms 后骤降至 35 Mbps，验证拥塞收敛不雪崩）
  let rep_step = run_wan_sim_dynamic(
    SimLink {
      bw_bps: 120_000_000,
      rtt_us: 80_000,
      loss_ppm: 1_000,
      queue_pkts: 256,
      seed: 66,
    },
    8 << 20,
    Some((250_000, 35_000_000)),
  );
  assert!(
    rep_step.goodput_bps >= 22_000_000,
    "dynamic step-down goodput too low: {:?}",
    rep_step
  );

  // 7. 突发连丢 + 乱序抖动 + 反向尾 ACK 丢失恶劣网络（25 Mbps, 200 ms RTT, ±15 ms 乱序抖动, 3 包连丢, 3% ACK 丢包）
  let rep_harsh = run_wan_sim_harsh(
    SimLink::harsh_burst_jitter_25m(),
    8 << 20,
    15_000,
    30_000,
    3,
  );
  assert!(
    rep_harsh.efficiency_pct >= 50.0,
    "harsh_burst_jitter_25m efficiency too low: {:?}",
    rep_harsh
  );

  // 8. 高轨卫星 / 超高 RTT 代理弱网（20 Mbps, 500 ms RTT, ±10 ms 抖动, 2 包连丢, 2% ACK 丢包 + 尾包挥手保持期校验）
  let rep_sat = run_wan_sim_harsh(SimLink::satellite_geo_20m(), 8 << 20, 10_000, 20_000, 2);
  assert!(
    rep_sat.efficiency_pct >= 45.0 && rep_sat.rtt_min_us >= 480_000,
    "satellite_geo_20m efficiency too low: {:?}",
    rep_sat
  );

  // 9. 多跳卫星 / 海事 BGAN / 2G 重传 3 秒高时延恶劣网络（15 Mbps, 3000 ms = 3s RTT, ±50 ms 乱序抖动, 2 包连丢, 2% ACK 丢包 + 强制首轮尾 ACK 丢失）
  let rep_3s = run_wan_sim_harsh(SimLink::extreme_delay_3s(), 8 << 20, 50_000, 20_000, 2);
  assert!(
    rep_3s.goodput_bps > 0 && rep_3s.rtt_min_us >= 2_900_000 && rep_3s.srtt_us >= 2_900_000,
    "extreme_delay_3s failed or clamped RTT: {:?}",
    rep_3s
  );

  // 10. 极限时延网络（Iridium / 多跳卫星叠加深度 Bufferbloat：8 Mbps, 10000 ms = 10s RTT, ±100 ms 乱序抖动, 2 包连丢, 2% ACK 丢包 + 强制首轮尾 ACK 丢失）
  let rep_10s = run_wan_sim_harsh(SimLink::ultra_extreme_10s(), 4 << 20, 100_000, 20_000, 2);
  assert!(
    rep_10s.goodput_bps > 0 && rep_10s.rtt_min_us >= 9_800_000 && rep_10s.srtt_us >= 9_800_000,
    "ultra_extreme_10s failed or clamped RTT: {:?}",
    rep_10s
  );
  OK
}
