use std::{collections::VecDeque, mem::replace};

mod delta_sim;
mod link;

pub use delta_sim::{DeltaSimReport, DeltaSimSpec, run_delta_wan_sim};
use fastrand::Rng;
pub use link::{SimLink, SimReport};
use s0_cc::Cc;
use s0_cdc::SliceWr;
use s0_const::{DATA_HLEN, DEFAULT_PKT, MAX_BLOCK, MAX_DATAGRAM, RETX, WINDOW_BLOCKS};
use s0_udp::{AckGen, Config, Outcome, Receiver, Sender};
use s0_wire::{Ack, AckOut, Kind, SessionHeader, StreamDataHeader, encode_ack};

struct InflightData {
  deliver_us: u64,
  sent_us: u64,
  hdr: StreamDataHeader,
  payload_len: usize,
}

struct InflightAck {
  deliver_us: u64,
  raw: Vec<u8>,
}

/// 运行毫秒级确定性离散事件 WAN 传输仿真（无先知盲启动，直接驱动生产级 `Sender` + `Cc` + `Receiver` + `AckGen`）
pub fn run_wan_sim(link: SimLink, total_bytes: u64) -> SimReport {
  run_wan_sim_dynamic(link, total_bytes, None)
}

/// 运行带乱序抖动、突发连丢、反向 ACK 丢包及尾包挥手保持期校验的恶劣网络仿真
pub fn run_wan_sim_harsh(
  link: SimLink,
  total_bytes: u64,
  jitter_us: u64,
  ack_loss_ppm: u32,
  burst_len: u8,
) -> SimReport {
  run_wan_sim_core(
    link,
    total_bytes,
    None,
    (jitter_us, ack_loss_ppm, burst_len.max(1)),
  )
}

/// 运行支持中途带宽阶跃变化的离散事件 WAN 传输仿真（`bw_step`: `Some((触发时刻_us, 新带宽_bps))`）
pub fn run_wan_sim_dynamic(
  link: SimLink,
  total_bytes: u64,
  bw_step: Option<(u64, u64)>,
) -> SimReport {
  run_wan_sim_core(link, total_bytes, bw_step, (0, 0, 1))
}

fn run_wan_sim_core(
  link: SimLink,
  total_bytes: u64,
  bw_step: Option<(u64, u64)>,
  (jitter_us, ack_loss_ppm, burst_len): (u64, u32, u8),
) -> SimReport {
  let nblk = total_bytes.div_ceil(MAX_BLOCK as u64).max(1) as u32;
  let pkt_payload = DEFAULT_PKT;
  let pkt_wire = (pkt_payload + DATA_HLEN) as u64;
  let wire_pkt_bits = pkt_wire * 8;
  let half_rtt_us = (link.rtt_us / 2).max(1);

  let mut rng = Rng::with_seed(link.seed);
  let mut sender = Sender::new(65_536, 0).expect("valid ring");
  sender.seed_rtt(link.rtt_us);
  let mut cc = Cc::new();
  let cfg = Config::new(WINDOW_BLOCKS as usize, MAX_BLOCK, pkt_payload).expect("valid cfg");
  let mut recv = Receiver::new(cfg).expect("valid receiver");
  let mut ack_gen = AckGen::new();

  let blk_seg_cnt = |blk: u32| -> u8 {
    let rem = total_bytes.saturating_sub(u64::from(blk) * (MAX_BLOCK as u64));
    let b_len = rem.min(MAX_BLOCK as u64) as usize;
    b_len.div_ceil(pkt_payload).max(1) as u8
  };

  let (mut now_us, mut bottleneck_free_us, mut next_send_us) = (1_000u64, 1_000u64, 1_000u64);
  let start_us = now_us;
  let (mut next_blk, mut cur_idx, mut done_lo) = (0u32, 0u8, 0u32);
  let (mut inflight_bytes, mut total_packets, mut retx_packets) = (0u64, 0u64, 0u64);
  let (mut burst_rem, mut dropped_first_fin_ack, mut rx_complete_at) = (0u8, false, None::<u64>);
  let rx_linger_us = (link.rtt_us.saturating_mul(4)).clamp(1_000_000, 60_000_000);
  let max_sim_us = (link.rtt_us.saturating_mul(40)).clamp(60_000_000, 300_000_000);

  let mut acked_bits = vec![0u128; WINDOW_BLOCKS as usize];
  let mut lost_q: VecDeque<(u32, u8)> = VecDeque::new();
  let mut fwd_pipe: VecDeque<InflightData> = VecDeque::new();
  let mut rev_pipe: VecDeque<InflightAck> = VecDeque::new();
  let dummy_payload = [0u8; DEFAULT_PKT];

  while done_lo < nblk && now_us < start_us + max_sim_us {
    // 1. 接收端处理到达的数据包并按 TACK 策略生成 ACK
    while let Some(front) = fwd_pipe.front() {
      if front.deliver_us > now_us {
        break;
      }
      let pkt = fwd_pipe.pop_front().unwrap();
      let deliver_t = pkt.deliver_us;
      if rx_complete_at.is_some_and(|at| deliver_t.saturating_sub(at) > rx_linger_us) {
        continue;
      }
      ack_gen.on_packet(pkt.hdr.pn);
      if let Ok(Outcome::BlockReady { blk, .. }) =
        recv.on_data(&pkt.hdr, &dummy_payload[..pkt.payload_len])
      {
        let _ = recv.release(blk);
      }
      if recv.done_lo() >= nblk {
        rx_complete_at.get_or_insert(deliver_t);
      }
      if (recv.done_lo() >= nblk || ack_gen.should_send(deliver_t, link.rtt_us))
        && let Some(view) = ack_gen.build()
      {
        let drop_fin_ack =
          ack_loss_ppm > 0 && recv.done_lo() >= nblk && !replace(&mut dropped_first_fin_ack, true);
        let drop_rand_ack = ack_loss_ppm > 0 && rng.u32(0..1_000_000) < ack_loss_ppm;
        if drop_fin_ack || drop_rand_ack {
          ack_gen.mark_sent(deliver_t);
          continue;
        }
        let (blocks, nb) = recv.collect_ack_blocks();
        let hdr = SessionHeader {
          kind: Kind::Ack,
          flag: 0,
          x: 0,
          sid: 1,
          pn: 0,
        };
        let mut raw = vec![0u8; MAX_DATAGRAM];
        let mut w = SliceWr::new(&mut raw);
        encode_ack(
          &mut w,
          &hdr,
          &AckOut {
            largest: view.largest,
            delay: 0,
            ts_echo: pkt.sent_us as u32,
            rx_pkts: 0,
            credit: recv.credit(),
            done_lo: recv.done_lo(),
            dec: 2_000,
            flag: view.flag,
            ranges: view.ranges,
            blocks: &blocks[..nb],
          },
        )
        .expect("ack encode");
        ack_gen.mark_sent(deliver_t);
        let n = w.pos();
        raw.truncate(n);
        rev_pipe.push_back(InflightAck {
          deliver_us: deliver_t + half_rtt_us,
          raw,
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
      let Ok((hdr, body)) = SessionHeader::parse(&ack_evt.raw) else {
        continue;
      };
      let Ok(ack) = Ack::parse(&hdr, body) else {
        continue;
      };
      let prev_lo = done_lo;
      done_lo = done_lo.max(ack.done_lo);
      for b in prev_lo..done_lo {
        acked_bits[(b as usize) & ((WINDOW_BLOCKS as usize) - 1)] = 0;
      }
      for (b, have) in ack.blocks() {
        if b >= done_lo && b < done_lo + WINDOW_BLOCKS {
          acked_bits[(b as usize) & ((WINDOW_BLOCKS as usize) - 1)] |= have;
        }
      }
      let mut newly_lost = Vec::new();
      if let Ok(stats) = sender.on_ack(
        &ack,
        ack_t,
        |b, i| {
          if b >= done_lo && b < done_lo + WINDOW_BLOCKS {
            acked_bits[(b as usize) & ((WINDOW_BLOCKS as usize) - 1)] |= 1u128 << i;
          }
        },
        |lpn| newly_lost.push(lpn),
      ) {
        let pkt_wire = (pkt_payload + DATA_HLEN) as u64;
        inflight_bytes =
          inflight_bytes.saturating_sub(u64::from(stats.acked + stats.lost) * pkt_wire);
        for lpn in newly_lost {
          if let Some(info) = sender.sent(lpn)
            && info.blk >= done_lo
          {
            let have = acked_bits[(info.blk as usize) & ((WINDOW_BLOCKS as usize) - 1)];
            if (have >> info.idx) & 1 == 0 {
              lost_q.push_back((info.blk, info.idx));
            }
          }
        }
        cc.on_ack(
          ack_t,
          sender.delivered(),
          stats.acked,
          stats.lost,
          sender.rtt(),
        );
      }
    }

    if done_lo >= nblk {
      break;
    }

    // 3. 发送端按零先知 Chirp + BBRv3/Copa 自适应 Pacing 与 CWND 发送重传包或新分片
    let cwnd = cc.cwnd_pkts(pkt_wire, sender.rtt()) * pkt_wire;
    if now_us >= next_send_us && inflight_bytes < cwnd {
      let send_cand = loop {
        if let Some((blk, idx)) = lost_q.pop_front() {
          if blk >= done_lo
            && (acked_bits[(blk as usize) & ((WINDOW_BLOCKS as usize) - 1)] >> idx) & 1 == 0
          {
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
        let rem_blk = total_bytes
          .saturating_sub(u64::from(blk) * (MAX_BLOCK as u64))
          .min(MAX_BLOCK as u64) as usize;
        let payload_len = if idx + 1 == cnt {
          rem_blk
            .saturating_sub(usize::from(idx) * pkt_payload)
            .max(1)
        } else {
          pkt_payload
        };
        let wire_len = payload_len + DATA_HLEN;
        let flag = if is_retx { RETX } else { 0 };
        if let Ok(pn) = sender.on_sent(blk, idx, wire_len as u16, now_us) {
          let sent_seq = total_packets;
          inflight_bytes += wire_len as u64;
          total_packets += 1;
          if is_retx {
            retx_packets += 1;
          }
          let cur_bw_bps = match bw_step {
            Some((step_t, new_bw)) if now_us.saturating_sub(start_us) >= step_t => new_bw,
            _ => link.bw_bps,
          };
          let serial_us = (wire_pkt_bits * 1_000_000 / cur_bw_bps.max(1)).max(1);
          let q_wait_us = bottleneck_free_us.saturating_sub(now_us);
          let max_q_us = (link.queue_pkts as u64) * serial_us;
          let dropped_by_q = q_wait_us > max_q_us;
          let dropped_by_loss = if burst_rem > 0 {
            burst_rem -= 1;
            true
          } else if link.loss_ppm > 0 && rng.u32(0..1_000_000) < link.loss_ppm {
            burst_rem = burst_len - 1;
            true
          } else {
            false
          };
          if !dropped_by_q && !dropped_by_loss {
            let depart_us = bottleneck_free_us.max(now_us) + serial_us;
            bottleneck_free_us = depart_us;
            let jit = if jitter_us > 0 {
              rng.u64(0..=jitter_us)
            } else {
              0
            };
            let deliver_us = depart_us + half_rtt_us + jit;
            let pos = fwd_pipe.partition_point(|x| x.deliver_us <= deliver_us);
            fwd_pipe.insert(
              pos,
              InflightData {
                deliver_us,
                sent_us: now_us,
                hdr: StreamDataHeader {
                  flag,
                  idx,
                  cnt,
                  sid: 1,
                  pn,
                  blk,
                  ts: now_us as u32,
                },
                payload_len,
              },
            );
          }
          let gap_us = cc.pacing_gap_us(wire_len as u64, sender.rtt(), sent_seq);
          next_send_us = (next_send_us + gap_us).max(now_us);
          continue;
        }
      }
    }

    // 4. 推进离散事件时钟到下一个最早事件（若超时未收到 ACK 则触发 PTO 重传）
    let mut next_t = u64::MAX;
    if inflight_bytes < cwnd
      && (!lost_q.is_empty() || next_blk < (done_lo + WINDOW_BLOCKS).min(nblk))
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
      let rtt_est = sender.rtt().srtt_us();
      let pto_us = (rtt_est + (rtt_est >> 2) + sender.rtt().rttvar_us()).clamp(20_000, 60_000_000);
      now_us += pto_us;
      inflight_bytes = 0;
      let have = acked_bits[(done_lo as usize) & ((WINDOW_BLOCKS as usize) - 1)];
      let cnt = blk_seg_cnt(done_lo);
      for i in 0..cnt {
        if (have >> i) & 1 == 0 {
          lost_q.push_back((done_lo, i));
        }
      }
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
    srtt_us: rtt.srtt_us(),
    rtt_min_us: rtt.rtt_min_us(),
  }
}
