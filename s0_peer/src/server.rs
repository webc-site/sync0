use std::{
  io::{self, Read, Write},
  net::{SocketAddr, TcpStream},
  sync::Arc,
  time::Duration,
};

use gxhash::HashMap;
use parking_lot::RwLock;
use s0_cdc::Manifest;
use s0_const::{
  FLAG_DELETE, FLAG_PARTIAL, FLAG_RANGE, FLAG_SEED, HAVE_ALL_ONES, Op, Perm, SUB_MAGIC, Status,
};
use s0_fs::{FsError, SharedSink, SourceReader, build_manifest_from_path, is_all_have};
use s0_profile::SignedProfile;
use s0_region::Region;
use s0_udp::{RangeSpec, bind_udp_ephemeral, punch_udp, recv_range_udp, send_range_udp};
use s0_wire::{P2pReq, P2pResp, SubPush, SwarmPeer, decode_socket_addrs, encode_socket_addrs};

use crate::{PeerError, ShareConfig, min_nonzero};

/// 单帧控制面消息最大字节数上限（64 MiB，防止恶意帧长度触发 OOM）
const MAX_CTRL_FRAME_LEN: usize = 64 * 1024 * 1024;

#[derive(Clone)]
pub(crate) struct SubscriberRecord {
  pub(crate) endpoint_id: [u8; 32],
  pub(crate) ctrl_addr: SocketAddr,
  pub(crate) prefix: String,
  pub(crate) eff_rate: u64,
}

#[derive(Clone)]
pub(crate) struct SwarmSeedRecord {
  pub(crate) endpoint_id: [u8; 32],
  pub(crate) region: Region,
  pub(crate) ctrl_addr: SocketAddr,
  pub(crate) profile_card: Vec<u8>,
  pub(crate) root_hash: [u8; 16],
  pub(crate) blk_lo: u32,
  pub(crate) blk_hi: u32,
  pub(crate) weight_rate: u64,
}

pub(crate) struct ShareState {
  pub(crate) cfg: ShareConfig,
  pub(crate) sub_prefix: String,
  pub(crate) subscribers: Vec<SubscriberRecord>,
  pub(crate) seeds: Vec<SwarmSeedRecord>,
}

#[derive(Clone)]
pub(crate) struct ServerCtx {
  pub(crate) my_id: [u8; 32],
  pub(crate) my_region: Region,
  pub(crate) my_card: Vec<u8>,
  pub(crate) my_ctrl_addr: SocketAddr,
  pub(crate) shares: Arc<RwLock<HashMap<[u8; 16], ShareState>>>,
  pub(crate) known_profiles: Arc<RwLock<HashMap<[u8; 32], SignedProfile>>>,
}

pub(crate) struct RangePullJob<'a> {
  pub(crate) peer: &'a SwarmPeer,
  pub(crate) share_id: [u8; 16],
  pub(crate) subpath: &'a str,
  pub(crate) root_hash: [u8; 16],
  pub(crate) blk_lo: u32,
  pub(crate) blk_hi: u32,
  pub(crate) req_rate: u64,
  pub(crate) token: Option<[u8; 16]>,
  pub(crate) my_card: Vec<u8>,
  pub(crate) sink: &'a SharedSink,
}

/// 从 `root_hash` 前 3 字节与区间起始块号派生 24-bit UDP 会话标识
#[inline]
pub(crate) const fn derive_session_id(root_hash: &[u8; 16], salt: u32) -> u32 {
  (((root_hash[0] as u32) << 16) | ((root_hash[1] as u32) << 8) | (root_hash[2] as u32)) ^ salt
}

/// 从 `root_hash` 前 3 字节派生 Push 会话标识
#[inline]
pub(crate) const fn derive_push_session_id(root_hash: &[u8; 16]) -> u32 {
  u32::from_le_bytes([root_hash[0], root_hash[1], root_hash[2], 0])
}

pub(crate) fn pull_range_from_swarm_peer(job: RangePullJob<'_>) -> Result<(u32, u64), PeerError> {
  let addrs = decode_socket_addrs(&job.peer.peer_addrs)?;
  let ctrl_addr = *addrs.first().ok_or(PeerError::NoHealthyPeer)?;

  let (udp_sock, my_udp_addrs) = bind_udp_ephemeral()?;
  let req = P2pReq {
    op: Op::Pull,
    flags: FLAG_RANGE,
    share_id: job.share_id,
    req_rate: job.req_rate,
    path: job.subpath.to_owned(),
    peer_addrs: my_udp_addrs,
    profile_card: job.my_card,
    range: Some((job.root_hash, job.blk_lo, job.blk_hi)),
    token: job.token,
  };

  let mut stream = TcpStream::connect_timeout(&ctrl_addr, Duration::from_secs(3))?;
  let _ = stream.set_nodelay(true);
  write_frame(&mut stream, &req.encode()?)?;

  let have = job.sink.snapshot_have_bits();
  write_frame(&mut stream, &have)?;

  let resp_raw = read_frame(&mut stream)?;
  let resp = P2pResp::decode(&resp_raw)?;
  if resp.status != Status::Ok {
    return Err(PeerError::RemoteStatus(resp.status));
  }

  let sid = derive_session_id(&job.root_hash, job.blk_lo);
  let peer_udp = punch_udp(&udp_sock, &resp.peer_addrs, sid)?;
  let st = recv_range_udp(
    &udp_sock,
    &peer_udp,
    job.sink,
    RangeSpec {
      session_id: sid,
      blk_lo: job.blk_lo,
      blk_hi: job.blk_hi,
      eff_rate: resp.eff_rate,
      timeout: Duration::from_secs(8),
    },
  )?;
  Ok((st.blocks_transferred, st.bytes_transferred))
}

pub(crate) fn handle_conn(mut stream: TcpStream, ctx: &ServerCtx) -> Result<(), PeerError> {
  let frame = read_frame(&mut stream)?;
  if let Some(&hdr4) = frame.first_chunk::<4>()
    && u32::from_le_bytes(hdr4) == SUB_MAGIC
  {
    return handle_incoming_sub_push(&mut stream, &frame, ctx);
  }

  let req = P2pReq::decode(&frame)?;
  let mut remote_id = [0u8; 32];
  let mut remote_region = Region::Unknown;
  if !req.profile_card.is_empty() {
    let card = SignedProfile::decode_and_verify(&req.profile_card)?;
    remote_id = card.pubkey;
    remote_region = card.region;
    ctx.known_profiles.write().insert(remote_id, card);
  }

  let (share_cfg, seeds_snapshot) = {
    let guard = ctx.shares.read();
    let Some(st) = guard.get(&req.share_id) else {
      write_frame(&mut stream, &P2pResp::err(Status::NotFound).encode()?)?;
      return Ok(());
    };
    (st.cfg.clone(), st.seeds.clone())
  };

  let Some(access) = share_cfg.resolve_access(&remote_id, req.req_rate, req.token) else {
    write_frame(&mut stream, &P2pResp::err(Status::Unauth).encode()?)?;
    return Ok(());
  };

  if req.op == Op::Push && access.perm != Perm::Rw {
    write_frame(&mut stream, &P2pResp::err(Status::Forbidden).encode()?)?;
    return Ok(());
  }

  match req.op {
    Op::Stat => {
      let Ok(mf) =
        build_manifest_from_path(&share_cfg.root, &req.path, req.flags & FLAG_DELETE != 0)
      else {
        write_frame(&mut stream, &P2pResp::err(Status::NotFound).encode()?)?;
        return Ok(());
      };
      let root_hash = mf.root_hash().map_err(FsError::Cdc)?;
      let mf_bytes = mf.encode().map_err(FsError::Cdc)?;
      let total_blks = mf.total_blocks();

      let mut peers = Vec::with_capacity(1 + seeds_snapshot.len());
      peers.push(SwarmPeer {
        endpoint_id: ctx.my_id,
        region: ctx.my_region,
        weight_rate: access.eff_rate,
        blk_lo: 0,
        blk_hi: total_blks,
        peer_addrs: encode_socket_addrs(&[ctx.my_ctrl_addr]),
        profile_card: ctx.my_card.clone(),
      });
      for s in &seeds_snapshot {
        if s.root_hash == root_hash && s.endpoint_id != remote_id {
          peers.push(SwarmPeer {
            endpoint_id: s.endpoint_id,
            region: s.region,
            weight_rate: min_nonzero(s.weight_rate, access.eff_rate),
            blk_lo: s.blk_lo,
            blk_hi: s.blk_hi,
            peer_addrs: encode_socket_addrs(&[s.ctrl_addr]),
            profile_card: s.profile_card.clone(),
          });
        }
      }

      let resp = P2pResp {
        status: Status::Ok,
        flags: 0,
        eff_rate: access.eff_rate,
        root_hash,
        peer_addrs: encode_socket_addrs(&[ctx.my_ctrl_addr]),
        profile_card: ctx.my_card.clone(),
        peers,
        manifest: mf_bytes,
      };
      write_frame(&mut stream, &resp.encode()?)?;
    }

    Op::Pull => {
      let have_bits = read_frame(&mut stream)?;
      let Ok(mf) = build_manifest_from_path(&share_cfg.root, &req.path, false) else {
        write_frame(&mut stream, &P2pResp::err(Status::NotFound).encode()?)?;
        return Ok(());
      };
      let root_hash = mf.root_hash().map_err(FsError::Cdc)?;
      let total = mf.total_blocks();
      let (blk_lo, blk_hi) = if let Some((req_rh, lo, hi)) = req.range {
        if req_rh != root_hash {
          write_frame(&mut stream, &P2pResp::err(Status::Conflict).encode()?)?;
          return Ok(());
        }
        (lo.min(total), hi.min(total))
      } else {
        (0, total)
      };

      let (udp_sock, my_udp_addrs) = bind_udp_ephemeral()?;
      let resp = P2pResp {
        status: Status::Ok,
        flags: 0,
        eff_rate: access.eff_rate,
        root_hash,
        peer_addrs: my_udp_addrs,
        profile_card: ctx.my_card.clone(),
        peers: Vec::new(),
        manifest: Vec::new(),
      };
      write_frame(&mut stream, &resp.encode()?)?;

      if blk_lo < blk_hi {
        let reader = SourceReader::open(&share_cfg.root, mf)?;
        let sid = derive_session_id(&root_hash, blk_lo);
        let peer_udp = punch_udp(&udp_sock, &req.peer_addrs, sid)?;
        let _ = send_range_udp(
          &udp_sock,
          &peer_udp,
          &reader,
          &have_bits,
          RangeSpec {
            session_id: sid,
            blk_lo,
            blk_hi,
            eff_rate: access.eff_rate,
            timeout: Duration::from_secs(8),
          },
        );
      }
    }

    Op::Push => {
      let mf_raw = read_frame(&mut stream)?;
      let mut mf = Manifest::decode(&mf_raw).map_err(|e| PeerError::Fs(FsError::Cdc(e)))?;
      mf.delete_extraneous = req.flags & FLAG_DELETE != 0;
      mf.partial = req.flags & FLAG_PARTIAL != 0;
      let root_hash = mf.root_hash().map_err(FsError::Cdc)?;
      let sink = SharedSink::prepare(&share_cfg.root, mf)?;
      let total = sink.total_blocks();

      let (udp_sock, my_udp_addrs) = bind_udp_ephemeral()?;
      let resp = P2pResp {
        status: Status::Ok,
        flags: 0,
        eff_rate: access.eff_rate,
        root_hash,
        peer_addrs: my_udp_addrs,
        profile_card: ctx.my_card.clone(),
        peers: Vec::new(),
        manifest: Vec::new(),
      };
      write_frame(&mut stream, &resp.encode()?)?;

      let init_have = sink.initial_have_bits();
      if total == 0 || is_all_have(init_have, total) {
        write_frame(&mut stream, &HAVE_ALL_ONES.to_le_bytes())?;
      } else {
        write_frame(&mut stream, init_have)?;
        let sid = derive_push_session_id(&root_hash);
        let peer_udp = punch_udp(&udp_sock, &req.peer_addrs, sid)?;
        recv_range_udp(
          &udp_sock,
          &peer_udp,
          &sink,
          RangeSpec {
            session_id: sid,
            blk_lo: 0,
            blk_hi: total,
            eff_rate: access.eff_rate,
            timeout: Duration::from_secs(10),
          },
        )?;
      }
      sink.commit()?;
      let done = P2pResp {
        status: Status::Ok,
        flags: 0,
        eff_rate: access.eff_rate,
        root_hash,
        peer_addrs: Vec::new(),
        profile_card: Vec::new(),
        peers: Vec::new(),
        manifest: Vec::new(),
      };
      write_frame(&mut stream, &done.encode()?)?;
    }

    Op::Sub | Op::GroupJoin => {
      let peer_ctrl_addrs = decode_socket_addrs(&req.peer_addrs)?;
      if let Some(&peer_ctrl) = peer_ctrl_addrs.first() {
        let mut guard = ctx.shares.write();
        if let Some(st) = guard.get_mut(&req.share_id) {
          st.subscribers.retain(|s| s.ctrl_addr != peer_ctrl);
          st.subscribers.push(SubscriberRecord {
            endpoint_id: remote_id,
            ctrl_addr: peer_ctrl,
            prefix: req.path.clone(),
            eff_rate: access.eff_rate,
          });
          if let Some((rh, lo, hi)) = req.range {
            st.seeds.retain(|s| s.ctrl_addr != peer_ctrl);
            st.seeds.push(SwarmSeedRecord {
              endpoint_id: remote_id,
              region: remote_region,
              ctrl_addr: peer_ctrl,
              profile_card: req.profile_card,
              root_hash: rh,
              blk_lo: lo,
              blk_hi: hi,
              weight_rate: access.eff_rate,
            });
          }
        }
      }
      let resp = P2pResp {
        status: Status::Ok,
        flags: 0,
        eff_rate: access.eff_rate,
        root_hash: [0u8; 16],
        peer_addrs: encode_socket_addrs(&[ctx.my_ctrl_addr]),
        profile_card: ctx.my_card.clone(),
        peers: Vec::new(),
        manifest: Vec::new(),
      };
      write_frame(&mut stream, &resp.encode()?)?;
    }

    Op::Have => {
      let peer_ctrl_addrs = decode_socket_addrs(&req.peer_addrs)?;
      if let (Some(&peer_ctrl), Some((rh, lo, hi))) = (peer_ctrl_addrs.first(), req.range) {
        let mut guard = ctx.shares.write();
        if let Some(st) = guard.get_mut(&req.share_id) {
          st.seeds.retain(|s| s.ctrl_addr != peer_ctrl);
          st.seeds.push(SwarmSeedRecord {
            endpoint_id: remote_id,
            region: remote_region,
            ctrl_addr: peer_ctrl,
            profile_card: req.profile_card,
            root_hash: rh,
            blk_lo: lo,
            blk_hi: hi,
            weight_rate: access.eff_rate,
          });
        }
      }
      let resp = P2pResp {
        status: Status::Ok,
        flags: 0,
        eff_rate: access.eff_rate,
        root_hash: [0u8; 16],
        peer_addrs: Vec::new(),
        profile_card: Vec::new(),
        peers: Vec::new(),
        manifest: Vec::new(),
      };
      write_frame(&mut stream, &resp.encode()?)?;
    }
  }

  Ok(())
}

fn handle_incoming_sub_push(
  stream: &mut TcpStream,
  frame: &[u8],
  ctx: &ServerCtx,
) -> Result<(), PeerError> {
  let push = SubPush::decode(frame)?;
  let (local_root, sub_prefix) = {
    let guard = ctx.shares.read();
    let Some(st) = guard.get(&push.share_id) else {
      return Ok(());
    };
    (st.cfg.root.clone(), st.sub_prefix.clone())
  };

  let mf = Manifest::decode(&push.manifest).map_err(|e| PeerError::Fs(FsError::Cdc(e)))?;
  let sink = SharedSink::prepare(&local_root, mf)?;
  let total = sink.total_blocks();

  if total > 0 && !sink.is_range_complete(0, total) {
    for peer in &push.peers {
      if peer.endpoint_id == ctx.my_id {
        continue;
      }
      if sink.is_range_complete(0, total) {
        break;
      }
      let _ = pull_range_from_swarm_peer(RangePullJob {
        peer,
        share_id: push.share_id,
        subpath: &sub_prefix,
        root_hash: push.root_hash,
        blk_lo: 0,
        blk_hi: total,
        req_rate: push.eff_rate,
        token: None,
        my_card: ctx.my_card.clone(),
        sink: &sink,
      });
    }
  }
  sink.commit()?;

  if let Ok(origin_addrs) = decode_socket_addrs(&push.origin_addrs)
    && let Some(&origin_ctrl) = origin_addrs.first()
  {
    let have_req = P2pReq {
      op: Op::Have,
      flags: FLAG_SEED,
      share_id: push.share_id,
      req_rate: 0,
      path: String::new(),
      peer_addrs: encode_socket_addrs(&[ctx.my_ctrl_addr]),
      profile_card: ctx.my_card.clone(),
      range: Some((push.root_hash, 0, total)),
      token: None,
    };
    if let Ok(mut s) = TcpStream::connect_timeout(&origin_ctrl, Duration::from_secs(2)) {
      let _ = write_frame(&mut s, &have_req.encode()?);
      let _ = read_frame(&mut s);
    }
  }

  write_frame(stream, &[0u8])?;
  Ok(())
}

#[inline]
pub(crate) fn is_have_all_ones_frame(buf: &[u8]) -> bool {
  buf
    .first_chunk::<4>()
    .is_some_and(|&b| buf.len() == 4 && u32::from_le_bytes(b) == HAVE_ALL_ONES)
}

pub(crate) fn write_frame(stream: &mut TcpStream, payload: &[u8]) -> io::Result<()> {
  let len = payload.len() as u32;
  stream.write_all(&len.to_le_bytes())?;
  stream.write_all(payload)?;
  stream.flush()
}

pub(crate) fn read_frame(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
  let mut len_buf = [0u8; 4];
  stream.read_exact(&mut len_buf)?;
  let len = u32::from_le_bytes(len_buf) as usize;
  if len > MAX_CTRL_FRAME_LEN {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "control frame exceeds maximum length",
    ));
  }
  let mut out = vec![0u8; len];
  stream.read_exact(&mut out)?;
  Ok(out)
}
