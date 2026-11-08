use std::{
  collections::VecDeque,
  env::var_os,
  fs::File,
  io::ErrorKind,
  mem,
  net::{SocketAddr, UdpSocket},
  path::Path,
  time::Instant,
};

use s0_cc::Cc;
use s0_cdc::{CdcConfig, SliceWr as Wr, chunk_file};

use super::{
  BLK, DGRAM, FIN_MAGIC, PKT, Role, RootBuf, WINDOW_BLOCKS,
  manifest::{
    Entry, ManifestMode, encode_manifest, entries_from_changes, entries_from_scanned, scan_dir,
    send_manifest,
  },
  now_us, send_udp,
  source::{Src, SrcKind, build_tx_spans, cnt_mask},
  stats::{Stats, done_bytes},
  wait_until,
};
use crate::{
  AckGen, BIND_V4, BIND_V6, Progress, Sender, UdpError, bind_std_udp,
  codec::skip_name,
  consts::{MAX_RANGES, RETX, ZSTD},
  fs::ScannedFile,
  wire::{Ack, Kind, SessionHeader, StreamDataHeader as DataHeader},
};

/// 在途环容量（包）
const RING: usize = 1 << 16;
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
/// 尾部探测：单次补发包数上限
const TAIL_BUDGET: u64 = 128;
/// `DIR_MAGIC -> DIR_ACK` 握手 RTT 等待期间预热/预压缩的最大块数（32 块 = 4 MiB）
const PREWARM_BLOCKS: u32 = 32;

enum SrcInput {
  File(File, Vec<Entry>),
  Dir(RootBuf, Vec<Entry>, Vec<u8>),
}

struct SendPlan {
  input: SrcInput,
  logical_total: u64,
  skip_all: bool,
  mode: ManifestMode,
}

/// 极简单向传输（无认证/加密），支持单文件与目录的 FastCDC 增量去重传输
pub fn send(path: &Path, peer: SocketAddr, rate: u64, show: bool) -> Result<Stats, UdpError> {
  send_opt(path, peer, rate, show, false)
}

/// 支持可选 `delete`（删除远端存在但本地不存在的多余文件，类似 `rsync --delete`）的单向传输
pub fn send_opt(
  path: &Path,
  peer: SocketAddr,
  rate: u64,
  show: bool,
  delete: bool,
) -> Result<Stats, UdpError> {
  let bind_addr = if peer.is_ipv4() { BIND_V4 } else { BIND_V6 };
  let sock = bind_std_udp(bind_addr, false)?;
  send_on_sock_opt(&sock, path, peer, rate, show, delete)
}

/// 在已绑定的 UDP 套接字上发送文件或目录（复用探测打洞成功的同一本地端口与 NAT 映射）
pub fn send_on_sock_opt(
  sock: &UdpSocket,
  path: &Path,
  peer: SocketAddr,
  rate: u64,
  show: bool,
  delete: bool,
) -> Result<Stats, UdpError> {
  if path.is_dir() {
    let (entries, arena, logical_total) = scan_dir(path)?;
    send_inner_sock(
      sock,
      SendPlan {
        input: SrcInput::Dir(RootBuf::new(path), entries, arena),
        logical_total,
        skip_all: false,
        mode: ManifestMode {
          delete,
          partial: false,
        },
      },
      peer,
      rate,
      show,
    )
  } else {
    let file = File::open(path)?;
    let logical_total = file.metadata()?.len();
    let skip = path
      .file_name()
      .and_then(|n| n.to_str())
      .is_some_and(skip_name);
    let chunks = chunk_file(&file, logical_total, CdcConfig::DEFAULT)?;
    let entries = vec![Entry {
      path: String::new(),
      size: logical_total,
      chunks,
      del: false,
      cached: None,
    }];
    send_inner_sock(
      sock,
      SendPlan {
        input: SrcInput::File(file, entries),
        logical_total,
        skip_all: skip,
        mode: ManifestMode::default(),
      },
      peer,
      rate,
      show,
    )
  }
}

/// 发送目录下指定的增量文件子集
pub fn send_files(
  root: &Path,
  files: Vec<ScannedFile>,
  peer: SocketAddr,
  rate: u64,
  show: bool,
) -> Result<Stats, UdpError> {
  let (entries, arena, logical_total) = entries_from_scanned(root, files)?;
  send_inner(
    SendPlan {
      input: SrcInput::Dir(RootBuf::new(root), entries, arena),
      logical_total,
      skip_all: false,
      mode: ManifestMode {
        delete: false,
        partial: true,
      },
    },
    peer,
    rate,
    show,
  )
}

/// 发送目录下指定的增量变更批次
pub fn send_changes(
  root: &Path,
  files: Vec<ScannedFile>,
  removed: Vec<String>,
  peer: SocketAddr,
  rate: u64,
  show: bool,
) -> Result<Stats, UdpError> {
  let (entries, arena, logical_total) = entries_from_changes(root, files, removed)?;
  send_inner(
    SendPlan {
      input: SrcInput::Dir(RootBuf::new(root), entries, arena),
      logical_total,
      skip_all: false,
      mode: ManifestMode {
        delete: false,
        partial: true,
      },
    },
    peer,
    rate,
    show,
  )
}

fn send_inner(plan: SendPlan, peer: SocketAddr, rate: u64, show: bool) -> Result<Stats, UdpError> {
  let bind_addr = if peer.is_ipv4() { BIND_V4 } else { BIND_V6 };
  let sock = bind_std_udp(bind_addr, false)?;
  send_inner_sock(&sock, plan, peer, rate, show)
}

fn send_inner_sock(
  sock: &UdpSocket,
  plan: SendPlan,
  peer: SocketAddr,
  rate: u64,
  show: bool,
) -> Result<Stats, UdpError> {
  let SendPlan {
    input,
    logical_total,
    skip_all,
    mode,
  } = plan;
  let (kind, entries) = match input {
    SrcInput::File(f, e) => (SrcKind::File(f), e),
    SrcInput::Dir(r, e, arena) => (SrcKind::Dir(r, arena), e),
  };
  let m = encode_manifest(&entries, mode)?;
  let (full_spans, full_total) = build_tx_spans(&entries, &[]);
  let full_nblk =
    u32::try_from(full_total.div_ceil(BLK as u64)).map_err(|_| UdpError::Arg("文件过大"))?;
  let is_loop = peer.ip().is_loopback();
  let mut src = Src::new(kind, entries, full_spans, full_total, skip_all)?;
  if is_loop {
    src.set_net_state(250_000_000, 0);
  }
  let mut warm_blk = 0u32;
  let warm_limit = full_nblk.min(PREWARM_BLOCKS);

  let start = Instant::now();
  let (manifest_rtt, have_bits) = send_manifest(sock, peer, &m, || {
    if warm_blk < warm_limit {
      let _ = src.get(warm_blk);
      warm_blk += 1;
      true
    } else {
      false
    }
  })?;
  let total = if have_bits.is_empty() {
    full_total
  } else {
    src.rebuild_spans(&have_bits)
  };
  let nblk = u32::try_from(total.div_ceil(BLK as u64)).map_err(|_| UdpError::Arg("文件过大"))?;
  let mut bar = show.then(|| Progress::new(Role::Send, total));
  let fin_le = FIN_MAGIC.to_le_bytes();

  if nblk == 0 {
    for _ in 0..3 {
      let _ = send_udp(sock, &fin_le, peer);
    }
    if let Some(b) = bar.as_mut() {
      b.finish(0, 0, 0);
    }
    return Ok(Stats {
      bytes: logical_total,
      us: start.elapsed().as_micros() as u64,
      packets: 0,
      retx: 0,
      blocks: 0,
      srtt_us: manifest_rtt,
      rtt_min_us: manifest_rtt,
    });
  }

  let mut sender = Sender::new(RING, 0)?;
  sender.seed_rtt(manifest_rtt);
  let start_dbg = now_us();
  let mut tx = Tx {
    sock,
    sender,
    peer,
    src,
    pkt_buf: [0u8; DGRAM],
    done_lo: 0,
    credit: 512,
    first_send_us: start_dbg,
    last_ack: start_dbg,
    got_ack: false,
    backoff: 0,
    lost: VecDeque::new(),
    lost_pns: Vec::new(),
    rack_lost: 0,
    spurious: 0,
    ack_trunc: 0,
    inflight: 0,
    packets: 0,
    retx: 0,
    last_tx_us: start_dbg,
    last_pkt: None,
    cc: Cc::new(),
  };

  let no_rack = var_os("S0_NO_RACK").is_some();
  let dbg = var_os("S0_DEBUG").is_some();
  let mut next_send = start_dbg;
  let mut next_blk = 0u32;
  let mut small_dup_done = false;
  let mut stall_pkts = 0u64;
  let mut last_dbg = start_dbg;
  let mut last_stall = start_dbg;
  let mut buf = [0u8; DGRAM];
  let mut last_done_lo = 0u32;
  let mut last_progress = start_dbg;

  loop {
    tx.drain_acks(&mut buf, no_rack)?;
    if let Some(b) = bar.as_mut() {
      b.tick(done_bytes(tx.done_lo, total), tx.retx, tx.packets);
    }
    if tx.done_lo >= nblk {
      for _ in 0..3 {
        let _ = send_udp(tx.sock, &fin_le, peer);
      }
      break;
    }

    let cwnd = tx.cwnd();

    if now_us() >= next_send {
      let batch_t = now_us();
      let mut retx_batch = 0u32;
      let mut retx_bytes = 0usize;
      while let Some((blk, idx)) = tx.lost.pop_front() {
        if blk < tx.done_lo || tx.is_acked(blk, idx) {
          continue;
        }
        let t = now_us();
        retx_bytes += tx.send(blk, idx, t, true)?;
        retx_batch += 1;
        if retx_batch >= 64 {
          break;
        }
      }
      if retx_batch > 0 {
        if rate > 0 {
          let send_at = now_us();
          next_send = next_send.max(send_at.saturating_sub(200_000))
            + ((retx_bytes as u128 * 8 * 1_000_000) / u128::from(rate)) as u64;
        } else if !is_loop {
          let gap = tx.pkt_gap_us();
          next_send = (next_send + u64::from(retx_batch) * gap).max(batch_t.saturating_sub(2_000));
        }
        let cur = now_us();
        if cur < next_send {
          wait_until(next_send.min(cur + 1_000));
        }
        continue;
      }
    }

    let limit = tx.credit.min(tx.done_lo + WINDOW_BLOCKS).min(nblk);
    let mut sent_blk = false;
    if next_blk < limit && tx.inflight < cwnd {
      let t = now_us();
      if t >= next_send {
        let b = tx.src.get(next_blk)?;
        let (cnt, len) = (b.cnt, b.data.len());
        let gap = tx.pkt_gap_us();
        let mut send_at = now_us();
        let burst_base = next_send.max(send_at.saturating_sub(2_000));
        if rate > 0 {
          next_send = next_send.max(send_at.saturating_sub(200_000))
            + ((len as u128 * 8 * 1_000_000) / u128::from(rate)) as u64;
        } else if !is_loop {
          // 令牌桶虚拟时钟推进：吸收读盘/压缩耗时与内核定时器抖动，避免每块重置损失稳态吞吐
          next_send = burst_base + u64::from(cnt) * gap;
        }
        let quantum = tx.cc.burst(PKT as u64, tx.sender.rtt(), is_loop) as u8;
        for idx in 0..cnt {
          if !is_loop && rate == 0 && idx > 0 && idx.is_multiple_of(quantum) {
            let sub_target = burst_base + u64::from(idx) * gap;
            if now_us() < sub_target {
              wait_until(sub_target);
            }
            tx.drain_acks(&mut buf, no_rack)?;
            send_at = now_us();
          }
          tx.send(next_blk, idx, send_at, false)?;
          if !is_loop && next_blk + 1 == nblk && idx + 2 >= cnt && cnt > 2 {
            tx.send(next_blk, idx, send_at, true)?;
          }
        }
        next_blk += 1;
        sent_blk = true;
        // 短流/高压缩归档（总包数 ≤ 160，如 many-files 或可压 tar）：首轮发完后交错补发第二遍，消除同微秒背靠背突发被同一路由微突发双杀的风险，确保零额外 RTT 完成
        if !is_loop && !small_dup_done && next_blk == nblk && nblk <= 128 && tx.packets <= 160 {
          small_dup_done = true;
          for blk in tx.done_lo..nblk {
            let cnt2 = tx.src.get(blk)?.cnt;
            if cnt2 <= 2 {
              for idx2 in 0..cnt2 {
                if !tx.is_acked(blk, idx2) {
                  let t2 = now_us();
                  tx.send(blk, idx2, t2, true)?;
                }
              }
            }
          }
        }
      }
    }

    let now = now_us();
    let tail = next_blk >= nblk;
    if now.saturating_sub(tx.last_tx_us) > tx.probe_interval_us(tail) {
      let mut sent = 0;
      if tail {
        for blk in tx.done_lo..nblk.min(tx.done_lo + WINDOW_BLOCKS) {
          let due = tx.due_mask(blk, now);
          if due != 0 {
            sent += tx.resend_missing_limited(blk, due, now, TAIL_BUDGET - sent)?;
            if sent >= TAIL_BUDGET {
              break;
            }
          }
        }
      }
      if sent == 0 {
        if let Some((blk, idx)) = tx.last_pkt
          && tx.should_retx(blk, idx, now)
        {
          tx.send(blk, idx, now, true)?;
          sent = 1;
        } else if tail && tx.done_lo < nblk && tx.should_retx(tx.done_lo, 0, now) {
          tx.send(tx.done_lo, 0, now, true)?;
          sent = 1;
        }
      }
      if sent > 0 {
        tx.last_tx_us = now;
      }
    }

    let stall_us = tx.stall_us();
    if tx.done_lo != last_done_lo {
      last_done_lo = tx.done_lo;
      last_progress = now;
    } else if tx.done_lo < next_blk
      && now.saturating_sub(last_progress) > stall_us
      && now.saturating_sub(last_stall) > stall_us
    {
      let lo = tx.done_lo;
      let mask = tx.missing_mask(lo);
      let n = if mask == 0 {
        tx.resend_all(lo, now)?
      } else {
        tx.resend_missing(lo, mask, now)?
      };
      stall_pkts += n;
      if !tx.got_ack {
        tx.backoff = (tx.backoff + 1).min(4);
      }
      last_stall = now;
    }

    if dbg && now.saturating_sub(last_dbg) > 1_000_000 {
      log::debug!(
        "DBG t={}ms done_lo={} next_blk={next_blk} credit={} cwnd={cwnd} packets={} retx={} rack={} spurious={} stall={stall_pkts} rtt_min={} reo={}",
        now.saturating_sub(start_dbg) / 1000,
        tx.done_lo,
        tx.credit,
        tx.packets,
        tx.retx,
        tx.rack_lost,
        tx.spurious,
        tx.sender.rtt().rtt_min_us(),
        tx.sender.rtt().reo_wnd_us()
      );
      last_dbg = now;
    }

    let idle_timeout = tx.idle_timeout_us();
    if now.saturating_sub(tx.last_ack) > idle_timeout
      && now.saturating_sub(last_progress) > idle_timeout
    {
      return Err(UdpError::StreamTimeout);
    }
    if sent_blk && now_us() >= next_send {
      continue;
    }
    if next_blk < limit && tx.inflight < cwnd {
      let cur = now_us();
      if cur < next_send {
        wait_until(next_send.min(cur + 1_000));
      }
    } else {
      wait_until(now_us() + if is_loop { 10 } else { 200 });
    }
  }

  if let Some(b) = bar.as_mut() {
    b.finish(total, tx.retx, tx.packets);
  }
  if dbg {
    log::debug!(
      "DBG done_lo={} packets={} retx={} rack_lost={} spurious={} ack_trunc={} stall_pkts={stall_pkts} rtt_min={} reo={} srtt={}",
      tx.done_lo,
      tx.packets,
      tx.retx,
      tx.rack_lost,
      tx.spurious,
      tx.ack_trunc,
      tx.sender.rtt().rtt_min_us(),
      tx.sender.rtt().reo_wnd_us(),
      tx.sender.rtt().srtt_us()
    );
  }
  Ok(Stats {
    bytes: logical_total,
    us: start.elapsed().as_micros() as u64,
    packets: tx.packets,
    retx: tx.retx,
    blocks: nblk,
    srtt_us: tx.sender.rtt().srtt_us(),
    rtt_min_us: tx.sender.rtt().rtt_min_us(),
  })
}

struct Tx<'a> {
  sock: &'a UdpSocket,
  sender: Sender,
  peer: SocketAddr,
  src: Src,
  pkt_buf: [u8; DGRAM],
  done_lo: u32,
  credit: u32,
  first_send_us: u64,
  last_ack: u64,
  got_ack: bool,
  backoff: u32,
  lost: VecDeque<(u32, u8)>,
  lost_pns: Vec<u64>,
  rack_lost: u64,
  spurious: u64,
  ack_trunc: u64,
  inflight: u64,
  packets: u64,
  retx: u64,
  last_tx_us: u64,
  last_pkt: Option<(u32, u8)>,
  cc: Cc,
}

impl Tx<'_> {
  const fn est_rtt_us(&self) -> u64 {
    let srtt = self.sender.rtt().srtt_us();
    if srtt > 0 {
      srtt
    } else {
      250_000u64 << self.backoff
    }
  }

  const fn init_gap_us(&self) -> u64 {
    let rtt = self.est_rtt_us();
    if rtt <= 2_000 {
      15
    } else if rtt <= 30_000 {
      35
    } else {
      INIT_GAP_US
    }
  }

  fn cwnd(&self) -> u64 {
    let init = (self.est_rtt_us() / self.init_gap_us()).clamp(INIT_CWND, MAX_INIT_CWND);
    self
      .cc
      .cwnd_bytes(self.sender.rtt())
      .map_or(init, |b| (b / PKT as u64).clamp(MIN_CWND, MAX_CWND))
  }

  fn pkt_gap_us(&self) -> u64 {
    self.cc.pacing_rate(self.sender.rtt()).map_or_else(
      || self.init_gap_us(),
      |r| (PKT as u64 * 1_000_000 / r.max(1)).clamp(MIN_GAP_US, MAX_GAP_US),
    )
  }

  fn probe_interval_us(&self, tail: bool) -> u64 {
    let rtt = self.est_rtt_us();
    let var = self.sender.rtt().rttvar_us();
    if tail {
      (rtt + (rtt >> 2) + var).clamp(20_000, 2_000_000)
    } else {
      (rtt.saturating_mul(2) + var.saturating_mul(2)).clamp(150_000, 2_000_000)
    }
  }

  fn stall_us(&self) -> u64 {
    let rtt = self.est_rtt_us();
    let var = self.sender.rtt().rttvar_us();
    (rtt.saturating_mul(5) / 2 + var.saturating_mul(2)).clamp(150_000, 3_000_000)
  }

  fn idle_timeout_us(&self) -> u64 {
    (self.est_rtt_us().saturating_mul(24)).clamp(10_000_000, 60_000_000)
  }

  fn drain_acks(&mut self, buf: &mut [u8], no_rack: bool) -> Result<(), UdpError> {
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
          if ack.nr as usize >= MAX_RANGES {
            self.ack_trunc += 1;
          }
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
          self.rack_lost += lost_pns.len() as u64;
          self.lost_pns = lost_pns;
          self.spurious += u64::from(st.spurious);
          self.inflight = self
            .inflight
            .saturating_sub(u64::from(st.acked) + u64::from(st.lost));
        }
        Err(e) if e.kind() == ErrorKind::WouldBlock => return Ok(()),
        Err(e) => return Err(e.into()),
      }
    }
  }

  fn is_acked(&self, blk: u32, idx: u8) -> bool {
    self.src.slot(blk).is_some_and(|s| (s.have >> idx) & 1 != 0)
  }

  fn should_retx(&self, blk: u32, idx: u8, now: u64) -> bool {
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

  fn due_mask(&self, blk: u32, now: u64) -> u128 {
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

  fn missing_mask(&self, blk: u32) -> u128 {
    self.src.slot(blk).map_or(0, |s| cnt_mask(s.cnt) & !s.have)
  }

  fn send(&mut self, blk: u32, idx: u8, now: u64, retx: bool) -> Result<usize, UdpError> {
    let b = self.src.get(blk)?;
    let start = usize::from(idx) * PKT;
    let end = (start + PKT).min(b.data.len());
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
    send_udp(self.sock, &self.pkt_buf[..n], self.peer)?;
    self.packets += 1;
    self.inflight += 1;
    self.retx += u64::from(retx);
    self.last_tx_us = now;
    self.last_pkt = Some((blk, idx));
    Ok(payload_len)
  }

  fn resend_missing(&mut self, blk: u32, mask: u128, now: u64) -> Result<u64, UdpError> {
    self.resend_missing_limited(blk, mask, now, u64::MAX)
  }

  fn resend_missing_limited(
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

  fn resend_all(&mut self, blk: u32, now: u64) -> Result<u64, UdpError> {
    let cnt = self.src.get(blk)?.cnt;
    for idx in 0..cnt {
      self.send(blk, idx, now, true)?;
    }
    Ok(u64::from(cnt))
  }
}
