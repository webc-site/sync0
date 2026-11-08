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
use s0_cdc::{FLAG_DELETE, FLAG_PARTIAL};

use super::{
  PeerError, PeerId, PeerNode, ShareConfig, SyncReport, TransferOpts, derive_share_id,
  pull::{
    RangePullJob, derive_push_session_id, execute_striped_pull, is_have_all_ones_frame,
    probe_ctrl_addrs, read_frame, write_frame, write_two_frames,
  },
  register_iroh_endpoints, resolve_iroh_endpoints,
  server::ShareState,
};
use crate::{
  RangeSpec, bind_udp_for_ip,
  consts::{FLAG_SEED, Op, Status},
  fs::{FsError, SharedSink, SourceReader, build_manifest_from_paths, sanitize_rel_path},
  punch_udp, send_range_udp,
  wire::{P2pReq, P2pResp, SwarmPeer, decode_socket_addrs, encode_socket_addrs},
};

#[inline]
fn rtt_weighted_rate(base_rate: u64, rtt_us: u64) -> u64 {
  let base = if base_rate > 0 {
    base_rate
  } else {
    100_000_000
  };
  base.saturating_mul(100_000) / rtt_us.clamp(2_000, 1_000_000)
}

impl PeerNode {
  /// 从指定 `PeerId` 或其返回的 P2P Swarm 节点群拉取全部、多选或指定字节区间 `[byte_lo, byte_hi)` (`Op::Pull = 1`)
  pub fn pull(
    &self,
    peer_id: PeerId,
    share_name: &str,
    local_dst: &Path,
    opts: TransferOpts<'_>,
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
    opts: TransferOpts<'_>,
  ) -> Result<SyncReport, PeerError> {
    if seed_ids.is_empty() {
      return Err(PeerError::NoHealthyPeer);
    }

    let share_id = derive_share_id(share_name);
    let sel_paths = opts.resolved_paths();
    let mut candidate_peers: Vec<SwarmPeer> = Vec::new();
    let reused_conns: Mutex<HashMap<PeerId, TcpStream>> = Mutex::new(HashMap::default());
    let mut rtt_map: HashMap<PeerId, u64> = HashMap::default();
    let mut stat_ok = None;
    let mut origin_pid = seed_ids[0];

    let stat_claimed = AtomicBool::new(false);
    let opts_ref = &opts;
    let outcomes = thread::scope(|s| {
      let handles: Vec<_> = seed_ids
        .iter()
        .map(|&pid| {
          let claimed = &stat_claimed;
          s.spawn(move || {
            let t0 = Instant::now();
            let Ok(stream) = self.connect_peer(&pid) else {
              return (pid, 0u64, None, None);
            };
            let rtt_us = (t0.elapsed().as_micros() as u64).max(1);
            if !claimed.swap(true, Ordering::AcqRel) {
              match self.stat_on_stream(stream, share_name, opts_ref) {
                Ok(res) => return (pid, rtt_us, Some(res), None),
                Err(_) => claimed.store(false, Ordering::Release),
              }
              return (pid, rtt_us, None, None);
            }
            (pid, rtt_us, None, Some(stream))
          })
        })
        .collect();
      handles
        .into_iter()
        .filter_map(|h| h.join().ok())
        .collect::<Vec<_>>()
    });

    for (pid, rtt_us, stat_res, stream_opt) in outcomes {
      if rtt_us > 0 {
        rtt_map.insert(pid, rtt_us);
      }
      if let Some((resp, mf, stream)) = stat_res {
        reused_conns.lock().insert(pid, stream);
        origin_pid = pid;
        stat_ok = Some((resp, mf));
      } else if let Some(stream) = stream_opt {
        reused_conns.lock().insert(pid, stream);
      }
    }

    if stat_ok.is_none() {
      for &pid in seed_ids {
        let stream_opt = reused_conns.lock().remove(&pid);
        let res = match stream_opt {
          Some(s) => self.stat_on_stream(s, share_name, &opts),
          None => self.stat_with_stream(pid, share_name, &opts),
        };
        if let Ok((resp, mf, stream)) = res {
          reused_conns.lock().insert(pid, stream);
          rtt_map.entry(pid).or_insert(50_000);
          origin_pid = pid;
          stat_ok = Some((resp, mf));
          break;
        }
      }
    }

    let Some((resp, mut mf)) = stat_ok else {
      return Err(PeerError::NoHealthyPeer);
    };
    let root_hash = resp.root_hash;
    let total_blks = mf.total_blocks();

    for sp in resp.peers {
      if sp.endpoint_id != [0u8; 32]
        && let Ok(addrs) = decode_socket_addrs(&sp.peer_addrs)
        && !addrs.is_empty()
      {
        register_iroh_endpoints(sp.endpoint_id, &addrs);
      }
      if sp.endpoint_id != self.endpoint_id
        && !candidate_peers
          .iter()
          .any(|p| p.endpoint_id == sp.endpoint_id && sp.endpoint_id != [0u8; 32])
      {
        candidate_peers.push(sp);
      }
    }

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
        .filter(|id| *id != [0u8; 32] && !rtt_map.contains_key(id))
        .collect();
      if !unprobed.is_empty() {
        let probed = thread::scope(|s| {
          let handles: Vec<_> = unprobed
            .iter()
            .map(|&pid| {
              s.spawn(move || {
                let addrs = resolve_iroh_endpoints(&pid);
                if addrs.is_empty() {
                  return None;
                }
                let t0 = Instant::now();
                let stream = probe_ctrl_addrs(&addrs).ok()?;
                Some((pid, (t0.elapsed().as_micros() as u64).max(1), stream))
              })
            })
            .collect();
          handles
            .into_iter()
            .filter_map(|h| h.join().ok().flatten())
            .collect::<Vec<_>>()
        });
        for (pid, rtt_us, stream) in probed {
          rtt_map.insert(pid, rtt_us);
          reused_conns.lock().insert(pid, stream);
        }
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

    if opts.delete_extraneous && sel_paths.is_empty() && opts.byte_range.is_none() {
      mf.delete_extraneous = true;
    }

    let sink = Arc::new(SharedSink::prepare(local_dst, mf)?);
    let total_blocks = sink.total_blocks();
    let (req_lo, req_hi) = if let Some((b_lo, b_hi)) = opts.byte_range {
      sink.manifest().locate_byte_range(b_lo, b_hi)
    } else {
      (0, total_blocks)
    };

    if total_blocks == 0 || req_lo >= req_hi || sink.is_range_complete(req_lo, req_hi) {
      if opts.byte_range.is_some() {
        sink.commit_range(req_lo, req_hi)?;
      } else {
        sink.commit()?;
      }
      return Ok(SyncReport {
        root_hash,
        total_blocks,
        blocks_transferred: 0,
        bytes_transferred: 0,
        peers_used: 0,
      });
    }

    let was_registered = {
      let mut guard = self.shares.write();
      let existed = guard.contains_key(&share_id);
      let st = guard.entry(share_id).or_insert_with(|| ShareState {
        cfg: ShareConfig::public_ro(share_name, local_dst, 0),
        sub_paths: sel_paths.clone(),
        subscribers: Vec::new(),
        seeds: Vec::new(),
        active_sink: None,
        cached_manifest: None,
      });
      st.active_sink = Some((root_hash, Arc::clone(&sink)));
      st.cached_manifest = None;
      existed
    };

    let leech_done = Arc::new(AtomicBool::new(false));
    if was_registered && sel_paths.is_empty() && opts.byte_range.is_none() && total_blocks >= 8 {
      let leech_flag = Arc::clone(&leech_done);
      let sink_watch = Arc::clone(&sink);
      let my_ctrl_addrs = self.ctrl_addrs_encoded();
      let my_card = self.profile_bytes.clone();
      let req_rate = opts.req_rate;
      let tok = opts.token;
      let paths_watch = sel_paths.clone();
      thread::spawn(move || {
        let mut reported_hi = 0u32;
        let mut conn_opt: Option<TcpStream> = None;
        while !leech_flag.load(Ordering::Acquire) {
          thread::sleep(Duration::from_millis(60));
          if leech_flag.load(Ordering::Acquire) {
            break;
          }
          let mut cur_hi = reported_hi;
          while cur_hi < total_blocks && sink_watch.has_block(cur_hi) {
            cur_hi += 1;
          }
          if cur_hi >= reported_hi + 8 || (cur_hi == total_blocks && cur_hi > reported_hi) {
            reported_hi = cur_hi;
            if conn_opt.is_none() {
              let addrs = resolve_iroh_endpoints(&origin_pid);
              conn_opt = probe_ctrl_addrs(&addrs).ok();
            }
            if let Some(s) = conn_opt.as_mut() {
              let have_req = P2pReq {
                op: Op::Have,
                flags: FLAG_SEED,
                share_id,
                req_rate,
                paths: paths_watch.clone(),
                peer_addrs: my_ctrl_addrs.clone(),
                profile_card: my_card.clone(),
                range: Some((root_hash, 0, cur_hi)),
                token: tok,
                byte_range: None,
              };
              let ok = have_req
                .encode()
                .is_ok_and(|enc| write_frame(s, &enc).is_ok() && read_frame(s).is_ok());
              if !ok {
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
    opts: TransferOpts<'_>,
  ) -> Result<SyncReport, PeerError> {
    let share_id = derive_share_id(share_name);
    let sel_paths = opts.resolved_paths();
    for p in &sel_paths {
      sanitize_rel_path(p)?;
    }
    let mf = build_manifest_from_paths(
      local_src,
      &sel_paths,
      opts.delete_extraneous && sel_paths.is_empty(),
    )?;
    let root_hash = mf.root_hash().map_err(FsError::Cdc)?;
    let mf_bytes = mf.encode().map_err(FsError::Cdc)?;
    let reader = SourceReader::open(local_src, mf)?;
    let total_blocks = reader.total_blocks();

    let mut stream = self.connect_peer(&peer_id)?;
    let (udp_sock, my_udp_addrs) = bind_udp_for_ip(stream.local_addr()?.ip())?;
    let mut flags = 0u8;
    if opts.delete_extraneous {
      flags |= FLAG_DELETE;
    }
    if !sel_paths.is_empty() {
      flags |= FLAG_PARTIAL;
    }

    let req = P2pReq {
      op: Op::Push,
      flags,
      share_id,
      req_rate: opts.req_rate,
      paths: sel_paths,
      peer_addrs: my_udp_addrs,
      profile_card: self.profile_bytes.clone(),
      range: Some((root_hash, 0, total_blocks)),
      token: opts.token,
      byte_range: None,
    };

    let req_bytes = req.encode()?;
    write_two_frames(&mut stream, &req_bytes, &mf_bytes)?;

    let resp_raw = read_frame(&mut stream)?;
    let resp = P2pResp::decode(&resp_raw)?;
    self.ingest_resp_profiles(&resp, false);
    if resp.status != Status::Ok {
      return Err(PeerError::RemoteStatus(resp.status));
    }

    let have_frame = read_frame(&mut stream)?;
    let session_id = derive_push_session_id(&root_hash);
    let mut blocks_transferred = 0u32;
    let mut bytes_transferred = 0u64;

    if total_blocks > 0 && !is_have_all_ones_frame(&have_frame) {
      let peer_udp_addrs = punch_udp(&udp_sock, &resp.peer_addrs, session_id)?;
      let st = send_range_udp(
        &udp_sock,
        &peer_udp_addrs,
        &reader,
        &have_frame,
        RangeSpec {
          session_id,
          blk_lo: 0,
          blk_hi: total_blocks,
          eff_rate: resp.eff_rate,
          timeout: Duration::from_secs(10),
        },
      )?;
      blocks_transferred = st.blocks_transferred;
      bytes_transferred = st.bytes_transferred;
    }

    let done_raw = read_frame(&mut stream)?;
    let done_resp = P2pResp::decode(&done_raw)?;
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
