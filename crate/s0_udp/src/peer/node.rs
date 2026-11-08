use std::{
  fs::create_dir_all,
  io,
  iter::once,
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
use parking_lot::{Mutex, RwLock};
use s0_cdc::{FLAG_DELETE, FLAG_PARTIAL, Manifest, WireError};
use s0_conf::{ConfError, extend_clean_paths_csv};

use super::{
  KeyMode, NodeConfig, PeerId, ShareConfig, derive_share_id, encode_id_b64, encode_iroh_b64,
  register_iroh_endpoint, resolve_iroh_endpoint,
  server::{
    RangePullJob, ServerCtx, ShareState, SwarmSeedRecord, build_swarm_peers, connect_ctrl_tcp,
    derive_push_session_id, execute_striped_pull, handle_conn, is_have_all_ones_frame, read_frame,
    write_frame,
  },
};
use crate::{
  RangeSpec, UdpError, bind_udp_for_ip,
  consts::{FLAG_SEED, Op, Status},
  fs::{FsError, SharedSink, SourceReader, build_manifest_from_paths, sanitize_rel_path},
  profile::{ProfileError, SignedProfile},
  punch_udp, send_range_udp,
  wire::{
    P2pReq, P2pResp, ShareSummary, SubPush, SwarmPeer, decode_share_summaries, decode_socket_addrs,
    encode_socket_addrs,
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
  #[error("非法 iroh 地址或群组 URI")]
  BadAddress,
  #[error(transparent)]
  Conf(#[from] ConfError),
}

/// 同步/拉取/推送请求参数 (`TransferOpts`)
///
/// 支持同步全量（空路径列表）或同时多选多个文件与目录（`paths`）。
#[derive(Debug, Default, Clone)]
pub struct TransferOpts<'a> {
  pub subpath: &'a str,
  pub paths: Vec<String>,
  pub delete_extraneous: bool,
  pub req_rate: u64,
  pub token: Option<[u8; 16]>,
}

impl<'a> TransferOpts<'a> {
  pub const fn new() -> Self {
    Self {
      subpath: "",
      paths: Vec::new(),
      delete_extraneous: false,
      req_rate: 0,
      token: None,
    }
  }

  pub fn with_subpath(mut self, subpath: &'a str) -> Self {
    self.subpath = subpath;
    extend_clean_paths_csv(&mut self.paths, subpath);
    self
  }

  /// 多选任意多个文件或子目录（空列表表示同步整个分享组全部内容）
  pub fn with_paths<I, S>(mut self, paths: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
  {
    for p in paths {
      extend_clean_paths_csv(&mut self.paths, p.as_ref());
    }
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

  /// 导出归一化后的多选路径列表（空 `Vec` 代表整个分享组根目录）
  pub fn resolved_paths(&self) -> Vec<String> {
    if !self.paths.is_empty() {
      return self.paths.clone();
    }
    let mut out = Vec::new();
    extend_clean_paths_csv(&mut out, self.subpath);
    out
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
///
/// - 内部使用 32 字节二进制 `PeerId` (`[u8; 32]`) 寻址；
/// - 单个用户节点支持同时托管多个独立文件分享组（`shares`）；
/// - 支持按组员 `PeerId` 配置只读/读写权限、速率配额与允许上传的路径前缀。
pub struct PeerNode {
  endpoint_id: PeerId,
  profile: SignedProfile,
  profile_bytes: Vec<u8>,
  ctrl_addr: SocketAddr,
  shares: Arc<RwLock<HashMap<[u8; 16], ShareState>>>,
  known_profiles: Arc<RwLock<HashMap<PeerId, SignedProfile>>>,
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

    let bind_target = cfg
      .bind_addr
      .unwrap_or_else(|| SocketAddr::from((Ipv4Addr::LOCALHOST, 0)));
    let listener = TcpListener::bind(bind_target)?;
    let ctrl_addr = listener.local_addr()?;
    listener.set_nonblocking(true)?;

    // 注册本节点 32B 二进制 iroh PeerId 路由
    register_iroh_endpoint(endpoint_id, ctrl_addr);

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
            let _ = stream.set_nonblocking(false);
            let _ = stream.set_nodelay(true);
            let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
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

    let node = Arc::new(Self {
      endpoint_id,
      profile,
      profile_bytes,
      ctrl_addr,
      shares,
      known_profiles,
      stop,
      accept_thread: Some(accept_thread),
    });

    if let KeyMode::Persistent(ref dir) = cfg.key_mode
      && let Ok(groups) = ShareConfig::load_from_dir(dir)
    {
      node.add_shares(groups);
    }

    Ok(node)
  }

  /// 本节点 32B 二进制 `iroh` `PeerId` 公钥
  #[inline]
  pub fn endpoint_id(&self) -> PeerId {
    self.endpoint_id
  }

  /// 本节点 43 字符 `base64url` `iroh` 身份地址
  #[inline]
  pub fn iroh_id_b64(&self) -> String {
    encode_id_b64(&self.endpoint_id)
  }

  /// 本节点带紧凑二进制路由后缀的 `base64url` `iroh` 地址
  #[inline]
  pub fn iroh_addr_b64(&self) -> String {
    encode_iroh_b64(&self.endpoint_id, self.ctrl_addr)
  }

  /// 本节点自签名名片
  #[inline]
  pub fn profile(&self) -> &SignedProfile {
    &self.profile
  }

  /// 本节点控制面地址字节编码
  #[inline]
  pub fn ctrl_addrs_encoded(&self) -> Vec<u8> {
    encode_socket_addrs(&[self.ctrl_addr])
  }

  /// 查询已验证的远端对等节点自签名名片
  pub fn get_peer_profile(&self, endpoint_id: &PeerId) -> Option<SignedProfile> {
    self.known_profiles.read().get(endpoint_id).cloned()
  }

  /// 所有已知对等节点名片快照
  pub fn known_profiles(&self) -> Vec<SignedProfile> {
    self.known_profiles.read().values().cloned().collect()
  }

  /// 注册或更新本地文件分享组 (`ShareConfig`)，单个节点支持同时托管多个分享组
  pub fn add_share(&self, cfg: ShareConfig) {
    self.add_share_with_paths(cfg, Vec::new());
  }

  /// 批量注册多个本地文件分享组
  pub fn add_shares(&self, cfgs: impl IntoIterator<Item = ShareConfig>) {
    for cfg in cfgs {
      self.add_share(cfg);
    }
  }

  fn add_share_with_paths(&self, cfg: ShareConfig, sub_paths: Vec<String>) {
    let cached_manifest = if sub_paths.is_empty() && cfg.root.exists() {
      build_manifest_from_paths::<&str>(&cfg.root, &[], false)
        .ok()
        .and_then(|mf| Some((mf.root_hash().ok()?, mf)))
    } else {
      None
    };
    let mut guard = self.shares.write();
    if let Some(existing) = guard.get_mut(&cfg.share_id) {
      existing.cfg = cfg;
      existing.sub_paths = sub_paths;
      existing.cached_manifest = cached_manifest;
    } else {
      guard.insert(
        cfg.share_id,
        ShareState {
          cfg,
          sub_paths,
          subscribers: Vec::new(),
          seeds: Vec::new(),
          active_sink: None,
          cached_manifest,
        },
      );
    }
  }

  fn connect_peer(&self, peer_id: &PeerId) -> Result<TcpStream, PeerError> {
    let addr = resolve_iroh_endpoint(peer_id).ok_or(PeerError::NoHealthyPeer)?;
    connect_ctrl_tcp(&addr)
  }

  /// 查询远端对等节点托管的全部文件分享组与当前访问权限 (`Op::ListShares = 7`)
  pub fn list_shares(
    &self,
    peer_id: PeerId,
    token: Option<[u8; 16]>,
  ) -> Result<Vec<ShareSummary>, PeerError> {
    let req = P2pReq {
      op: Op::ListShares,
      flags: 0,
      share_id: [0u8; 16],
      req_rate: 0,
      paths: Vec::new(),
      peer_addrs: self.ctrl_addrs_encoded(),
      profile_card: self.profile_bytes.clone(),
      range: None,
      token,
    };
    let mut stream = self.connect_peer(&peer_id)?;
    write_frame(&mut stream, &req.encode()?)?;
    let resp_raw = read_frame(&mut stream)?;
    let resp = P2pResp::decode(&resp_raw)?;
    self.ingest_resp_profiles(&resp, true);
    if resp.status != Status::Ok {
      return Err(PeerError::RemoteStatus(resp.status));
    }
    Ok(decode_share_summaries(&resp.manifest)?)
  }

  /// 查询远端对等节点的分享状态、清单 `Manifest` 与可用 P2P Swarm 节点列表 (`Op::Stat = 3`)
  pub fn stat(
    &self,
    peer_id: PeerId,
    share_name: &str,
    opts: &TransferOpts<'_>,
  ) -> Result<(P2pResp, Manifest), PeerError> {
    let (resp, mf, _stream) = self.stat_with_stream(peer_id, share_name, opts)?;
    Ok((resp, mf))
  }

  fn stat_with_stream(
    &self,
    peer_id: PeerId,
    share_name: &str,
    opts: &TransferOpts<'_>,
  ) -> Result<(P2pResp, Manifest, TcpStream), PeerError> {
    let paths = opts.resolved_paths();
    let req = P2pReq {
      op: Op::Stat,
      flags: 0,
      share_id: derive_share_id(share_name),
      req_rate: opts.req_rate,
      paths,
      peer_addrs: self.ctrl_addrs_encoded(),
      profile_card: self.profile_bytes.clone(),
      range: None,
      token: opts.token,
    };
    let mut stream = self.connect_peer(&peer_id)?;
    write_frame(&mut stream, &req.encode()?)?;
    let resp_raw = read_frame(&mut stream)?;
    let resp = P2pResp::decode(&resp_raw)?;
    self.ingest_resp_profiles(&resp, true);
    if resp.status != Status::Ok {
      return Err(PeerError::RemoteStatus(resp.status));
    }
    self.ingest_swarm_seeds(req.share_id, resp.root_hash, &resp.peers);
    let mf = Manifest::decode(&resp.manifest).map_err(|e| PeerError::Fs(FsError::Cdc(e)))?;
    Ok((resp, mf, stream))
  }

  /// 从任意对等服务器获取指定分享组当前可用的全部种子节点 (`SwarmPeer`)，并自动写入本机做种路由表
  pub fn discover_seeds(
    &self,
    peer_id: PeerId,
    share_name: &str,
  ) -> Result<Vec<SwarmPeer>, PeerError> {
    let (resp, _) = self.stat(peer_id, share_name, &TransferOpts::new())?;
    Ok(resp.peers)
  }

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
  /// 通过二进制 `PeerId` 连接种子节点并自动通过 PEX 发现组内所有做种节点，按地域亲和度与带宽权重条带化并发拉取 `[blk_lo, blk_hi)`
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
    if let Ok((resp, mf, stream)) = first_res {
      reused_conns.lock().insert(seed_ids[0], stream);
      stat_ok = Some((resp, mf));
    }

    for (pid, conn_res) in extra_conns {
      if stat_ok.is_none() {
        if let Ok((resp, mf, stream)) = self.stat_with_stream(pid, share_name, &opts) {
          reused_conns.lock().insert(pid, stream);
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

    // 注册活跃下载池，开启边下载边上传（下载过程中即可向其他节点供应已校验块，完成后自动转为常规做种）
    {
      let mut guard = self.shares.write();
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
  ///
  /// 对端会根据本节点的 32B `PeerId` 校验是否拥有 `Perm::Rw` 以及上传文件是否落在该用户允许上传的路径前缀内。
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

    write_frame(&mut stream, &req.encode()?)?;
    write_frame(&mut stream, &mf_bytes)?;

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

  /// 长连接订阅远端对等节点的分享组 (`Op::Sub = 4`)：
  /// 支持同步全部或多选文件/目录集合，并在同步完成后自动作为 Dragonfly P2P 分发网络的做种节点（Seed Peer）
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

    let my_ctrl_addrs = self.ctrl_addrs_encoded();
    let have_req = P2pReq {
      op: Op::Have,
      flags: FLAG_SEED,
      share_id,
      req_rate: opts.req_rate,
      paths: sel_paths.clone(),
      peer_addrs: my_ctrl_addrs.clone(),
      profile_card: self.profile_bytes.clone(),
      range: Some((initial.root_hash, 0, initial.total_blocks)),
      token: opts.token,
    };
    let mut stream = self.connect_peer(&origin_id)?;
    let _ = write_frame(&mut stream, &have_req.encode()?);
    if let Ok(raw) = read_frame(&mut stream)
      && let Ok(resp) = P2pResp::decode(&raw)
    {
      self.ingest_resp_profiles(&resp, true);
      self.ingest_swarm_seeds(share_id, initial.root_hash, &resp.peers);
    }

    let req = P2pReq {
      op: Op::Sub,
      flags: FLAG_SEED,
      share_id,
      req_rate: opts.req_rate,
      paths: sel_paths,
      peer_addrs: my_ctrl_addrs,
      profile_card: self.profile_bytes.clone(),
      range: Some((initial.root_hash, 0, initial.total_blocks)),
      token: opts.token,
    };
    write_frame(&mut stream, &req.encode()?)?;
    let resp_raw = read_frame(&mut stream)?;
    let resp = P2pResp::decode(&resp_raw)?;
    self.ingest_resp_profiles(&resp, true);
    if resp.status != Status::Ok {
      return Err(PeerError::RemoteStatus(resp.status));
    }
    self.ingest_swarm_seeds(share_id, initial.root_hash, &resp.peers);

    Ok(initial)
  }

  /// 加入对等分享备份群组 (`Op::GroupJoin = 6`)，支持全量同步或多选文件/目录集合同步并自动做种
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
      };
      if let Ok(mut stream) = self.connect_peer(&peer_id) {
        let _ = write_frame(&mut stream, &req.encode()?);
        if let Ok(raw) = read_frame(&mut stream)
          && let Ok(resp) = P2pResp::decode(&raw)
        {
          self.ingest_resp_profiles(&resp, true);
          if let Some((rh, ..)) = range {
            self.ingest_swarm_seeds(share_id, rh, &resp.peers);
          }
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

    // 单次扫描全目录构建基础清单，各订阅节点的多选路径直接在内存中单次过滤，消除重复磁盘 IO 与重复分块
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
      let target_addr = resolve_iroh_endpoint(&sub.endpoint_id).unwrap_or(sub.ctrl_addr);
      if let Ok(mut stream) = TcpStream::connect_timeout(&target_addr, Duration::from_secs(3)) {
        let _ = stream.set_nodelay(true);
        if write_frame(&mut stream, &push_ev.encode()?).is_ok() && read_frame(&mut stream).is_ok() {
          pushed += 1;
        }
      }
    }
    Ok(pushed)
  }

  fn ingest_resp_profiles(&self, resp: &P2pResp, peer_addrs_is_ctrl: bool) {
    let mut guard = self.known_profiles.write();
    let iter = once((
      resp.profile_card.as_slice(),
      resp.peer_addrs.as_slice(),
      peer_addrs_is_ctrl,
    ))
    .chain(
      resp
        .peers
        .iter()
        .map(|p| (p.profile_card.as_slice(), p.peer_addrs.as_slice(), true)),
    );
    for (card_raw, addrs_raw, register_ctrl) in iter {
      if !card_raw.is_empty()
        && let Ok(card) = SignedProfile::decode_and_verify(card_raw)
      {
        if register_ctrl
          && let Ok(addrs) = decode_socket_addrs(addrs_raw)
          && let Some(&first) = addrs.first()
        {
          register_iroh_endpoint(card.pubkey, first);
        }
        guard.insert(card.pubkey, card);
      }
    }
  }

  fn ingest_swarm_seeds(&self, share_id: [u8; 16], root_hash: [u8; 16], peers: &[SwarmPeer]) {
    if peers.is_empty() || root_hash == [0u8; 16] {
      return;
    }
    let mut guard = self.shares.write();
    let Some(st) = guard.get_mut(&share_id) else {
      return;
    };
    for p in peers {
      if p.endpoint_id == [0u8; 32] || p.endpoint_id == self.endpoint_id {
        continue;
      }
      let Ok(addrs) = decode_socket_addrs(&p.peer_addrs) else {
        continue;
      };
      let Some(&ctrl_addr) = addrs.first() else {
        continue;
      };
      register_iroh_endpoint(p.endpoint_id, ctrl_addr);
      st.upsert_seed(SwarmSeedRecord {
        endpoint_id: p.endpoint_id,
        region: p.region,
        ctrl_addr,
        profile_card: p.profile_card.clone(),
        root_hash,
        blk_lo: p.blk_lo,
        blk_hi: p.blk_hi,
        weight_rate: p.weight_rate,
      });
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
