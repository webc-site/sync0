use std::{
  net::TcpStream,
  path::Path,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  thread,
  time::Duration,
};

use gxhash::HashMap;
use parking_lot::Mutex;
use s0_cdc::{FLAG_DELETE, FLAG_PARTIAL};

use super::{
  PeerError, PeerId, PeerNode, ShareConfig, SyncReport, TransferOpts, derive_share_id,
  pull::{
    RangePullJob, derive_push_session_id, execute_striped_pull, is_have_all_ones_frame, read_frame,
    write_frame, write_two_frames,
  },
  register_iroh_endpoint, resolve_iroh_endpoint,
  server::ShareState,
};
use crate::{
  RangeSpec, bind_udp_for_ip,
  consts::{FLAG_SEED, Op, Status},
  fs::{FsError, SharedSink, SourceReader, build_manifest_from_paths, sanitize_rel_path},
  punch_udp, send_range_udp,
  wire::{P2pReq, P2pResp, SwarmPeer, decode_socket_addrs, encode_socket_addrs},
};

impl PeerNode {
  /// 从指定 `PeerId` 或其返回的 P2P Swarm 节点群拉取全部或多选的文件/目录 (`Op::Pull = 1`)
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
  /// 通过二进制 `PeerId` 连接种子节点并自动通过 PEX 发现组内所有做种节点，按地域亲和度与带宽权重条带化并发拉取 `[blk_lo, blk_hi)`，
  /// 下载过程中实时在后台汇报已校验连续块进展（Leecher Seeding 边下边传）。
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

    let (first_res, extra_conns) = if seed_ids.len() == 1 {
      (
        self.stat_with_stream(seed_ids[0], share_name, &opts),
        Vec::new(),
      )
    } else {
      thread::scope(|s| {
        let first_pid = seed_ids[0];
        let opts_ref = &opts;
        let h_first = s.spawn(move || self.stat_with_stream(first_pid, share_name, opts_ref));
        let h_rest: Vec<_> = seed_ids[1..]
          .iter()
          .map(|&pid| s.spawn(move || (pid, self.connect_peer(&pid))))
          .collect();
        let first = h_first.join().unwrap_or(Err(PeerError::NoHealthyPeer));
        let rest: Vec<_> = h_rest.into_iter().filter_map(|h| h.join().ok()).collect();
        (first, rest)
      })
    };

    let mut stat_ok = None;
    let mut origin_pid = seed_ids[0];
    if let Ok((resp, mf, stream)) = first_res {
      reused_conns.lock().insert(seed_ids[0], stream);
      stat_ok = Some((resp, mf));
    }

    for (pid, conn_res) in extra_conns {
      if stat_ok.is_none() {
        if let Ok((resp, mf, stream)) = self.stat_with_stream(pid, share_name, &opts) {
          reused_conns.lock().insert(pid, stream);
          origin_pid = pid;
          stat_ok = Some((resp, mf));
        }
      } else if let Ok(stream) = conn_res {
        reused_conns.lock().insert(pid, stream);
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
        && let Some(&first) = addrs.first()
      {
        register_iroh_endpoint(sp.endpoint_id, first);
      }
      if sp.endpoint_id != self.endpoint_id
        && !candidate_peers
          .iter()
          .any(|p| p.endpoint_id == sp.endpoint_id && sp.endpoint_id != [0u8; 32])
      {
        candidate_peers.push(sp);
      }
    }

    for &pid in &seed_ids[1..] {
      if pid != self.endpoint_id
        && reused_conns.lock().contains_key(&pid)
        && !candidate_peers.iter().any(|p| p.endpoint_id == pid)
        && let Some(addr) = resolve_iroh_endpoint(&pid)
      {
        candidate_peers.push(SwarmPeer {
          endpoint_id: pid,
          region: self.profile.region,
          weight_rate: resp.eff_rate,
          blk_lo: 0,
          blk_hi: total_blks,
          peer_addrs: encode_socket_addrs(&[addr]),
          profile_card: Vec::new(),
        });
      }
    }

    if opts.delete_extraneous && sel_paths.is_empty() {
      mf.delete_extraneous = true;
    }

    let sink = Arc::new(SharedSink::prepare(local_dst, mf)?);
    let total_blocks = sink.total_blocks();
    if total_blocks == 0 || sink.is_range_complete(0, total_blocks) {
      sink.commit()?;
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
    if was_registered && sel_paths.is_empty() && total_blocks >= 8 {
      let leech_flag = Arc::clone(&leech_done);
      let sink_watch = Arc::clone(&sink);
      let my_ctrl_addrs = self.ctrl_addrs_encoded();
      let my_card = self.profile_bytes.clone();
      let req_rate = opts.req_rate;
      let tok = opts.token;
      let paths_watch = sel_paths.clone();
      thread::spawn(move || {
        let mut reported_hi = 0u32;
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
            if let Some(addr) = resolve_iroh_endpoint(&origin_pid)
              && let Ok(mut s) = TcpStream::connect_timeout(&addr, Duration::from_millis(200))
            {
              let _ = s.set_nodelay(true);
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
              };
              if let Ok(enc) = have_req.encode() {
                let _ = write_frame(&mut s, &enc);
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
        blk_lo: 0,
        blk_hi: total_blocks,
        req_rate: opts.req_rate,
        token: opts.token,
        my_card: &self.profile_bytes,
        sink: &sink,
      },
      &reused_conns,
    );

    leech_done.store(true, Ordering::Release);

    let commit_res = if pull_res.is_ok() {
      sink.commit().map_err(PeerError::Fs)
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
