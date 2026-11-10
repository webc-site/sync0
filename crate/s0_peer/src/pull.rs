use std::{
  io::{self, Write},
  net::{SocketAddr, TcpStream},
  sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
    mpsc::{RecvTimeoutError, channel},
  },
  thread,
  time::Duration,
};

use gxhash::HashMap;
use parking_lot::Mutex;
use s0_cdc::encode_roaring_bits;
pub(crate) use s0_const::derive_session_id;
use s0_const::{
  HASH_LEN, LEECH_MIN_BLOCKS, Op, PROTO_VERSION, SHARE_ID_LEN, Status, TOKEN_LEN,
  V6_PREFER_MARGIN_US, ZERO_PEER_ID, macro_piece_blocks, region::Region,
};
use s0_fs::SharedSink;
use s0_udp::{RangeSpec, UdpError, bind_udp_for_ip, punch_udp_with_peer_ip, recv_range_udp};
use s0_wire::{P2pReq, P2pResp, SwarmPeer, decode_socket_addrs};

use crate::{
  PeerError, PeerId,
  frame::{read_frame, write_two_frames},
  plan_stripes_range, rank_swarm_peers, register_iroh_endpoints, resolve_iroh_endpoints,
};

/// 动态折半抢占 Work-Stealing 的最小剩余块数或字节阈值（低于阈值免抢占以消除握手开销）
const MIN_STEAL_BLOCKS: u32 = 64;
const MIN_STEAL_BYTES: u64 = 256 * 1024;

#[derive(Clone, Copy)]
pub(crate) struct RangePullJob<'a> {
  pub(crate) peer: &'a SwarmPeer,
  pub(crate) share_id: [u8; SHARE_ID_LEN],
  pub(crate) paths: &'a [String],
  pub(crate) root_hash: [u8; HASH_LEN],
  pub(crate) blk_lo: u32,
  pub(crate) blk_hi: u32,
  pub(crate) byte_range: Option<(u64, u64)>,
  pub(crate) req_rate: u64,
  pub(crate) token: Option<[u8; TOKEN_LEN]>,
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

fn resolve_swarm_peer_addrs(peer: &SwarmPeer) -> Result<Vec<SocketAddr>, PeerError> {
  let mut out = resolve_iroh_endpoints(&peer.endpoint_id);
  if let Ok(decoded) = decode_socket_addrs(&peer.peer_addrs)
    && !decoded.is_empty()
  {
    for a in decoded {
      if !out.contains(&a) {
        out.push(a);
      }
    }
    if peer.endpoint_id != ZERO_PEER_ID {
      register_iroh_endpoints(peer.endpoint_id, &out);
    }
  }
  if out.is_empty() {
    return Err(PeerError::NoHealthyPeer);
  }
  Ok(out)
}

#[inline]
fn connect_once(addr: &SocketAddr, mult: u32) -> io::Result<TcpStream> {
  let step = if addr.ip().is_loopback() {
    Duration::from_millis(350)
  } else {
    Duration::from_millis(600 * u64::from(mult))
  };
  let mut s = TcpStream::connect_timeout(addr, step)?;
  let _ = s.set_nodelay(true);
  let _ = s.set_read_timeout(Some(Duration::from_secs(60)));
  let _ = s.set_write_timeout(Some(Duration::from_secs(60)));
  s.write_all(&[PROTO_VERSION])?;
  Ok(s)
}

pub(crate) fn probe_ctrl_addrs(addrs: &[SocketAddr]) -> Result<TcpStream, PeerError> {
  if addrs.len() == 1 {
    return connect_once(&addrs[0], 1).map_err(PeerError::Io);
  }
  connect_ctrl_addrs(addrs)
}

fn connect_ctrl_tcp(addr: &SocketAddr) -> Result<TcpStream, PeerError> {
  let mut last_err = None;
  for mult in [1u32, 3, 10] {
    match connect_once(addr, mult) {
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
    let (tx, rx) = channel();
    let tx6 = tx.clone();
    thread::spawn(move || {
      let _ = tx6.send(connect_once(&v6, 1));
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
      let _ = tx.send(connect_once(&v4, 1));
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
      if let Ok(s) = connect_once(addr, 1) {
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

fn pull_range_from_swarm_peer(
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
  let req = P2pReq {
    req_rate: job.req_rate,
    paths: job.paths.to_vec(),
    range: Some((job.root_hash, job.blk_lo, job.blk_hi)),
    token: job.token,
    byte_range: job.byte_range,
    ..P2pReq::new(
      Op::Pull,
      0,
      job.share_id,
      my_udp_addrs,
      job.my_card.to_vec(),
    )
  };

  let have_roaring = encode_roaring_bits(&job.sink.snapshot_have_bits(), job.sink.total_blocks());
  write_two_frames(&mut stream, &req.encode()?, &have_roaring)?;
  let resp = P2pResp::decode(&read_frame(&mut stream)?)?;
  if resp.status != Status::Ok {
    return Err(PeerError::RemoteStatus(resp.status));
  }

  let peer_udp = punch_udp_with_peer_ip(
    &udp_sock,
    &resp.peer_addrs,
    stream.peer_addr().ok().map(|a| a.ip()),
    sid,
  )?;
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

/// 跨节点并发条带化拉取、点播首屏优先调度、动态 PEX 边下边传抢占与恶意节点自动剔除重拉
pub(crate) fn execute_striped_pull(
  local_region: Region,
  candidate_peers: Vec<SwarmPeer>,
  base_job: RangePullJob<'_>,
  reused_conns: &Mutex<HashMap<PeerId, TcpStream>>,
  dyn_peers: Option<&Mutex<Vec<SwarmPeer>>>,
) -> Result<(u32, u64, usize), PeerError> {
  let total_blocks = base_job.sink.total_blocks();
  let req_hi = base_job.blk_hi.min(total_blocks);
  let req_lo = base_job.blk_lo.min(req_hi);
  let vod_head_first = base_job.byte_range.is_some() || req_lo > 0;
  let Some((req_lo, req_hi)) = base_job.sink.trim_missing_range(req_lo, req_hi) else {
    return Ok((0, 0, 0));
  };

  let ranked = rank_swarm_peers(local_region, candidate_peers);
  if ranked.is_empty() {
    return Err(PeerError::NoHealthyPeer);
  }

  let stripes: Vec<(usize, u32, u32)> = plan_stripes_range(
    local_region,
    &ranked,
    &base_job.sink.block_weights(),
    req_lo,
    req_hi,
    vod_head_first,
  )
  .into_iter()
  .filter_map(|(p_idx, lo, hi)| {
    base_job
      .sink
      .trim_missing_range(lo, hi)
      .map(|(t_lo, t_hi)| (p_idx, t_lo, t_hi))
  })
  .collect();
  if stripes.is_empty() {
    return Ok((0, 0, 0));
  }
  let mut unique_peers: Vec<PeerId> = Vec::with_capacity(stripes.len() + 2);
  for &(p_idx, ..) in &stripes {
    let pid = ranked[p_idx].endpoint_id;
    if !unique_peers.contains(&pid) {
      unique_peers.push(pid);
    }
  }
  let used_peers = Mutex::new(unique_peers);
  let banned_peers: Mutex<Vec<PeerId>> = Mutex::new(Vec::new());
  let stripe_bounds: Arc<Vec<(AtomicU32, AtomicU32)>> = Arc::new(
    stripes
      .iter()
      .map(|&(_, lo, hi)| (AtomicU32::new(lo), AtomicU32::new(hi)))
      .collect(),
  );

  let mut blocks_transferred = 0u32;
  let mut bytes_transferred = 0u64;
  let mut conflict = false;

  thread::scope(|s| {
    let mut handles = Vec::with_capacity(stripes.len());
    for &(p_idx, lo, hi) in &stripes {
      let peer = &ranked[p_idx];
      let conn = reused_conns.lock().remove(&peer.endpoint_id);
      let bounds = Arc::clone(&stripe_bounds);
      let banned_ref = &banned_peers;
      let used_ref = &used_peers;
      handles.push(s.spawn(move || -> Result<(u32, u64), PeerError> {
        let (mut blks, mut bytes, stream) =
          match pull_range_from_swarm_peer(base_job.with_peer_range(peer, lo, hi), conn) {
            Ok(ok) => ok,
            Err(e) => {
              banned_ref.lock().push(peer.endpoint_id);
              return Err(e);
            }
          };
        let mut stream_opt = Some(stream);
        while !base_job.sink.is_range_complete(req_lo, req_hi) {
          if let Some((steal_lo, steal_hi)) = try_steal_tail(base_job.sink, &bounds, peer) {
            match pull_range_from_swarm_peer(
              base_job.with_peer_range(peer, steal_lo, steal_hi),
              stream_opt.take(),
            ) {
              Ok((b, by, s_next)) => {
                blks += b;
                bytes += by;
                stream_opt = Some(s_next);
                continue;
              }
              Err(_) => {
                banned_ref.lock().push(peer.endpoint_id);
                break;
              }
            }
          }
          let Some(dp_lock) = dyn_peers else {
            break;
          };
          let dyn_snap = dp_lock.lock().clone();
          let mut stole_dyn = false;
          for dp in &dyn_snap {
            if banned_ref.lock().contains(&dp.endpoint_id) {
              continue;
            }
            if let Some((steal_lo, steal_hi)) = try_steal_tail(base_job.sink, &bounds, dp) {
              let dp_conn = reused_conns.lock().remove(&dp.endpoint_id);
              if let Ok((b, by, dp_s)) = pull_range_from_swarm_peer(
                base_job.with_peer_range(dp, steal_lo, steal_hi),
                dp_conn,
              ) {
                blks += b;
                bytes += by;
                reused_conns.lock().insert(dp.endpoint_id, dp_s);
                let mut u = used_ref.lock();
                if !u.contains(&dp.endpoint_id) {
                  u.push(dp.endpoint_id);
                }
                stole_dyn = true;
                break;
              }
              banned_ref.lock().push(dp.endpoint_id);
            }
          }
          if !stole_dyn {
            break;
          }
        }
        if let Some(s_final) = stream_opt {
          reused_conns.lock().insert(peer.endpoint_id, s_final);
        }
        Ok((blks, bytes))
      }));
    }
    for h in handles {
      match h.join() {
        Ok(Ok((blks, bytes))) => {
          blocks_transferred += blks;
          bytes_transferred += bytes;
        }
        Ok(Err(PeerError::RemoteStatus(Status::Conflict))) => conflict = true,
        _ => {}
      }
    }
  });

  if conflict {
    return Err(PeerError::SnapshotConflict);
  }

  if !base_job.sink.is_range_complete(req_lo, req_hi) {
    let banned = banned_peers.lock().clone();
    let mut fallback_peers = ranked;
    if let Some(dp_lock) = dyn_peers {
      for dp in dp_lock.lock().iter() {
        if let Some(ex) = fallback_peers
          .iter_mut()
          .find(|p| p.endpoint_id == dp.endpoint_id)
        {
          ex.blk_lo = ex.blk_lo.min(dp.blk_lo);
          ex.blk_hi = ex.blk_hi.max(dp.blk_hi);
        } else {
          fallback_peers.push(dp.clone());
        }
      }
    }
    for peer in fallback_peers
      .iter()
      .filter(|p| !banned.contains(&p.endpoint_id))
    {
      if base_job.sink.is_range_complete(req_lo, req_hi) {
        break;
      }
      let p_lo = peer.blk_lo.max(req_lo);
      let p_hi = peer.blk_hi.min(req_hi);
      if let Some((min_lo, max_hi)) = base_job.sink.trim_missing_range(p_lo, p_hi) {
        let conn = reused_conns.lock().remove(&peer.endpoint_id);
        if let Ok((blks, bytes, stream)) =
          pull_range_from_swarm_peer(base_job.with_peer_range(peer, min_lo, max_hi), conn)
        {
          blocks_transferred += blks;
          bytes_transferred += bytes;
          reused_conns.lock().insert(peer.endpoint_id, stream);
          let mut u = used_peers.lock();
          if !u.contains(&peer.endpoint_id) {
            u.push(peer.endpoint_id);
          }
        }
      }
    }
  }

  if !base_job.sink.is_range_complete(req_lo, req_hi) {
    return Err(PeerError::Udp(UdpError::Timeout(req_lo, req_hi)));
  }

  let peers_used = used_peers.into_inner().len();
  Ok((blocks_transferred, bytes_transferred, peers_used))
}

fn try_steal_tail(
  sink: &SharedSink,
  bounds: &[(AtomicU32, AtomicU32)],
  thief: &SwarmPeer,
) -> Option<(u32, u32)> {
  let min_steal = macro_piece_blocks(sink.total_blocks()).max(MIN_STEAL_BLOCKS);
  let mut best: Option<(usize, u32, u32, u64)> = None;
  for (idx, (lo_atom, hi_atom)) in bounds.iter().enumerate() {
    let lo = lo_atom.load(Ordering::Acquire);
    let hi = hi_atom.load(Ordering::Acquire);
    if lo >= hi {
      continue;
    }
    let mut first_missing = hi;
    let mut rem_bytes = 0u64;
    sink.for_each_missing_span(lo, hi, |m_lo, m_hi| {
      first_missing = first_missing.min(m_lo);
      rem_bytes += sink.span_weight(m_lo, m_hi);
    });
    if first_missing > lo {
      lo_atom.fetch_max(first_missing, Ordering::Release);
    }
    let rem = hi.saturating_sub(first_missing);
    if rem >= LEECH_MIN_BLOCKS && (rem >= min_steal || rem_bytes >= MIN_STEAL_BYTES) {
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
