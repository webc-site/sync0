use std::{
  env::var_os,
  fs::{self, OpenOptions},
  io,
  io::{Read, Seek, SeekFrom, Write},
  net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
  path::PathBuf,
  sync::OnceLock,
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use fastrand::u64 as rand_u64;
use gxhash::HashMap;
use parking_lot::RwLock;

use super::PeerError;
use crate::{
  profile::SecretKey,
  wire::{decode_raw_socket_addrs, encode_raw_socket_addrs},
};

/// 内部 32 字节二进制 `iroh` 节点身份标识（Ed25519 公钥，栈上定长 32B，零堆分配）
pub type PeerId = [u8; 32];

const REG_FILE_PATH: &str = "/tmp/s0_iroh_reg.bin";
const REG_ENTRY_LEN: usize = 51;
const MAX_REG_ENTRIES: usize = 512;

fn global_iroh_routes() -> &'static RwLock<HashMap<PeerId, Vec<SocketAddr>>> {
  static ROUTES: OnceLock<RwLock<HashMap<PeerId, Vec<SocketAddr>>>> = OnceLock::new();
  ROUTES.get_or_init(|| RwLock::new(HashMap::default()))
}

/// 注册 32 字节二进制 `PeerId` 的多个候选端点（支持 IPv6 + IPv4 双栈同时注册）到进程内高速路由表与跨进程注册表
pub fn register_iroh_endpoints(id: PeerId, addrs: &[SocketAddr]) {
  if addrs.is_empty() {
    return;
  }
  let mut dedup = Vec::with_capacity(addrs.len());
  for &a in addrs {
    if !dedup.contains(&a) {
      dedup.push(a);
    }
  }
  global_iroh_routes().write().insert(id, dedup.clone());
  for &a in &dedup {
    let _ = write_reg_entry(&id, a);
  }
}

/// 从 32 字节二进制 `PeerId` 解析全部可达控制面端点列表（IPv6 + IPv4 双栈候选）
pub fn resolve_iroh_endpoints(id: &PeerId) -> Vec<SocketAddr> {
  if let Some(list) = global_iroh_routes().read().get(id)
    && !list.is_empty()
  {
    return list.clone();
  }
  let entries = read_reg_entries(id);
  if !entries.is_empty() {
    global_iroh_routes().write().insert(*id, entries.clone());
  }
  entries
}

/// 从 32 字节二进制 `PeerId` 解析首选控制面可达端点
pub fn resolve_iroh_endpoint(id: &PeerId) -> Option<SocketAddr> {
  resolve_iroh_endpoints(id).into_iter().next()
}

/// 获取当前已知或本机活跃的全部 32 字节二进制 `PeerId` 列表（用于免 `--peer` 自动发现组内做种节点）
pub fn active_iroh_peers() -> Vec<PeerId> {
  let mut out: Vec<PeerId> = global_iroh_routes().read().keys().copied().collect();
  if let Ok(mut f) = OpenOptions::new().read(true).open(REG_FILE_PATH) {
    let _ = f.try_lock();
    let mut raw = Vec::new();
    if f.read_to_end(&mut raw).is_ok() {
      let mut routes = global_iroh_routes().write();
      for chunk in raw.rchunks_exact(REG_ENTRY_LEN) {
        if let Ok(arr) = chunk.try_into()
          && let Some(addr) = decode_reg_addr(arr)
        {
          let mut id = [0u8; 32];
          id.copy_from_slice(&chunk[..32]);
          let list = routes.entry(id).or_default();
          if !list.iter().any(|a| a.is_ipv6() == addr.is_ipv6()) {
            list.push(addr);
          }
          if !out.contains(&id) {
            out.push(id);
          }
        }
      }
    }
    let _ = f.unlock();
  }
  out
}

fn encode_reg_entry(id: &PeerId, addr: SocketAddr) -> [u8; REG_ENTRY_LEN] {
  let mut buf = [0u8; REG_ENTRY_LEN];
  buf[..32].copy_from_slice(id);
  match addr {
    SocketAddr::V4(v4) => {
      buf[32] = 4;
      buf[33..37].copy_from_slice(&v4.ip().octets());
      buf[49..51].copy_from_slice(&v4.port().to_le_bytes());
    }
    SocketAddr::V6(v6) => {
      buf[32] = 6;
      buf[33..49].copy_from_slice(&v6.ip().octets());
      buf[49..51].copy_from_slice(&v6.port().to_le_bytes());
    }
  }
  buf
}

fn decode_reg_addr(buf: &[u8; REG_ENTRY_LEN]) -> Option<SocketAddr> {
  let port = u16::from_le_bytes([buf[49], buf[50]]);
  match buf[32] {
    4 => {
      let ip = Ipv4Addr::new(buf[33], buf[34], buf[35], buf[36]);
      Some(SocketAddr::V4(SocketAddrV4::new(ip, port)))
    }
    6 => {
      let mut oct = [0u8; 16];
      oct.copy_from_slice(&buf[33..49]);
      Some(SocketAddr::V6(SocketAddrV6::new(
        Ipv6Addr::from(oct),
        port,
        0,
        0,
      )))
    }
    _ => None,
  }
}

fn write_reg_entry(id: &PeerId, addr: SocketAddr) -> io::Result<()> {
  let mut f = OpenOptions::new()
    .create(true)
    .read(true)
    .write(true)
    .truncate(false)
    .open(REG_FILE_PATH)?;
  let _ = f.try_lock();
  let mut raw = Vec::new();
  f.read_to_end(&mut raw)?;

  let rec = encode_reg_entry(id, addr);
  let mut found = false;
  for chunk in raw.as_chunks_mut::<REG_ENTRY_LEN>().0 {
    if chunk[..32] == id[..] && chunk[32] == rec[32] {
      chunk.copy_from_slice(&rec);
      found = true;
      break;
    }
  }
  if !found {
    if raw.len() >= MAX_REG_ENTRIES * REG_ENTRY_LEN {
      raw.drain(..REG_ENTRY_LEN);
    }
    raw.extend_from_slice(&rec);
  }
  f.seek(SeekFrom::Start(0))?;
  f.set_len(0)?;
  f.write_all(&raw)?;
  let _ = f.unlock();
  Ok(())
}

fn read_reg_entries(id: &PeerId) -> Vec<SocketAddr> {
  let Ok(mut f) = OpenOptions::new().read(true).open(REG_FILE_PATH) else {
    return Vec::new();
  };
  let _ = f.try_lock();
  let mut raw = Vec::new();
  let _ = f.read_to_end(&mut raw);
  let _ = f.unlock();
  let mut out = Vec::new();
  for chunk in raw.rchunks_exact(REG_ENTRY_LEN) {
    if chunk[..32] == id[..]
      && let Ok(arr) = chunk.try_into()
      && let Some(addr) = decode_reg_addr(arr)
      && !out
        .iter()
        .any(|a: &SocketAddr| a.is_ipv6() == addr.is_ipv6())
    {
      out.push(addr);
    }
  }
  out
}

/// 默认配置目录：`$HOME/.config/sync0`（Windows 回退 `%USERPROFILE%/.config/sync0`）
pub fn default_config_dir() -> Option<PathBuf> {
  var_os("HOME")
    .or_else(|| var_os("USERPROFILE"))
    .filter(|s| !s.is_empty())
    .map(|h| PathBuf::from(h).join(".config").join("sync0"))
}

/// 对等节点密钥管理模式
#[derive(Debug, Clone, Default)]
pub enum KeyMode {
  /// 内存中随机生成临时 `iroh` 密钥对（默认零配置）
  #[default]
  Ephemeral,
  /// 使用固定 32 字节私钥种子
  Fixed([u8; 32]),
  /// 从指定配置目录加载或生成持久化 `iroh.key` 与 `iroh.id`
  Persistent(PathBuf),
}

impl KeyMode {
  pub fn resolve_secret_key(&self) -> Result<SecretKey, PeerError> {
    match self {
      Self::Ephemeral => {
        let mut seed = [0u8; 32];
        for chunk in seed.as_chunks_mut::<8>().0 {
          chunk.copy_from_slice(&rand_u64(..).to_le_bytes());
        }
        Ok(SecretKey::from_bytes(&seed))
      }
      Self::Fixed(seed) => Ok(SecretKey::from_bytes(seed)),
      Self::Persistent(dir) => {
        fs::create_dir_all(dir)?;
        let sk_path = dir.join("iroh.key");
        let id_path = dir.join("iroh.id");
        if let Ok(text) = fs::read_to_string(&sk_path)
          && let Some(bytes) = decode_id_b64(text.trim())
        {
          let sk = SecretKey::from_bytes(&bytes);
          let _ = fs::write(
            &id_path,
            format!("{}\n", encode_id_b64(sk.public().as_bytes())),
          );
          return Ok(sk);
        }
        let sk = Self::Ephemeral.resolve_secret_key()?;
        fs::write(&sk_path, format!("{}\n", encode_id_b64(&sk.to_bytes())))?;
        fs::write(
          &id_path,
          format!("{}\n", encode_id_b64(sk.public().as_bytes())),
        )?;
        Ok(sk)
      }
    }
  }
}

/// 将 32 字节 `iroh` 公钥/私钥编码为 43 字符无填充 `base64url`
#[inline]
pub fn encode_id_b64(bytes: &PeerId) -> String {
  URL_SAFE_NO_PAD.encode(bytes)
}

/// 将 32 字节 `iroh` 公钥与多个双栈候选端点紧凑编码为单一 `base64url` 地址
pub fn encode_iroh_addrs_b64(id: &PeerId, addrs: &[SocketAddr]) -> String {
  let mut raw = Vec::with_capacity(32 + addrs.len() * 19);
  raw.extend_from_slice(id);
  encode_raw_socket_addrs(addrs, 8, &mut raw);
  URL_SAFE_NO_PAD.encode(&raw)
}

/// 解码 `base64url` 配置的 `iroh` 地址为 32 字节二进制 `PeerId`（自动注册内嵌的 IPv4/IPv6 双栈端点）
pub fn decode_id_b64(s: &str) -> Option<PeerId> {
  let trimmed = s.trim();
  let clean = trimmed
    .strip_prefix("iroh://")
    .or_else(|| trimmed.strip_prefix("iroh:"))
    .unwrap_or(trimmed);
  let raw = URL_SAFE_NO_PAD.decode(clean).ok()?;
  if raw.len() == 32 {
    return raw.try_into().ok();
  }
  if raw.len() > 32 {
    let mut id = [0u8; 32];
    id.copy_from_slice(&raw[..32]);
    if let Ok(addrs) = decode_raw_socket_addrs(&raw[32..], 16)
      && !addrs.is_empty()
    {
      register_iroh_endpoints(id, &addrs);
    }
    return Some(id);
  }
  None
}

/// 解析逗号分隔的多个 `base64url` `iroh` 地址为二进制 `Vec<PeerId>`
pub fn parse_iroh_ids_csv(s: &str) -> Option<Vec<PeerId>> {
  let mut out = Vec::new();
  for part in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
    out.push(decode_id_b64(part)?);
  }
  Some(out)
}
