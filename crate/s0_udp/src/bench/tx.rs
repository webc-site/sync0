use std::{
  collections::VecDeque,
  env::var_os,
  fs::File,
  net::{SocketAddr, UdpSocket},
  path::Path,
  time::Instant,
};

use s0_cc::Cc;
use s0_cdc::{CdcConfig, chunk_file};

use super::{
  DEFAULT_PKT, FIN_MAGIC, MAX_BLOCK, MAX_DATAGRAM, Role, RootBuf, TUNNEL_PKT, WINDOW_BLOCKS,
  manifest::{Entry, ManifestMode, encode_manifest, entries_from_changes, scan_dir, send_manifest},
  now_us, send_udp,
  source::{Src, SrcKind, build_tx_spans},
  stats::{Stats, done_bytes},
  tx_state::{RING, Tx},
  wait_until,
};
use crate::{
  BIND_V4, BIND_V6, Progress, Sender, UdpError, bind_std_udp, codec::skip_name, fs::ScannedFile,
  is_tunnel_path,
};

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
  send_changes(root, files, Vec::new(), peer, rate, show)
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
  let bind_addr = if peer.is_ipv4() { BIND_V4 } else { BIND_V6 };
  let sock = bind_std_udp(bind_addr, false)?;
  send_inner_sock(
    &sock,
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
    u32::try_from(full_total.div_ceil(MAX_BLOCK as u64)).map_err(|_| UdpError::Arg("文件过大"))?;
  let is_loop = peer.ip().is_loopback();
  let init_pkt = if is_tunnel_path(sock, peer) && var_os("S0_FORCE_1200").is_some() {
    TUNNEL_PKT
  } else {
    DEFAULT_PKT
  };
  let mut src = Src::new(kind, entries, full_spans, full_total, skip_all, init_pkt)?;
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
  let nblk =
    u32::try_from(total.div_ceil(MAX_BLOCK as u64)).map_err(|_| UdpError::Arg("文件过大"))?;
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
    pkt_size: init_pkt,
    pkt_buf: [0u8; MAX_DATAGRAM],
    done_lo: 0,
    credit: WINDOW_BLOCKS,
    first_send_us: start_dbg,
    last_ack: start_dbg,
    got_ack: false,
    backoff: 0,
    lost: VecDeque::new(),
    lost_pns: Vec::new(),
    inflight: 0,
    packets: 0,
    retx: 0,
    last_tx_us: start_dbg,
    last_pkt: None,
    cc: Cc::new(),
  };

  let no_rack = var_os("S0_NO_RACK").is_some();
  let (mut next_send, mut next_blk, mut small_dup_done) = (start_dbg, 0u32, false);
  let (mut last_stall, mut last_done_lo, mut last_progress) = (start_dbg, 0u32, start_dbg);
  let mut buf = [0u8; MAX_DATAGRAM];

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
          next_send = burst_base + u64::from(cnt) * gap;
        }
        let pkt_before = tx.pkt_size;
        let quantum = tx.cc.burst(pkt_before as u64, tx.sender.rtt(), is_loop) as u8;
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
          if tx.pkt_size != pkt_before {
            break;
          }
          if !is_loop && next_blk + 1 == nblk && idx + 2 >= cnt && cnt > 2 {
            tx.send(next_blk, idx, send_at, true)?;
          }
        }
        if tx.pkt_size != pkt_before {
          next_blk = tx.done_lo;
          continue;
        }
        next_blk += 1;
        sent_blk = true;
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
      if !tx.got_ack {
        tx.backoff = (tx.backoff + 1).min(4);
        if tx.backoff >= 2 && tx.pkt_size > TUNNEL_PKT {
          tx.downgrade_mtu();
          next_blk = tx.done_lo;
        }
      }
      let lo = tx.done_lo;
      let mask = tx.missing_mask(lo);
      if mask == 0 {
        tx.resend_all(lo, now)?;
      } else {
        tx.resend_missing(lo, mask, now)?;
      }
      last_stall = now;
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
