use std::{
  io,
  iter::once,
  net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream},
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
use s0_conf::ConfError;

use super::{
  KeyMode, NodeConfig, PeerId, ShareConfig, TransferOpts, derive_share_id, encode_id_b64,
  encode_iroh_addrs_b64,
  pull::{connect_ctrl_addrs, send_req_recv_resp},
  register_iroh_endpoints, resolve_iroh_endpoints,
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

/// 全对称 P2P 节点引擎 (`PeerNode`)
pub struct PeerNode {
  pub(crate) endpoint_id: PeerId,
  pub(crate) profile: SignedProfile,
  pub(crate) profile_bytes: Vec<u8>,
  pub(crate) ctrl_addr: SocketAddr,
  pub(crate) ctrl_addrs: Vec<SocketAddr>,
  pub(crate) shares: Arc<RwLock<HashMap<[u8; 16], ShareState>>>,
  pub(crate) known_profiles: Arc<RwLock<HashMap<PeerId, SignedProfile>>>,
  stop: Arc<AtomicBool>,
  accept_thread: Option<JoinHandle<()>>,
}

impl PeerNode {
  /// 启动对等节点守护引擎（支持 IPv4 + IPv6 双栈控制面监听与自动注册）
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

    let mut listeners = vec![listener];
    let mut ctrl_addrs = vec![ctrl_addr];
    if cfg.bind_addr.is_none() || (bind_target.ip().is_loopback() && bind_target.port() == 0) {
      let alt_target = if ctrl_addr.is_ipv6() {
        SocketAddr::from((Ipv4Addr::LOCALHOST, 0))
      } else {
        SocketAddr::from((Ipv6Addr::LOCALHOST, 0))
      };
      if let Ok(alt_l) = TcpListener::bind(alt_target)
        && let Ok(alt_a) = alt_l.local_addr()
        && alt_l.set_nonblocking(true).is_ok()
      {
        ctrl_addrs.push(alt_a);
        listeners.push(alt_l);
      }
    }

    register_iroh_endpoints(endpoint_id, &ctrl_addrs);

    let shares = Arc::new(RwLock::new(HashMap::default()));
    let mut kp = HashMap::default();
    kp.insert(endpoint_id, profile.clone());
    let known_profiles = Arc::new(RwLock::new(kp));
    let stop = Arc::new(AtomicBool::new(false));

    let srv_ctx = ServerCtx {
      my_id: endpoint_id,
      my_region: profile.region,
      my_card: profile_bytes.clone(),
      my_ctrl_addrs: encode_socket_addrs(&ctrl_addrs),
      shares: Arc::clone(&shares),
      known_profiles: Arc::clone(&known_profiles),
    };
    let stop_bg = Arc::clone(&stop);

    let accept_thread = thread::spawn(move || {
      while !stop_bg.load(Ordering::Acquire) {
        let mut accepted = false;
        for l in &listeners {
          match l.accept() {
            Ok((stream, _peer)) => {
              accepted = true;
              let _ = stream.set_nonblocking(false);
              let _ = stream.set_nodelay(true);
              let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
              let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
              let ctx = srv_ctx.clone();
              thread::spawn(move || {
                let _ = handle_conn(stream, &ctx);
              });
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(_) => return,
          }
        }
        if !accepted {
          thread::sleep(Duration::from_millis(5));
        }
      }
    });

    let node = Arc::new(Self {
      endpoint_id,
      profile,
      profile_bytes,
      ctrl_addr,
      ctrl_addrs,
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

  /// 本节点主控制面监听地址
  #[inline]
  pub fn ctrl_addr(&self) -> SocketAddr {
    self.ctrl_addr
  }

  /// 本节点全部双栈控制面监听地址列表
  #[inline]
  pub fn ctrl_addrs(&self) -> &[SocketAddr] {
    &self.ctrl_addrs
  }

  /// 本节点 43 字符 `base64url` `iroh` 身份地址
  #[inline]
  pub fn iroh_id_b64(&self) -> String {
    encode_id_b64(&self.endpoint_id)
  }

  /// 本节点带紧凑二进制双栈路由后缀的 `base64url` `iroh` 地址
  #[inline]
  pub fn iroh_addr_b64(&self) -> String {
    encode_iroh_addrs_b64(&self.endpoint_id, &self.ctrl_addrs)
  }

  /// 本节点自签名名片
  #[inline]
  pub fn profile(&self) -> &SignedProfile {
    &self.profile
  }

  /// 本节点控制面地址字节编码
  #[inline]
  pub fn ctrl_addrs_encoded(&self) -> Vec<u8> {
    encode_socket_addrs(&self.ctrl_addrs)
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
    let addrs = resolve_iroh_endpoints(peer_id);
    if addrs.is_empty() {
      return Err(PeerError::NoHealthyPeer);
    }
    connect_ctrl_addrs(&addrs)
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
      byte_range: None,
    };
    let mut stream = self.connect_peer(&peer_id)?;
    let resp = send_req_recv_resp(&mut stream, &req)?;
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
      byte_range: opts.byte_range,
    };
    let resp = send_req_recv_resp(&mut stream, &req)?;
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
          && !addrs.is_empty()
        {
          register_iroh_endpoints(card.pubkey, &addrs);
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
      register_iroh_endpoints(p.endpoint_id, &addrs);
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
