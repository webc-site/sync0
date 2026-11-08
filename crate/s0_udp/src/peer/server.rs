use std::{
  net::{SocketAddr, TcpStream},
  path::Path,
  sync::Arc,
  time::Duration,
};

use gxhash::HashMap;
use parking_lot::RwLock;
use s0_cdc::Manifest;

use super::{
  PeerError, PeerId, ShareConfig,
  pull::{derive_push_session_id, derive_session_id, read_frame, write_resp, write_two_frames},
  register_iroh_endpoints,
  sub::{build_swarm_peers, handle_incoming_sub_push},
};
use crate::{
  RangeSpec, bind_udp_for_ip,
  consts::{FLAG_DELETE, FLAG_PARTIAL, HAVE_ALL_ONES, Op, SUB_MAGIC, Status},
  fs::{BlockSource, FsError, SharedSink, SourceReader, build_manifest_from_paths, is_all_have},
  profile::SignedProfile,
  punch_udp, recv_range_udp,
  region::Region,
  send_range_udp,
  wire::{P2pReq, P2pResp, ShareSummary, SwarmPeer, decode_socket_addrs, encode_share_summaries},
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

  fn ok_empty(&self, eff_rate: u64, root_hash: [u8; 16], peer_addrs: Vec<u8>) -> P2pResp {
    self.ok_resp(eff_rate, root_hash, peer_addrs, Vec::new(), Vec::new())
  }

  fn swarm_peers(
    &self,
    eff_rate: u64,
    total_blks: u32,
    root_hash: [u8; 16],
    seeds: &[SwarmSeedRecord],
    exclude_id: PeerId,
  ) -> Vec<SwarmPeer> {
    build_swarm_peers(
      (
        self.my_id,
        self.my_region,
        &self.my_ctrl_addrs,
        &self.my_card,
      ),
      (eff_rate, total_blks, root_hash),
      seeds,
      exclude_id,
    )
  }

  fn build_and_cache(
    &self,
    share_id: [u8; 16],
    root: &Path,
    paths: &[String],
    del: bool,
  ) -> Option<([u8; 16], Manifest)> {
    let mf = build_manifest_from_paths(root, paths, del && paths.is_empty()).ok()?;
    let rh = mf.root_hash().ok()?;
    if paths.is_empty()
      && let Some(st) = self.shares.write().get_mut(&share_id)
    {
      st.cached_manifest = Some((rh, mf.clone()));
    }
    Some((rh, mf))
  }
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
      let mut summaries: Vec<ShareSummary> = ctx
        .shares
        .read()
        .values()
        .filter_map(|st| {
          let acc = st.cfg.resolve_access(&remote_id, req.req_rate, req.token)?;
          Some(ShareSummary {
            share_id: st.cfg.share_id,
            name: st.cfg.name.clone(),
            perm: acc.perm,
            eff_rate: acc.eff_rate,
            write_prefixes: acc.write_prefixes,
          })
        })
        .collect();
      summaries.sort_unstable_by(|a, b| a.name.cmp(&b.name));
      let resp = ctx.ok_resp(
        0,
        [0u8; 16],
        ctx.my_ctrl_addrs.clone(),
        Vec::new(),
        encode_share_summaries(&summaries)?,
      );
      write_resp(&mut stream, &resp)?;
      continue;
    }

    let (share_cfg, seeds_snapshot, active_sink, cached_mf) = {
      let guard = ctx.shares.read();
      let Some(st) = guard.get(&req.share_id) else {
        reject_req(&mut stream, req.op, Status::NotFound)?;
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
      reject_req(&mut stream, req.op, Status::Unauth)?;
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
          let Some((rh, mf)) = ctx.build_and_cache(
            req.share_id,
            &share_cfg.root,
            &req.paths,
            req.flags & FLAG_DELETE != 0,
          ) else {
            write_resp(&mut stream, &P2pResp::err(Status::NotFound))?;
            continue;
          };
          (rh, mf.encode().map_err(FsError::Cdc)?, mf.total_blocks())
        };

        let peers = ctx.swarm_peers(
          access.eff_rate,
          total_blks,
          root_hash,
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
        write_resp(&mut stream, &resp)?;
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
            (&req, &have_bits),
            (access.eff_rate, active_rh),
            sink_arc.as_ref(),
          )?;
          continue;
        }

        let (root_hash, mf) = if let Some((cached_rh, cached)) = cached_mf
          && req.paths.is_empty()
          && req.range.is_none_or(|(req_rh, ..)| req_rh == cached_rh)
        {
          (cached_rh, cached)
        } else {
          let Some(built) = ctx.build_and_cache(req.share_id, &share_cfg.root, &req.paths, false)
          else {
            write_resp(&mut stream, &P2pResp::err(Status::NotFound))?;
            continue;
          };
          built
        };
        if let Some((req_rh, ..)) = req.range
          && req_rh != root_hash
        {
          write_resp(&mut stream, &P2pResp::err(Status::Conflict))?;
          continue;
        }
        let reader = SourceReader::open(&share_cfg.root, mf)?;
        serve_pull_udp(
          &mut stream,
          ctx,
          (&req, &have_bits),
          (access.eff_rate, root_hash),
          &reader,
        )?;
      }

      Op::Push => {
        let mf_raw = read_frame(&mut stream)?;
        let mut mf = Manifest::decode(&mf_raw).map_err(|e| PeerError::Fs(FsError::Cdc(e)))?;

        let forbid_del = !access.write_prefixes.is_empty()
          && (req.flags & FLAG_DELETE != 0)
          && req.paths.is_empty();
        if !access.can_write_manifest(&mf, &req.paths) || forbid_del {
          write_resp(&mut stream, &P2pResp::err(Status::Forbidden))?;
          continue;
        }

        let is_sub_restricted = !access.write_prefixes.is_empty() || !req.paths.is_empty();
        mf.delete_extraneous = (req.flags & FLAG_DELETE != 0) && !is_sub_restricted;
        mf.partial = (req.flags & FLAG_PARTIAL != 0) || is_sub_restricted;

        let root_hash = mf.root_hash().map_err(FsError::Cdc)?;
        let sink = SharedSink::prepare(&share_cfg.root, mf)?;
        let total = sink.total_blocks();

        let (udp_sock, my_udp_addrs) = bind_udp_for_ip(stream.local_addr()?.ip())?;
        let resp_bytes = ctx
          .ok_empty(access.eff_rate, root_hash, my_udp_addrs)
          .encode()?;
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
        write_resp(
          &mut stream,
          &ctx.ok_empty(access.eff_rate, root_hash, Vec::new()),
        )?;
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
          if let Some(st) = ctx.shares.write().get_mut(&req.share_id) {
            if is_sub {
              st.subscribers.retain(|s| {
                s.ctrl_addr != peer_ctrl && (remote_id == [0u8; 32] || s.endpoint_id != remote_id)
              });
              st.subscribers.push(SubscriberRecord {
                endpoint_id: remote_id,
                ctrl_addr: peer_ctrl,
                paths: req.paths,
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
              swarm_peers = ctx.swarm_peers(access.eff_rate, hi, rh, &st.seeds, remote_id);
            }
          }
        }
        let rh = req.range.map_or([0u8; 16], |(rh, ..)| rh);
        let resp = ctx.ok_resp(
          access.eff_rate,
          rh,
          ctx.my_ctrl_addrs.clone(),
          swarm_peers,
          Vec::new(),
        );
        write_resp(&mut stream, &resp)?;
      }

      Op::ListShares => {}
    }
  }

  Ok(())
}

fn reject_req(stream: &mut TcpStream, op: Op, status: Status) -> Result<(), PeerError> {
  if matches!(op, Op::Pull | Op::Push) {
    let _ = read_frame(stream);
  }
  write_resp(stream, &P2pResp::err(status))
}

fn serve_pull_udp<'a>(
  stream: &mut TcpStream,
  ctx: &ServerCtx,
  (req, have_bits): (&P2pReq, &[u8]),
  (eff_rate, root_hash): (u64, [u8; 16]),
  source: impl Into<BlockSource<'a>>,
) -> Result<(), PeerError> {
  let source = source.into();
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
  write_resp(stream, &ctx.ok_empty(eff_rate, root_hash, my_udp_addrs))?;
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
