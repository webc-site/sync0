use std::{
  env::var_os,
  fs::{self, OpenOptions},
  io,
  io::{Read, Seek, SeekFrom, Write},
  net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6},
  path::{Path, PathBuf},
  sync::OnceLock,
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use gxhash::HashMap;
use parking_lot::RwLock;
use s0_cdc::{Manifest, hash128, matches_any_selected};
use s0_conf::{normalize_prefixes, parse_nt_groups};

use super::PeerError;
use crate::{
  consts::Perm,
  profile::{ProfileDraft, SecretKey, SignedProfile},
  region::Region,
};

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
///
/// - 支持纯 32B `EndpointId`（43 字符 `base64url`）或带紧凑二进制路由提示的 `iroh` 地址；
/// - 支持可选 `iroh://` 或 `iroh:` 前缀；
/// - 若包含二进制路由后缀，解码时自动注册到进程内高速路由表。
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

/// 从分享名称或群组名称派生 128-bit 二进制 `ShareId`
#[inline]
pub fn derive_share_id(name: &str) -> [u8; 16] {
  hash128(name.as_bytes())
}

/// 单对等节点专属访问配额（内部按 32 字节二进制 `PeerId` 索引）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerQuota {
  pub perm: Perm,
  /// 该节点专属带宽限额（字节/秒，`0` 表示不限制）
  pub max_rate: u64,
  /// 该用户在此分享组允许上传的路径前缀列表（空列表表示默认允许整个分享根目录）
  pub write_prefixes: Vec<String>,
}

/// 有效访问决议结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAccess {
  pub perm: Perm,
  pub eff_rate: u64,
  /// 允许上传写入的路径前缀（空列表表示默认允许整个分享根目录）
  pub write_prefixes: Vec<String>,
}

impl ResolvedAccess {
  /// 检查是否有权上传/修改指定相对路径 `rel_path`（默认 `write_prefixes` 为空时允许整个分享根目录）
  pub fn can_write_path(&self, rel_path: &str) -> bool {
    if !self.perm.can_write() {
      return false;
    }
    if self.write_prefixes.is_empty() {
      return true;
    }
    let clean = rel_path.trim_matches(['/', '\\']);
    !clean.is_empty() && matches_any_selected(clean, &self.write_prefixes)
  }

  /// 检查上传请求的路径集合与 `Manifest` 中的全部条目是否均在允许上传的路径前缀范围内
  pub fn can_write_manifest(&self, mf: &Manifest, req_paths: &[String]) -> bool {
    if !self.perm.can_write() {
      return false;
    }
    if self.write_prefixes.is_empty() {
      return true;
    }
    if req_paths.is_empty() && mf.entries.is_empty() {
      return false;
    }
    for p in req_paths {
      let clean = p.trim_matches(['/', '\\']);
      if clean.is_empty() || !matches_any_selected(clean, &self.write_prefixes) {
        return false;
      }
    }
    for e in &mf.entries {
      let clean = e.path.trim_matches(['/', '\\']);
      if clean.is_empty() || !matches_any_selected(clean, &self.write_prefixes) {
        return false;
      }
    }
    true
  }
}

/// 单个文件分享组的访问、组员权限与上传路径前缀配置 (`ShareConfig`)
#[derive(Debug, Clone)]
pub struct ShareConfig {
  pub name: String,
  pub share_id: [u8; 16],
  pub root: PathBuf,
  /// 是否开启白名单/令牌认证（默认 `false` 匿名公开访问）
  pub auth_required: bool,
  /// 匿名或未单独配置配额节点的默认权限（默认 `Perm::Ro`）
  pub default_perm: Perm,
  /// 默认带宽限额（字节/秒，`0` 表示不限速）
  pub default_max_rate: u64,
  /// 默认允许上传的路径前缀（空列表表示默认允许整个分享根目录）
  pub default_write_prefixes: Vec<String>,
  /// 可选只读能力密钥（Resilio Sync 风格 Read-Only Secret，内部 16B 二进制）
  pub ro_token: Option<[u8; 16]>,
  /// 可选读写能力密钥（Resilio Sync 风格 Read-Write Secret，内部 16B 二进制）
  pub rw_token: Option<[u8; 16]>,
  /// 基于组员 32 字节二进制 `iroh` `PeerId` 的权限、限速与可上传路径前缀表
  pub peer_quotas: HashMap<PeerId, PeerQuota>,
}

impl ShareConfig {
  /// 创建默认公开只读分享组
  pub fn public_ro(name: &str, root: impl Into<PathBuf>, default_max_rate: u64) -> Self {
    Self {
      name: name.to_owned(),
      share_id: derive_share_id(name),
      root: root.into(),
      auth_required: false,
      default_perm: Perm::Ro,
      default_max_rate,
      default_write_prefixes: Vec::new(),
      ro_token: None,
      rw_token: None,
      peer_quotas: HashMap::default(),
    }
  }

  /// 创建默认公开读写分享组（允许上传到分享根目录）
  pub fn public_rw(name: &str, root: impl Into<PathBuf>, default_max_rate: u64) -> Self {
    Self {
      name: name.to_owned(),
      share_id: derive_share_id(name),
      root: root.into(),
      auth_required: false,
      default_perm: Perm::Rw,
      default_max_rate,
      default_write_prefixes: Vec::new(),
      ro_token: None,
      rw_token: None,
      peer_quotas: HashMap::default(),
    }
  }

  pub fn with_auth(mut self, auth_required: bool) -> Self {
    self.auth_required = auth_required;
    self
  }

  pub fn with_default_perm(mut self, perm: Perm) -> Self {
    self.default_perm = perm;
    self
  }

  pub fn with_default_write_prefixes<I, S>(mut self, prefixes: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
  {
    self.default_write_prefixes = normalize_prefixes(prefixes);
    self
  }

  pub fn with_ro_token(mut self, token: [u8; 16]) -> Self {
    self.ro_token = Some(token);
    self
  }

  pub fn with_rw_token(mut self, token: [u8; 16]) -> Self {
    self.rw_token = Some(token);
    self
  }

  /// 授予指定 `PeerId` 权限与带宽限额（默认允许上传至整个分享根目录）
  pub fn grant_peer(mut self, endpoint_id: PeerId, perm: Perm, max_rate: u64) -> Self {
    self.peer_quotas.insert(
      endpoint_id,
      PeerQuota {
        perm,
        max_rate,
        write_prefixes: Vec::new(),
      },
    );
    self
  }

  /// 授予指定 `PeerId` 权限、带宽限额以及该用户在此分享组允许上传的路径前缀列表（空列表表示整个分享根目录）
  pub fn grant_peer_with_prefixes<I, S>(
    mut self,
    endpoint_id: PeerId,
    perm: Perm,
    max_rate: u64,
    write_prefixes: I,
  ) -> Self
  where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
  {
    self.peer_quotas.insert(
      endpoint_id,
      PeerQuota {
        perm,
        max_rate,
        write_prefixes: normalize_prefixes(write_prefixes),
      },
    );
    self
  }

  /// 通过 `base64url` `iroh` 地址字符串授予组员权限与可上传路径前缀（内部立即转为 32B 二进制 `PeerId`）
  pub fn grant_peer_b64<I, S>(
    self,
    iroh_b64: &str,
    perm: Perm,
    max_rate: u64,
    write_prefixes: I,
  ) -> Option<Self>
  where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
  {
    let id = decode_id_b64(iroh_b64)?;
    Some(self.grant_peer_with_prefixes(id, perm, max_rate, write_prefixes))
  }

  /// 根据对端 32B `PeerId`、可选令牌与请求速率计算有效权限、协商速率与允许上传路径前缀
  pub fn resolve_access(
    &self,
    remote_id: &PeerId,
    req_rate: u64,
    token: Option<[u8; 16]>,
  ) -> Option<ResolvedAccess> {
    let has_valid_rw_token = self.rw_token.is_some() && self.rw_token == token;
    let has_valid_ro_token = self.ro_token.is_some() && self.ro_token == token;

    if let Some(q) = self.peer_quotas.get(remote_id) {
      let perm = if has_valid_rw_token && q.perm == Perm::Ro {
        Perm::Rw
      } else {
        q.perm
      };
      let peer_or_def = min_nonzero(q.max_rate, self.default_max_rate);
      return Some(ResolvedAccess {
        perm,
        eff_rate: min_nonzero(req_rate, peer_or_def),
        write_prefixes: q.write_prefixes.clone(),
      });
    }

    if self.auth_required && !has_valid_rw_token && !has_valid_ro_token {
      return None;
    }

    let perm = if has_valid_rw_token {
      Perm::Rw
    } else if has_valid_ro_token {
      Perm::Ro
    } else {
      self.default_perm
    };
    Some(ResolvedAccess {
      perm,
      eff_rate: min_nonzero(req_rate, self.default_max_rate),
      write_prefixes: self.default_write_prefixes.clone(),
    })
  }

  /// 解析 NestedText (`.nt`) 格式的分享组与用户配置文件
  ///
  /// - 支持通过 `users_dir` 指定用户配置目录，自动包含该目录下的全部 `*.nt` 用户文件；
  /// - 每个用户单独一个 `.nt` 配置文件，用户列表与上传路径前缀均采用无序列表（`- ...`）；
  /// - `rate` 支持 `GB`、`MB`、`KB`、`B` 等单位或纯数字字节/秒。
  pub fn parse_nt(text: &str, base_dir: Option<&Path>) -> Result<Vec<Self>, PeerError> {
    let raw_groups = parse_nt_groups(text, base_dir)?;
    let mut out = Vec::with_capacity(raw_groups.len());
    for g in raw_groups {
      let mut cfg = Self::public_ro(&g.name, g.root, g.default_max_rate);
      cfg.auth_required = g.auth_required;
      if let Some(ref p_str) = g.default_perm
        && let Some(p) = Perm::parse_str(p_str)
      {
        cfg.default_perm = p;
      }
      cfg.default_write_prefixes = g.default_write_prefixes;
      if let Some(ref ro) = g.ro_secret {
        cfg.ro_token = Some(hash128(ro.as_bytes()));
      }
      if let Some(ref rw) = g.rw_secret {
        cfg.rw_token = Some(hash128(rw.as_bytes()));
      }
      for grant in g.grants {
        if let Some(pid) = decode_id_b64(&grant.id) {
          let perm = grant
            .perm
            .as_deref()
            .and_then(Perm::parse_str)
            .unwrap_or(cfg.default_perm);
          cfg.peer_quotas.insert(
            pid,
            PeerQuota {
              perm,
              max_rate: grant.rate,
              write_prefixes: grant.write_prefixes,
            },
          );
        }
      }
      out.push(cfg);
    }
    Ok(out)
  }

  /// 从配置目录加载 `config.nt` 并自动包含用户配置目录下的全部 `.nt` 文件
  pub fn load_from_dir(dir: &Path) -> Result<Vec<Self>, PeerError> {
    let p = dir.join("config.nt");
    if !p.is_file() {
      return Ok(Vec::new());
    }
    let text = fs::read_to_string(&p)?;
    Self::parse_nt(&text, Some(dir))
  }
}

#[inline]
pub const fn min_nonzero(a: u64, b: u64) -> u64 {
  match (a, b) {
    (0, x) | (x, 0) => x,
    (x, y) => {
      if x < y {
        x
      } else {
        y
      }
    }
  }
}

/// 对等节点启动配置
#[derive(Debug, Clone)]
pub struct NodeConfig {
  pub key_mode: KeyMode,
  pub profile: ProfileDraft,
  /// 控制面 TCP 监听地址（默认 `None` 自动绑定 `127.0.0.1:0`）
  pub bind_addr: Option<SocketAddr>,
  /// 是否启用 `iroh` 广域网/内网端点
  pub enable_iroh: bool,
}

impl Default for NodeConfig {
  fn default() -> Self {
    Self {
      key_mode: KeyMode::Ephemeral,
      profile: ProfileDraft::new("peer", Region::Unknown),
      bind_addr: None,
      enable_iroh: false,
    }
  }
}

impl NodeConfig {
  pub fn sign_profile(&self, sk: &SecretKey) -> Result<SignedProfile, PeerError> {
    Ok(self.profile.clone().sign(sk)?)
  }
}
