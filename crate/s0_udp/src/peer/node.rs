use std::{
  io,
  iter::once,
  net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  thread::{self, JoinHandle},
  time::Duration,
};

use gxhash::HashMap;
use parking_lot::RwLock;
use s0_cdc::{Manifest, WireError};
use s0_conf::{ConfError, extend_clean_paths_csv};

use super::{
  KeyMode, NodeConfig, PeerId, ShareConfig, derive_share_id, encode_id_b64, encode_iroh_b64,
  pull::{connect_ctrl_tcp, read_frame, write_frame},
  register_iroh_endpoint, resolve_iroh_endpoint,
  server::{ServerCtx, ShareState, SwarmSeedRecord, handle_conn},
};
use crate::{
  UdpError,
  consts::{Op, Status},
  fs::{FsError, build_manifest_from_paths},
  profile::{ProfileError, SignedProfile},
  wire::{
    P2pReq, P2pResp, ShareSummary, SwarmPeer, decode_share_summaries, decode_socket_addrs,
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
pub struct PeerNode {
  pub(crate) endpoint_id: PeerId,
  pub(crate) profile: SignedProfile,
  pub(crate) profile_bytes: Vec<u8>,
  pub(crate) ctrl_addr: SocketAddr,
  pub(crate) shares: Arc<RwLock<HashMap<[u8; 16], ShareState>>>,
  pub(crate) known_profiles: Arc<RwLock<HashMap<PeerId, SignedProfile>>>,
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

  /// 注册或更新本地文件分享组 (`ShareConfig`)
  pub fn add_share(&self, cfg: ShareConfig) {
    self.add_share_with_paths(cfg, Vec::new());
  }

  /// 批量注册多个本地文件分享组
  pub fn add_shares(&self, cfgs: impl IntoIterator<Item = ShareConfig>) {
    for cfg in cfgs {
      self.add_share(cfg);
    }
  }

  pub(crate) fn add_share_with_paths(&self, cfg: ShareConfig, sub_paths: Vec<String>) {
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

  pub(crate) fn connect_peer(&self, peer_id: &PeerId) -> Result<TcpStream, PeerError> {
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

  pub(crate) fn stat_with_stream(
    &self,
    peer_id: PeerId,
    share_name: &str,
    opts: &TransferOpts<'_>,
  ) -> Result<(P2pResp, Manifest, TcpStream), PeerError> {
    let stream = self.connect_peer(&peer_id)?;
    self.stat_on_stream(stream, share_name, opts)
  }

  pub(crate) fn stat_on_stream(
    &self,
    mut stream: TcpStream,
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

  pub(crate) fn ingest_resp_profiles(&self, resp: &P2pResp, peer_addrs_is_ctrl: bool) {
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

  pub(crate) fn ingest_swarm_seeds(
    &self,
    share_id: [u8; 16],
    root_hash: [u8; 16],
    peers: &[SwarmPeer],
  ) {
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
