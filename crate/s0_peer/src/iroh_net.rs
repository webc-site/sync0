use std::{
  fmt::Display,
  future::Future,
  io,
  net::{Ipv4Addr, Ipv6Addr, SocketAddr},
  sync::OnceLock,
  time::Duration,
};

use gxhash::HashMap;
use iroh::{
  Endpoint, EndpointAddr, PublicKey, SecretKey as IrohSecretKey, TransportAddr,
  endpoint::{PortmapperConfig, QuicTransportConfig, RecvStream, SendStream, presets},
};
use parking_lot::RwLock;
use s0_const::{CTRL_LEN_HDR, PROTO_VERSION, S0_IROH_ALPN, SECRET_KEY_LEN, Status, ZERO_PEER_ID};
use s0_wire::SubPush;
use tokio::{
  runtime::{Builder, Runtime},
  task::spawn_blocking,
  time::timeout,
};

use crate::{
  PeerError, PeerId,
  frame::{
    decode_zstd_frame_body, encode_zstd_frame, parse_zstd_frame_hdr, read_frame, write_frame,
  },
  pull::connect_ctrl_addrs,
  registry::dedup_addrs,
  resolve_iroh_endpoints,
  server::ServerCtx,
  sub::handle_incoming_sub_push_frame,
};

const IROH_DIAL_TIMEOUT: Duration = Duration::from_secs(5);
const IROH_IO_TIMEOUT: Duration = Duration::from_secs(10);

fn iroh_rt() -> &'static Runtime {
  static RT: OnceLock<Runtime> = OnceLock::new();
  RT.get_or_init(|| {
    Builder::new_multi_thread()
      .worker_threads(2)
      .thread_name("s0-iroh")
      .enable_all()
      .build()
      .expect("failed to build iroh runtime")
  })
}

fn global_iroh_quic_routes() -> &'static RwLock<HashMap<PeerId, Vec<SocketAddr>>> {
  static QUIC_ROUTES: OnceLock<RwLock<HashMap<PeerId, Vec<SocketAddr>>>> = OnceLock::new();
  QUIC_ROUTES.get_or_init(|| RwLock::new(HashMap::default()))
}

pub(crate) fn register_iroh_quic_addrs(id: PeerId, addrs: &[SocketAddr]) {
  if id == ZERO_PEER_ID || addrs.is_empty() {
    return;
  }
  global_iroh_quic_routes()
    .write()
    .insert(id, dedup_addrs(addrs));
}

pub(crate) fn resolve_iroh_quic_addrs(id: &PeerId) -> Vec<SocketAddr> {
  let mut out = global_iroh_quic_routes()
    .read()
    .get(id)
    .cloned()
    .unwrap_or_default();
  for addr in resolve_iroh_endpoints(id) {
    if !out.contains(&addr) {
      out.push(addr);
    }
  }
  out
}

#[inline]
fn iroh_err(e: impl Display) -> PeerError {
  PeerError::Io(io::Error::other(e.to_string()))
}

#[inline]
async fn with_timeout<T>(dur: Duration, fut: impl Future<Output = T>) -> Result<T, PeerError> {
  timeout(dur, fut)
    .await
    .map_err(|_| PeerError::Io(io::Error::from(io::ErrorKind::TimedOut)))
}

async fn bind_iroh_with_addrs(
  sk_bytes: [u8; SECRET_KEY_LEN],
  bind_addrs: &[SocketAddr],
) -> Result<Endpoint, PeerError> {
  let mut builder = Endpoint::builder(presets::Minimal)
    .secret_key(IrohSecretKey::from_bytes(&sk_bytes))
    .alpns(vec![S0_IROH_ALPN.to_vec()])
    .portmapper_config(PortmapperConfig::Disabled)
    .transport_config(QuicTransportConfig::default())
    .clear_ip_transports();

  let (mut has_v4, mut has_v6) = (false, false);
  for &addr in bind_addrs {
    let slot = if addr.is_ipv4() {
      &mut has_v4
    } else {
      &mut has_v6
    };
    if !*slot {
      *slot = true;
      builder = builder.bind_addr(addr).map_err(iroh_err)?;
    }
  }
  if !has_v4 && !has_v6 {
    builder = builder
      .bind_addr(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
      .map_err(iroh_err)?;
  }

  builder.bind().await.map_err(iroh_err)
}

/// 单条 `iroh` QUIC 流分批推送的最大路径变更条数（超过自动切分为多帧连续流式发送，支持 `u64::MAX` 级超大改动零内存溢出）
const SUB_STREAM_BATCH_CHANGES: usize = 65_536;

async fn write_iroh_frame(send: &mut SendStream, payload: &[u8]) -> Result<(), PeerError> {
  let framed = encode_zstd_frame(payload);
  with_timeout(IROH_IO_TIMEOUT, send.write_all(&framed))
    .await?
    .map_err(iroh_err)
}

async fn read_iroh_frame(recv: &mut RecvStream) -> Result<Vec<u8>, PeerError> {
  let mut len_buf = [0u8; CTRL_LEN_HDR];
  with_timeout(IROH_IO_TIMEOUT, recv.read_exact(&mut len_buf))
    .await?
    .map_err(iroh_err)?;
  let (hdr, len) = parse_zstd_frame_hdr(len_buf)?;
  let mut raw = vec![0u8; len];
  with_timeout(IROH_IO_TIMEOUT, recv.read_exact(&mut raw))
    .await?
    .map_err(iroh_err)?;
  decode_zstd_frame_body(hdr, raw)
}

fn encode_push_batches(push: &SubPush) -> Result<Vec<Vec<u8>>, PeerError> {
  if push.changes.len() <= SUB_STREAM_BATCH_CHANGES {
    return Ok(vec![push.encode()?]);
  }
  let mut batches = Vec::with_capacity(push.changes.len().div_ceil(SUB_STREAM_BATCH_CHANGES));
  for chunk in push.changes.chunks(SUB_STREAM_BATCH_CHANGES) {
    let sub_batch = SubPush {
      flags: push.flags,
      share_id: push.share_id,
      perm: push.perm,
      eff_rate: push.eff_rate,
      write_prefixes: push.write_prefixes.clone(),
      root_hash: push.root_hash,
      origin_addrs: push.origin_addrs.clone(),
      peers: push.peers.clone(),
      changes: chunk.to_vec(),
    };
    batches.push(sub_batch.encode()?);
  }
  Ok(batches)
}

/// 基于 `iroh::Endpoint` 的控制与实时增量变更推送通道（ALPN: `"s0/delta/1"`，大变更帧自动经 `mzstd` 压缩）
#[derive(Clone)]
pub(crate) struct IrohCtrl {
  endpoint: Endpoint,
  quic_addrs: Vec<SocketAddr>,
}

impl IrohCtrl {
  pub(crate) fn bind(
    sk_bytes: [u8; SECRET_KEY_LEN],
    endpoint_id: PeerId,
    preferred_addrs: &[SocketAddr],
  ) -> Result<Self, PeerError> {
    let endpoint = iroh_rt().block_on(async {
      if let Ok(ep) = bind_iroh_with_addrs(sk_bytes, preferred_addrs).await {
        return Ok(ep);
      }
      let fallback: Vec<SocketAddr> = preferred_addrs
        .iter()
        .map(|a| {
          if a.is_ipv6() {
            SocketAddr::from((Ipv6Addr::LOCALHOST, 0))
          } else {
            SocketAddr::from((Ipv4Addr::LOCALHOST, 0))
          }
        })
        .collect();
      bind_iroh_with_addrs(sk_bytes, &fallback).await
    })?;

    let quic_addrs = endpoint.bound_sockets();
    register_iroh_quic_addrs(endpoint_id, &quic_addrs);
    Ok(Self {
      endpoint,
      quic_addrs,
    })
  }

  #[inline]
  pub(crate) fn quic_addrs(&self) -> &[SocketAddr] {
    &self.quic_addrs
  }

  pub(crate) fn spawn_accept_loop(&self, ctx: ServerCtx) {
    let ep = self.endpoint.clone();
    iroh_rt().spawn(async move {
      while let Some(incoming) = ep.accept().await {
        let ctx_conn = ctx.clone();
        tokio::spawn(async move {
          let Ok(conn) = incoming.await else {
            return;
          };
          let Ok((mut send, mut recv)) = conn.accept_bi().await else {
            return;
          };
          let mut ver = [0u8; 1];
          if !with_timeout(IROH_IO_TIMEOUT, recv.read_exact(&mut ver))
            .await
            .is_ok_and(|r| r.is_ok())
            || ver[0] != PROTO_VERSION
          {
            let _ = write_iroh_frame(&mut send, &[Status::BadReq as u8]).await;
            let _ = send.finish();
            return;
          }
          let mut status = Status::BadReq;
          while let Ok(frame) = read_iroh_frame(&mut recv).await {
            let ctx_step = ctx_conn.clone();
            match spawn_blocking(move || handle_incoming_sub_push_frame(&frame, &ctx_step)).await {
              Ok(Ok(())) => status = Status::Ok,
              _ => {
                status = Status::BadReq;
                break;
              }
            }
          }
          if write_iroh_frame(&mut send, &[status as u8]).await.is_ok() {
            let _ = send.finish();
            let _ = timeout(Duration::from_millis(200), conn.closed()).await;
          }
        });
      }
    });
  }

  /// 通过 `iroh` QUIC 双向流向订阅/做种节点发送 `mzstd` 压缩的 `SubPush` 增量变更通知（包含路径、权限、分块签名）
  pub(crate) fn send_sub_push(
    &self,
    target_id: PeerId,
    fallback_addrs: &[SocketAddr],
    push: &SubPush,
  ) -> Result<(), PeerError> {
    let batches = encode_push_batches(push)?;
    let mut target_addrs = resolve_iroh_quic_addrs(&target_id);
    for &addr in fallback_addrs {
      if !target_addrs.contains(&addr) {
        target_addrs.push(addr);
      }
    }

    if target_id != ZERO_PEER_ID
      && target_id != *self.endpoint.id().as_bytes()
      && let Ok(pk) = PublicKey::from_bytes(&target_id)
      && !target_addrs.is_empty()
    {
      let ep = self.endpoint.clone();
      let addrs_ref = &target_addrs;
      let batches_ref = &batches;
      let iroh_res: Result<(), PeerError> = iroh_rt().block_on(async move {
        let ep_addr =
          EndpointAddr::from_parts(pk, addrs_ref.iter().copied().map(TransportAddr::Ip));
        let conn = with_timeout(IROH_DIAL_TIMEOUT, ep.connect(ep_addr, S0_IROH_ALPN))
          .await?
          .map_err(iroh_err)?;
        let (mut send, mut recv) = with_timeout(IROH_IO_TIMEOUT, conn.open_bi())
          .await?
          .map_err(iroh_err)?;
        with_timeout(IROH_IO_TIMEOUT, send.write_all(&[PROTO_VERSION]))
          .await?
          .map_err(iroh_err)?;
        for payload in batches_ref {
          write_iroh_frame(&mut send, payload).await?;
        }
        let _ = send.finish();
        let ack = read_iroh_frame(&mut recv).await?;
        conn.close(0u32.into(), b"ok");
        if ack.first().copied() == Some(Status::Ok as u8) {
          Ok(())
        } else {
          Err(PeerError::RemoteStatus(Status::BadReq))
        }
      });
      if iroh_res.is_ok() {
        return Ok(());
      }
    }

    let mut stream = connect_ctrl_addrs(&target_addrs)?;
    for payload in &batches {
      write_frame(&mut stream, payload)?;
      let ack = read_frame(&mut stream)?;
      if ack.first().copied() != Some(Status::Ok as u8) {
        return Err(PeerError::RemoteStatus(Status::BadReq));
      }
    }
    Ok(())
  }

  pub(crate) fn close(&self) {
    let ep = self.endpoint.clone();
    iroh_rt().spawn(async move {
      ep.close().await;
    });
  }
}
