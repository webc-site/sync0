use std::{
  fs::create_dir_all,
  io,
  net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
  path::{Path, PathBuf},
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  thread::{self, JoinHandle},
  time::Duration,
};

use gxhash::HashMap;
use parking_lot::RwLock;
use s0_cdc::{Manifest, for_each_missing_span};
use s0_const::{FLAG_DELETE, FLAG_PARTIAL, FLAG_SEED, Op, Status};
use s0_fs::{FsError, SharedSink, SourceReader, build_manifest_from_path};
use s0_profile::{ProfileError, SignedProfile};
use s0_udp::{RangeSpec, UdpError, bind_udp_ephemeral, punch_udp, send_range_udp};
use s0_wire::{P2pReq, P2pResp, SubPush, SwarmPeer, WireError, encode_socket_addrs};

use crate::{
  NodeConfig, ShareConfig, derive_share_id, plan_stripes, rank_swarm_peers,
  server::{
    RangePullJob, ServerCtx, ShareState, derive_push_session_id, handle_conn,
    is_have_all_ones_frame, pull_range_from_swarm_peer, read_frame, write_frame,
  },
};

#[derive(Debug, thiserror::Error)]
pub enum PeerError {
  #[error("IO 错误: {0}")]
  Io(#[from] io::Error),
  #[error("线协议错误: {0}")]
  Wire(#[from] WireError),
  #[error("存储错误: {0}")]
  Fs(#[from] FsError),
  #[error("UDP 传输错误: {0}")]
  Udp(#[from] UdpError),
  #[error("节点名片错误: {0}")]
  Profile(#[from] ProfileError),
  #[error("对端拒绝请求，状态码: {0:?}")]
  RemoteStatus(Status),
  #[error("多源快照 root_hash 不一致")]
  SnapshotConflict,
  #[error("无可用对等源节点")]
  NoHealthyPeer,
}

/// 同步/拉取/推送请求参数 (`TransferOpts`)
#[derive(Debug, Default, Clone, Copy)]
pub struct TransferOpts<'a> {
  pub subpath: &'a str,
  pub delete_extraneous: bool,
  pub req_rate: u64,
  pub token: Option<[u8; 16]>,
}

impl<'a> TransferOpts<'a> {
  pub const fn new() -> Self {
    Self {
      subpath: "",
      delete_extraneous: false,
      req_rate: 0,
      token: None,
    }
  }

  pub const fn with_subpath(mut self, subpath: &'a str) -> Self {
    self.subpath = subpath;
    self
  }

  pub const fn with_delete(mut self, delete_extraneous: bool) -> Self {
    self.delete_extraneous = delete_extraneous;
    self
  }

  pub const fn with_rate(mut self, req_rate: u64) -> Self {
    self.req_rate = req_rate;
    self
  }

  pub const fn with_token(mut self, token: Option<[u8; 16]>) -> Self {
    self.token = token;
    self
  }
}

/// 同步/拉取结果统计
#[derive(Debug, Default, Clone)]
pub struct SyncReport {
  pub root_hash: [u8; 16],
  pub total_blocks: u32,
  pub blocks_transferred: u32,
  pub bytes_transferred: u64,
  pub peers_used: usize,
}

/// 全对称 P2P 节点引擎 (`PeerNode`)
pub struct PeerNode {
  endpoint_id: [u8; 32],
  profile: SignedProfile,
  profile_bytes: Vec<u8>,
  ctrl_addr: SocketAddr,
  shares: Arc<RwLock<HashMap<[u8; 16], ShareState>>>,
  known_profiles: Arc<RwLock<HashMap<[u8; 32], SignedProfile>>>,
  stop: Arc<AtomicBool>,
  accept_thread: Option<JoinHandle<()>>,
}

impl PeerNode {
  /// 启动对等节点守护引擎
  pub fn start(cfg: NodeConfig) -> Result<Arc<Self>, PeerError> {
    let secret_key = cfg.key_mode.resolve_secret_key()?;
    let endpoint_id = *secret_key.public().as_bytes();
    let profile = cfg.sign_profile(&secret_key)?;
    let profile_bytes = profile.encode()?;

    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
    let ctrl_addr = listener.local_addr()?;
    listener.set_nonblocking(true)?;

    let shares = Arc::new(RwLock::new(HashMap::default()));
    let mut kp = HashMap::default();
    kp.insert(endpoint_id, profile.clone());
    let known_profiles = Arc::new(RwLock::new(kp));
    let stop = Arc::new(AtomicBool::new(false));

    let srv_ctx = ServerCtx {
      my_id: endpoint_id,
      my_region: profile.region,
      my_card: profile_bytes.clone(),
      my_ctrl_addr: ctrl_addr,
      shares: Arc::clone(&shares),
      known_profiles: Arc::clone(&known_profiles),
    };
    let stop_bg = Arc::clone(&stop);

    let accept_thread = thread::spawn(move || {
      while !stop_bg.load(Ordering::Acquire) {
        match listener.accept() {
          Ok((stream, _peer)) => {
            let _ = stream.set_nodelay(true);
            let ctx = srv_ctx.clone();
            thread::spawn(move || {
              let _ = handle_conn(stream, &ctx);
            });
          }
          Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
            thread::sleep(Duration::from_millis(5));
          }
          Err(_) => break,
        }
      }
    });

    Ok(Arc::new(Self {
      endpoint_id,
      profile,
      profile_bytes,
      ctrl_addr,
      shares,
      known_profiles,
      stop,
      accept_thread: Some(accept_thread),
    }))
  }

  /// 本节点 32B `iroh` `EndpointId` 公钥
  #[inline]
  pub fn endpoint_id(&self) -> [u8; 32] {
    self.endpoint_id
  }

  /// 本节点自签名名片
  #[inline]
  pub fn profile(&self) -> &SignedProfile {
    &self.profile
  }

  /// 本节点控制面监听地址
  #[inline]
  pub fn ctrl_addr(&self) -> SocketAddr {
    self.ctrl_addr
  }

  /// 本节点控制面地址字节编码
  #[inline]
  pub fn ctrl_addrs_encoded(&self) -> Vec<u8> {
    encode_socket_addrs(&[self.ctrl_addr])
  }

  /// 查询已验证的远端对等节点自签名名片
  pub fn get_peer_profile(&self, endpoint_id: &[u8; 32]) -> Option<SignedProfile> {
    self.known_profiles.read().get(endpoint_id).cloned()
  }

  /// 所有已知对等节点名片快照
  pub fn known_profiles(&self) -> Vec<SignedProfile> {
    self.known_profiles.read().values().cloned().collect()
  }

  /// 注册或更新本地分享目录 (`ShareConfig`)
  pub fn add_share(&self, cfg: ShareConfig) {
    self.add_share_with_prefix(cfg, String::new());
  }

  fn add_share_with_prefix(&self, cfg: ShareConfig, sub_prefix: String) {
    let mut guard = self.shares.write();
    if let Some(existing) = guard.get_mut(&cfg.share_id) {
      existing.cfg = cfg;
      existing.sub_prefix = sub_prefix;
    } else {
      guard.insert(
        cfg.share_id,
        ShareState {
          cfg,
          sub_prefix,
          subscribers: Vec::new(),
          seeds: Vec::new(),
        },
      );
    }
  }

  /// 查询远端对等节点的分享状态、清单 `Manifest` 与可用 P2P Swarm 节点列表 (`Op::Stat = 3`)
  pub fn stat(
    &self,
    peer_addr: SocketAddr,
    share_name: &str,
    opts: TransferOpts<'_>,
  ) -> Result<(P2pResp, Manifest), PeerError> {
    let req = P2pReq {
      op: Op::Stat,
      flags: 0,
      share_id: derive_share_id(share_name),
      req_rate: opts.req_rate,
      path: opts.subpath.to_owned(),
      peer_addrs: self.ctrl_addrs_encoded(),
      profile_card: self.profile_bytes.clone(),
      range: None,
      token: opts.token,
    };
    let mut stream = TcpStream::connect_timeout(&peer_addr, Duration::from_secs(5))?;
    let _ = stream.set_nodelay(true);
    write_frame(&mut stream, &req.encode()?)?;
    let resp_raw = read_frame(&mut stream)?;
    let resp = P2pResp::decode(&resp_raw)?;
    self.ingest_resp_profiles(&resp);
    if resp.status != Status::Ok {
      return Err(PeerError::RemoteStatus(resp.status));
    }
    let mf = Manifest::decode(&resp.manifest).map_err(|e| PeerError::Fs(FsError::Cdc(e)))?;
    Ok((resp, mf))
  }

  /// 从单个对等节点或该节点返回的 P2P Swarm 节点群拉取单文件、单目录或子路径 (`Op::Pull = 1`)
  pub fn pull(
    &self,
    peer_addr: SocketAddr,
    share_name: &str,
    local_dst: &Path,
    opts: TransferOpts<'_>,
  ) -> Result<SyncReport, PeerError> {
    self.swarm_pull(&[peer_addr], share_name, local_dst, opts)
  }

  /// 多源并发拉取 (`swarm_pull`)：
  /// 同时连接多个源对等节点及处于长连接订阅中的 P2P Seed 节点，按地域亲和度与带宽权重条带化并发拉取 `[blk_lo, blk_hi)`
  pub fn swarm_pull(
    &self,
    seed_addrs: &[SocketAddr],
    share_name: &str,
    local_dst: &Path,
    opts: TransferOpts<'_>,
  ) -> Result<SyncReport, PeerError> {
    if seed_addrs.is_empty() {
      return Err(PeerError::NoHealthyPeer);
    }

    let share_id = derive_share_id(share_name);
    let mut candidate_peers: Vec<SwarmPeer> = Vec::new();
    let mut locked_mf: Option<Manifest> = None;
    let mut locked_hash: Option<[u8; 16]> = None;

    for &addr in seed_addrs {
      let Ok((resp, mf)) = self.stat(addr, share_name, opts) else {
        continue;
      };
      if let Some(expected_rh) = locked_hash {
        if resp.root_hash != expected_rh {
          return Err(PeerError::SnapshotConflict);
        }
      } else {
        locked_hash = Some(resp.root_hash);
        locked_mf = Some(mf);
      }

      for sp in resp.peers {
        if sp.endpoint_id != self.endpoint_id
          && !candidate_peers
            .iter()
            .any(|p| p.peer_addrs == sp.peer_addrs && !sp.peer_addrs.is_empty())
        {
          candidate_peers.push(sp);
        }
      }
    }

    let mut mf = locked_mf.ok_or(PeerError::NoHealthyPeer)?;
    let root_hash = locked_hash.unwrap_or_default();
    if opts.delete_extraneous && opts.subpath.trim_matches('/').is_empty() {
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

    let ranked = rank_swarm_peers(self.profile.region, candidate_peers);
    if ranked.is_empty() {
      return Err(PeerError::NoHealthyPeer);
    }

    let stripes = plan_stripes(self.profile.region, &ranked, total_blocks);
    let peers_used = stripes.len();
    let my_card = self.profile_bytes.clone();
    let subpath_owned = opts.subpath.to_owned();
    let req_rate = opts.req_rate;
    let token = opts.token;

    let mut handles = Vec::with_capacity(stripes.len());
    for (p_idx, lo, hi) in stripes {
      let peer = ranked[p_idx].clone();
      let sink_c = Arc::clone(&sink);
      let card_c = my_card.clone();
      let path_c = subpath_owned.clone();
      handles.push(thread::spawn(move || {
        pull_range_from_swarm_peer(RangePullJob {
          peer: &peer,
          share_id,
          subpath: &path_c,
          root_hash,
          blk_lo: lo,
          blk_hi: hi,
          req_rate,
          token,
          my_card: card_c,
          sink: &sink_c,
        })
      }));
    }

    let mut blocks_transferred = 0u32;
    let mut bytes_transferred = 0u64;
    for h in handles {
      if let Ok(Ok((blks, bytes))) = h.join() {
        blocks_transferred += blks;
        bytes_transferred += bytes;
      }
    }

    // Work-Stealing Failover：若有任何条带未完成，由健康节点接管补齐
    if !sink.is_range_complete(0, total_blocks) {
      for peer in &ranked {
        if sink.is_range_complete(0, total_blocks) {
          break;
        }
        let have = sink.snapshot_have_bits();
        let mut missing = Vec::new();
        for_each_missing_span(&have, 0, total_blocks, |lo, hi| {
          missing.push((lo, hi));
        });
        for (lo, hi) in missing {
          if let Ok((blks, bytes)) = pull_range_from_swarm_peer(RangePullJob {
            peer,
            share_id,
            subpath: &subpath_owned,
            root_hash,
            blk_lo: lo,
            blk_hi: hi,
            req_rate,
            token,
            my_card: my_card.clone(),
            sink: &sink,
          }) {
            blocks_transferred += blks;
            bytes_transferred += bytes;
          }
        }
      }
    }

    sink.commit()?;
    Ok(SyncReport {
      root_hash,
      total_blocks,
      blocks_transferred,
      bytes_transferred,
      peers_used,
    })
  }

  /// 向拥有写权限的对等节点推送本地文件或目录 (`Op::Push = 2`)
  pub fn push(
    &self,
    peer_addr: SocketAddr,
    share_name: &str,
    local_src: &Path,
    opts: TransferOpts<'_>,
  ) -> Result<SyncReport, PeerError> {
    let share_id = derive_share_id(share_name);
    let mf = build_manifest_from_path(local_src, opts.subpath, opts.delete_extraneous)?;
    let root_hash = mf.root_hash().map_err(FsError::Cdc)?;
    let mf_bytes = mf.encode().map_err(FsError::Cdc)?;
    let reader = SourceReader::open(local_src, mf)?;
    let total_blocks = reader.total_blocks();

    let (udp_sock, my_udp_addrs) = bind_udp_ephemeral()?;
    let mut flags = 0u8;
    if opts.delete_extraneous {
      flags |= FLAG_DELETE;
    }
    if !opts.subpath.trim_matches('/').is_empty() {
      flags |= FLAG_PARTIAL;
    }

    let req = P2pReq {
      op: Op::Push,
      flags,
      share_id,
      req_rate: opts.req_rate,
      path: opts.subpath.to_owned(),
      peer_addrs: my_udp_addrs,
      profile_card: self.profile_bytes.clone(),
      range: Some((root_hash, 0, total_blocks)),
      token: opts.token,
    };

    let mut stream = TcpStream::connect_timeout(&peer_addr, Duration::from_secs(5))?;
    let _ = stream.set_nodelay(true);
    write_frame(&mut stream, &req.encode()?)?;
    write_frame(&mut stream, &mf_bytes)?;

    let resp_raw = read_frame(&mut stream)?;
    let resp = P2pResp::decode(&resp_raw)?;
    self.ingest_resp_profiles(&resp);
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

  /// 长连接订阅远端对等节点的分享 (`Op::Sub = 4`)：
  /// 本节点在同步完成后自动作为 Dragonfly P2P 分发网络的供块节点（Seed Peer）
  pub fn subscribe(
    self: &Arc<Self>,
    origin_addr: SocketAddr,
    share_name: &str,
    local_dir: PathBuf,
    opts: TransferOpts<'_>,
  ) -> Result<SyncReport, PeerError> {
    let share_id = derive_share_id(share_name);
    create_dir_all(&local_dir)?;

    self.add_share_with_prefix(
      ShareConfig::public_ro(share_name, &local_dir, 0),
      opts.subpath.to_owned(),
    );

    let initial = self.pull(
      origin_addr,
      share_name,
      &local_dir,
      opts.with_delete(opts.subpath.trim_matches('/').is_empty()),
    )?;

    let my_ctrl_addrs = self.ctrl_addrs_encoded();
    let have_req = P2pReq {
      op: Op::Have,
      flags: FLAG_SEED,
      share_id,
      req_rate: opts.req_rate,
      path: opts.subpath.to_owned(),
      peer_addrs: my_ctrl_addrs.clone(),
      profile_card: self.profile_bytes.clone(),
      range: Some((initial.root_hash, 0, initial.total_blocks)),
      token: opts.token,
    };
    if let Ok(mut hs) = TcpStream::connect_timeout(&origin_addr, Duration::from_secs(3)) {
      let _ = hs.set_nodelay(true);
      let _ = write_frame(&mut hs, &have_req.encode()?);
      let _ = read_frame(&mut hs);
    }

    let req = P2pReq {
      op: Op::Sub,
      flags: FLAG_SEED,
      share_id,
      req_rate: opts.req_rate,
      path: opts.subpath.to_owned(),
      peer_addrs: my_ctrl_addrs,
      profile_card: self.profile_bytes.clone(),
      range: Some((initial.root_hash, 0, initial.total_blocks)),
      token: opts.token,
    };
    let mut stream = TcpStream::connect_timeout(&origin_addr, Duration::from_secs(5))?;
    let _ = stream.set_nodelay(true);
    write_frame(&mut stream, &req.encode()?)?;
    let resp_raw = read_frame(&mut stream)?;
    let resp = P2pResp::decode(&resp_raw)?;
    if resp.status != Status::Ok {
      return Err(PeerError::RemoteStatus(resp.status));
    }

    Ok(initial)
  }

  /// 加入对等备份群组 (`Op::GroupJoin = 6`)
  pub fn join_group(
    self: &Arc<Self>,
    group_name: &str,
    local_dir: PathBuf,
    backup_prefix: &str,
    peers: &[SocketAddr],
  ) -> Result<(), PeerError> {
    create_dir_all(&local_dir)?;
    let share_id = derive_share_id(group_name);

    self.add_share_with_prefix(
      ShareConfig::public_rw(group_name, &local_dir, 0),
      backup_prefix.to_owned(),
    );

    let opts = TransferOpts::new().with_subpath(backup_prefix);
    let my_ctrl_addrs = self.ctrl_addrs_encoded();
    for &peer_addr in peers {
      if peer_addr == self.ctrl_addr {
        continue;
      }
      let _ = self.pull(peer_addr, group_name, &local_dir, opts);

      let req = P2pReq {
        op: Op::GroupJoin,
        flags: FLAG_SEED | FLAG_PARTIAL,
        share_id,
        req_rate: 0,
        path: backup_prefix.to_owned(),
        peer_addrs: my_ctrl_addrs.clone(),
        profile_card: self.profile_bytes.clone(),
        range: None,
        token: None,
      };
      if let Ok(mut stream) = TcpStream::connect_timeout(&peer_addr, Duration::from_secs(3)) {
        let _ = write_frame(&mut stream, &req.encode()?);
        let _ = read_frame(&mut stream);
      }
    }
    Ok(())
  }

  /// 当本节点某个分享或群组目录发生文件新增/修改时，主动向所有订阅节点与群组备份节点推送更新 (`SubPush`)
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

    // 单次扫描全目录构建基础清单，子路径订阅直接内存过滤，消除重复磁盘 IO 与重复分块
    let base_mf = build_manifest_from_path(&root_dir, "", false)?;
    let origin_addrs = self.ctrl_addrs_encoded();
    let mut pushed = 0usize;

    for sub in subs {
      let clean_pre = sub.prefix.trim_matches('/');
      let filtered = if clean_pre.is_empty() {
        base_mf.clone()
      } else {
        base_mf.filter_subpath(clean_pre)
      };
      if !clean_pre.is_empty() && filtered.entries.is_empty() {
        continue;
      }
      let rh = filtered.root_hash().map_err(FsError::Cdc)?;
      let mf_bytes = filtered.encode().map_err(FsError::Cdc)?;

      let mut swarm_peers = Vec::with_capacity(1 + seeds.len());
      swarm_peers.push(SwarmPeer {
        endpoint_id: self.endpoint_id,
        region: self.profile.region,
        weight_rate: sub.eff_rate,
        blk_lo: 0,
        blk_hi: filtered.total_blocks(),
        peer_addrs: origin_addrs.clone(),
        profile_card: self.profile_bytes.clone(),
      });
      for s in &seeds {
        if s.root_hash == rh && s.endpoint_id != sub.endpoint_id {
          swarm_peers.push(SwarmPeer {
            endpoint_id: s.endpoint_id,
            region: s.region,
            weight_rate: s.weight_rate,
            blk_lo: s.blk_lo,
            blk_hi: s.blk_hi,
            peer_addrs: encode_socket_addrs(&[s.ctrl_addr]),
            profile_card: s.profile_card.clone(),
          });
        }
      }

      let push_ev = SubPush {
        flags: FLAG_PARTIAL,
        share_id,
        eff_rate: sub.eff_rate,
        root_hash: rh,
        origin_addrs: origin_addrs.clone(),
        peers: swarm_peers,
        manifest: mf_bytes,
      };
      if let Ok(mut stream) = TcpStream::connect_timeout(&sub.ctrl_addr, Duration::from_secs(3)) {
        let _ = stream.set_nodelay(true);
        if write_frame(&mut stream, &push_ev.encode()?).is_ok() && read_frame(&mut stream).is_ok() {
          pushed += 1;
        }
      }
    }
    Ok(pushed)
  }

  fn ingest_resp_profiles(&self, resp: &P2pResp) {
    let mut guard = self.known_profiles.write();
    if !resp.profile_card.is_empty()
      && let Ok(card) = SignedProfile::decode_and_verify(&resp.profile_card)
    {
      guard.insert(card.pubkey, card);
    }
    for p in &resp.peers {
      if !p.profile_card.is_empty()
        && let Ok(card) = SignedProfile::decode_and_verify(&p.profile_card)
      {
        guard.insert(card.pubkey, card);
      }
    }
  }
}

impl Drop for PeerNode {
  fn drop(&mut self) {
    self.stop.store(true, Ordering::Release);
    let _ = TcpStream::connect(self.ctrl_addr);
    if let Some(h) = self.accept_thread.take() {
      let _ = h.join();
    }
  }
}
