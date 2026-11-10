use std::{io::Read, net::TcpStream, path::Path, sync::Arc, time::Duration};

use gxhash::HashMap;
use parking_lot::{Mutex, RwLock};
use s0_cdc::{
  Manifest, WireError, decode_roaring_bits, encode_roaring_bits, is_roaring_full_frame,
};
use s0_const::{
  FLAG_AUTH, FLAG_DELETE, FLAG_PARTIAL, HASH_LEN, MAGIC_LEN, Op, PROTO_VERSION, Perm, SUB_MAGIC,
  Status, ZERO_HASH, ZERO_PEER_ID, region::Region,
};
use s0_fs::{BlockSource, SharedSink, SourceReader, build_manifest_from_paths};
use s0_udp::{RangeSpec, bind_udp_for_ip, punch_udp_with_peer_ip, recv_range_udp, send_range_udp};
use s0_wire::{P2pReq, P2pResp, ShareSummary, SignedProfile, SwarmPeer, encode_share_summaries};

use crate::{
  PeerError, PeerId, ShareConfig,
  frame::{read_frame, write_frame, write_resp, write_two_frames},
  iroh_net::IrohCtrl,
  pull::derive_session_id,
  register_encoded_endpoints,
  sub::{handle_incoming_sub_push_frame, register_sub_or_have},
  watch::{ShareState, SwarmSeedRecord, build_swarm_peers},
};

const PUSH_RECV_TIMEOUT: Duration = Duration::from_secs(10);
const PULL_SEND_TIMEOUT: Duration = Duration::from_secs(12);

#[derive(Clone)]
pub(crate) struct ServerCtx {
  pub(crate) my_id: PeerId,
  pub(crate) my_region: Region,
  pub(crate) my_card: Vec<u8>,
  pub(crate) my_ctrl_addrs: Vec<u8>,
  pub(crate) iroh_ctrl: IrohCtrl,
  pub(crate) shares: Arc<RwLock<HashMap<[u8; HASH_LEN], ShareState>>>,
  pub(crate) known_profiles: Arc<RwLock<HashMap<PeerId, SignedProfile>>>,
  pub(crate) publish_lock: Arc<Mutex<()>>,
}

impl ServerCtx {
  fn ok_resp(
    &self,
    auth: bool,
    eff_rate: u64,
    root_hash: [u8; HASH_LEN],
    peer_addrs: Vec<u8>,
    peers: Vec<SwarmPeer>,
    manifest: Vec<u8>,
  ) -> P2pResp {
    P2pResp {
      status: Status::Ok,
      flags: if auth { FLAG_AUTH } else { 0 },
      eff_rate,
      root_hash,
      peer_addrs,
      profile_card: self.my_card.clone(),
      peers,
      manifest,
    }
  }

  fn ok_empty(&self, eff_rate: u64, root_hash: [u8; HASH_LEN], peer_addrs: Vec<u8>) -> P2pResp {
    self.ok_resp(
      false,
      eff_rate,
      root_hash,
      peer_addrs,
      Vec::new(),
      Vec::new(),
    )
  }

  pub(crate) fn swarm_peers(
    &self,
    share_cfg: &ShareConfig,
    eff_rate: u64,
    total_blks: u32,
    root_hash: [u8; HASH_LEN],
    seeds: &[SwarmSeedRecord],
    exclude_id: PeerId,
  ) -> Vec<SwarmPeer> {
    let origin = (
      self.my_id,
      self.my_region,
      &self.my_ctrl_addrs[..],
      &self.my_card[..],
    );
    let mut peers = build_swarm_peers(origin, (eff_rate, total_blks, root_hash), seeds, exclude_id);
    if share_cfg.auth_required {
      for (&pid, q) in &share_cfg.peer_quotas {
        if q.perm.can_read() && pid != ZERO_PEER_ID && !peers.iter().any(|p| p.endpoint_id == pid) {
          peers.push(SwarmPeer {
            endpoint_id: pid,
            region: Region::Unknown,
            weight_rate: 0,
            blk_lo: 0,
            blk_hi: 0,
            peer_addrs: Vec::new(),
            profile_card: Vec::new(),
          });
        }
      }
    }
    peers
  }

  fn resolve_or_build(
    &self,
    share_id: [u8; HASH_LEN],
    root: &Path,
    req: &P2pReq,
    cached_mf: Option<(Vec<String>, [u8; HASH_LEN], Manifest)>,
    allow_del: bool,
  ) -> Option<([u8; HASH_LEN], Manifest)> {
    let del = allow_del && (req.flags & FLAG_DELETE != 0);
    if let Some((cp, crh, cmf)) = cached_mf
      && !del
    {
      if cp == req.paths && req.range.is_none_or(|(req_rh, ..)| req_rh == crh) {
        return Some((crh, cmf));
      }
      if cp.is_empty() && !req.paths.is_empty() {
        let filtered = cmf.filter_paths(&req.paths);
        if let Ok(rh) = filtered.root_hash()
          && req.range.is_none_or(|(req_rh, ..)| req_rh == rh)
        {
          return Some((rh, filtered));
        }
      }
    }
    let mf = build_manifest_from_paths(root, &req.paths, del && req.paths.is_empty()).ok()?;
    let rh = mf.root_hash().ok()?;
    if let Some(st) = self.shares.write().get_mut(&share_id)
      && (req.paths.is_empty()
        || st
          .cached_manifest
          .as_ref()
          .is_none_or(|(c, ..)| !c.is_empty()))
    {
      st.cached_manifest = Some((req.paths.clone(), rh, mf.clone()));
    }
    Some((rh, mf))
  }
}

pub(crate) fn handle_conn(mut stream: TcpStream, ctx: &ServerCtx) -> Result<(), PeerError> {
  let mut ver = [0u8; 1];
  if stream.read_exact(&mut ver).is_err() {
    return Ok(());
  }
  if ver[0] != PROTO_VERSION {
    return Err(WireError::BadVersion.into());
  }
  while let Ok(frame) = read_frame(&mut stream) {
    if let Some(&hdr4) = frame.first_chunk::<MAGIC_LEN>()
      && u32::from_le_bytes(hdr4) == SUB_MAGIC
    {
      handle_incoming_sub_push_frame(&frame, ctx)?;
      write_frame(&mut stream, &[Status::Ok as u8])?;
      continue;
    }

    let req = P2pReq::decode(&frame)?;
    let mut remote_id = ZERO_PEER_ID;
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
        false,
        0,
        ZERO_HASH,
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
        if !access.perm.can_read() {
          reject_req(&mut stream, req.op, Status::Forbidden)?;
          continue;
        }
        register_encoded_endpoints(remote_id, &req.peer_addrs);

        let (root_hash, mf_bytes, total_blks) = if let Some((active_rh, ref sink_arc)) = active_sink
          && req.paths.is_empty()
        {
          (
            active_rh,
            sink_arc.manifest().encode()?,
            sink_arc.total_blocks(),
          )
        } else {
          let Some((rh, mf)) =
            ctx.resolve_or_build(req.share_id, &share_cfg.root, &req, cached_mf, true)
          else {
            write_resp(&mut stream, &P2pResp::err(Status::NotFound))?;
            continue;
          };
          (rh, mf.encode()?, mf.total_blocks())
        };

        let peers = ctx.swarm_peers(
          &share_cfg,
          access.eff_rate,
          total_blks,
          root_hash,
          &seeds_snapshot,
          remote_id,
        );
        let resp = ctx.ok_resp(
          share_cfg.auth_required,
          access.eff_rate,
          root_hash,
          ctx.my_ctrl_addrs.clone(),
          peers,
          mf_bytes,
        );
        write_resp(&mut stream, &resp)?;
      }

      Op::Pull => {
        if !access.perm.can_read() {
          reject_req(&mut stream, req.op, Status::Forbidden)?;
          continue;
        }
        let have_bits = read_frame(&mut stream)?;
        if let Some((active_rh, ref sink_arc)) = active_sink
          && req.paths.is_empty()
          && req.range.is_none_or(|(req_rh, ..)| req_rh == active_rh)
        {
          let meta = (access.eff_rate, active_rh);
          serve_pull_udp(
            &mut stream,
            ctx,
            (&req, &have_bits),
            meta,
            sink_arc.as_ref(),
          )?;
          continue;
        }

        let Some((root_hash, mf)) =
          ctx.resolve_or_build(req.share_id, &share_cfg.root, &req, cached_mf, false)
        else {
          write_resp(&mut stream, &P2pResp::err(Status::NotFound))?;
          continue;
        };
        if req.range.is_some_and(|(req_rh, ..)| req_rh != root_hash) {
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
        let mut mf = Manifest::decode(&read_frame(&mut stream)?)?;
        let forbid_del = (req.flags & FLAG_DELETE != 0)
          && (!access.perm.can_read()
            || (!access.write_prefixes.is_empty() && req.paths.is_empty()));
        if !access.can_write_manifest(&mf, &req.paths) || forbid_del {
          write_resp(&mut stream, &P2pResp::err(Status::Forbidden))?;
          continue;
        }

        let is_sub_restricted = !access.write_prefixes.is_empty() || !req.paths.is_empty();
        mf.delete_extraneous = (req.flags & FLAG_DELETE != 0) && !is_sub_restricted;
        mf.partial = (req.flags & FLAG_PARTIAL != 0) || is_sub_restricted;

        let root_hash = mf.root_hash()?;
        let sink = SharedSink::prepare(&share_cfg.root, mf)?;
        let total = sink.total_blocks();

        let (udp_sock, my_udp_addrs) = bind_udp_for_ip(stream.local_addr()?.ip())?;
        let resp_bytes = ctx
          .ok_empty(access.eff_rate, root_hash, my_udp_addrs)
          .encode()?;
        let have_frame = encode_roaring_bits(sink.initial_have_bits(), total);
        write_two_frames(&mut stream, &resp_bytes, &have_frame)?;
        if !is_roaring_full_frame(&have_frame) {
          let sid = derive_session_id(&root_hash, 0);
          let peer_ip = stream.peer_addr().ok().map(|a| a.ip());
          let peer_udp = punch_udp_with_peer_ip(&udp_sock, &req.peer_addrs, peer_ip, sid)?;
          recv_range_udp(
            &udp_sock,
            &peer_udp,
            &sink,
            RangeSpec {
              session_id: sid,
              blk_lo: 0,
              blk_hi: total,
              eff_rate: access.eff_rate,
              timeout: PUSH_RECV_TIMEOUT,
            },
          )?;
        }

        sink.commit()?;
        let has_subs = ctx.shares.write().get_mut(&req.share_id).is_some_and(|st| {
          st.cached_manifest = None;
          !st.subscribers.is_empty()
        });
        write_resp(
          &mut stream,
          &ctx.ok_empty(access.eff_rate, root_hash, Vec::new()),
        )?;
        if has_subs {
          let _ = ctx.publish_share_id(req.share_id);
        }
      }

      Op::Sub | Op::GroupJoin | Op::Have => {
        if !access.perm.can_read() || (req.op == Op::GroupJoin && access.perm != Perm::Rw) {
          reject_req(&mut stream, req.op, Status::Forbidden)?;
          continue;
        }
        let rh = req.range.map_or(ZERO_HASH, |(rh, ..)| rh);
        let swarm_peers = register_sub_or_have(
          ctx,
          &share_cfg,
          req,
          (remote_id, remote_region, access.eff_rate),
          cached_mf,
        )?;
        let resp = ctx.ok_resp(
          share_cfg.auth_required,
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
  (req, have_frame): (&P2pReq, &[u8]),
  (eff_rate, root_hash): (u64, [u8; HASH_LEN]),
  source: impl Into<BlockSource<'a>>,
) -> Result<(), PeerError> {
  let source = source.into();
  let total = source.total_blocks();
  let have_bits = decode_roaring_bits(have_frame, total)?;
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
  let peer_ip = stream.peer_addr().ok().map(|a| a.ip());
  if blk_lo < blk_hi
    && let Ok(peer_udp) = punch_udp_with_peer_ip(&udp_sock, &req.peer_addrs, peer_ip, 0)
    && let Some(&first_peer) = peer_udp.first()
  {
    let cli_port = first_peer.port();
    let sid = derive_session_id(&root_hash, blk_lo ^ (u32::from(cli_port) << 8));
    let _ = punch_udp_with_peer_ip(&udp_sock, &req.peer_addrs, peer_ip, sid);
    let _ = send_range_udp(
      &udp_sock,
      &peer_udp,
      source,
      &have_bits,
      RangeSpec {
        session_id: sid,
        blk_lo,
        blk_hi,
        eff_rate,
        timeout: PULL_SEND_TIMEOUT,
      },
    );
  }
  Ok(())
}
