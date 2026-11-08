use std::{fs::create_dir_all, net::TcpStream, path::PathBuf, sync::Arc};

use gxhash::HashMap;
use parking_lot::Mutex;
use s0_cdc::{FLAG_PARTIAL, Manifest};

use super::{
  PeerError, PeerId, PeerNode, ShareConfig, SyncReport, TransferOpts, derive_share_id, min_nonzero,
  pull::{
    RangePullJob, connect_ctrl_addrs, execute_striped_pull, read_frame, send_req_recv_resp,
    write_frame,
  },
  resolve_iroh_endpoints,
  server::{ServerCtx, SwarmSeedRecord},
};
use crate::{
  consts::{FLAG_SEED, Op, Status},
  fs::{FsError, SharedSink, build_manifest_from_paths},
  region::Region,
  wire::{P2pReq, SubPush, SwarmPeer, decode_socket_addrs, encode_socket_addrs},
};

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

impl PeerNode {
  /// 长连接订阅远端对等节点的分享组 (`Op::Sub = 4`)
  pub fn subscribe(
    self: &Arc<Self>,
    origin_id: PeerId,
    share_name: &str,
    local_dir: PathBuf,
    opts: TransferOpts<'_>,
  ) -> Result<SyncReport, PeerError> {
    let share_id = derive_share_id(share_name);
    let sel_paths = opts.resolved_paths();
    create_dir_all(&local_dir)?;

    self.add_share_with_paths(
      ShareConfig::public_ro(share_name, &local_dir, 0),
      sel_paths.clone(),
    );

    let initial = self.pull(
      origin_id,
      share_name,
      &local_dir,
      opts.clone().with_delete(sel_paths.is_empty()),
    )?;

    let mut stream = self.connect_peer(&origin_id)?;
    let req = P2pReq {
      op: Op::Sub,
      flags: FLAG_SEED,
      share_id,
      req_rate: opts.req_rate,
      paths: sel_paths,
      peer_addrs: self.ctrl_addrs_encoded(),
      profile_card: self.profile_bytes.clone(),
      range: Some((initial.root_hash, 0, initial.total_blocks)),
      token: opts.token,
      byte_range: None,
    };
    let resp = send_req_recv_resp(&mut stream, &req)?;
    self.ingest_resp_profiles(&resp, true);
    if resp.status != Status::Ok {
      return Err(PeerError::RemoteStatus(resp.status));
    }
    self.ingest_swarm_seeds(share_id, initial.root_hash, &resp.peers);

    Ok(initial)
  }

  /// 加入对等分享备份群组 (`Op::GroupJoin = 6`)
  pub fn join_group(
    self: &Arc<Self>,
    group_name: &str,
    local_dir: PathBuf,
    backup_paths: &[String],
    peers: &[PeerId],
  ) -> Result<(), PeerError> {
    create_dir_all(&local_dir)?;
    let share_id = derive_share_id(group_name);
    let opts = TransferOpts::new().with_paths(backup_paths);
    let clean_paths = opts.resolved_paths();

    self.add_share_with_paths(
      ShareConfig::public_rw(group_name, &local_dir, 0),
      clean_paths.clone(),
    );

    let remote_peers: Vec<PeerId> = peers
      .iter()
      .copied()
      .filter(|&pid| pid != self.endpoint_id)
      .collect();
    let sync_rep = self
      .swarm_pull(&remote_peers, group_name, &local_dir, opts)
      .ok();

    let range = sync_rep
      .as_ref()
      .map(|r| (r.root_hash, 0, r.total_blocks))
      .or_else(|| {
        build_manifest_from_paths(&local_dir, &clean_paths, false)
          .ok()
          .and_then(|mf| Some((mf.root_hash().ok()?, 0, mf.total_blocks())))
      });

    let my_ctrl_addrs = self.ctrl_addrs_encoded();
    for &peer_id in &remote_peers {
      let req = P2pReq {
        op: Op::GroupJoin,
        flags: FLAG_SEED | FLAG_PARTIAL,
        share_id,
        req_rate: 0,
        paths: clean_paths.clone(),
        peer_addrs: my_ctrl_addrs.clone(),
        profile_card: self.profile_bytes.clone(),
        range,
        token: None,
        byte_range: None,
      };
      if let Ok(mut stream) = self.connect_peer(&peer_id)
        && let Ok(resp) = send_req_recv_resp(&mut stream, &req)
      {
        self.ingest_resp_profiles(&resp, true);
        if let Some((rh, ..)) = range {
          self.ingest_swarm_seeds(share_id, rh, &resp.peers);
        }
      }
    }
    Ok(())
  }

  /// 当本节点某个分享组目录发生文件新增/修改时，主动向所有订阅节点与群组节点推送更新 (`SubPush`)
  pub fn publish_updates(&self, share_name: &str) -> Result<usize, PeerError> {
    let share_id = derive_share_id(share_name);
    let (root_dir, subs, seeds) = {
      let guard = self.shares.read();
      let Some(st) = guard.get(&share_id) else {
        return Ok(0);
      };
      (
        st.cfg.root.clone(),
        st.subscribers.clone(),
        st.seeds.clone(),
      )
    };

    if subs.is_empty() {
      return Ok(0);
    }

    let base_mf = build_manifest_from_paths::<&str>(&root_dir, &[], false)?;
    if let Some(st) = self.shares.write().get_mut(&share_id) {
      if let Ok(rh) = base_mf.root_hash() {
        st.cached_manifest = Some((rh, base_mf.clone()));
      } else {
        st.cached_manifest = None;
      }
    }
    let origin_addrs = self.ctrl_addrs_encoded();
    let mut pushed = 0usize;

    for sub in subs {
      let filtered = if sub.paths.is_empty() {
        base_mf.clone()
      } else {
        base_mf.filter_paths(&sub.paths)
      };
      if !sub.paths.is_empty() && filtered.entries.is_empty() {
        continue;
      }
      let rh = filtered.root_hash().map_err(FsError::Cdc)?;
      let mf_bytes = filtered.encode().map_err(FsError::Cdc)?;

      let swarm_peers = build_swarm_peers(
        (
          self.endpoint_id,
          self.profile.region,
          &origin_addrs,
          &self.profile_bytes,
        ),
        (sub.eff_rate, filtered.total_blocks(), rh),
        &seeds,
        sub.endpoint_id,
      );

      let push_ev = SubPush {
        flags: FLAG_PARTIAL,
        share_id,
        eff_rate: sub.eff_rate,
        root_hash: rh,
        origin_addrs: origin_addrs.clone(),
        peers: swarm_peers,
        manifest: mf_bytes,
      };
      let mut target_addrs = resolve_iroh_endpoints(&sub.endpoint_id);
      if !target_addrs.contains(&sub.ctrl_addr) {
        target_addrs.push(sub.ctrl_addr);
      }
      if let Ok(mut stream) = connect_ctrl_addrs(&target_addrs)
        && write_frame(&mut stream, &push_ev.encode()?).is_ok()
        && read_frame(&mut stream).is_ok()
      {
        pushed += 1;
        continue;
      }
      if let Some(st) = self.shares.write().get_mut(&share_id) {
        st.subscribers
          .retain(|s| s.ctrl_addr != sub.ctrl_addr && s.endpoint_id != sub.endpoint_id);
        st.seeds
          .retain(|s| s.ctrl_addr != sub.ctrl_addr && s.endpoint_id != sub.endpoint_id);
      }
    }
    Ok(pushed)
  }
}

pub(crate) fn handle_incoming_sub_push(
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
  let sink = Arc::new(SharedSink::prepare(&local_root, mf)?);
  let total = sink.total_blocks();

  if let Some(st) = ctx.shares.write().get_mut(&push.share_id) {
    st.active_sink = Some((push.root_hash, Arc::clone(&sink)));
    st.cached_manifest = None;
  }

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
          byte_range: None,
          req_rate: push.eff_rate,
          token: None,
          my_card: &ctx.my_card,
          sink: &sink,
        },
        &reused_conns,
      );
    }
  }
  let commit_res = sink.commit();
  if let Some(st) = ctx.shares.write().get_mut(&push.share_id) {
    st.active_sink = None;
  }
  commit_res?;

  if let Ok(origin_addrs) = decode_socket_addrs(&push.origin_addrs)
    && !origin_addrs.is_empty()
  {
    let have_req = P2pReq {
      op: Op::Have,
      flags: FLAG_SEED,
      share_id: push.share_id,
      req_rate: 0,
      paths: Vec::new(),
      peer_addrs: ctx.my_ctrl_addrs.clone(),
      profile_card: ctx.my_card.clone(),
      range: Some((push.root_hash, 0, total)),
      token: None,
      byte_range: None,
    };
    if let Ok(mut s) = connect_ctrl_addrs(&origin_addrs) {
      let _ = send_req_recv_resp(&mut s, &have_req);
    }
  }

  write_frame(stream, &[0u8])?;
  Ok(())
}
