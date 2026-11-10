use std::{fs::create_dir_all, iter::once, path::PathBuf, sync::Arc};

use gxhash::HashMap;
use parking_lot::Mutex;
use s0_cdc::{Manifest, PathChange, matches_any_selected};
use s0_const::{
  FLAG_AUTH, FLAG_PARTIAL, FLAG_SEED, FsOp, HASH_LEN, Op, Perm, ZERO_PEER_ID, region::Region,
};
use s0_fs::{
  SharedSink, apply_file_metas, build_manifest_from_paths, build_manifest_incremental,
  remove_tree_and_empty_parents, sanitize_rel_path, scan_files,
};
use s0_udp::now_us;
use s0_wire::{P2pReq, SubPush, SwarmPeer, decode_socket_addrs};

use crate::{
  PeerError, PeerId, PeerNode, ShareConfig, SyncReport, TransferOpts, derive_share_id,
  frame::send_req_recv_resp,
  pull::{RangePullJob, connect_ctrl_addrs, execute_striped_pull},
  register_encoded_endpoints, register_iroh_endpoints,
  server::ServerCtx,
  watch::{SubscriberRecord, SwarmSeedRecord},
};

#[inline]
pub(crate) fn path_matches_sel(path: &str, sels: &[String]) -> bool {
  sels.is_empty() || matches_any_selected(path, sels)
}

impl PeerNode {
  /// 长连接订阅远端对等节点的分享组 (`Op::Sub = 4`)
  pub fn subscribe(
    self: &Arc<Self>,
    origin_id: PeerId,
    share_name: &str,
    local_dir: PathBuf,
    opts: TransferOpts,
  ) -> Result<SyncReport, PeerError> {
    let share_id = derive_share_id(share_name);
    let sel_paths = opts.paths.clone();
    create_dir_all(&local_dir)?;

    let initial = self.pull(
      origin_id,
      share_name,
      &local_dir,
      opts.clone().with_delete(sel_paths.is_empty()),
    )?;

    let mut stream = self.connect_peer(&origin_id)?;
    let req = P2pReq {
      req_rate: opts.req_rate,
      paths: sel_paths.clone(),
      range: Some((initial.root_hash, 0, initial.total_blocks)),
      token: opts.token,
      ..P2pReq::new(
        Op::Sub,
        FLAG_SEED,
        share_id,
        self.ctrl_addrs_encoded(),
        self.profile_bytes.clone(),
      )
    };
    let resp = self.rpc_ok(&mut stream, &req)?;

    let mut share_cfg = ShareConfig::public_r(share_name, &local_dir, 0);
    if resp.flags & FLAG_AUTH != 0 {
      share_cfg.r_token = opts.token;
      share_cfg.grant_peers_default(
        [origin_id, self.endpoint_id]
          .into_iter()
          .chain(resp.peers.iter().map(|sp| sp.endpoint_id)),
        Perm::R,
      );
    }
    self.add_share_with_paths(share_cfg, sel_paths);
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
    let clean_paths = opts.paths.clone();

    let mut group_cfg = ShareConfig::public_rw(group_name, &local_dir, 0);
    if !peers.is_empty() {
      group_cfg.grant_peers_default(
        once(self.endpoint_id).chain(peers.iter().copied()),
        Perm::Rw,
      );
    }
    self.add_share_with_paths(group_cfg, clean_paths.clone());

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
        paths: clean_paths.clone(),
        range,
        ..P2pReq::new(
          Op::GroupJoin,
          FLAG_SEED | FLAG_PARTIAL,
          share_id,
          my_ctrl_addrs.clone(),
          self.profile_bytes.clone(),
        )
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

  /// 当本节点某个分享组目录发生文件新增/修改/删除时，主动向所有订阅节点与群组节点推送路径变更通知 (`SubPush`)
  pub fn publish_updates(&self, share_name: &str) -> Result<usize, PeerError> {
    self.ctx.publish_share_id(derive_share_id(share_name))
  }
}

/// 处理通过 `iroh`（或 TCP 备用通道）收到的 `SubPush` 增量变更帧：
/// 直接使用内联 `ChunkSig` 执行本地 FastCDC 块级增量比对（`match_basis_file` + `in_place`），仅通过自定义 UDP 协议拉取差异块，并在内存中原位合并 `Manifest`
pub(crate) fn handle_incoming_sub_push_frame(
  frame: &[u8],
  ctx: &ServerCtx,
) -> Result<(), PeerError> {
  let push = SubPush::decode(frame)?;
  for p in &push.peers {
    if p.endpoint_id != ZERO_PEER_ID && !p.peer_addrs.is_empty() {
      register_encoded_endpoints(p.endpoint_id, &p.peer_addrs);
    }
  }

  let (local_root, sub_paths, r_token) = {
    let mut guard = ctx.shares.write();
    let Some(st) = guard.get_mut(&push.share_id) else {
      return Ok(());
    };
    st.cfg.default_perm = push.perm;
    st.cfg.default_write_prefixes = push.write_prefixes;
    if push.flags & FLAG_AUTH != 0 {
      st.cfg
        .grant_peers_default(push.peers.iter().map(|p| p.endpoint_id), Perm::R);
    }
    (st.cfg.root.clone(), st.sub_paths.clone(), st.cfg.r_token)
  };

  let mut upsert_paths: Vec<String> = Vec::new();
  let mut meta_jobs: Vec<(PathBuf, u16, u64)> = Vec::new();
  let mut applied_changes: Vec<PathChange> = Vec::with_capacity(push.changes.len());

  for change in push.changes {
    if !path_matches_sel(&change.path, &sub_paths) {
      continue;
    }
    let Ok(clean_rel) = sanitize_rel_path(&change.path) else {
      continue;
    };
    match change.op {
      FsOp::Remove => {
        let _ = remove_tree_and_empty_parents(&local_root, &clean_rel);
        applied_changes.push(PathChange::remove(clean_rel));
      }
      FsOp::Meta => {
        meta_jobs.push((local_root.join(&clean_rel), change.mode, change.mtime));
        applied_changes.push(PathChange::meta(clean_rel, change.mode, change.mtime));
      }
      FsOp::Upsert => {
        upsert_paths.push(clean_rel.clone());
        applied_changes.push(PathChange::upsert_meta(
          clean_rel,
          change.size,
          change.mode,
          change.mtime,
          change.chunks,
        ));
      }
    }
  }

  if !meta_jobs.is_empty() {
    apply_file_metas(&meta_jobs);
  }

  upsert_paths.sort_unstable();
  upsert_paths.dedup();

  let origin_addrs = decode_socket_addrs(&push.origin_addrs).unwrap_or_default();

  if !upsert_paths.is_empty() {
    let delta_mf = Manifest::from_upsert_changes(&applied_changes);
    let delta_rh = delta_mf.root_hash()?;
    let sink = Arc::new(SharedSink::prepare(&local_root, delta_mf)?);
    let total = sink.total_blocks();

    if let Some(st) = ctx.shares.write().get_mut(&push.share_id) {
      st.active_sink = Some((delta_rh, Arc::clone(&sink)));
    }

    if total > 0 && !sink.is_range_complete(0, total) {
      let candidates: Vec<SwarmPeer> = push
        .peers
        .into_iter()
        .filter(|p| p.blk_hi > p.blk_lo && p.endpoint_id != ctx.my_id && !p.peer_addrs.is_empty())
        .map(|mut p| {
          p.blk_lo = 0;
          p.blk_hi = total;
          p
        })
        .collect();
      if let Some(first_peer) = candidates.first().cloned() {
        let reused_conns = Mutex::new(HashMap::default());
        let _ = execute_striped_pull(
          ctx.my_region,
          candidates,
          RangePullJob {
            peer: &first_peer,
            share_id: push.share_id,
            paths: &upsert_paths,
            root_hash: delta_rh,
            blk_lo: 0,
            blk_hi: total,
            byte_range: None,
            req_rate: push.eff_rate,
            token: r_token,
            my_card: &ctx.my_card,
            sink: &sink,
          },
          &reused_conns,
          None,
        );
      }
    }

    let commit_res = sink.commit();
    if let Some(st) = ctx.shares.write().get_mut(&push.share_id) {
      st.active_sink = None;
    }
    commit_res?;
  }

  let mut have_total = 0u32;
  if let Some(st) = ctx.shares.write().get_mut(&push.share_id) {
    let mut base_mf = st
      .baseline_manifest
      .take()
      .or_else(|| st.cached_manifest.as_ref().map(|(_, _, m)| m.clone()))
      .unwrap_or_default();
    base_mf.apply_changes(&applied_changes);
    have_total = if sub_paths.is_empty() {
      base_mf.total_blocks()
    } else {
      base_mf.filter_paths(&sub_paths).total_blocks()
    };
    st.cached_manifest = base_mf
      .root_hash()
      .ok()
      .map(|rh| (Vec::new(), rh, base_mf.clone()));
    st.set_scan_baseline(Some(base_mf), Vec::new(), now_us());
  }

  if !origin_addrs.is_empty() && have_total > 0 {
    let have_req = P2pReq {
      range: Some((push.root_hash, 0, have_total)),
      ..P2pReq::new(
        Op::Have,
        FLAG_SEED,
        push.share_id,
        ctx.my_ctrl_addrs.clone(),
        ctx.my_card.clone(),
      )
    };
    if let Ok(mut s) = connect_ctrl_addrs(&origin_addrs) {
      let _ = send_req_recv_resp(&mut s, &have_req);
    }
  }

  Ok(())
}

pub(crate) fn register_sub_or_have(
  ctx: &ServerCtx,
  share_cfg: &ShareConfig,
  req: P2pReq,
  (remote_id, remote_region, eff_rate): (PeerId, Region, u64),
  cached_mf: Option<(Vec<String>, [u8; HASH_LEN], Manifest)>,
) -> Result<Vec<SwarmPeer>, PeerError> {
  let is_sub = matches!(req.op, Op::Sub | Op::GroupJoin);
  let peer_ctrl_addrs = decode_socket_addrs(&req.peer_addrs)?;
  let mut swarm_peers = Vec::new();
  if let Some(&peer_ctrl) = peer_ctrl_addrs.first()
    && (is_sub || req.range.is_some())
  {
    if remote_id != ZERO_PEER_ID {
      register_iroh_endpoints(remote_id, &peer_ctrl_addrs);
    }
    let init_base = if is_sub {
      if let Some((cp, _, cmf)) = cached_mf
        && cp.is_empty()
      {
        Some((cmf, scan_files(&share_cfg.root).unwrap_or_default()))
      } else {
        build_manifest_incremental(&share_cfg.root, None).ok()
      }
    } else {
      None
    };
    if let Some(st) = ctx.shares.write().get_mut(&req.share_id) {
      if is_sub {
        if st.baseline_manifest.is_none()
          && let Some((bmf, bscan)) = init_base
        {
          st.set_scan_baseline(Some(bmf), bscan, now_us());
        }
        let sub_rec = SubscriberRecord {
          endpoint_id: remote_id,
          ctrl_addrs: peer_ctrl_addrs,
          paths: req.paths,
          eff_rate,
        };
        st.upsert_subscriber(sub_rec.clone());
        st.persist_sub_upsert(&sub_rec);
      }
      if let Some((rh, lo, hi)) = req.range {
        if hi > lo {
          st.upsert_seed(SwarmSeedRecord::new(
            remote_id,
            remote_region,
            peer_ctrl,
            req.profile_card,
            rh,
            (lo, hi),
            eff_rate,
          ));
        }
        swarm_peers = ctx.swarm_peers(share_cfg, eff_rate, hi, rh, &st.seeds, remote_id);
      }
    }
  }
  Ok(swarm_peers)
}
