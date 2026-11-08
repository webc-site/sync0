use std::{
  io::{self, Read, Write},
  net::{SocketAddr, TcpStream},
  sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
  },
  thread,
  time::Duration,
};

use gxhash::HashMap;
use parking_lot::Mutex;
use s0_cdc::for_each_missing_span;

use super::{
  PeerError, PeerId, plan_stripes, rank_swarm_peers, register_iroh_endpoint, resolve_iroh_endpoint,
};
use crate::{
  RangeSpec, UdpError, bind_udp_for_ip,
  consts::{FLAG_RANGE, HAVE_ALL_ONES, Op, Status},
  fs::SharedSink,
  punch_udp, recv_range_udp,
  region::Region,
  wire::{P2pReq, P2pResp, SwarmPeer, decode_socket_addrs},
};

/// 单帧控制面消息最大字节数上限（64 MiB，防止恶意帧长度触发 OOM）
pub(crate) const MAX_CTRL_FRAME_LEN: usize = 64 * 1024 * 1024;
/// 动态折半抢占 Work-Stealing 的最小剩余块数阈值（低于该阈值免抢占以消除握手开销）
const MIN_STEAL_BLOCKS: u32 = 64;

#[derive(Clone, Copy)]
pub(crate) struct RangePullJob<'a> {
  pub(crate) peer: &'a SwarmPeer,
  pub(crate) share_id: [u8; 16],
  pub(crate) paths: &'a [String],
  pub(crate) root_hash: [u8; 16],
  pub(crate) blk_lo: u32,
  pub(crate) blk_hi: u32,
  pub(crate) req_rate: u64,
  pub(crate) token: Option<[u8; 16]>,
  pub(crate) my_card: &'a [u8],
  pub(crate) sink: &'a SharedSink,
}

/// 从 `root_hash` 前 3 字节与区间起始块号/客户端端口盐值派生 24-bit UDP 会话标识
#[inline]
pub(crate) const fn derive_session_id(root_hash: &[u8; 16], salt: u32) -> u32 {
  (((root_hash[0] as u32) << 16) | ((root_hash[1] as u32) << 8) | (root_hash[2] as u32)) ^ salt
}

/// 从 `root_hash` 前 3 字节派生 Push 会话标识
#[inline]
pub(crate) const fn derive_push_session_id(root_hash: &[u8; 16]) -> u32 {
  u32::from_le_bytes([root_hash[0], root_hash[1], root_hash[2], 0])
}

pub(crate) fn resolve_swarm_peer_addr(peer: &SwarmPeer) -> Result<SocketAddr, PeerError> {
  if let Some(addr) = resolve_iroh_endpoint(&peer.endpoint_id) {
    return Ok(addr);
  }
  let addrs = decode_socket_addrs(&peer.peer_addrs)?;
  if let Some(&first) = addrs.first() {
    if peer.endpoint_id != [0u8; 32] {
      register_iroh_endpoint(peer.endpoint_id, first);
    }
    return Ok(first);
  }
  Err(PeerError::NoHealthyPeer)
}

pub(crate) fn connect_ctrl_tcp(addr: &SocketAddr) -> Result<TcpStream, PeerError> {
  let step = if addr.ip().is_loopback() {
    Duration::from_millis(500)
  } else {
    Duration::from_millis(850)
  };
  let mut last_err = None;
  for _ in 0..3 {
    match TcpStream::connect_timeout(addr, step) {
      Ok(s) => {
        let _ = s.set_nodelay(true);
        let _ = s.set_read_timeout(Some(Duration::from_secs(10)));
        let _ = s.set_write_timeout(Some(Duration::from_secs(10)));
        return Ok(s);
      }
      Err(e) => last_err = Some(e),
    }
  }
  Err(PeerError::Io(last_err.unwrap()))
}

pub(crate) fn pull_range_from_swarm_peer(
  job: RangePullJob<'_>,
  reused_stream: Option<TcpStream>,
) -> Result<(u32, u64, TcpStream), PeerError> {
  let ctrl_addr = resolve_swarm_peer_addr(job.peer)?;
  let mut stream = match reused_stream {
    Some(s) => s,
    None => connect_ctrl_tcp(&ctrl_addr)?,
  };

  let (udp_sock, my_udp_addrs) = bind_udp_for_ip(stream.local_addr()?.ip())?;
  let cli_port = udp_sock.local_addr()?.port();
  let sid = derive_session_id(&job.root_hash, job.blk_lo ^ (u32::from(cli_port) << 8));

  let req = P2pReq {
    op: Op::Pull,
    flags: FLAG_RANGE,
    share_id: job.share_id,
    req_rate: job.req_rate,
    paths: job.paths.to_vec(),
    peer_addrs: my_udp_addrs,
    profile_card: job.my_card.to_vec(),
    range: Some((job.root_hash, job.blk_lo, job.blk_hi)),
    token: job.token,
  };

  let req_bytes = req.encode()?;
  let have = job.sink.snapshot_have_bits();
  write_two_frames(&mut stream, &req_bytes, &have)?;

  let resp_raw = read_frame(&mut stream)?;
  let resp = P2pResp::decode(&resp_raw)?;
  if resp.status != Status::Ok {
    return Err(PeerError::RemoteStatus(resp.status));
  }

  let peer_udp = punch_udp(&udp_sock, &resp.peer_addrs, sid)?;
  let st = recv_range_udp(
    &udp_sock,
    &peer_udp,
    job.sink,
    RangeSpec {
      session_id: sid,
      blk_lo: job.blk_lo,
      blk_hi: job.blk_hi,
      eff_rate: resp.eff_rate,
      timeout: Duration::from_secs(12),
    },
  )?;
  Ok((st.blocks_transferred, st.bytes_transferred, stream))
}

/// 跨节点并发条带化拉取与动态折半抢占 Work-Stealing：
/// - 复用已建立 TCP 控制流，消除二次握手；
/// - 移除多节点单条带硬编码速率上限，充分发挥多路径并发聚合带宽；
/// - 快节点完成自身条带后，立即检查慢节点尚未完成的条带尾部并执行动态折半抢占（Work-Stealing），消除木桶效应长尾延迟。
pub(crate) fn execute_striped_pull(
  local_region: Region,
  candidate_peers: Vec<SwarmPeer>,
  base_job: RangePullJob<'_>,
  reused_conns: &Mutex<HashMap<PeerId, TcpStream>>,
) -> Result<(u32, u64, usize), PeerError> {
  let total_blocks = base_job.sink.total_blocks();
  if total_blocks == 0 || base_job.sink.is_range_complete(0, total_blocks) {
    return Ok((0, 0, 0));
  }

  let ranked = rank_swarm_peers(local_region, candidate_peers);
  if ranked.is_empty() {
    return Err(PeerError::NoHealthyPeer);
  }

  let stripes = plan_stripes(local_region, &ranked, total_blocks);
  let peers_used = stripes.len();
  let per_stripe_rate = base_job.req_rate;
  let stripe_bounds: Arc<Vec<(AtomicU32, AtomicU32)>> = Arc::new(
    stripes
      .iter()
      .map(|&(_, lo, hi)| (AtomicU32::new(lo), AtomicU32::new(hi)))
      .collect(),
  );

  let mut blocks_transferred = 0u32;
  let mut bytes_transferred = 0u64;
  let mut poisoned_indices = Vec::new();
  let mut conflict = false;

  thread::scope(|s| {
    let mut handles = Vec::with_capacity(stripes.len());
    for &(p_idx, lo, hi) in &stripes {
      let peer = &ranked[p_idx];
      let conn = reused_conns.lock().remove(&peer.endpoint_id);
      let bounds = Arc::clone(&stripe_bounds);
      handles.push((
        p_idx,
        s.spawn(move || -> Result<(u32, u64), PeerError> {
          let (mut blks, mut bytes, stream) = pull_range_from_swarm_peer(
            RangePullJob {
              peer,
              blk_lo: lo,
              blk_hi: hi,
              req_rate: per_stripe_rate,
              ..base_job
            },
            conn,
          )?;

          // 动态折半抢占 Work-Stealing：快节点完成后主动接管慢节点尚未完成的条带后半区间
          let mut stream_opt = Some(stream);
          while !base_job.sink.is_range_complete(0, total_blocks) {
            let Some((steal_lo, steal_hi)) = try_steal_tail(base_job.sink, &bounds, peer) else {
              break;
            };
            match pull_range_from_swarm_peer(
              RangePullJob {
                peer,
                blk_lo: steal_lo,
                blk_hi: steal_hi,
                req_rate: per_stripe_rate,
                ..base_job
              },
              stream_opt.take(),
            ) {
              Ok((b, by, s_next)) => {
                blks += b;
                bytes += by;
                stream_opt = Some(s_next);
              }
              Err(_) => break,
            }
          }
          if let Some(s_final) = stream_opt {
            reused_conns.lock().insert(peer.endpoint_id, s_final);
          }
          Ok((blks, bytes))
        }),
      ));
    }
    for (p_idx, h) in handles {
      match h.join() {
        Ok(Ok((blks, bytes))) => {
          blocks_transferred += blks;
          bytes_transferred += bytes;
        }
        Ok(Err(PeerError::Udp(UdpError::PoisonedPeer(_)))) => {
          poisoned_indices.push(p_idx);
        }
        Ok(Err(PeerError::RemoteStatus(Status::Conflict))) => {
          conflict = true;
        }
        _ => {}
      }
    }
  });

  if conflict {
    return Err(PeerError::SnapshotConflict);
  }

  if !base_job.sink.is_range_complete(0, total_blocks) {
    for (p_idx, peer) in ranked.iter().enumerate() {
      if poisoned_indices.contains(&p_idx) {
        continue;
      }
      if base_job.sink.is_range_complete(0, total_blocks) {
        break;
      }
      let have = base_job.sink.snapshot_have_bits();
      let mut min_lo = total_blocks;
      let mut max_hi = 0u32;
      for_each_missing_span(&have, 0, total_blocks, |lo, hi| {
        if lo < min_lo {
          min_lo = lo;
        }
        if hi > max_hi {
          max_hi = hi;
        }
      });
      if min_lo < max_hi {
        let conn = reused_conns.lock().remove(&peer.endpoint_id);
        match pull_range_from_swarm_peer(
          RangePullJob {
            peer,
            blk_lo: min_lo,
            blk_hi: max_hi,
            ..base_job
          },
          conn,
        ) {
          Ok((blks, bytes, stream)) => {
            blocks_transferred += blks;
            bytes_transferred += bytes;
            reused_conns.lock().insert(peer.endpoint_id, stream);
          }
          Err(PeerError::Udp(UdpError::PoisonedPeer(_))) => {
            poisoned_indices.push(p_idx);
          }
          Err(_) => {}
        }
      }
    }
  }

  if !base_job.sink.is_range_complete(0, total_blocks) {
    if !poisoned_indices.is_empty() {
      return Err(PeerError::Udp(UdpError::PoisonedPeer(0)));
    }
    return Err(PeerError::Udp(UdpError::Timeout(0, total_blocks)));
  }

  Ok((blocks_transferred, bytes_transferred, peers_used))
}

fn try_steal_tail(
  sink: &SharedSink,
  bounds: &[(AtomicU32, AtomicU32)],
  thief: &SwarmPeer,
) -> Option<(u32, u32)> {
  let have = sink.snapshot_have_bits();
  let mut best: Option<(usize, u32, u32, u32)> = None;
  for (idx, (lo_atom, hi_atom)) in bounds.iter().enumerate() {
    let lo = lo_atom.load(Ordering::Acquire);
    let hi = hi_atom.load(Ordering::Acquire);
    if lo >= hi || sink.is_range_complete(lo, hi) {
      continue;
    }
    let mut first_missing = hi;
    for_each_missing_span(&have, lo, hi, |m_lo, _m_hi| {
      if m_lo < first_missing {
        first_missing = m_lo;
      }
    });
    let rem = hi.saturating_sub(first_missing);
    if rem >= MIN_STEAL_BLOCKS {
      let mid = first_missing + (rem >> 1);
      if thief.blk_lo <= mid
        && thief.blk_hi >= hi
        && best.is_none_or(|(_, _, _, best_rem)| rem > best_rem)
      {
        best = Some((idx, mid, hi, rem));
      }
    }
  }
  let (target_idx, mid, hi, _) = best?;
  if bounds[target_idx]
    .1
    .compare_exchange(hi, mid, Ordering::AcqRel, Ordering::Acquire)
    .is_ok()
  {
    Some((mid, hi))
  } else {
    None
  }
}

#[inline]
pub(crate) fn is_have_all_ones_frame(buf: &[u8]) -> bool {
  buf
    .first_chunk::<4>()
    .is_some_and(|&b| buf.len() == 4 && u32::from_le_bytes(b) == HAVE_ALL_ONES)
}

pub(crate) fn write_frame(stream: &mut TcpStream, payload: &[u8]) -> io::Result<()> {
  let len = payload.len() as u32;
  stream.write_all(&len.to_le_bytes())?;
  stream.write_all(payload)?;
  stream.flush()
}

pub(crate) fn write_two_frames(stream: &mut TcpStream, f1: &[u8], f2: &[u8]) -> io::Result<()> {
  let mut buf = Vec::with_capacity(8 + f1.len() + f2.len());
  buf.extend_from_slice(&(f1.len() as u32).to_le_bytes());
  buf.extend_from_slice(f1);
  buf.extend_from_slice(&(f2.len() as u32).to_le_bytes());
  buf.extend_from_slice(f2);
  stream.write_all(&buf)?;
  stream.flush()
}

pub(crate) fn read_frame(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
  let mut len_buf = [0u8; 4];
  stream.read_exact(&mut len_buf)?;
  let len = u32::from_le_bytes(len_buf) as usize;
  if len > MAX_CTRL_FRAME_LEN {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "control frame exceeds maximum length",
    ));
  }
  let mut out = vec![0u8; len];
  stream.read_exact(&mut out)?;
  Ok(out)
}
