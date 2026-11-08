use std::{
  io::{self, Read, Write},
  net::{SocketAddr, TcpStream},
  sync::Arc,
  thread,
  time::Duration,
};

use gxhash::HashMap;
use parking_lot::{Mutex, RwLock};
use s0_cdc::{FLAG_DELETE, FLAG_PARTIAL, Manifest, for_each_missing_span};

use super::{
  PeerError, PeerId, ShareConfig, min_nonzero, plan_stripes, rank_swarm_peers,
  register_iroh_endpoint, resolve_iroh_endpoint,
};
use crate::{
  RangeSpec, UdpError, bind_udp_for_ip,
  consts::{FLAG_RANGE, FLAG_SEED, HAVE_ALL_ONES, Op, SUB_MAGIC, Status},
  fs::{BlockSource, FsError, SharedSink, SourceReader, build_manifest_from_paths, is_all_have},
  profile::SignedProfile,
  punch_udp, recv_range_udp,
  region::Region,
  send_range_udp,
  wire::{
    P2pReq, P2pResp, ShareSummary, SubPush, SwarmPeer, decode_socket_addrs, encode_share_summaries,
    encode_socket_addrs,
  },
};

/// 单帧控制面消息最大字节数上限（64 MiB，防止恶意帧长度触发 OOM）
const MAX_CTRL_FRAME_LEN: usize = 64 * 1024 * 1024;

#[derive(Clone)]
pub(crate) struct SubscriberRecord {
  pub(crate) endpoint_id: PeerId,
  pub(crate) ctrl_addr: SocketAddr,
  pub(crate) paths: Vec<String>,
  pub(crate) eff_rate: u64,
}

#[derive(Clone)]
pub(crate) struct SwarmSeedRecord {
  pub(crate) endpoint_id: PeerId,
  pub(crate) region: Region,
  pub(crate) ctrl_addr: SocketAddr,
  pub(crate) profile_card: Vec<u8>,
  pub(crate) root_hash: [u8; 16],
  pub(crate) blk_lo: u32,
  pub(crate) blk_hi: u32,
  pub(crate) weight_rate: u64,
}

pub(crate) struct ShareState {
  pub(crate) cfg: ShareConfig,
  pub(crate) sub_paths: Vec<String>,
  pub(crate) subscribers: Vec<SubscriberRecord>,
  pub(crate) seeds: Vec<SwarmSeedRecord>,
  /// 正在下载中的暂存池（实现边下载边上传，无需等待整个分享下载完成即可向其他节点供应已校验块）
  pub(crate) active_sink: Option<([u8; 16], Arc<SharedSink>)>,
  /// 最近一次全量扫描缓存的 `(root_hash, Manifest)`，使紧随 `Stat` 的 `Pull` 免除重复全盘扫描与分块
  pub(crate) cached_manifest: Option<([u8; 16], Manifest)>,
}

impl ShareState {
  pub(crate) fn upsert_seed(&mut self, seed: SwarmSeedRecord) {
    self
      .seeds
      .retain(|s| s.ctrl_addr != seed.ctrl_addr && s.endpoint_id != seed.endpoint_id);
    self.seeds.push(seed);
  }
}

#[derive(Clone)]
pub(crate) struct ServerCtx {
  pub(crate) my_id: PeerId,
  pub(crate) my_region: Region,
  pub(crate) my_card: Vec<u8>,
  pub(crate) my_ctrl_addr: SocketAddr,
  pub(crate) shares: Arc<RwLock<HashMap<[u8; 16], ShareState>>>,
  pub(crate) known_profiles: Arc<RwLock<HashMap<PeerId, SignedProfile>>>,
}

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

/// 构建本节点 + 已匹配 `root_hash` 的在线做种节点列表（`Stat` 与 `SubPush` 一处定义复用）
pub(crate) fn build_swarm_peers(
  origin: (PeerId, Region, &[u8], &[u8]),
  snap: (u64, u32, [u8; 16]),
  seeds: &[SwarmSeedRecord],
  exclude_id: PeerId,
) -> Vec<SwarmPeer> {
  let (my_id, my_region, my_ctrl_addrs, my_card) = origin;
  let (eff_rate, total_blks, root_hash) = snap;
  let mut peers = Vec::with_capacity(1 + seeds.len());
  peers.push(SwarmPeer {
    endpoint_id: my_id,
    region: my_region,
    weight_rate: eff_rate,
    blk_lo: 0,
    blk_hi: total_blks,
    peer_addrs: my_ctrl_addrs.to_vec(),
    profile_card: my_card.to_vec(),
  });
  for s in seeds {
    if s.root_hash == root_hash && s.endpoint_id != exclude_id {
      peers.push(SwarmPeer {
        endpoint_id: s.endpoint_id,
        region: s.region,
        weight_rate: min_nonzero(s.weight_rate, eff_rate),
        blk_lo: s.blk_lo,
        blk_hi: s.blk_hi,
        peer_addrs: encode_socket_addrs(&[s.ctrl_addr]),
        profile_card: s.profile_card.clone(),
      });
    }
  }
  peers
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

  write_frame(&mut stream, &req.encode()?)?;

  let have = job.sink.snapshot_have_bits();
  write_frame(&mut stream, &have)?;

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

/// 跨节点并发条带化拉取与动态折半抢占 Work-Stealing（复用已建立 TCP 控制流，快节点完成后自动折半接管慢节点剩余尾部区间）
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
  let per_stripe_rate = if base_job.req_rate == 0 && peers_used >= 2 {
    16_000_000
  } else {
    base_job.req_rate
  };
  let mut blocks_transferred = 0u32;
  let mut bytes_transferred = 0u64;
  let mut poisoned_indices = Vec::new();
  let mut conflict = false;

  thread::scope(|s| {
    let mut handles = Vec::with_capacity(stripes.len());
    for &(p_idx, lo, hi) in &stripes {
      let peer = &ranked[p_idx];
      let conn = reused_conns.lock().remove(&peer.endpoint_id);
      handles.push((
        p_idx,
        s.spawn(move || {
          let (blks, bytes, _stream) = pull_range_from_swarm_peer(
            RangePullJob {
              peer,
              blk_lo: lo,
              blk_hi: hi,
              req_rate: per_stripe_rate,
              ..base_job
            },
            conn,
          )?;
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
          Ok((blks, bytes, _)) => {
            blocks_transferred += blks;
            bytes_transferred += bytes;
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

pub(crate) fn handle_conn(mut stream: TcpStream, ctx: &ServerCtx) -> Result<(), PeerError> {
  while let Ok(frame) = read_frame(&mut stream) {
    if let Some(&hdr4) = frame.first_chunk::<4>()
      && u32::from_le_bytes(hdr4) == SUB_MAGIC
    {
      return handle_incoming_sub_push(&mut stream, &frame, ctx);
    }

    let req = P2pReq::decode(&frame)?;
    let mut remote_id = [0u8; 32];
    let mut remote_region = Region::Unknown;
    if !req.profile_card.is_empty() {
      let card = SignedProfile::decode_and_verify(&req.profile_card)?;
      remote_id = card.pubkey;
      remote_region = card.region;
      ctx.known_profiles.write().insert(remote_id, card);
    }

    if req.op == Op::ListShares {
      let mut summaries = Vec::new();
      {
        let guard = ctx.shares.read();
        for st in guard.values() {
          if let Some(acc) = st.cfg.resolve_access(&remote_id, req.req_rate, req.token) {
            summaries.push(ShareSummary {
              share_id: st.cfg.share_id,
              name: st.cfg.name.clone(),
              perm: acc.perm,
              eff_rate: acc.eff_rate,
              write_prefixes: acc.write_prefixes,
            });
          }
        }
      }
      summaries.sort_unstable_by(|a, b| a.name.cmp(&b.name));
      let resp = P2pResp {
        status: Status::Ok,
        flags: 0,
        eff_rate: 0,
        root_hash: [0u8; 16],
        peer_addrs: encode_socket_addrs(&[ctx.my_ctrl_addr]),
        profile_card: ctx.my_card.clone(),
        peers: Vec::new(),
        manifest: encode_share_summaries(&summaries)?,
      };
      write_frame(&mut stream, &resp.encode()?)?;
      continue;
    }

    let (share_cfg, seeds_snapshot, active_sink, cached_mf) = {
      let guard = ctx.shares.read();
      let Some(st) = guard.get(&req.share_id) else {
        if matches!(req.op, Op::Pull | Op::Push) {
          let _ = read_frame(&mut stream);
        }
        write_frame(&mut stream, &P2pResp::err(Status::NotFound).encode()?)?;
        continue;
      };
      (
        st.cfg.clone(),
        st.seeds.clone(),
        st.active_sink.clone(),
        st.cached_manifest.clone(),
      )
    };

    let Some(access) = share_cfg.resolve_access(&remote_id, req.req_rate, req.token) else {
      if matches!(req.op, Op::Pull | Op::Push) {
        let _ = read_frame(&mut stream);
      }
      write_frame(&mut stream, &P2pResp::err(Status::Unauth).encode()?)?;
      continue;
    };

    match req.op {
      Op::Stat => {
        if remote_id != [0u8; 32]
          && let Ok(addrs) = decode_socket_addrs(&req.peer_addrs)
          && let Some(&first) = addrs.first()
        {
          register_iroh_endpoint(remote_id, first);
        }

        let (root_hash, mf_bytes, total_blks) = if let Some((active_rh, ref sink_arc)) = active_sink
          && req.paths.is_empty()
        {
          let bytes = sink_arc.manifest().encode().map_err(FsError::Cdc)?;
          (active_rh, bytes, sink_arc.total_blocks())
        } else {
          let Ok(mf) = build_manifest_from_paths(
            &share_cfg.root,
            &req.paths,
            req.flags & FLAG_DELETE != 0 && req.paths.is_empty(),
          ) else {
            write_frame(&mut stream, &P2pResp::err(Status::NotFound).encode()?)?;
            continue;
          };
          let rh = mf.root_hash().map_err(FsError::Cdc)?;
          let bytes = mf.encode().map_err(FsError::Cdc)?;
          let blks = mf.total_blocks();
          if req.paths.is_empty()
            && let Some(st) = ctx.shares.write().get_mut(&req.share_id)
          {
            st.cached_manifest = Some((rh, mf));
          }
          (rh, bytes, blks)
        };

        let my_ctrl_addrs = encode_socket_addrs(&[ctx.my_ctrl_addr]);
        let peers = build_swarm_peers(
          (ctx.my_id, ctx.my_region, &my_ctrl_addrs, &ctx.my_card),
          (access.eff_rate, total_blks, root_hash),
          &seeds_snapshot,
          remote_id,
        );

        let resp = P2pResp {
          status: Status::Ok,
          flags: 0,
          eff_rate: access.eff_rate,
          root_hash,
          peer_addrs: my_ctrl_addrs,
          profile_card: ctx.my_card.clone(),
          peers,
          manifest: mf_bytes,
        };
        write_frame(&mut stream, &resp.encode()?)?;
      }

      Op::Pull => {
        let have_bits = read_frame(&mut stream)?;

        // 若当前节点正在边下边传 (`active_sink`) 且请求的快照匹配，直接从暂存池无锁供应已校验块
        if let Some((active_rh, ref sink_arc)) = active_sink
          && req.paths.is_empty()
          && req.range.is_none_or(|(req_rh, ..)| req_rh == active_rh)
        {
          serve_pull_udp(
            &mut stream,
            ctx,
            &req,
            &have_bits,
            access.eff_rate,
            active_rh,
            sink_arc.as_ref().into(),
          )?;
          continue;
        }

        let (root_hash, mf) = if let Some((cached_rh, cached)) = cached_mf
          && req.paths.is_empty()
          && req.range.is_none_or(|(req_rh, ..)| req_rh == cached_rh)
        {
          (cached_rh, cached)
        } else {
          let Ok(built) = build_manifest_from_paths(&share_cfg.root, &req.paths, false) else {
            write_frame(&mut stream, &P2pResp::err(Status::NotFound).encode()?)?;
            continue;
          };
          let rh = built.root_hash().map_err(FsError::Cdc)?;
          if req.paths.is_empty()
            && let Some(st) = ctx.shares.write().get_mut(&req.share_id)
          {
            st.cached_manifest = Some((rh, built.clone()));
          }
          (rh, built)
        };
        if let Some((req_rh, ..)) = req.range
          && req_rh != root_hash
        {
          write_frame(&mut stream, &P2pResp::err(Status::Conflict).encode()?)?;
          continue;
        }
        let reader = SourceReader::open(&share_cfg.root, mf)?;
        serve_pull_udp(
          &mut stream,
          ctx,
          &req,
          &have_bits,
          access.eff_rate,
          root_hash,
          (&reader).into(),
        )?;
      }

      Op::Push => {
        let mf_raw = read_frame(&mut stream)?;
        let mut mf = Manifest::decode(&mf_raw).map_err(|e| PeerError::Fs(FsError::Cdc(e)))?;

        // 严格校验该组员在此分享组中允许上传的路径前缀（默认空列表表示允许整个分享根目录）
        if !access.can_write_manifest(&mf, &req.paths) {
          write_frame(&mut stream, &P2pResp::err(Status::Forbidden).encode()?)?;
          continue;
        }
        if !access.write_prefixes.is_empty()
          && (req.flags & FLAG_DELETE != 0)
          && req.paths.is_empty()
        {
          write_frame(&mut stream, &P2pResp::err(Status::Forbidden).encode()?)?;
          continue;
        }

        let is_sub_restricted = !access.write_prefixes.is_empty() || !req.paths.is_empty();
        mf.delete_extraneous = (req.flags & FLAG_DELETE != 0) && !is_sub_restricted;
        mf.partial = (req.flags & FLAG_PARTIAL != 0) || is_sub_restricted;

        let root_hash = mf.root_hash().map_err(FsError::Cdc)?;
        let sink = SharedSink::prepare(&share_cfg.root, mf)?;
        let total = sink.total_blocks();

        let (udp_sock, my_udp_addrs) = bind_udp_for_ip(stream.local_addr()?.ip())?;
        let resp = P2pResp {
          status: Status::Ok,
          flags: 0,
          eff_rate: access.eff_rate,
          root_hash,
          peer_addrs: my_udp_addrs,
          profile_card: ctx.my_card.clone(),
          peers: Vec::new(),
          manifest: Vec::new(),
        };
        write_frame(&mut stream, &resp.encode()?)?;

        let init_have = sink.initial_have_bits();
        if total == 0 || is_all_have(init_have, total) {
          write_frame(&mut stream, &HAVE_ALL_ONES.to_le_bytes())?;
        } else {
          write_frame(&mut stream, init_have)?;
          let sid = derive_push_session_id(&root_hash);
          let peer_udp = punch_udp(&udp_sock, &req.peer_addrs, sid)?;
          recv_range_udp(
            &udp_sock,
            &peer_udp,
            &sink,
            RangeSpec {
              session_id: sid,
              blk_lo: 0,
              blk_hi: total,
              eff_rate: access.eff_rate,
              timeout: Duration::from_secs(10),
            },
          )?;
        }

        sink.commit()?;
        if let Some(st) = ctx.shares.write().get_mut(&req.share_id) {
          st.cached_manifest = None;
        }
        let done = P2pResp {
          status: Status::Ok,
          flags: 0,
          eff_rate: access.eff_rate,
          root_hash,
          peer_addrs: Vec::new(),
          profile_card: Vec::new(),
          peers: Vec::new(),
          manifest: Vec::new(),
        };
        write_frame(&mut stream, &done.encode()?)?;
      }

      Op::Sub | Op::GroupJoin | Op::Have => {
        let is_sub = matches!(req.op, Op::Sub | Op::GroupJoin);
        let peer_ctrl_addrs = decode_socket_addrs(&req.peer_addrs)?;
        let mut swarm_peers = Vec::new();
        let my_ctrl_addrs = encode_socket_addrs(&[ctx.my_ctrl_addr]);
        if let Some(&peer_ctrl) = peer_ctrl_addrs.first()
          && (is_sub || req.range.is_some())
        {
          if remote_id != [0u8; 32] {
            register_iroh_endpoint(remote_id, peer_ctrl);
          }
          let mut guard = ctx.shares.write();
          if let Some(st) = guard.get_mut(&req.share_id) {
            if is_sub {
              st.subscribers.retain(|s| s.ctrl_addr != peer_ctrl);
              st.subscribers.push(SubscriberRecord {
                endpoint_id: remote_id,
                ctrl_addr: peer_ctrl,
                paths: req.paths.clone(),
                eff_rate: access.eff_rate,
              });
            }
            if let Some((rh, lo, hi)) = req.range {
              st.upsert_seed(SwarmSeedRecord {
                endpoint_id: remote_id,
                region: remote_region,
                ctrl_addr: peer_ctrl,
                profile_card: req.profile_card,
                root_hash: rh,
                blk_lo: lo,
                blk_hi: hi,
                weight_rate: access.eff_rate,
              });
              swarm_peers = build_swarm_peers(
                (ctx.my_id, ctx.my_region, &my_ctrl_addrs, &ctx.my_card),
                (access.eff_rate, hi, rh),
                &st.seeds,
                remote_id,
              );
            }
          }
        }
        let resp = P2pResp {
          status: Status::Ok,
          flags: 0,
          eff_rate: access.eff_rate,
          root_hash: req.range.map_or([0u8; 16], |(rh, ..)| rh),
          peer_addrs: my_ctrl_addrs,
          profile_card: ctx.my_card.clone(),
          peers: swarm_peers,
          manifest: Vec::new(),
        };
        write_frame(&mut stream, &resp.encode()?)?;
      }

      Op::ListShares => {}
    }
  }

  Ok(())
}

fn serve_pull_udp(
  stream: &mut TcpStream,
  ctx: &ServerCtx,
  req: &P2pReq,
  have_bits: &[u8],
  eff_rate: u64,
  root_hash: [u8; 16],
  source: BlockSource<'_>,
) -> Result<(), PeerError> {
  let total = source.total_blocks();
  let (blk_lo, blk_hi) = req
    .range
    .map_or((0, total), |(_, lo, hi)| (lo.min(total), hi.min(total)));
  let (udp_sock, my_udp_addrs) = bind_udp_for_ip(stream.local_addr()?.ip())?;
  let resp = P2pResp {
    status: Status::Ok,
    flags: 0,
    eff_rate,
    root_hash,
    peer_addrs: my_udp_addrs,
    profile_card: ctx.my_card.clone(),
    peers: Vec::new(),
    manifest: Vec::new(),
  };
  write_frame(stream, &resp.encode()?)?;
  if blk_lo < blk_hi
    && let Ok(peer_udp) = decode_socket_addrs(&req.peer_addrs)
    && let Some(&first_peer) = peer_udp.first()
  {
    let cli_port = first_peer.port();
    let sid = derive_session_id(&root_hash, blk_lo ^ (u32::from(cli_port) << 8));
    let _ = punch_udp(&udp_sock, &req.peer_addrs, sid);
    let _ = send_range_udp(
      &udp_sock,
      &peer_udp,
      source,
      have_bits,
      RangeSpec {
        session_id: sid,
        blk_lo,
        blk_hi,
        eff_rate,
        timeout: Duration::from_secs(12),
      },
    );
  }
  Ok(())
}

fn handle_incoming_sub_push(
  stream: &mut TcpStream,
  frame: &[u8],
  ctx: &ServerCtx,
) -> Result<(), PeerError> {
  let push = SubPush::decode(frame)?;
  let (local_root, sub_paths) = {
    let guard = ctx.shares.read();
    let Some(st) = guard.get(&push.share_id) else {
      return Ok(());
    };
    (st.cfg.root.clone(), st.sub_paths.clone())
  };

  let mf = Manifest::decode(&push.manifest).map_err(|e| PeerError::Fs(FsError::Cdc(e)))?;
  let sink = SharedSink::prepare(&local_root, mf)?;
  let total = sink.total_blocks();

  if total > 0 && !sink.is_range_complete(0, total) {
    let candidates: Vec<SwarmPeer> = push
      .peers
      .into_iter()
      .filter(|p| p.endpoint_id != ctx.my_id && !p.peer_addrs.is_empty())
      .collect();
    if let Some(first_peer) = candidates.first().cloned() {
      let reused_conns = Mutex::new(HashMap::default());
      let _ = execute_striped_pull(
        ctx.my_region,
        candidates,
        RangePullJob {
          peer: &first_peer,
          share_id: push.share_id,
          paths: &sub_paths,
          root_hash: push.root_hash,
          blk_lo: 0,
          blk_hi: total,
          req_rate: push.eff_rate,
          token: None,
          my_card: &ctx.my_card,
          sink: &sink,
        },
        &reused_conns,
      );
    }
  }
  sink.commit()?;

  if let Ok(origin_addrs) = decode_socket_addrs(&push.origin_addrs)
    && let Some(&origin_ctrl) = origin_addrs.first()
  {
    let have_req = P2pReq {
      op: Op::Have,
      flags: FLAG_SEED,
      share_id: push.share_id,
      req_rate: 0,
      paths: Vec::new(),
      peer_addrs: encode_socket_addrs(&[ctx.my_ctrl_addr]),
      profile_card: ctx.my_card.clone(),
      range: Some((push.root_hash, 0, total)),
      token: None,
    };
    if let Ok(mut s) = TcpStream::connect_timeout(&origin_ctrl, Duration::from_secs(2)) {
      let _ = write_frame(&mut s, &have_req.encode()?);
      let _ = read_frame(&mut s);
    }
  }

  write_frame(stream, &[0u8])?;
  Ok(())
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
