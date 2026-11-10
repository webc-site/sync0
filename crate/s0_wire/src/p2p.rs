use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

use s0_cdc::{PathChange, Rd, WireError, Wr};
use s0_const::{
  ADDR_TAG_V4, ADDR_TAG_V6, FLAG_BYTE_RANGE, FLAG_RANGE, FLAG_RW, HASH_LEN, IPV4_OCTETS,
  IPV6_OCTETS, MAX_ENCODED_ADDR_LEN, MAX_SOCKET_ADDRS, Op, P2P_MAGIC, PEER_ID_LEN, PROTO_VERSION,
  Perm, SHARE_ID_LEN, SUB_MAGIC, Status, TOKEN_LEN, ZERO_HASH, region::Region,
};

const REQ_USER_FLAG_MASK: u8 = !(FLAG_RANGE | FLAG_RW | FLAG_BYTE_RANGE);

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
    Ok(Self {
      endpoint_id: r.array::<PEER_ID_LEN>()?,
      region: Region::from_u16(r.u16()?),
      weight_rate: r.uvar()?,
      blk_lo: r.uvar_u32()?,
      blk_hi: r.uvar_u32()?,
      peer_addrs: r.vec_u15()?.to_vec(),
      profile_card: r.vec_u15()?.to_vec(),
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
    Ok(Self {
      share_id: r.array::<SHARE_ID_LEN>()?,
      name: r.str_u15()?.to_owned(),
      perm: Perm::from_repr(r.u8()?).ok_or(WireError::BadStatus)?,
      eff_rate: r.uvar()?,
      write_prefixes: r.strs_front()?,
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
  #[inline]
  pub const fn new(
    op: Op,
    flags: u8,
    share_id: [u8; SHARE_ID_LEN],
    peer_addrs: Vec<u8>,
    profile_card: Vec<u8>,
  ) -> Self {
    Self {
      op,
      flags,
      share_id,
      req_rate: 0,
      paths: Vec::new(),
      peer_addrs,
      profile_card,
      range: None,
      token: None,
      byte_range: None,
    }
  }

  pub fn encode(&self) -> Result<Vec<u8>, WireError> {
    let mut out = Vec::with_capacity(96 + self.paths.len() * 12);
    let mut w = Wr::new(&mut out);
    w.u32(P2P_MAGIC);
    w.u8(PROTO_VERSION);
    w.u8(self.op as u8);
    let mut flags = self.flags & REQ_USER_FLAG_MASK;
    if self.range.is_some() {
      flags |= FLAG_RANGE;
    }
    if self.token.is_some() {
      flags |= FLAG_RW;
    }
    if self.byte_range.is_some() {
      flags |= FLAG_BYTE_RANGE;
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
    if r.u8()? != PROTO_VERSION {
      return Err(WireError::BadVersion);
    }
    let op = Op::from_repr(r.u8()?).ok_or(WireError::BadOp)?;
    let raw_flags = r.u8()?;
    let flags = raw_flags & REQ_USER_FLAG_MASK;
    let share_id = r.array::<SHARE_ID_LEN>()?;
    let req_rate = r.uvar()?;
    let mut paths = r.strs_front()?;
    paths.retain_mut(|p| {
      let clean = p.trim_matches(['/', '\\']);
      if clean.is_empty() {
        return false;
      }
      if clean.len() != p.len() {
        *p = clean.to_owned();
      }
      true
    });
    let peer_addrs = r.vec_u15()?.to_vec();
    let profile_card = r.vec_u15()?.to_vec();
    let range = if raw_flags & FLAG_RANGE != 0 {
      let rh = r.array::<HASH_LEN>()?;
      let lo = r.uvar_u32()?;
      let hi = r.uvar_u32()?;
      Some((rh, lo, hi))
    } else {
      None
    };
    let token = if raw_flags & FLAG_RW != 0 {
      Some(r.array::<TOKEN_LEN>()?)
    } else {
      None
    };
    let byte_range = if raw_flags & FLAG_BYTE_RANGE != 0 {
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
      root_hash: ZERO_HASH,
      peer_addrs: Vec::new(),
      profile_card: Vec::new(),
      peers: Vec::new(),
      manifest: Vec::new(),
    }
  }

  pub fn encode(&self) -> Result<Vec<u8>, WireError> {
    if self.status != Status::Ok {
      let mut out = Vec::with_capacity(5);
      let mut w = Wr::new(&mut out);
      w.u32(P2P_MAGIC);
      w.u8(self.status as u8);
      return Ok(out);
    }
    let mut out = Vec::with_capacity(48 + self.manifest.len());
    let mut w = Wr::new(&mut out);
    w.u32(P2P_MAGIC);
    w.u8(self.status as u8);
    w.u8(self.flags);
    w.uvar(self.eff_rate);
    w.bytes(&self.root_hash);
    w.slice_u15(&self.peer_addrs)?;
    w.slice_u15(&self.profile_card)?;
    encode_peers(&mut w, &self.peers)?;
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
    if status != Status::Ok {
      return Ok(Self::err(status));
    }
    let flags = r.u8()?;
    let eff_rate = r.uvar()?;
    let root_hash = r.array::<HASH_LEN>()?;
    let peer_addrs = r.vec_u15()?.to_vec();
    let profile_card = r.vec_u15()?.to_vec();
    let peers = decode_peers(&mut r)?;
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

/// `iroh` 订阅/群组实时增量变更推送帧 (`SubPush`)：
/// 携带接收方有效权限 (`Perm`)、可写路径前缀 (`write_prefixes`)、做种节点列表及内联 `ChunkSig` 的增量路径变更表
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubPush {
  pub flags: u8,
  pub share_id: [u8; SHARE_ID_LEN],
  pub perm: Perm,
  pub eff_rate: u64,
  pub write_prefixes: Vec<String>,
  pub root_hash: [u8; HASH_LEN],
  pub origin_addrs: Vec<u8>,
  pub peers: Vec<SwarmPeer>,
  pub changes: Vec<PathChange>,
}

impl SubPush {
  pub fn encode(&self) -> Result<Vec<u8>, WireError> {
    let mut out = Vec::with_capacity(64 + self.changes.len() * 32);
    let mut w = Wr::new(&mut out);
    w.u32(SUB_MAGIC);
    w.u8(PROTO_VERSION);
    w.u8(self.flags);
    w.bytes(&self.share_id);
    w.u8(self.perm as u8);
    w.uvar(self.eff_rate);
    w.strs_front(&self.write_prefixes)?;
    w.bytes(&self.root_hash);
    w.slice_u15(&self.origin_addrs)?;
    encode_peers(&mut w, &self.peers)?;
    PathChange::encode_slice(&self.changes, &mut w)?;
    Ok(out)
  }

  pub fn decode(buf: &[u8]) -> Result<Self, WireError> {
    let mut r = Rd::new(buf);
    if r.u32()? != SUB_MAGIC {
      return Err(WireError::BadMagic);
    }
    if r.u8()? != PROTO_VERSION {
      return Err(WireError::BadVersion);
    }
    let flags = r.u8()?;
    let share_id = r.array::<SHARE_ID_LEN>()?;
    let perm = Perm::from_repr(r.u8()?).ok_or(WireError::BadStatus)?;
    let eff_rate = r.uvar()?;
    let write_prefixes = r.strs_front()?;
    let root_hash = r.array::<HASH_LEN>()?;
    let origin_addrs = r.vec_u15()?.to_vec();
    let peers = decode_peers(&mut r)?;
    let changes = PathChange::decode_vec(&mut r)?;
    Ok(Self {
      flags,
      share_id,
      perm,
      eff_rate,
      write_prefixes,
      root_hash,
      origin_addrs,
      peers,
      changes,
    })
  }
}

fn encode_peers(w: &mut Wr<'_>, peers: &[SwarmPeer]) -> Result<(), WireError> {
  w.uvar(peers.len() as u64);
  for p in peers {
    p.encode_into(w)?;
  }
  Ok(())
}

fn decode_peers(r: &mut Rd<'_>) -> Result<Vec<SwarmPeer>, WireError> {
  let peer_cnt = r.uvar_usize()?;
  let mut peers = Vec::with_capacity(peer_cnt.min(64));
  for _ in 0..peer_cnt {
    peers.push(SwarmPeer::decode_from(r)?);
  }
  Ok(peers)
}

/// 将多个 `SocketAddr` 编码为无长度前缀的紧凑二进制标签序列（`ADDR_TAG_V4` + 4B IPv4 + 2B Port 或 `ADDR_TAG_V6` + 16B IPv6 + 2B Port）
pub fn encode_raw_socket_addrs(addrs: &[SocketAddr], max_cnt: usize, out: &mut Vec<u8>) {
  for addr in addrs.iter().take(max_cnt) {
    match addr {
      SocketAddr::V4(v4) => {
        out.push(ADDR_TAG_V4);
        out.extend_from_slice(&v4.ip().octets());
      }
      SocketAddr::V6(v6) => {
        out.push(ADDR_TAG_V6);
        out.extend_from_slice(&v6.ip().octets());
      }
    }
    out.extend_from_slice(&addr.port().to_le_bytes());
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
        out.push(SocketAddr::V4(SocketAddrV4::new(ip, r.u16()?)));
      }
      ADDR_TAG_V6 => {
        let ip = Ipv6Addr::from(r.array::<IPV6_OCTETS>()?);
        out.push(SocketAddr::V6(SocketAddrV6::new(ip, r.u16()?, 0, 0)));
      }
      _ => return Err(WireError::BadMagic),
    }
  }
  Ok(out)
}

/// 编解码多个 UDP 候选打洞地址 (`SocketAddr`)
pub fn encode_socket_addrs(addrs: &[SocketAddr]) -> Vec<u8> {
  let mut out = Vec::with_capacity(addrs.len().min(MAX_SOCKET_ADDRS) * MAX_ENCODED_ADDR_LEN);
  encode_raw_socket_addrs(addrs, MAX_SOCKET_ADDRS, &mut out);
  out
}

pub fn decode_socket_addrs(buf: &[u8]) -> Result<Vec<SocketAddr>, WireError> {
  decode_raw_socket_addrs(buf, MAX_SOCKET_ADDRS)
}
