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
use parking_lot::{Mutex, RwLock};
use s0_cdc::{Manifest, WireError};
use s0_conf::ConfError;
use s0_const::{HASH_LEN, Op, Status, TOKEN_LEN, ZERO_HASH, ZERO_PEER_ID};
use s0_fs::{FsError, build_manifest_from_paths};
use s0_udp::UdpError;
use s0_wire::{
  P2pReq, P2pResp, ProfileError, ShareSummary, SignedProfile, SwarmPeer, decode_share_summaries,
  decode_socket_addrs, encode_socket_addrs,
};
use thiserror::Error;

use crate::{
  KeyMode, NodeConfig, PeerId, ShareConfig, TransferOpts, derive_share_id, encode_id_b64,
  encode_iroh_addrs_b64,
  frame::send_req_recv_resp,
  iroh_net::IrohCtrl,
  pull::connect_ctrl_addrs,
  register_encoded_endpoints, register_iroh_endpoints, resolve_iroh_endpoints,
  server::{ServerCtx, handle_conn},
  subs_store::load_subs_file,
  watch::{ShareState, SwarmSeedRecord, run_watch_loop},
};

const CTRL_IO_TIMEOUT: Duration = Duration::from_secs(10);
const ACCEPT_POLL_SLEEP: Duration = Duration::from_millis(5);
const WATCH_POLL_SLEEP: Duration = Duration::from_millis(25);

#[derive(Debug, Error)]
pub enum PeerError {
  #[error(transparent)]
  Io(#[from] io::Error),
  #[error(transparent)]
  Wire(#[from] WireError),
  #[error(transparent)]
  Fs(#[from] FsError),
  #[error(transparent)]
  Udp(#[from] UdpError),
  #[error(transparent)]
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
  pub(crate) ctrl_addrs: Vec<SocketAddr>,
  pub(crate) shares: Arc<RwLock<HashMap<[u8; HASH_LEN], ShareState>>>,
  pub(crate) known_profiles: Arc<RwLock<HashMap<PeerId, SignedProfile>>>,
  pub(crate) ctx: ServerCtx,
  stop: Arc<AtomicBool>,
  accept_thread: Option<JoinHandle<()>>,
  watch_thread: Option<JoinHandle<()>>,
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
    let iroh_ctrl = IrohCtrl::bind(secret_key.to_bytes(), endpoint_id, &ctrl_addrs)?;

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
      iroh_ctrl: iroh_ctrl.clone(),
      shares: Arc::clone(&shares),
      known_profiles: Arc::clone(&known_profiles),
      publish_lock: Arc::new(Mutex::new(())),
    };
    iroh_ctrl.spawn_accept_loop(srv_ctx.clone());
    let stop_bg = Arc::clone(&stop);
    let accept_ctx = srv_ctx.clone();

    let accept_thread = thread::spawn(move || {
      while !stop_bg.load(Ordering::Acquire) {
        let mut accepted = false;
        for l in &listeners {
          match l.accept() {
            Ok((stream, _peer)) => {
              accepted = true;
              let _ = stream.set_nonblocking(false);
              let _ = stream.set_nodelay(true);
              let _ = stream.set_read_timeout(Some(CTRL_IO_TIMEOUT));
              let _ = stream.set_write_timeout(Some(CTRL_IO_TIMEOUT));
              let ctx = accept_ctx.clone();
              thread::spawn(move || {
                let _ = handle_conn(stream, &ctx);
              });
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(_) => return,
          }
        }
        if !accepted {
          thread::sleep(ACCEPT_POLL_SLEEP);
        }
      }
    });

    let stop_watch = Arc::clone(&stop);
    let watch_ctx = srv_ctx.clone();
    let watch_thread = thread::spawn(move || {
      run_watch_loop(stop_watch, watch_ctx, WATCH_POLL_SLEEP);
    });

    let node = Arc::new(Self {
      endpoint_id,
      profile,
      profile_bytes,
      ctrl_addrs,
      shares,
      known_profiles,
      ctx: srv_ctx,
      stop,
      accept_thread: Some(accept_thread),
      watch_thread: Some(watch_thread),
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

  /// 本节点全部双栈控制面监听地址列表
  #[inline]
  pub fn ctrl_addrs(&self) -> &[SocketAddr] {
    &self.ctrl_addrs
  }

  /// 本节点 `iroh::Endpoint` QUIC 监听地址列表
  #[inline]
  pub fn iroh_quic_addrs(&self) -> &[SocketAddr] {
    self.ctx.iroh_ctrl.quic_addrs()
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
  pub(crate) fn ctrl_addrs_encoded(&self) -> Vec<u8> {
    encode_socket_addrs(&self.ctrl_addrs)
  }

  /// 查询已验证的远端对等节点自签名名片
  pub fn get_peer_profile(&self, endpoint_id: &PeerId) -> Option<SignedProfile> {
    self.known_profiles.read().get(endpoint_id).cloned()
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
        .and_then(|mf| Some((Vec::new(), mf.root_hash().ok()?, mf)))
    } else {
      None
    };
    let persisted = if sub_paths.is_empty() && cfg.root.exists() {
      load_subs_file(&cfg.root)
    } else {
      None
    };
    let mut guard = self.shares.write();
    let st = if let Some(existing) = guard.get_mut(&cfg.share_id) {
      existing.cfg = cfg;
      existing.sub_paths = sub_paths;
      existing.cached_manifest = cached_manifest.clone();
      existing
    } else {
      let share_id = cfg.share_id;
      let mut st = ShareState::new(cfg, sub_paths);
      st.cached_manifest = cached_manifest.clone();
      guard.entry(share_id).or_insert(st)
    };
    if let Some((persisted_base, persisted_subs, rdb_bytes)) = persisted {
      if st.baseline_manifest.is_none() {
        st.baseline_manifest = persisted_base.or_else(|| cached_manifest.map(|(_, _, mf)| mf));
      }
      st.subs_rdb_bytes = rdb_bytes;
      for sub in persisted_subs {
        st.upsert_subscriber(sub);
      }
    }
  }

  pub(crate) fn connect_peer(&self, peer_id: &PeerId) -> Result<TcpStream, PeerError> {
    let addrs = resolve_iroh_endpoints(peer_id);
    if addrs.is_empty() {
      return Err(PeerError::NoHealthyPeer);
    }
    connect_ctrl_addrs(&addrs)
  }

  pub(crate) fn rpc_ok(&self, stream: &mut TcpStream, req: &P2pReq) -> Result<P2pResp, PeerError> {
    let resp = send_req_recv_resp(stream, req)?;
    self.ingest_resp_profiles(&resp, true);
    if resp.status != Status::Ok {
      return Err(PeerError::RemoteStatus(resp.status));
    }
    Ok(resp)
  }

  /// 查询远端对等节点托管的全部文件分享组与当前访问权限 (`Op::ListShares = 7`)
  pub fn list_shares(
    &self,
    peer_id: PeerId,
    token: Option<[u8; TOKEN_LEN]>,
  ) -> Result<Vec<ShareSummary>, PeerError> {
    let req = P2pReq {
      token,
      ..P2pReq::new(
        Op::ListShares,
        0,
        ZERO_HASH,
        self.ctrl_addrs_encoded(),
        self.profile_bytes.clone(),
      )
    };
    let mut stream = self.connect_peer(&peer_id)?;
    let resp = self.rpc_ok(&mut stream, &req)?;
    Ok(decode_share_summaries(&resp.manifest)?)
  }

  /// 查询远端对等节点的分享状态、清单 `Manifest` 与可用 P2P Swarm 节点列表 (`Op::Stat = 3`)
  pub fn stat(
    &self,
    peer_id: PeerId,
    share_name: &str,
    opts: &TransferOpts,
  ) -> Result<(P2pResp, Manifest), PeerError> {
    let (resp, mf, _stream) = self.stat_with_stream(peer_id, share_name, opts)?;
    Ok((resp, mf))
  }

  pub(crate) fn stat_with_stream(
    &self,
    peer_id: PeerId,
    share_name: &str,
    opts: &TransferOpts,
  ) -> Result<(P2pResp, Manifest, TcpStream), PeerError> {
    let stream = self.connect_peer(&peer_id)?;
    self.stat_on_stream(stream, share_name, opts)
  }

  pub(crate) fn stat_on_stream(
    &self,
    mut stream: TcpStream,
    share_name: &str,
    opts: &TransferOpts,
  ) -> Result<(P2pResp, Manifest, TcpStream), PeerError> {
    let req = P2pReq {
      req_rate: opts.req_rate,
      paths: opts.paths.clone(),
      token: opts.token,
      byte_range: opts.byte_range,
      ..P2pReq::new(
        Op::Stat,
        0,
        derive_share_id(share_name),
        self.ctrl_addrs_encoded(),
        self.profile_bytes.clone(),
      )
    };
    let resp = self.rpc_ok(&mut stream, &req)?;
    let mf = Manifest::decode(&resp.manifest)?;
    if mf.root_hash()? != resp.root_hash {
      return Err(WireError::Truncated.into());
    }
    self.ingest_swarm_seeds(req.share_id, resp.root_hash, &resp.peers);
    Ok((resp, mf, stream))
  }

  pub(crate) fn ingest_resp_profiles(&self, resp: &P2pResp, peer_addrs_is_ctrl: bool) {
    let mut guard = self.known_profiles.write();
    let iter = once((
      None,
      resp.profile_card.as_slice(),
      resp.peer_addrs.as_slice(),
      peer_addrs_is_ctrl,
    ))
    .chain(resp.peers.iter().map(|p| {
      (
        Some(p.endpoint_id),
        p.profile_card.as_slice(),
        p.peer_addrs.as_slice(),
        true,
      )
    }));
    for (expected_id, card_raw, addrs_raw, register_ctrl) in iter {
      if !card_raw.is_empty()
        && let Ok(card) = SignedProfile::decode_and_verify(card_raw)
        && expected_id.is_none_or(|id| id == card.pubkey)
      {
        if register_ctrl {
          register_encoded_endpoints(card.pubkey, addrs_raw);
        }
        if guard
          .get(&card.pubkey)
          .is_none_or(|prev| card.ts_sec >= prev.ts_sec)
        {
          guard.insert(card.pubkey, card);
        }
      }
    }
  }

  pub(crate) fn ingest_swarm_seeds(
    &self,
    share_id: [u8; HASH_LEN],
    root_hash: [u8; HASH_LEN],
    peers: &[SwarmPeer],
  ) {
    if peers.is_empty() || root_hash == ZERO_HASH {
      return;
    }
    let mut guard = self.shares.write();
    let Some(st) = guard.get_mut(&share_id) else {
      return;
    };
    for p in peers {
      if p.endpoint_id == ZERO_PEER_ID || p.endpoint_id == self.endpoint_id {
        continue;
      }
      let Ok(addrs) = decode_socket_addrs(&p.peer_addrs) else {
        continue;
      };
      let Some(&ctrl_addr) = addrs.first() else {
        continue;
      };
      register_iroh_endpoints(p.endpoint_id, &addrs);
      st.upsert_seed(SwarmSeedRecord::new(
        p.endpoint_id,
        p.region,
        ctrl_addr,
        p.profile_card.clone(),
        root_hash,
        (p.blk_lo, p.blk_hi),
        p.weight_rate,
      ));
    }
  }
}

impl Drop for PeerNode {
  fn drop(&mut self) {
    self.stop.store(true, Ordering::Release);
    self.ctx.iroh_ctrl.close();
    if let Some(&addr) = self.ctrl_addrs.first() {
      let _ = TcpStream::connect(addr);
    }
    if let Some(h) = self.accept_thread.take() {
      let _ = h.join();
    }
    if let Some(h) = self.watch_thread.take() {
      let _ = h.join();
    }
  }
}
