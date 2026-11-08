use std::{
  net::{SocketAddr, TcpStream},
  sync::Arc,
  time::Duration,
};

use gxhash::HashMap;
use parking_lot::RwLock;
use s0_cdc::Manifest;

use super::{
  PeerError, PeerId, ShareConfig, min_nonzero,
  pull::{derive_push_session_id, derive_session_id, read_frame, write_frame, write_two_frames},
  register_iroh_endpoints, resolve_iroh_endpoints,
  sub::handle_incoming_sub_push,
};
use crate::{
  RangeSpec, bind_udp_for_ip,
  consts::{FLAG_DELETE, FLAG_PARTIAL, HAVE_ALL_ONES, Op, SUB_MAGIC, Status},
  fs::{BlockSource, FsError, SharedSink, SourceReader, build_manifest_from_paths, is_all_have},
  profile::SignedProfile,
  punch_udp, recv_range_udp,
  region::Region,
  send_range_udp,
  wire::{
    P2pReq, P2pResp, ShareSummary, SwarmPeer, decode_socket_addrs, encode_share_summaries,
    encode_socket_addrs,
  },
};

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
  pub(crate) my_ctrl_addrs: Vec<u8>,
  pub(crate) shares: Arc<RwLock<HashMap<[u8; 16], ShareState>>>,
  pub(crate) known_profiles: Arc<RwLock<HashMap<PeerId, SignedProfile>>>,
}

impl ServerCtx {
  fn ok_resp(
    &self,
    eff_rate: u64,
    root_hash: [u8; 16],
    peer_addrs: Vec<u8>,
    peers: Vec<SwarmPeer>,
    manifest: Vec<u8>,
  ) -> P2pResp {
    P2pResp {
      status: Status::Ok,
      flags: 0,
      eff_rate,
      root_hash,
      peer_addrs,
      profile_card: self.my_card.clone(),
      peers,
      manifest,
    }
  }
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
      let seed_addrs = resolve_iroh_endpoints(&s.endpoint_id);
      let encoded_addrs = if seed_addrs.is_empty() {
        encode_socket_addrs(&[s.ctrl_addr])
      } else {
        encode_socket_addrs(&seed_addrs)
      };
      peers.push(SwarmPeer {
        endpoint_id: s.endpoint_id,
        region: s.region,
        weight_rate: min_nonzero(s.weight_rate, eff_rate),
        blk_lo: s.blk_lo,
        blk_hi: s.blk_hi,
        peer_addrs: encoded_addrs,
        profile_card: s.profile_card.clone(),
      });
    }
  }
  peers
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
      let resp = ctx.ok_resp(
        0,
        [0u8; 16],
        ctx.my_ctrl_addrs.clone(),
        Vec::new(),
        encode_share_summaries(&summaries)?,
      );
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
          && !addrs.is_empty()
        {
          register_iroh_endpoints(remote_id, &addrs);
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

        let peers = build_swarm_peers(
          (ctx.my_id, ctx.my_region, &ctx.my_ctrl_addrs, &ctx.my_card),
          (access.eff_rate, total_blks, root_hash),
          &seeds_snapshot,
          remote_id,
        );

        let resp = ctx.ok_resp(
          access.eff_rate,
          root_hash,
          ctx.my_ctrl_addrs.clone(),
          peers,
          mf_bytes,
        );
        write_frame(&mut stream, &resp.encode()?)?;
      }

      Op::Pull => {
        let have_bits = read_frame(&mut stream)?;

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
        let resp = ctx.ok_resp(
          access.eff_rate,
          root_hash,
          my_udp_addrs,
          Vec::new(),
          Vec::new(),
        );
        let resp_bytes = resp.encode()?;
        let init_have = sink.initial_have_bits();
        if total == 0 || is_all_have(init_have, total) {
          write_two_frames(&mut stream, &resp_bytes, &HAVE_ALL_ONES.to_le_bytes())?;
        } else {
          write_two_frames(&mut stream, &resp_bytes, init_have)?;
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
        let done = ctx.ok_resp(
          access.eff_rate,
          root_hash,
          Vec::new(),
          Vec::new(),
          Vec::new(),
        );
        write_frame(&mut stream, &done.encode()?)?;
      }

      Op::Sub | Op::GroupJoin | Op::Have => {
        let is_sub = matches!(req.op, Op::Sub | Op::GroupJoin);
        let peer_ctrl_addrs = decode_socket_addrs(&req.peer_addrs)?;
        let mut swarm_peers = Vec::new();
        if let Some(&peer_ctrl) = peer_ctrl_addrs.first()
          && (is_sub || req.range.is_some())
        {
          if remote_id != [0u8; 32] {
            register_iroh_endpoints(remote_id, &peer_ctrl_addrs);
          }
          let mut guard = ctx.shares.write();
          if let Some(st) = guard.get_mut(&req.share_id) {
            if is_sub {
              st.subscribers.retain(|s| {
                s.ctrl_addr != peer_ctrl && (remote_id == [0u8; 32] || s.endpoint_id != remote_id)
              });
              st.subscribers.push(SubscriberRecord {
                endpoint_id: remote_id,
                ctrl_addr: peer_ctrl,
                paths: req.paths.clone(),
                eff_rate: access.eff_rate,
              });
            }
            if let Some((rh, lo, hi)) = req.range {
              if hi > lo {
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
              }
              swarm_peers = build_swarm_peers(
                (ctx.my_id, ctx.my_region, &ctx.my_ctrl_addrs, &ctx.my_card),
                (access.eff_rate, hi, rh),
                &st.seeds,
                remote_id,
              );
            }
          }
        }
        let resp = ctx.ok_resp(
          access.eff_rate,
          req.range.map_or([0u8; 16], |(rh, ..)| rh),
          ctx.my_ctrl_addrs.clone(),
          swarm_peers,
          Vec::new(),
        );
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
  let (mut blk_lo, mut blk_hi) = req
    .range
    .map_or((0, total), |(_, lo, hi)| (lo.min(total), hi.min(total)));
  if let Some((b_lo, b_hi)) = req.byte_range {
    let (r_lo, r_hi) = source.manifest().locate_byte_range(b_lo, b_hi);
    blk_lo = blk_lo.max(r_lo).min(total);
    blk_hi = blk_hi.min(r_hi).min(total);
  }
  let (udp_sock, my_udp_addrs) = bind_udp_for_ip(stream.local_addr()?.ip())?;
  let resp = ctx.ok_resp(eff_rate, root_hash, my_udp_addrs, Vec::new(), Vec::new());
  write_frame(stream, &resp.encode()?)?;
  if blk_lo < blk_hi
    && let Ok(peer_udp) = punch_udp(&udp_sock, &req.peer_addrs, 0)
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
