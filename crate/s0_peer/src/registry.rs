use std::{
  env::var_os,
  fs::{self, OpenOptions},
  io::{self, Read, Seek, SeekFrom, Write},
  net::SocketAddr,
  path::{Path, PathBuf},
  sync::OnceLock,
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use fastrand::u64 as rand_u64;
use gxhash::HashMap;
use parking_lot::RwLock;
use s0_const::{MAX_ENCODED_ADDR_LEN, MAX_SOCKET_ADDRS, PEER_ID_LEN, SECRET_KEY_LEN, ZERO_PEER_ID};
use s0_wire::{SecretKey, decode_raw_socket_addrs, encode_raw_socket_addrs};

use crate::PeerError;

/// 内部 32 字节二进制 `iroh` 节点身份标识（Ed25519 公钥，栈上定长 32B，零堆分配）
pub type PeerId = [u8; PEER_ID_LEN];

const REG_FILE_PATH: &str = "/tmp/s0_iroh_reg.bin";
const REG_ENTRY_LEN: usize = PEER_ID_LEN + MAX_ENCODED_ADDR_LEN;
const MAX_REG_ENTRIES: usize = 512;

fn global_iroh_routes() -> &'static RwLock<HashMap<PeerId, Vec<SocketAddr>>> {
  static ROUTES: OnceLock<RwLock<HashMap<PeerId, Vec<SocketAddr>>>> = OnceLock::new();
  ROUTES.get_or_init(|| RwLock::new(HashMap::default()))
}

pub(crate) fn dedup_addrs(addrs: &[SocketAddr]) -> Vec<SocketAddr> {
  let mut dedup = Vec::with_capacity(addrs.len());
  for &a in addrs {
    if !dedup.contains(&a) {
      dedup.push(a);
    }
  }
  dedup
}

/// 注册 32 字节二进制 `PeerId` 的多个候选端点（支持 IPv6 + IPv4 双栈同时注册）到进程内高速路由表与跨进程注册表
pub fn register_iroh_endpoints(id: PeerId, addrs: &[SocketAddr]) {
  if addrs.is_empty() {
    return;
  }
  let dedup = dedup_addrs(addrs);
  {
    let mut routes = global_iroh_routes().write();
    if routes.get(&id) == Some(&dedup) {
      return;
    }
    routes.insert(id, dedup.clone());
  }
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
  for_each_reg_entry(|id, addr| {
    let mut routes = global_iroh_routes().write();
    let list = routes.entry(id).or_default();
    if !list.iter().any(|a| a.is_ipv6() == addr.is_ipv6()) {
      list.push(addr);
    }
    if !out.contains(&id) {
      out.push(id);
    }
  });
  out
}

fn encode_reg_entry(id: &PeerId, addr: SocketAddr) -> [u8; REG_ENTRY_LEN] {
  let mut buf = [0u8; REG_ENTRY_LEN];
  buf[..PEER_ID_LEN].copy_from_slice(id);
  let mut raw_addr = Vec::with_capacity(MAX_ENCODED_ADDR_LEN);
  encode_raw_socket_addrs(&[addr], 1, &mut raw_addr);
  buf[PEER_ID_LEN..PEER_ID_LEN + raw_addr.len()].copy_from_slice(&raw_addr);
  buf
}

fn decode_reg_addr(buf: &[u8; REG_ENTRY_LEN]) -> Option<SocketAddr> {
  decode_raw_socket_addrs(&buf[PEER_ID_LEN..], 1)
    .ok()
    .and_then(|mut v| v.pop())
}

fn write_reg_entry(id: &PeerId, addr: SocketAddr) -> io::Result<()> {
  let mut f = OpenOptions::new()
    .create(true)
    .read(true)
    .write(true)
    .truncate(false)
    .open(REG_FILE_PATH)?;
  let _ = f.try_lock();
  let rec = encode_reg_entry(id, addr);
  let len = f.metadata().map(|m| m.len() as usize).unwrap_or(0);
  if len < MAX_REG_ENTRIES * REG_ENTRY_LEN && len.is_multiple_of(REG_ENTRY_LEN) {
    f.seek(SeekFrom::End(0))?;
    f.write_all(&rec)?;
    let _ = f.unlock();
    return Ok(());
  }

  let mut raw = Vec::with_capacity(len + REG_ENTRY_LEN);
  f.seek(SeekFrom::Start(0))?;
  f.read_to_end(&mut raw)?;
  raw.extend_from_slice(&rec);

  let mut compacted: Vec<[u8; REG_ENTRY_LEN]> = Vec::with_capacity(MAX_REG_ENTRIES / 2);
  for chunk in raw.as_chunks::<REG_ENTRY_LEN>().0.iter().rev() {
    if !compacted
      .iter()
      .any(|c| c[..PEER_ID_LEN] == chunk[..PEER_ID_LEN] && c[PEER_ID_LEN] == chunk[PEER_ID_LEN])
    {
      compacted.push(*chunk);
      if compacted.len() >= MAX_REG_ENTRIES / 2 {
        break;
      }
    }
  }
  compacted.reverse();
  let flat: &[u8] = compacted.as_flattened();
  f.seek(SeekFrom::Start(0))?;
  f.set_len(0)?;
  f.write_all(flat)?;
  let _ = f.unlock();
  Ok(())
}

fn for_each_reg_entry(mut f: impl FnMut(PeerId, SocketAddr)) {
  let Ok(mut file) = OpenOptions::new().read(true).open(REG_FILE_PATH) else {
    return;
  };
  let _ = file.try_lock();
  let mut raw = Vec::new();
  let ok = file.read_to_end(&mut raw).is_ok();
  let _ = file.unlock();
  if !ok {
    return;
  }
  for chunk in raw.as_chunks::<REG_ENTRY_LEN>().0.iter().rev() {
    if let Some(addr) = decode_reg_addr(chunk) {
      let mut id = ZERO_PEER_ID;
      id.copy_from_slice(&chunk[..PEER_ID_LEN]);
      f(id, addr);
    }
  }
}

fn read_reg_entries(id: &PeerId) -> Vec<SocketAddr> {
  let mut out = Vec::new();
  for_each_reg_entry(|entry_id, addr| {
    if &entry_id == id
      && !out
        .iter()
        .any(|a: &SocketAddr| a.is_ipv6() == addr.is_ipv6())
    {
      out.push(addr);
    }
  });
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
  Fixed([u8; SECRET_KEY_LEN]),
  /// 从指定配置目录加载或生成持久化 `iroh.key` 与 `iroh.id`
  Persistent(PathBuf),
}

impl KeyMode {
  pub fn resolve_secret_key(&self) -> Result<SecretKey, PeerError> {
    match self {
      Self::Ephemeral => {
        let mut seed = [0u8; SECRET_KEY_LEN];
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
          let _ = write_b64_line(&id_path, sk.public().as_bytes());
          return Ok(sk);
        }
        let sk = Self::Ephemeral.resolve_secret_key()?;
        write_b64_line(&sk_path, &sk.to_bytes())?;
        write_b64_line(&id_path, sk.public().as_bytes())?;
        Ok(sk)
      }
    }
  }
}

fn write_b64_line(path: &Path, bytes: &PeerId) -> io::Result<()> {
  let mut line = encode_id_b64(bytes);
  line.push('\n');
  fs::write(path, line)
}

/// 将 32 字节 `iroh` 公钥/私钥编码为 43 字符无填充 `base64url`
#[inline]
pub fn encode_id_b64(bytes: &PeerId) -> String {
  URL_SAFE_NO_PAD.encode(bytes)
}

/// 将 32 字节 `iroh` 公钥与多个双栈候选端点紧凑编码为单一 `base64url` 地址
pub(crate) fn encode_iroh_addrs_b64(id: &PeerId, addrs: &[SocketAddr]) -> String {
  let mut raw = Vec::with_capacity(PEER_ID_LEN + addrs.len() * MAX_ENCODED_ADDR_LEN);
  raw.extend_from_slice(id);
  encode_raw_socket_addrs(addrs, MAX_SOCKET_ADDRS, &mut raw);
  URL_SAFE_NO_PAD.encode(&raw)
}

/// 解码紧凑二进制端点列表并注册到 `PeerId` 路由表（忽略全零 `PeerId` 或空地址）
pub(crate) fn register_encoded_endpoints(id: PeerId, raw_addrs: &[u8]) {
  if id != ZERO_PEER_ID
    && let Ok(addrs) = decode_raw_socket_addrs(raw_addrs, MAX_SOCKET_ADDRS)
    && !addrs.is_empty()
  {
    register_iroh_endpoints(id, &addrs);
  }
}

/// 解码 `base64url` 配置的 `iroh` 地址为 32 字节二进制 `PeerId`（自动注册内嵌的 IPv4/IPv6 双栈端点）
pub fn decode_id_b64(s: &str) -> Option<PeerId> {
  let trimmed = s.trim();
  let clean = trimmed
    .strip_prefix("iroh://")
    .or_else(|| trimmed.strip_prefix("iroh:"))
    .unwrap_or(trimmed);
  let raw = URL_SAFE_NO_PAD.decode(clean).ok()?;
  if raw.len() == PEER_ID_LEN {
    return raw.try_into().ok();
  }
  if raw.len() > PEER_ID_LEN {
    let mut id = ZERO_PEER_ID;
    id.copy_from_slice(&raw[..PEER_ID_LEN]);
    register_encoded_endpoints(id, &raw[PEER_ID_LEN..]);
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
