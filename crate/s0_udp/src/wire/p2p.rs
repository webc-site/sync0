use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

use s0_cdc::{Rd, WireError, Wr};

use crate::{
  consts::{
    FLAG_BYTE_RANGE, FLAG_RANGE, FLAG_RW, Op, P2P_MAGIC, PROTO_VERSION, Perm, SUB_MAGIC, Status,
  },
  region::Region,
};

/// P2P 分发群（Swarm）中的在线对等节点描述（内部全二进制 `endpoint_id: [u8; 32]`）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwarmPeer {
  pub endpoint_id: [u8; 32],
  pub region: Region,
  pub weight_rate: u64,
  pub blk_lo: u32,
  pub blk_hi: u32,
  pub peer_addrs: Vec<u8>,
  pub profile_card: Vec<u8>,
}

impl SwarmPeer {
  pub fn encode_into(&self, w: &mut Wr<'_>) -> Result<(), WireError> {
    w.bytes(&self.endpoint_id);
    w.u16(self.region.as_u16());
    w.u16(0);
    w.u64(self.weight_rate);
    w.u32(self.blk_lo);
    w.u32(self.blk_hi);
    w.slice_u15(&self.peer_addrs)?;
    w.slice_u15(&self.profile_card)?;
    Ok(())
  }

  pub fn decode_from(r: &mut Rd<'_>) -> Result<Self, WireError> {
    let endpoint_id = r.array::<32>()?;
    let region = Region::from_u16(r.u16()?);
    let _rsv = r.u16()?;
    let weight_rate = r.u64()?;
    let blk_lo = r.u32()?;
    let blk_hi = r.u32()?;
    let peer_addrs = r.vec_u15()?.to_vec();
    let profile_card = r.vec_u15()?.to_vec();
    Ok(Self {
      endpoint_id,
      region,
      weight_rate,
      blk_lo,
      blk_hi,
      peer_addrs,
      profile_card,
    })
  }
}

/// 单用户多文件分享组摘要条目（用于 `Op::ListShares = 7`）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareSummary {
  pub share_id: [u8; 16],
  pub name: String,
  pub perm: Perm,
  pub eff_rate: u64,
  /// 当前访问者在该分享组允许上传的路径前缀（空列表表示允许整个分享根目录）
  pub write_prefixes: Vec<String>,
}

impl ShareSummary {
  pub fn encode_into(&self, w: &mut Wr<'_>) -> Result<(), WireError> {
    w.bytes(&self.share_id);
    w.slice_u15(self.name.as_bytes())?;
    w.u8(self.perm as u8);
    w.u64(self.eff_rate);
    w.u15(self.write_prefixes.len())?;
    for p in &self.write_prefixes {
      w.slice_u15(p.as_bytes())?;
    }
    Ok(())
  }

  pub fn decode_from(r: &mut Rd<'_>) -> Result<Self, WireError> {
    let share_id = r.array::<16>()?;
    let name = r.str_u15()?.to_owned();
    let perm = Perm::from_repr(r.u8()?).ok_or(WireError::BadStatus)?;
    let eff_rate = r.u64()?;
    let cnt = r.u15()? as usize;
    let mut write_prefixes = Vec::with_capacity(cnt.min(64));
    for _ in 0..cnt {
      write_prefixes.push(r.str_u15()?.to_owned());
    }
    Ok(Self {
      share_id,
      name,
      perm,
      eff_rate,
      write_prefixes,
    })
  }
}

pub fn encode_share_summaries(list: &[ShareSummary]) -> Result<Vec<u8>, WireError> {
  let mut out = Vec::with_capacity(4 + list.len() * 32);
  let mut w = Wr::new(&mut out);
  w.u15(list.len())?;
  for item in list {
    item.encode_into(&mut w)?;
  }
  Ok(out)
}

pub fn decode_share_summaries(buf: &[u8]) -> Result<Vec<ShareSummary>, WireError> {
  if buf.is_empty() {
    return Ok(Vec::new());
  }
  let mut r = Rd::new(buf);
  let cnt = r.u15()? as usize;
  let mut out = Vec::with_capacity(cnt.min(128));
  for _ in 0..cnt {
    out.push(ShareSummary::decode_from(&mut r)?);
  }
  Ok(out)
}

/// 对等控制面请求帧 (`P2pReq`)
///
/// `paths` 为多选同步的文件或目录相对路径列表：
/// - `paths.is_empty()` 表示同步整个分享组（全量 `""`）；
/// - `paths` 含 1 个或多个元素时表示按需选择性同步（支持同时选择多个文件与多个目录）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P2pReq {
  pub op: Op,
  pub flags: u8,
  pub share_id: [u8; 16],
  pub req_rate: u64,
  pub paths: Vec<String>,
  pub peer_addrs: Vec<u8>,
  pub profile_card: Vec<u8>,
  pub range: Option<([u8; 16], u32, u32)>,
  pub token: Option<[u8; 16]>,
  pub byte_range: Option<(u64, u64)>,
}

impl P2pReq {
  pub fn encode(&self) -> Result<Vec<u8>, WireError> {
    let mut out = Vec::with_capacity(144 + self.paths.len() * 16);
    let mut w = Wr::new(&mut out);
    w.u32(P2P_MAGIC);
    w.u8(PROTO_VERSION);
    w.u8(self.op as u8);
    let mut flags = self.flags;
    if self.range.is_some() {
      flags |= FLAG_RANGE;
    } else {
      flags &= !FLAG_RANGE;
    }
    if self.token.is_some() {
      flags |= FLAG_RW;
    } else {
      flags &= !FLAG_RW;
    }
    if self.byte_range.is_some() {
      flags |= FLAG_BYTE_RANGE;
    } else {
      flags &= !FLAG_BYTE_RANGE;
    }
    w.u8(flags);
    w.u8(0);
    w.bytes(&self.share_id);
    w.u64(self.req_rate);
    w.u15(self.paths.len())?;
    for p in &self.paths {
      w.slice_u15(p.as_bytes())?;
    }
    w.slice_u15(&self.peer_addrs)?;
    w.slice_u15(&self.profile_card)?;
    if let Some((root_hash, lo, hi)) = self.range {
      w.bytes(&root_hash);
      w.u32(lo);
      w.u32(hi);
    }
    if let Some(tok) = self.token {
      w.bytes(&tok);
    }
    if let Some((b_lo, b_hi)) = self.byte_range {
      w.u64(b_lo);
      w.u64(b_hi);
    }
    Ok(out)
  }

  pub fn decode(buf: &[u8]) -> Result<Self, WireError> {
    let mut r = Rd::new(buf);
    if r.u32()? != P2P_MAGIC {
      return Err(WireError::BadMagic);
    }
    if r.u8()? != PROTO_VERSION {
      return Err(WireError::BadVersion);
    }
    let op = Op::from_repr(r.u8()?).ok_or(WireError::BadOp)?;
    let flags = r.u8()?;
    let _rsv = r.u8()?;
    let share_id = r.array::<16>()?;
    let req_rate = r.u64()?;
    let path_cnt = r.u15()? as usize;
    let mut paths = Vec::with_capacity(path_cnt.min(128));
    for _ in 0..path_cnt {
      let p = r.str_u15()?;
      let clean = p.trim_matches(['/', '\\']);
      if !clean.is_empty() {
        paths.push(clean.to_owned());
      }
    }
    let peer_addrs = r.vec_u15()?.to_vec();
    let profile_card = r.vec_u15()?.to_vec();
    let range = if flags & FLAG_RANGE != 0 {
      let rh = r.array::<16>()?;
      let lo = r.u32()?;
      let hi = r.u32()?;
      Some((rh, lo, hi))
    } else {
      None
    };
    let token = if flags & FLAG_RW != 0 {
      Some(r.array::<16>()?)
    } else {
      None
    };
    let byte_range = if flags & FLAG_BYTE_RANGE != 0 {
      Some((r.u64()?, r.u64()?))
    } else {
      None
    };
    Ok(Self {
      op,
      flags,
      share_id,
      req_rate,
      paths,
      peer_addrs,
      profile_card,
      range,
      token,
      byte_range,
    })
  }
}

/// 对等控制面响应帧 (`P2pResp`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct P2pResp {
  pub status: Status,
  pub flags: u8,
  pub eff_rate: u64,
  pub root_hash: [u8; 16],
  pub peer_addrs: Vec<u8>,
  pub profile_card: Vec<u8>,
  pub peers: Vec<SwarmPeer>,
  pub manifest: Vec<u8>,
}

impl P2pResp {
  pub fn err(status: Status) -> Self {
    Self {
      status,
      flags: 0,
      eff_rate: 0,
      root_hash: [0u8; 16],
      peer_addrs: Vec::new(),
      profile_card: Vec::new(),
      peers: Vec::new(),
      manifest: Vec::new(),
    }
  }

  pub fn encode(&self) -> Result<Vec<u8>, WireError> {
    let mut out = Vec::with_capacity(64 + self.manifest.len());
    let mut w = Wr::new(&mut out);
    w.u32(P2P_MAGIC);
    w.u8(self.status as u8);
    w.u8(self.flags);
    w.u16(0);
    w.u64(self.eff_rate);
    w.bytes(&self.root_hash);
    w.slice_u15(&self.peer_addrs)?;
    w.slice_u15(&self.profile_card)?;
    w.u15(self.peers.len())?;
    for p in &self.peers {
      p.encode_into(&mut w)?;
    }
    w.u32(self.manifest.len() as u32);
    w.bytes(&self.manifest);
    Ok(out)
  }

  pub fn decode(buf: &[u8]) -> Result<Self, WireError> {
    let mut r = Rd::new(buf);
    if r.u32()? != P2P_MAGIC {
      return Err(WireError::BadMagic);
    }
    let status = Status::from_repr(r.u8()?).ok_or(WireError::BadStatus)?;
    let flags = r.u8()?;
    let _rsv = r.u16()?;
    let eff_rate = r.u64()?;
    let root_hash = r.array::<16>()?;
    let peer_addrs = r.vec_u15()?.to_vec();
    let profile_card = r.vec_u15()?.to_vec();
    let peer_cnt = r.u15()? as usize;
    let mut peers = Vec::with_capacity(peer_cnt.min(64));
    for _ in 0..peer_cnt {
      peers.push(SwarmPeer::decode_from(&mut r)?);
    }
    let mf_len = r.u32()? as usize;
    let manifest = r.bytes(mf_len)?.to_vec();
    Ok(Self {
      status,
      flags,
      eff_rate,
      root_hash,
      peer_addrs,
      profile_card,
      peers,
      manifest,
    })
  }
}

/// 订阅/群组实时推送帧 (`SubPush`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubPush {
  pub flags: u8,
  pub share_id: [u8; 16],
  pub eff_rate: u64,
  pub root_hash: [u8; 16],
  pub origin_addrs: Vec<u8>,
  pub peers: Vec<SwarmPeer>,
  pub manifest: Vec<u8>,
}

impl SubPush {
  pub fn encode(&self) -> Result<Vec<u8>, WireError> {
    let mut out = Vec::with_capacity(64 + self.manifest.len());
    let mut w = Wr::new(&mut out);
    w.u32(SUB_MAGIC);
    w.u8(self.flags);
    w.u8(0);
    w.u16(0);
    w.bytes(&self.share_id);
    w.u64(self.eff_rate);
    w.bytes(&self.root_hash);
    w.slice_u15(&self.origin_addrs)?;
    w.u15(self.peers.len())?;
    for p in &self.peers {
      p.encode_into(&mut w)?;
    }
    w.u32(self.manifest.len() as u32);
    w.bytes(&self.manifest);
    Ok(out)
  }

  pub fn decode(buf: &[u8]) -> Result<Self, WireError> {
    let mut r = Rd::new(buf);
    if r.u32()? != SUB_MAGIC {
      return Err(WireError::BadMagic);
    }
    let flags = r.u8()?;
    let _ = r.u8()?;
    let _ = r.u16()?;
    let share_id = r.array::<16>()?;
    let eff_rate = r.u64()?;
    let root_hash = r.array::<16>()?;
    let origin_addrs = r.vec_u15()?.to_vec();
    let peer_cnt = r.u15()? as usize;
    let mut peers = Vec::with_capacity(peer_cnt.min(64));
    for _ in 0..peer_cnt {
      peers.push(SwarmPeer::decode_from(&mut r)?);
    }
    let mf_len = r.u32()? as usize;
    let manifest = r.bytes(mf_len)?.to_vec();
    Ok(Self {
      flags,
      share_id,
      eff_rate,
      root_hash,
      origin_addrs,
      peers,
      manifest,
    })
  }
}

/// 编解码多个 UDP 候选打洞地址 (`SocketAddr`)
pub fn encode_socket_addrs(addrs: &[SocketAddr]) -> Vec<u8> {
  let mut out = Vec::with_capacity(1 + addrs.len() * 19);
  out.push(addrs.len().min(16) as u8);
  for addr in addrs.iter().take(16) {
    match addr {
      SocketAddr::V4(v4) => {
        out.push(4);
        out.extend_from_slice(&v4.ip().octets());
        out.extend_from_slice(&v4.port().to_le_bytes());
      }
      SocketAddr::V6(v6) => {
        out.push(6);
        out.extend_from_slice(&v6.ip().octets());
        out.extend_from_slice(&v6.port().to_le_bytes());
      }
    }
  }
  out
}

pub fn decode_socket_addrs(buf: &[u8]) -> Result<Vec<SocketAddr>, WireError> {
  if buf.is_empty() {
    return Ok(Vec::new());
  }
  let mut r = Rd::new(buf);
  let cnt = r.u8()? as usize;
  let mut out = Vec::with_capacity(cnt.min(16));
  for _ in 0..cnt {
    match r.u8()? {
      4 => {
        let ip = Ipv4Addr::from(r.array::<4>()?);
        let port = r.u16()?;
        out.push(SocketAddr::V4(SocketAddrV4::new(ip, port)));
      }
      6 => {
        let ip = Ipv6Addr::from(r.array::<16>()?);
        let port = r.u16()?;
        out.push(SocketAddr::V6(SocketAddrV6::new(ip, port, 0, 0)));
      }
      _ => return Err(WireError::BadMagic),
    }
  }
  Ok(out)
}
