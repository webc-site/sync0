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
use gxhash::HashMap;
use parking_lot::RwLock;

use super::PeerError;
use crate::profile::SecretKey;

/// 内部 32 字节二进制 `iroh` 节点身份标识（Ed25519 公钥，栈上定长 32B，零堆分配）
pub type PeerId = [u8; 32];

const REG_FILE_PATH: &str = "/tmp/s0_iroh_reg.bin";
const REG_ENTRY_LEN: usize = 51;
const MAX_REG_ENTRIES: usize = 512;

fn global_iroh_routes() -> &'static RwLock<HashMap<PeerId, SocketAddr>> {
  static ROUTES: OnceLock<RwLock<HashMap<PeerId, SocketAddr>>> = OnceLock::new();
  ROUTES.get_or_init(|| RwLock::new(HashMap::default()))
}

/// 注册 32 字节二进制 `PeerId` 到进程内高速路由表与本机跨进程二进制注册表
pub fn register_iroh_endpoint(id: PeerId, addr: SocketAddr) {
  global_iroh_routes().write().insert(id, addr);
  let _ = write_reg_entry(&id, addr);
}

/// 从 32 字节二进制 `PeerId` 解析控制面可达端点
pub fn resolve_iroh_endpoint(id: &PeerId) -> Option<SocketAddr> {
  if let Some(&addr) = global_iroh_routes().read().get(id) {
    return Some(addr);
  }
  if let Some(addr) = read_reg_entry(id) {
    global_iroh_routes().write().insert(*id, addr);
    return Some(addr);
  }
  None
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
          routes.insert(id, addr);
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
    if chunk[..32] == id[..] {
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

fn read_reg_entry(id: &PeerId) -> Option<SocketAddr> {
  let mut f = OpenOptions::new().read(true).open(REG_FILE_PATH).ok()?;
  let _ = f.try_lock();
  let mut raw = Vec::new();
  f.read_to_end(&mut raw).ok()?;
  let _ = f.unlock();
  for chunk in raw.rchunks_exact(REG_ENTRY_LEN) {
    if chunk[..32] == id[..] {
      let arr: &[u8; REG_ENTRY_LEN] = chunk.try_into().ok()?;
      return decode_reg_addr(arr);
    }
  }
  None
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
          chunk.copy_from_slice(&fastrand::u64(..).to_le_bytes());
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

/// 将 32 字节 `iroh` 公钥与二进制路由端点紧凑编码为单一 `base64url` 地址（零明文 IP 暴露，栈上零堆分配组装）
pub fn encode_iroh_b64(id: &PeerId, addr: SocketAddr) -> String {
  let mut raw = [0u8; 51];
  raw[..32].copy_from_slice(id);
  let len = match addr {
    SocketAddr::V4(v4) => {
      raw[32] = 4;
      raw[33..37].copy_from_slice(&v4.ip().octets());
      raw[37..39].copy_from_slice(&v4.port().to_le_bytes());
      39
    }
    SocketAddr::V6(v6) => {
      raw[32] = 6;
      raw[33..49].copy_from_slice(&v6.ip().octets());
      raw[49..51].copy_from_slice(&v6.port().to_le_bytes());
      51
    }
  };
  URL_SAFE_NO_PAD.encode(&raw[..len])
}

/// 解码 `base64url` 配置的 `iroh` 地址为 32 字节二进制 `PeerId`
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
    let rest = &raw[32..];
    let addr = match rest.first().copied()? {
      4 if rest.len() >= 7 => {
        let ip = Ipv4Addr::new(rest[1], rest[2], rest[3], rest[4]);
        let port = u16::from_le_bytes([rest[5], rest[6]]);
        Some(SocketAddr::V4(SocketAddrV4::new(ip, port)))
      }
      6 if rest.len() >= 19 => {
        let mut oct = [0u8; 16];
        oct.copy_from_slice(&rest[1..17]);
        let port = u16::from_le_bytes([rest[17], rest[18]]);
        Some(SocketAddr::V6(SocketAddrV6::new(
          Ipv6Addr::from(oct),
          port,
          0,
          0,
        )))
      }
      _ => None,
    };
    if let Some(a) = addr {
      register_iroh_endpoint(id, a);
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
