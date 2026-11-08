use std::collections::VecDeque;

use fastrand::Rng;
use s0_cc::Cc;
use s0_const::{ACK_BLOCKS, ACK_DELAY_US, DATA_HLEN, DEFAULT_PKT, MAX_BLOCK, WINDOW_BLOCKS};
use s0_wire::{Ack, AckGen, Data, Receiver, Sender};

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

struct InflightData {
  deliver_us: u64,
  pn: u64,
  blk: u32,
  idx: u8,
  cnt: u8,
  payload_len: usize,
  retx: bool,
}

struct InflightAck {
  deliver_us: u64,
  ack: Ack,
}

/// 运行毫秒级确定性离散事件 WAN 传输仿真（直接驱动生产级 `Sender` + `Cc` + `Receiver` + `AckGen`）
pub fn run_wan_sim(link: SimLink, total_bytes: u64) -> SimReport {
  let nblk = total_bytes.div_ceil(MAX_BLOCK as u64).max(1) as u32;
  let pkt_payload = DEFAULT_PKT;
  let wire_pkt_bits = ((pkt_payload + DATA_HLEN) as u64) * 8;
  let serial_us = (wire_pkt_bits * 1_000_000 / link.bw_bps.max(1)).max(1);
  let half_rtt_us = (link.rtt_us / 2).max(1);

  let mut rng = Rng::with_seed(link.seed);
  let mut sender = Sender::new(8192, 0).expect("valid ring");
  sender.seed_rtt(link.rtt_us.min(80_000));
  let mut cc = Cc::new();
  let mut recv = Receiver::new(WINDOW_BLOCKS, nblk, pkt_payload as u16).expect("valid receiver");
  let mut ack_gen = AckGen::default();

  let blk_seg_cnt = |blk: u32| -> u8 {
    let rem = total_bytes.saturating_sub(u64::from(blk) * (MAX_BLOCK as u64));
    let b_len = rem.min(MAX_BLOCK as u64) as usize;
    b_len.div_ceil(pkt_payload).max(1) as u8
  };

  let mut now_us = 1_000u64;
  let start_us = now_us;
  let mut bottleneck_free_us = now_us;
  let mut next_send_us = now_us;
  let mut next_blk = 0u32;
  let mut cur_idx = 0u8;
  let mut done_lo = 0u32;
  let mut inflight_bytes = 0u64;
  let mut total_packets = 0u64;
  let mut retx_packets = 0u64;

  let mut lost_q: VecDeque<(u32, u8)> = VecDeque::new();
  let mut lost_pns: Vec<u64> = Vec::new();
  let mut fwd_pipe: VecDeque<InflightData> = VecDeque::new();
  let mut rev_pipe: VecDeque<InflightAck> = VecDeque::new();
  let dummy_payload = [0u8; DEFAULT_PKT];

  while done_lo < nblk && now_us < start_us + 60_000_000 {
    // 1. 接收端处理到达的数据包并按 TACK 策略生成 ACK
    while let Some(front) = fwd_pipe.front() {
      if front.deliver_us > now_us {
        break;
      }
      let pkt = fwd_pipe.pop_front().unwrap();
      let deliver_t = pkt.deliver_us;
      let data = Data {
        flags: 0,
        cnt: pkt.cnt,
        idx: pkt.idx,
        pn: pkt.pn as u32,
        blk: pkt.blk,
        payload: &dummy_payload[..pkt.payload_len],
      };
      let _ = recv.on_data(&data, deliver_t);
      while recv.is_done(recv.base_blk()) && recv.base_blk() < nblk {
        let _ = recv.release(recv.base_blk());
      }
      if ack_gen.on_Event(
        pkt.pn,
        pkt.retx,
        deliver_t,
        sender.rtt().srtt_us.max(link.rtt_us),
        &recv,
      ) {
        let mut ranges = Vec::new();
        let mut blocks = Vec::new();
        let ack = ack_gen.build_with(deliver_t, &recv, &mut ranges, &mut blocks);
        rev_pipe.push_back(InflightAck {
          deliver_us: deliver_t + half_rtt_us,
          ack: Ack {
            largest: ack.largest,
            delay: ack.delay,
            base_blk: ack.base_blk,
            credit: ack.credit,
            calib_mbps: 0,
            ranges: ranges.into(),
            blocks: blocks
              .into_iter()
              .take(ACK_BLOCKS)
              .collect::<Vec<_>>()
              .into(),
          },
        });
      }
    }

    // 2. 发送端处理到达的 ACK 并驱动 BBRv3/Copa 拥塞控制与 RACK-TLP 丢包检测
    while let Some(front) = rev_pipe.front() {
      if front.deliver_us > now_us {
        break;
      }
      let ack_evt = rev_pipe.pop_front().unwrap();
      let ack_t = ack_evt.deliver_us;
      done_lo = done_lo.max(ack_evt.ack.base_blk);
      let out = sender.on_ack(
        &ack_evt.ack,
        ack_evt.ack.delay * ACK_DELAY_US,
        ack_t,
        &mut lost_pns,
        false,
      );
      inflight_bytes = inflight_bytes.saturating_sub(out.acked_bytes + out.lost_bytes);
      for &lpn in &lost_pns {
        if let Some(m) = sender.get_meta(lpn)
          && m.blk >= done_lo
        {
          lost_q.push_back((m.blk, m.idx));
        }
      }
      let rtt = sender.rtt();
      let sample_rtt = out.latest_rtt_us.unwrap_or(rtt.srtt_us);
      cc.on_ack(
        out.acked_bytes,
        out.delivered_Rate_bps,
        sample_rtt,
        rtt,
        ack_t,
      );
      if out.lost_bytes > 0 {
        cc.on_loss(out.lost_bytes, rtt.srtt_us, ack_t);
      }
    }

    if done_lo >= nblk {
      break;
    }

    // 3. 发送端按 Pacing 与 CWND 发送重传包或新分片
    let cwnd = cc.cwnd(pkt_payload as u64, sender.rtt(), false);
    if now_us >= next_send_us && inflight_bytes < cwnd {
      let send_cand = loop {
        if let Some((blk, idx)) = lost_q.pop_front() {
          if blk >= done_lo {
            break Some((blk, idx, true));
          }
        } else if next_blk < (done_lo + WINDOW_BLOCKS).min(nblk) {
          let cnt = blk_seg_cnt(next_blk);
          let item = (next_blk, cur_idx, false);
          cur_idx += 1;
          if cur_idx >= cnt {
            cur_idx = 0;
            next_blk += 1;
          }
          break Some(item);
        } else {
          break None;
        }
      };

      if let Some((blk, idx, is_retx)) = send_cand {
        let cnt = blk_seg_cnt(blk);
        let wire_len = pkt_payload + DATA_HLEN;
        let pn = sender.next_pn();
        sender.on_Sent(pn, blk, idx, wire_len as u16, now_us, is_retx, false);
        inflight_bytes += wire_len as u64;
        total_packets += 1;
        if is_retx {
          retx_packets += 1;
        }

        let q_Wait_us = bottleneck_free_us.saturating_sub(now_us);
        let max_q_us = (link.queue_pkts as u64) * serial_us;
        let dropped_by_q = q_Wait_us > max_q_us;
        let dropped_by_loss = link.loss_ppm > 0 && rng.u32(0..1_000_000) < link.loss_ppm;
        if !dropped_by_q && !dropped_by_loss {
          let depart_us = bottleneck_free_us.max(now_us) + serial_us;
          bottleneck_free_us = depart_us;
          fwd_pipe.push_back(InflightData {
            deliver_us: depart_us + half_rtt_us,
            pn,
            blk,
            idx,
            cnt,
            payload_len: pkt_payload,
            retx: is_retx,
          });
        }
        let rate = cc.pacing_rate(pkt_payload as u64, sender.rtt()).max(1);
        let gap_us = (((wire_len as u128) * 8 * 1_000_000) / u128::from(rate)).max(1) as u64;
        next_send_us = (next_send_us + gap_us).max(now_us);
        continue;
      }
    }

    // 4. 推进离散事件时钟到下一个最早事件（若超时未收到 ACK 则触发 PTO 重传）
    let mut next_t = u64::MAX;
    if inflight_bytes < cwnd && (!lost_q.is_empty() || next_blk < (done_lo + WINDOW_BLOCKS).min(nblk))
    {
      next_t = next_t.min(next_send_us.max(now_us + 1));
    }
    if let Some(f) = fwd_pipe.front() {
      next_t = next_t.min(f.deliver_us);
    }
    if let Some(r) = rev_pipe.front() {
      next_t = next_t.min(r.deliver_us);
    }
    if next_t == u64::MAX {
      let pto_us = sender.rtt().pto_us().clamp(10_000, 200_000);
      now_us += pto_us;
      inflight_bytes = 0;
      lost_q.push_back((done_lo, 0));
      next_send_us = now_us;
    } else {
      now_us = next_t.max(now_us + 1);
    }
  }

  let sim_elapsed_us = now_us.saturating_sub(start_us).max(1);
  let goodput_bps = ((total_bytes as u128) * 8 * 1_000_000 / u128::from(sim_elapsed_us)) as u64;
  let hdr_eff = (pkt_payload as f64) / ((pkt_payload + DATA_HLEN) as f64);
  let loss_eff = 1.0 - (f64::from(link.loss_ppm) / 1_000_000.0);
  let theoretical_max_bps = ((link.bw_bps as f64) * hdr_eff * loss_eff) as u64;
  let efficiency_pct = if theoretical_max_bps > 0 {
    ((goodput_bps as f64) * 100.0 / (theoretical_max_bps as f64)).min(100.0)
  } else {
    0.0
  };
  let rtt = sender.rtt();
  SimReport {
    goodput_bps,
    theoretical_max_bps,
    efficiency_pct,
    sim_elapsed_us,
    total_packets,
    retx_packets,
    srtt_us: rtt.srtt_us,
    rtt_min_us: rtt.rtt_min_us,
  }
}
