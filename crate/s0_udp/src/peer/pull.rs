use std::{
  io::{self, Read, Write},
  net::{SocketAddr, TcpStream},
  sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
    mpsc::{self, RecvTimeoutError},
  },
  thread,
  time::Duration,
};

use gxhash::HashMap;
use parking_lot::Mutex;
use s0_cdc::for_each_missing_span;

use super::{
  PeerError, PeerId, plan_stripes_range, rank_swarm_peers, register_iroh_endpoints,
  resolve_iroh_endpoints,
};
use crate::{
  RangeSpec, UdpError, bind_udp_for_ip,
  consts::{
    FLAG_BYTE_RANGE, FLAG_RANGE, HAVE_ALL_ONES, MAX_CTRL_FRAME_LEN, Op, Status, V6_PREFER_MARGIN_US,
  },
  fs::SharedSink,
  punch_udp, recv_range_udp,
  region::Region,
  wire::{P2pReq, P2pResp, SwarmPeer, decode_socket_addrs},
};

/// 动态折半抢占 Work-Stealing 的最小剩余块数或字节阈值（低于阈值免抢占以消除握手开销）
const MIN_STEAL_BLOCKS: u32 = 64;
const MIN_STEAL_BYTES: u64 = 256 * 1024;

#[derive(Clone, Copy)]
pub(crate) struct RangePullJob<'a> {
  pub(crate) peer: &'a SwarmPeer,
  pub(crate) share_id: [u8; 16],
  pub(crate) paths: &'a [String],
  pub(crate) root_hash: [u8; 16],
  pub(crate) blk_lo: u32,
  pub(crate) blk_hi: u32,
  pub(crate) byte_range: Option<(u64, u64)>,
  pub(crate) req_rate: u64,
  pub(crate) token: Option<[u8; 16]>,
  pub(crate) my_card: &'a [u8],
  pub(crate) sink: &'a SharedSink,
}

impl<'a> RangePullJob<'a> {
  #[inline]
  fn with_peer_range(self, peer: &'a SwarmPeer, blk_lo: u32, blk_hi: u32) -> Self {
    Self {
      peer,
      blk_lo,
      blk_hi,
      ..self
    }
  }
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

pub(crate) fn resolve_swarm_peer_addrs(peer: &SwarmPeer) -> Result<Vec<SocketAddr>, PeerError> {
  let mut out = resolve_iroh_endpoints(&peer.endpoint_id);
  if let Ok(decoded) = decode_socket_addrs(&peer.peer_addrs)
    && !decoded.is_empty()
  {
    for a in decoded {
      if !out.contains(&a) {
        out.push(a);
      }
    }
    if peer.endpoint_id != [0u8; 32] {
      register_iroh_endpoints(peer.endpoint_id, &out);
    }
  }
  if out.is_empty() {
    return Err(PeerError::NoHealthyPeer);
  }
  Ok(out)
}

#[inline]
pub(crate) fn connect_once(addr: &SocketAddr) -> io::Result<TcpStream> {
  let step = if addr.ip().is_loopback() {
    Duration::from_millis(350)
  } else {
    Duration::from_millis(600)
  };
  let s = TcpStream::connect_timeout(addr, step)?;
  let _ = s.set_nodelay(true);
  let _ = s.set_read_timeout(Some(Duration::from_secs(10)));
  let _ = s.set_write_timeout(Some(Duration::from_secs(10)));
  Ok(s)
}

pub(crate) fn probe_ctrl_addrs(addrs: &[SocketAddr]) -> Result<TcpStream, PeerError> {
  if addrs.len() == 1 {
    return connect_once(&addrs[0]).map_err(PeerError::Io);
  }
  connect_ctrl_addrs(addrs)
}

pub fn connect_ctrl_tcp(addr: &SocketAddr) -> Result<TcpStream, PeerError> {
  let mut last_err = None;
  for _ in 0..3 {
    match connect_once(addr) {
      Ok(s) => return Ok(s),
      Err(e) => last_err = Some(e),
    }
  }
  Err(PeerError::Io(last_err.unwrap()))
}

/// RFC 8305 Happy Eyeballs 双栈竞速连接：
/// 当候选列表同时包含 IPv6 与 IPv4 地址时，优先发起 IPv6 连接，并在 `5ms` (`V6_PREFER_MARGIN_US`) 后并发发起 IPv4 连接，先完成握手者立即胜出
pub fn connect_ctrl_addrs(addrs: &[SocketAddr]) -> Result<TcpStream, PeerError> {
  if addrs.is_empty() {
    return Err(PeerError::NoHealthyPeer);
  }
  if addrs.len() == 1 {
    return connect_ctrl_tcp(&addrs[0]);
  }
  let v6_opt = addrs.iter().copied().find(SocketAddr::is_ipv6);
  let v4_opt = addrs.iter().copied().find(SocketAddr::is_ipv4);
  if let (Some(v6), Some(v4)) = (v6_opt, v4_opt) {
    let (tx, rx) = mpsc::channel();
    let tx6 = tx.clone();
    thread::spawn(move || {
      let _ = tx6.send(connect_once(&v6));
    });
    let mut pending = 1usize;
    let mut last_err = None;
    match rx.recv_timeout(Duration::from_micros(V6_PREFER_MARGIN_US)) {
      Ok(Ok(s)) => return Ok(s),
      Ok(Err(e)) => {
        pending = 0;
        last_err = Some(e);
      }
      Err(RecvTimeoutError::Timeout) => {}
      Err(RecvTimeoutError::Disconnected) => pending = 0,
    }
    thread::spawn(move || {
      let _ = tx.send(connect_once(&v4));
    });
    pending += 1;
    while pending > 0 {
      match rx.recv_timeout(Duration::from_millis(900)) {
        Ok(Ok(s)) => return Ok(s),
        Ok(Err(e)) => {
          pending -= 1;
          last_err = Some(e);
        }
        Err(_) => break,
      }
    }
    for addr in addrs.iter().filter(|&&a| a != v6 && a != v4) {
      if let Ok(s) = connect_once(addr) {
        return Ok(s);
      }
    }
    return Err(last_err.map_or(PeerError::NoHealthyPeer, PeerError::Io));
  }

  let mut last_err = None;
  for addr in addrs {
    match connect_ctrl_tcp(addr) {
      Ok(s) => return Ok(s),
      Err(e) => last_err = Some(e),
    }
  }
  Err(last_err.unwrap_or(PeerError::NoHealthyPeer))
}

pub(crate) fn pull_range_from_swarm_peer(
  job: RangePullJob<'_>,
  reused_stream: Option<TcpStream>,
) -> Result<(u32, u64, TcpStream), PeerError> {
  let mut stream = match reused_stream {
    Some(s) => s,
    None => connect_ctrl_addrs(&resolve_swarm_peer_addrs(job.peer)?)?,
  };

  let (udp_sock, my_udp_addrs) = bind_udp_for_ip(stream.local_addr()?.ip())?;
  let cli_port = udp_sock.local_addr()?.port();
  let sid = derive_session_id(&job.root_hash, job.blk_lo ^ (u32::from(cli_port) << 8));
  let flags = FLAG_RANGE
    | if job.byte_range.is_some() {
      FLAG_BYTE_RANGE
    } else {
      0
    };
  let req = P2pReq {
    op: Op::Pull,
    flags,
    share_id: job.share_id,
    req_rate: job.req_rate,
    paths: job.paths.to_vec(),
    peer_addrs: my_udp_addrs,
    profile_card: job.my_card.to_vec(),
    range: Some((job.root_hash, job.blk_lo, job.blk_hi)),
    token: job.token,
    byte_range: job.byte_range,
  };

  write_two_frames(&mut stream, &req.encode()?, &job.sink.snapshot_have_bits())?;
  let resp = P2pResp::decode(&read_frame(&mut stream)?)?;
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

/// 跨节点并发条带化拉取、点播首屏优先调度与动态折半抢占 Work-Stealing
pub(crate) fn execute_striped_pull(
  local_region: Region,
  candidate_peers: Vec<SwarmPeer>,
  base_job: RangePullJob<'_>,
  reused_conns: &Mutex<HashMap<PeerId, TcpStream>>,
) -> Result<(u32, u64, usize), PeerError> {
  let total_blocks = base_job.sink.total_blocks();
  let req_hi = base_job.blk_hi.min(total_blocks);
  let req_lo = base_job.blk_lo.min(req_hi);
  if req_lo >= req_hi || base_job.sink.is_range_complete(req_lo, req_hi) {
    return Ok((0, 0, 0));
  }

  let ranked = rank_swarm_peers(local_region, candidate_peers);
  if ranked.is_empty() {
    return Err(PeerError::NoHealthyPeer);
  }

  let stripes = plan_stripes_range(
    local_region,
    &ranked,
    &base_job.sink.block_weights(),
    req_lo,
    req_hi,
    base_job.byte_range.is_some() || req_lo > 0,
  );
  let mut unique_peers = Vec::with_capacity(stripes.len());
  for &(p_idx, ..) in &stripes {
    if !unique_peers.contains(&p_idx) {
      unique_peers.push(p_idx);
    }
  }
  let peers_used = unique_peers.len();
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
          let (mut blks, mut bytes, stream) =
            pull_range_from_swarm_peer(base_job.with_peer_range(peer, lo, hi), conn)?;
          let mut stream_opt = Some(stream);
          while !base_job.sink.is_range_complete(req_lo, req_hi) {
            let Some((steal_lo, steal_hi)) = try_steal_tail(base_job.sink, &bounds, peer) else {
              break;
            };
            let Ok((b, by, s_next)) = pull_range_from_swarm_peer(
              base_job.with_peer_range(peer, steal_lo, steal_hi),
              stream_opt.take(),
            ) else {
              break;
            };
            blks += b;
            bytes += by;
            stream_opt = Some(s_next);
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
        Ok(Err(PeerError::Udp(UdpError::PoisonedPeer(_)))) => poisoned_indices.push(p_idx),
        Ok(Err(PeerError::RemoteStatus(Status::Conflict))) => conflict = true,
        _ => {}
      }
    }
  });

  if conflict {
    return Err(PeerError::SnapshotConflict);
  }

  if !base_job.sink.is_range_complete(req_lo, req_hi) {
    for (p_idx, peer) in ranked.iter().enumerate() {
      if poisoned_indices.contains(&p_idx) || base_job.sink.is_range_complete(req_lo, req_hi) {
        continue;
      }
      let have = base_job.sink.snapshot_have_bits();
      let (mut min_lo, mut max_hi) = (req_hi, req_lo);
      for_each_missing_span(&have, req_lo, req_hi, |lo, hi| {
        min_lo = min_lo.min(lo);
        max_hi = max_hi.max(hi);
      });
      if min_lo < max_hi {
        let conn = reused_conns.lock().remove(&peer.endpoint_id);
        match pull_range_from_swarm_peer(base_job.with_peer_range(peer, min_lo, max_hi), conn) {
          Ok((blks, bytes, stream)) => {
            blocks_transferred += blks;
            bytes_transferred += bytes;
            reused_conns.lock().insert(peer.endpoint_id, stream);
          }
          Err(PeerError::Udp(UdpError::PoisonedPeer(_))) => poisoned_indices.push(p_idx),
          Err(_) => {}
        }
      }
    }
  }

  if !base_job.sink.is_range_complete(req_lo, req_hi) {
    if !poisoned_indices.is_empty() {
      return Err(PeerError::Udp(UdpError::PoisonedPeer(req_lo)));
    }
    return Err(PeerError::Udp(UdpError::Timeout(req_lo, req_hi)));
  }

  Ok((blocks_transferred, bytes_transferred, peers_used))
}

fn try_steal_tail(
  sink: &SharedSink,
  bounds: &[(AtomicU32, AtomicU32)],
  thief: &SwarmPeer,
) -> Option<(u32, u32)> {
  let have = sink.snapshot_have_bits();
  let mut best: Option<(usize, u32, u32, u64)> = None;
  for (idx, (lo_atom, hi_atom)) in bounds.iter().enumerate() {
    let lo = lo_atom.load(Ordering::Acquire);
    let hi = hi_atom.load(Ordering::Acquire);
    if lo >= hi || sink.is_range_complete(lo, hi) {
      continue;
    }
    let mut first_missing = hi;
    let mut rem_bytes = 0u64;
    for_each_missing_span(&have, lo, hi, |m_lo, m_hi| {
      if m_lo < first_missing {
        first_missing = m_lo;
      }
      for b in m_lo..m_hi {
        rem_bytes += u64::from(sink.block_size(b)).max(1024);
      }
    });
    let rem = hi.saturating_sub(first_missing);
    if rem >= 4 && (rem >= MIN_STEAL_BLOCKS || rem_bytes >= MIN_STEAL_BYTES) {
      let mid = first_missing + (rem >> 1);
      if thief.blk_lo <= mid
        && thief.blk_hi >= hi
        && best.is_none_or(|(_, _, _, best_bytes)| rem_bytes > best_bytes)
      {
        best = Some((idx, mid, hi, rem_bytes));
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
  stream.write_all(&(payload.len() as u32).to_le_bytes())?;
  stream.write_all(payload)?;
  stream.flush()
}

pub(crate) fn write_resp(stream: &mut TcpStream, resp: &P2pResp) -> Result<(), PeerError> {
  write_frame(stream, &resp.encode()?)?;
  Ok(())
}

pub(crate) fn send_req_recv_resp(
  stream: &mut TcpStream,
  req: &P2pReq,
) -> Result<P2pResp, PeerError> {
  write_frame(stream, &req.encode()?)?;
  Ok(P2pResp::decode(&read_frame(stream)?)?)
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
