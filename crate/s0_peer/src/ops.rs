use std::{
  net::TcpStream,
  path::Path,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  thread,
  time::{Duration, Instant},
};

use gxhash::HashMap;
use parking_lot::Mutex;
use s0_cdc::{decode_roaring_bits, is_roaring_full_frame};
use s0_const::{
  FLAG_AUTH, FLAG_DELETE, FLAG_PARTIAL, FLAG_SEED, HASH_LEN, LEECH_MIN_BLOCKS, Op, Perm, Status,
  ZERO_PEER_ID, macro_piece_blocks,
};
use s0_fs::{SharedSink, SourceReader, build_manifest_from_paths, sanitize_rel_path};
use s0_udp::{RangeSpec, bind_udp_for_ip, punch_udp_with_peer_ip, send_range_udp};
use s0_wire::{P2pReq, P2pResp, SwarmPeer, encode_socket_addrs};

use crate::{
  PeerError, PeerId, PeerNode, ShareConfig, SyncReport, TransferOpts, derive_share_id,
  frame::{read_frame, send_req_recv_resp, write_two_frames},
  pull::{RangePullJob, derive_session_id, execute_striped_pull, probe_ctrl_addrs},
  resolve_iroh_endpoints,
  swarm::{FALLBACK_RTT_US, merge_swarm_candidates, rtt_weighted_rate},
  watch::ShareState,
};

const LEECH_REPORT_INTERVAL: Duration = Duration::from_millis(30);
const PUSH_UDP_TIMEOUT: Duration = Duration::from_secs(10);

#[inline]
const fn noop_report(root_hash: [u8; HASH_LEN], total_blocks: u32) -> SyncReport {
  SyncReport {
    root_hash,
    total_blocks,
    blocks_transferred: 0,
    bytes_transferred: 0,
    peers_used: 0,
  }
}

impl PeerNode {
  /// 从指定 `PeerId` 或其返回的 P2P Swarm 节点群拉取全部、多选或指定字节区间 `[byte_lo, byte_hi)` (`Op::Pull = 1`)
  pub fn pull(
    &self,
    peer_id: PeerId,
    share_name: &str,
    local_dst: &Path,
    opts: TransferOpts,
  ) -> Result<SyncReport, PeerError> {
    self.swarm_pull(&[peer_id], share_name, local_dst, opts)
  }

  /// 多源并发拉取 (`swarm_pull`)：
  /// 支持全量/多选同步、崩溃 `.s0_stage` 断点续传以及 `with_byte_range` 点播首屏优先拉取
  pub fn swarm_pull(
    &self,
    seed_ids: &[PeerId],
    share_name: &str,
    local_dst: &Path,
    opts: TransferOpts,
  ) -> Result<SyncReport, PeerError> {
    if seed_ids.is_empty() {
      return Err(PeerError::NoHealthyPeer);
    }

    let share_id = derive_share_id(share_name);
    let mut candidate_peers: Vec<SwarmPeer> = Vec::new();
    let reused_conns: Mutex<HashMap<PeerId, TcpStream>> = Mutex::new(HashMap::default());
    let mut rtt_map: HashMap<PeerId, u64> = HashMap::default();
    let mut stat_ok = None;
    let mut last_err: Option<PeerError> = None;
    let mut origin_pid = seed_ids[0];
    let primary_pid = seed_ids[0];

    let opts_ref = &opts;
    thread::scope(|s| {
      let handles: Vec<_> = seed_ids
        .iter()
        .map(|&pid| {
          s.spawn(move || {
            let t0 = Instant::now();
            let Ok(stream) = self.connect_peer(&pid) else {
              return (pid, 0u64, None, None, None);
            };
            let rtt_us = (t0.elapsed().as_micros() as u64).max(1);
            if pid == primary_pid {
              match self.stat_on_stream(stream, share_name, opts_ref) {
                Ok(res) => return (pid, rtt_us, Some(res), None, None),
                Err(e) => return (pid, rtt_us, None, None, Some(e)),
              }
            }
            (pid, rtt_us, None, Some(stream), None)
          })
        })
        .collect();
      for h in handles {
        let Ok((pid, rtt_us, stat_res, stream_opt, err_opt)) = h.join() else {
          continue;
        };
        if rtt_us > 0 {
          rtt_map.insert(pid, rtt_us);
        }
        if let Some(e) = err_opt {
          last_err = Some(e);
        }
        if let Some((resp, mf, stream)) = stat_res {
          reused_conns.lock().insert(pid, stream);
          origin_pid = pid;
          stat_ok = Some((resp, mf));
        } else if let Some(stream) = stream_opt {
          reused_conns.lock().insert(pid, stream);
        }
      }
    });

    if stat_ok.is_none() {
      for &pid in seed_ids {
        let stream_opt = reused_conns.lock().remove(&pid);
        let res = match stream_opt {
          Some(s) => self.stat_on_stream(s, share_name, &opts),
          None => self.stat_with_stream(pid, share_name, &opts),
        };
        match res {
          Ok((resp, mf, stream)) => {
            reused_conns.lock().insert(pid, stream);
            rtt_map.entry(pid).or_insert(FALLBACK_RTT_US);
            origin_pid = pid;
            stat_ok = Some((resp, mf));
            break;
          }
          Err(e) => last_err = Some(e),
        }
      }
    }

    let Some((resp, mut mf)) = stat_ok else {
      return Err(last_err.unwrap_or(PeerError::NoHealthyPeer));
    };
    let root_hash = resp.root_hash;
    let resp_flags = resp.flags;
    let total_blks = mf.total_blocks();
    let mut acl_peers = vec![origin_pid, self.endpoint_id];
    for sp in &resp.peers {
      if sp.endpoint_id != ZERO_PEER_ID && !acl_peers.contains(&sp.endpoint_id) {
        acl_peers.push(sp.endpoint_id);
      }
    }
    merge_swarm_candidates(&mut candidate_peers, resp.peers, self.endpoint_id);

    for &pid in seed_ids {
      if pid != self.endpoint_id
        && reused_conns.lock().contains_key(&pid)
        && !candidate_peers.iter().any(|p| p.endpoint_id == pid)
      {
        let addrs = resolve_iroh_endpoints(&pid);
        if !addrs.is_empty() {
          let region = self
            .get_peer_profile(&pid)
            .map_or(self.profile.region, |p| p.region);
          candidate_peers.push(SwarmPeer {
            endpoint_id: pid,
            region,
            weight_rate: resp.eff_rate,
            blk_lo: 0,
            blk_hi: total_blks,
            peer_addrs: encode_socket_addrs(&addrs),
            profile_card: Vec::new(),
          });
        }
      }
    }

    if total_blks > 0 {
      let unprobed: Vec<PeerId> = candidate_peers
        .iter()
        .map(|p| p.endpoint_id)
        .filter(|id| *id != ZERO_PEER_ID && !rtt_map.contains_key(id))
        .collect();
      if !unprobed.is_empty() {
        thread::scope(|s| {
          let handles: Vec<_> = unprobed
            .iter()
            .map(|&pid| {
              s.spawn(move || {
                let addrs = resolve_iroh_endpoints(&pid);
                (!addrs.is_empty()).then(|| {
                  let t0 = Instant::now();
                  let stream = probe_ctrl_addrs(&addrs).ok()?;
                  Some((pid, (t0.elapsed().as_micros() as u64).max(1), stream))
                })?
              })
            })
            .collect();
          for h in handles {
            if let Ok(Some((pid, rtt_us, stream))) = h.join() {
              rtt_map.insert(pid, rtt_us);
              reused_conns.lock().insert(pid, stream);
            }
          }
        });
      }
      if candidate_peers
        .iter()
        .any(|p| rtt_map.contains_key(&p.endpoint_id))
      {
        candidate_peers.retain(|p| rtt_map.contains_key(&p.endpoint_id));
      }
    }

    for sp in &mut candidate_peers {
      if let Some(&rtt_us) = rtt_map.get(&sp.endpoint_id) {
        sp.weight_rate = rtt_weighted_rate(sp.weight_rate, rtt_us);
      }
    }

    let sel_paths = opts.paths;
    let is_full_share = sel_paths.is_empty() && opts.byte_range.is_none();
    mf.delete_extraneous = opts.delete_extraneous && is_full_share;

    if is_full_share
      && self
        .shares
        .read()
        .get(&share_id)
        .filter(|st| st.cfg.root == local_dst)
        .and_then(|st| st.cached_manifest.as_ref())
        .is_some_and(|(cp, cached_rh, _)| cp.is_empty() && *cached_rh == root_hash)
    {
      return Ok(noop_report(root_hash, total_blks));
    }

    let sink = Arc::new(SharedSink::prepare(local_dst, mf)?);
    let total_blocks = sink.total_blocks();
    let (req_lo, req_hi) = opts.byte_range.map_or((0, total_blocks), |(b_lo, b_hi)| {
      sink.manifest().locate_byte_range(b_lo, b_hi)
    });
    let is_full_sync = is_full_share && req_lo == 0 && req_hi == total_blocks;

    if total_blocks == 0 || req_lo >= req_hi || sink.is_range_complete(req_lo, req_hi) {
      if opts.byte_range.is_some() {
        sink.commit_range(req_lo, req_hi)?;
      } else {
        sink.commit()?;
      }
      if is_full_sync && let Some(st) = self.shares.write().get_mut(&share_id) {
        st.cached_manifest = Some((Vec::new(), root_hash, sink.manifest().clone()));
      }
      return Ok(noop_report(root_hash, total_blocks));
    }

    {
      let mut guard = self.shares.write();
      let st = guard.entry(share_id).or_insert_with(|| {
        let mut cfg = ShareConfig::public_r(share_name, local_dst, 0);
        if resp_flags & FLAG_AUTH != 0 {
          cfg.r_token = opts.token;
          cfg.grant_peers_default(acl_peers, Perm::R);
        }
        ShareState::new(cfg, sel_paths.clone())
      });
      st.cfg.root = local_dst.to_path_buf();
      st.active_sink = Some((root_hash, Arc::clone(&sink)));
      st.cached_manifest = None;
    }

    let leech_done = Arc::new(AtomicBool::new(false));
    let dyn_peers: Arc<Mutex<Vec<SwarmPeer>>> = Arc::new(Mutex::new(Vec::new()));
    if is_full_share && total_blocks >= LEECH_MIN_BLOCKS {
      let (leech_flag, sink_watch, dyn_ref) = (
        Arc::clone(&leech_done),
        Arc::clone(&sink),
        Arc::clone(&dyn_peers),
      );
      let (my_id, my_ctrl_addrs, my_card) = (
        self.endpoint_id,
        self.ctrl_addrs_encoded(),
        self.profile_bytes.clone(),
      );
      let (req_rate, tok) = (opts.req_rate, opts.token);
      let macro_step = macro_piece_blocks(total_blocks);
      thread::spawn(move || {
        let (mut reported_span, mut conn_opt) = (0u32, None);
        while !leech_flag.load(Ordering::Acquire) {
          thread::sleep(LEECH_REPORT_INTERVAL);
          if leech_flag.load(Ordering::Acquire) {
            break;
          }
          let (best_lo, best_hi) = sink_watch.longest_have_span();
          let span = best_hi.saturating_sub(best_lo);
          if span >= reported_span + macro_step
            || (span == total_blocks && reported_span < total_blocks)
          {
            reported_span = span;
            if conn_opt.is_none() {
              conn_opt = probe_ctrl_addrs(&resolve_iroh_endpoints(&origin_pid)).ok();
            }
            if let Some(s) = conn_opt.as_mut() {
              let have_req = P2pReq {
                req_rate,
                range: Some((root_hash, best_lo, best_hi)),
                token: tok,
                ..P2pReq::new(
                  Op::Have,
                  FLAG_SEED,
                  share_id,
                  my_ctrl_addrs.clone(),
                  my_card.clone(),
                )
              };
              if let Ok(have_resp) = send_req_recv_resp(s, &have_req) {
                merge_swarm_candidates(&mut dyn_ref.lock(), have_resp.peers, my_id);
              } else {
                conn_opt = None;
              }
            }
          }
        }
      });
    }

    let first_peer = candidate_peers
      .first()
      .cloned()
      .ok_or(PeerError::NoHealthyPeer)?;
    let pull_res = execute_striped_pull(
      self.profile.region,
      candidate_peers,
      RangePullJob {
        peer: &first_peer,
        share_id,
        paths: &sel_paths,
        root_hash,
        blk_lo: req_lo,
        blk_hi: req_hi,
        byte_range: opts.byte_range,
        req_rate: opts.req_rate,
        token: opts.token,
        my_card: &self.profile_bytes,
        sink: &sink,
      },
      &reused_conns,
      Some(&dyn_peers),
    );

    leech_done.store(true, Ordering::Release);

    let commit_res = if pull_res.is_ok() {
      if opts.byte_range.is_some() || req_lo > 0 || req_hi < total_blocks {
        sink.commit_range(req_lo, req_hi).map_err(PeerError::Fs)
      } else {
        sink.commit().map_err(PeerError::Fs)
      }
    } else {
      Ok(())
    };

    if let Some(st) = self.shares.write().get_mut(&share_id) {
      st.active_sink = None;
      if pull_res.is_ok() && commit_res.is_ok() && is_full_sync {
        st.cached_manifest = Some((Vec::new(), root_hash, sink.manifest().clone()));
      }
    }

    let (blocks_transferred, bytes_transferred, peers_used) = pull_res?;
    commit_res?;

    Ok(SyncReport {
      root_hash,
      total_blocks,
      blocks_transferred,
      bytes_transferred,
      peers_used,
    })
  }

  /// 向拥有写权限的对等节点上传文件、目录或多选路径集合 (`Op::Push = 2`)
  pub fn push(
    &self,
    peer_id: PeerId,
    share_name: &str,
    local_src: &Path,
    opts: TransferOpts,
  ) -> Result<SyncReport, PeerError> {
    let share_id = derive_share_id(share_name);
    let sel_paths = opts.paths;
    for p in &sel_paths {
      sanitize_rel_path(p)?;
    }
    let mf = build_manifest_from_paths(
      local_src,
      &sel_paths,
      opts.delete_extraneous && sel_paths.is_empty(),
    )?;
    let root_hash = mf.root_hash()?;
    let mf_bytes = mf.encode()?;
    let reader = SourceReader::open(local_src, mf)?;
    let total_blocks = reader.total_blocks();

    let mut stream = self.connect_peer(&peer_id)?;
    let (udp_sock, my_udp_addrs) = bind_udp_for_ip(stream.local_addr()?.ip())?;
    let mut flags = if opts.delete_extraneous {
      FLAG_DELETE
    } else {
      0
    };
    if !sel_paths.is_empty() {
      flags |= FLAG_PARTIAL;
    }

    let req = P2pReq {
      req_rate: opts.req_rate,
      paths: sel_paths,
      range: Some((root_hash, 0, total_blocks)),
      token: opts.token,
      ..P2pReq::new(
        Op::Push,
        flags,
        share_id,
        my_udp_addrs,
        self.profile_bytes.clone(),
      )
    };

    write_two_frames(&mut stream, &req.encode()?, &mf_bytes)?;
    let resp = P2pResp::decode(&read_frame(&mut stream)?)?;
    self.ingest_resp_profiles(&resp, false);
    if resp.status != Status::Ok {
      return Err(PeerError::RemoteStatus(resp.status));
    }

    let have_frame = read_frame(&mut stream)?;
    let session_id = derive_session_id(&root_hash, 0);
    let (mut blocks_transferred, mut bytes_transferred) = (0u32, 0u64);

    if total_blocks > 0 && !is_roaring_full_frame(&have_frame) {
      let have_bits = decode_roaring_bits(&have_frame, total_blocks)?;
      let peer_ip = stream.peer_addr().ok().map(|a| a.ip());
      let peer_udp_addrs =
        punch_udp_with_peer_ip(&udp_sock, &resp.peer_addrs, peer_ip, session_id)?;
      let st = send_range_udp(
        &udp_sock,
        &peer_udp_addrs,
        &reader,
        &have_bits,
        RangeSpec {
          session_id,
          blk_lo: 0,
          blk_hi: total_blocks,
          eff_rate: resp.eff_rate,
          timeout: PUSH_UDP_TIMEOUT,
        },
      )?;
      (blocks_transferred, bytes_transferred) = (st.blocks_transferred, st.bytes_transferred);
    }

    let done_resp = P2pResp::decode(&read_frame(&mut stream)?)?;
    if done_resp.status != Status::Ok {
      return Err(PeerError::RemoteStatus(done_resp.status));
    }

    Ok(SyncReport {
      root_hash,
      total_blocks,
      blocks_transferred,
      bytes_transferred,
      peers_used: 1,
    })
  }
}
