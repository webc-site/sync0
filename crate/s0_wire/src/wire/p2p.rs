use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

use s0_cdc::{Rd, WireError, Wr};

use crate::{
  consts::{
    ADDR_TAG_V4, ADDR_TAG_V6, FLAG_BYTE_RANGE, FLAG_RANGE, FLAG_RW, HASH_LEN, IPV4_OCTETS,
    IPV6_OCTETS, MAX_CTRL_PATHS, MAX_ENCODED_ADDR_LEN, MAX_SOCKET_ADDRS, MAX_WRITE_PREFIXES, Op,
    P2P_MAGIC, PEER_ID_LEN, PROTO_OP_MASK, PROTO_VER_SHIFT, PROTO_VERSION, Perm, SHARE_ID_LEN,
    SUB_MAGIC, Status, TOKEN_LEN,
  },
  region::Region,
};

/// P2P 分发群（Swarm）中的在线对等节点描述（内部全二进制 `endpoint_id: [u8; PEER_ID_LEN]`）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwarmPeer {
  pub endpoint_id: [u8; PEER_ID_LEN],
  pub region: Region,
  pub weight_rate: u64,
  pub blk_lo: u32,
  pub blk_hi: u32,
  pub peer_addrs: Vec<u8>,
  pub profile_card: Vec<u8>,
}

impl SwarmPeer {
  fn encode_into(&self, w: &mut Wr<'_>) -> Result<(), WireError> {
    w.bytes(&self.endpoint_id);
    w.u16(self.region.as_u16());
    w.uvar(self.weight_rate);
    w.uvar(u64::from(self.blk_lo));
    w.uvar(u64::from(self.blk_hi));
    w.slice_u15(&self.peer_addrs)?;
    w.slice_u15(&self.profile_card)?;
    Ok(())
  }

  fn decode_from(r: &mut Rd<'_>) -> Result<Self, WireError> {
    let endpoint_id = r.array::<PEER_ID_LEN>()?;
    let region = Region::from_u16(r.u16()?);
    let weight_rate = r.uvar()?;
    let blk_lo = r.uvar_u32()?;
    let blk_hi = r.uvar_u32()?;
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
  pub share_id: [u8; SHARE_ID_LEN],
  pub name: String,
  pub perm: Perm,
  pub eff_rate: u64,
  /// 当前访问者在该分享组允许上传的路径前缀（空列表表示允许整个分享根目录）
  pub write_prefixes: Vec<String>,
}

impl ShareSummary {
  fn encode_into(&self, w: &mut Wr<'_>) -> Result<(), WireError> {
    w.bytes(&self.share_id);
    w.slice_u15(self.name.as_bytes())?;
    w.u8(self.perm as u8);
    w.uvar(self.eff_rate);
    w.strs_front(&self.write_prefixes)?;
    Ok(())
  }

  fn decode_from(r: &mut Rd<'_>) -> Result<Self, WireError> {
    let share_id = r.array::<SHARE_ID_LEN>()?;
    let name = r.str_u15()?.to_owned();
    let perm = Perm::from_repr(r.u8()?).ok_or(WireError::BadStatus)?;
    let eff_rate = r.uvar()?;
    let write_prefixes = r.strs_front(MAX_WRITE_PREFIXES)?;
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
  w.uvar(list.len() as u64);
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
  let cnt = r.uvar_usize()?;
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
  pub share_id: [u8; SHARE_ID_LEN],
  pub req_rate: u64,
  pub paths: Vec<String>,
  pub peer_addrs: Vec<u8>,
  pub profile_card: Vec<u8>,
  pub range: Option<([u8; HASH_LEN], u32, u32)>,
  pub token: Option<[u8; TOKEN_LEN]>,
  pub byte_range: Option<(u64, u64)>,
}

impl P2pReq {
  pub fn encode(&self) -> Result<Vec<u8>, WireError> {
    let mut out = Vec::with_capacity(96 + self.paths.len() * 12);
    let mut w = Wr::new(&mut out);
    w.u32(P2P_MAGIC);
    w.u8((PROTO_VERSION << PROTO_VER_SHIFT) | (self.op as u8));
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
    w.bytes(&self.share_id);
    w.uvar(self.req_rate);
    w.strs_front(&self.paths)?;
    w.slice_u15(&self.peer_addrs)?;
    w.slice_u15(&self.profile_card)?;
    if let Some((root_hash, lo, hi)) = self.range {
      w.bytes(&root_hash);
      w.uvar(u64::from(lo));
      w.uvar(u64::from(hi));
    }
    if let Some(tok) = self.token {
      w.bytes(&tok);
    }
    if let Some((b_lo, b_hi)) = self.byte_range {
      w.uvar(b_lo);
      w.uvar(b_hi);
    }
    Ok(out)
  }

  pub fn decode(buf: &[u8]) -> Result<Self, WireError> {
    let mut r = Rd::new(buf);
    if r.u32()? != P2P_MAGIC {
      return Err(WireError::BadMagic);
    }
    let ver_op = r.u8()?;
    if (ver_op >> PROTO_VER_SHIFT) != PROTO_VERSION {
      return Err(WireError::BadVersion);
    }
    let op = Op::from_repr(ver_op & PROTO_OP_MASK).ok_or(WireError::BadOp)?;
    let flags = r.u8()?;
    let share_id = r.array::<SHARE_ID_LEN>()?;
    let req_rate = r.uvar()?;
    let raw_paths = r.strs_front(MAX_CTRL_PATHS)?;
    let mut paths = Vec::with_capacity(raw_paths.len());
    for p in raw_paths {
      let clean = p.trim_matches(['/', '\\']);
      if !clean.is_empty() {
        paths.push(clean.to_owned());
      }
    }
    let peer_addrs = r.vec_u15()?.to_vec();
    let profile_card = r.vec_u15()?.to_vec();
    let range = if flags & FLAG_RANGE != 0 {
      let rh = r.array::<HASH_LEN>()?;
      let lo = r.uvar_u32()?;
      let hi = r.uvar_u32()?;
      Some((rh, lo, hi))
    } else {
      None
    };
    let token = if flags & FLAG_RW != 0 {
      Some(r.array::<TOKEN_LEN>()?)
    } else {
      None
    };
    let byte_range = if flags & FLAG_BYTE_RANGE != 0 {
      Some((r.uvar()?, r.uvar()?))
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
  pub root_hash: [u8; HASH_LEN],
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
      root_hash: [0u8; HASH_LEN],
      peer_addrs: Vec::new(),
      profile_card: Vec::new(),
      peers: Vec::new(),
      manifest: Vec::new(),
    }
  }

  pub fn encode(&self) -> Result<Vec<u8>, WireError> {
    let mut out = Vec::with_capacity(48 + self.manifest.len());
    let mut w = Wr::new(&mut out);
    w.u32(P2P_MAGIC);
    w.u8(self.status as u8);
    w.u8(self.flags);
    w.uvar(self.eff_rate);
    w.bytes(&self.root_hash);
    w.slice_u15(&self.peer_addrs)?;
    w.slice_u15(&self.profile_card)?;
    w.uvar(self.peers.len() as u64);
    for p in &self.peers {
      p.encode_into(&mut w)?;
    }
    w.uvar(self.manifest.len() as u64);
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
    let eff_rate = r.uvar()?;
    let root_hash = r.array::<HASH_LEN>()?;
    let peer_addrs = r.vec_u15()?.to_vec();
    let profile_card = r.vec_u15()?.to_vec();
    let peer_cnt = r.uvar_usize()?;
    let mut peers = Vec::with_capacity(peer_cnt.min(64));
    for _ in 0..peer_cnt {
      peers.push(SwarmPeer::decode_from(&mut r)?);
    }
    let mf_len = r.uvar_usize()?;
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
  pub share_id: [u8; SHARE_ID_LEN],
  pub eff_rate: u64,
  pub root_hash: [u8; HASH_LEN],
  pub origin_addrs: Vec<u8>,
  pub peers: Vec<SwarmPeer>,
  pub manifest: Vec<u8>,
}

impl SubPush {
  pub fn encode(&self) -> Result<Vec<u8>, WireError> {
    let mut out = Vec::with_capacity(48 + self.manifest.len());
    let mut w = Wr::new(&mut out);
    w.u32(SUB_MAGIC);
    w.u8(self.flags);
    w.bytes(&self.share_id);
    w.uvar(self.eff_rate);
    w.bytes(&self.root_hash);
    w.slice_u15(&self.origin_addrs)?;
    w.uvar(self.peers.len() as u64);
    for p in &self.peers {
      p.encode_into(&mut w)?;
    }
    w.uvar(self.manifest.len() as u64);
    w.bytes(&self.manifest);
    Ok(out)
  }

  pub fn decode(buf: &[u8]) -> Result<Self, WireError> {
    let mut r = Rd::new(buf);
    if r.u32()? != SUB_MAGIC {
      return Err(WireError::BadMagic);
    }
    let flags = r.u8()?;
    let share_id = r.array::<SHARE_ID_LEN>()?;
    let eff_rate = r.uvar()?;
    let root_hash = r.array::<HASH_LEN>()?;
    let origin_addrs = r.vec_u15()?.to_vec();
    let peer_cnt = r.uvar_usize()?;
    let mut peers = Vec::with_capacity(peer_cnt.min(64));
    for _ in 0..peer_cnt {
      peers.push(SwarmPeer::decode_from(&mut r)?);
    }
    let mf_len = r.uvar_usize()?;
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

/// 将多个 `SocketAddr` 编码为无长度前缀的紧凑二进制标签序列（`ADDR_TAG_V4` + 4B IPv4 + 2B Port 或 `ADDR_TAG_V6` + 16B IPv6 + 2B Port）
pub fn encode_raw_socket_addrs(addrs: &[SocketAddr], max_cnt: usize, out: &mut Vec<u8>) {
  for addr in addrs.iter().take(max_cnt) {
    match addr {
      SocketAddr::V4(v4) => {
        out.push(ADDR_TAG_V4);
        out.extend_from_slice(&v4.ip().octets());
        out.extend_from_slice(&v4.port().to_le_bytes());
      }
      SocketAddr::V6(v6) => {
        out.push(ADDR_TAG_V6);
        out.extend_from_slice(&v6.ip().octets());
        out.extend_from_slice(&v6.port().to_le_bytes());
      }
    }
  }
}

/// 解码无长度前缀的紧凑二进制 `SocketAddr` 标签序列
pub fn decode_raw_socket_addrs(buf: &[u8], max_cnt: usize) -> Result<Vec<SocketAddr>, WireError> {
  let mut r = Rd::new(buf);
  let mut out = Vec::with_capacity(max_cnt.min(MAX_SOCKET_ADDRS));
  while !r.is_empty() && out.len() < max_cnt {
    match r.u8()? {
      ADDR_TAG_V4 => {
        let ip = Ipv4Addr::from(r.array::<IPV4_OCTETS>()?);
        let port = r.u16()?;
        out.push(SocketAddr::V4(SocketAddrV4::new(ip, port)));
      }
      ADDR_TAG_V6 => {
        let ip = Ipv6Addr::from(r.array::<IPV6_OCTETS>()?);
        let port = r.u16()?;
        out.push(SocketAddr::V6(SocketAddrV6::new(ip, port, 0, 0)));
      }
      _ => return Err(WireError::BadMagic),
    }
  }
  Ok(out)
}

/// 编解码多个 UDP 候选打洞地址 (`SocketAddr`)
pub fn encode_socket_addrs(addrs: &[SocketAddr]) -> Vec<u8> {
  let mut out = Vec::with_capacity(1 + addrs.len() * MAX_ENCODED_ADDR_LEN);
  out.push(addrs.len().min(MAX_SOCKET_ADDRS) as u8);
  encode_raw_socket_addrs(addrs, MAX_SOCKET_ADDRS, &mut out);
  out
}

pub fn decode_socket_addrs(buf: &[u8]) -> Result<Vec<SocketAddr>, WireError> {
  if buf.is_empty() {
    return Ok(Vec::new());
  }
  let cnt = buf[0] as usize;
  let out = decode_raw_socket_addrs(&buf[1..], cnt)?;
  if out.len() < cnt {
    return Err(WireError::Truncated);
  }
  Ok(out)
}
