use std::{
  fs,
  net::SocketAddr,
  path::{Path, PathBuf},
};

use gxhash::HashMap;
use s0_cdc::{Manifest, hash128, matches_any_selected};
use s0_conf::{extend_clean_paths_csv, normalize_prefixes, parse_nt_groups};

use super::{KeyMode, PeerError, PeerId, decode_id_b64};
use crate::{
  consts::Perm,
  profile::{ProfileDraft, SecretKey, SignedProfile},
  region::Region,
};

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
  fn parse_nt(text: &str, base_dir: Option<&Path>) -> Result<Vec<Self>, PeerError> {
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

/// 同步/拉取/推送请求参数 (`TransferOpts`)
#[derive(Debug, Default, Clone)]
pub struct TransferOpts<'a> {
  pub subpath: &'a str,
  pub paths: Vec<String>,
  pub delete_extraneous: bool,
  pub req_rate: u64,
  pub token: Option<[u8; 16]>,
  pub byte_range: Option<(u64, u64)>,
}

impl<'a> TransferOpts<'a> {
  pub const fn new() -> Self {
    Self {
      subpath: "",
      paths: Vec::new(),
      delete_extraneous: false,
      req_rate: 0,
      token: None,
      byte_range: None,
    }
  }

  pub fn with_subpath(mut self, subpath: &'a str) -> Self {
    self.subpath = subpath;
    extend_clean_paths_csv(&mut self.paths, subpath);
    self
  }

  /// 多选任意多个文件或子目录（空列表表示同步整个分享组全部内容）
  pub fn with_paths<I, S>(mut self, paths: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
  {
    for p in paths {
      extend_clean_paths_csv(&mut self.paths, p.as_ref());
    }
    self
  }

  pub const fn with_delete(mut self, delete_extraneous: bool) -> Self {
    self.delete_extraneous = delete_extraneous;
    self
  }

  pub const fn with_rate(mut self, req_rate: u64) -> Self {
    self.req_rate = req_rate;
    self
  }

  /// 指定点播/随机读取字节半开区间 `[byte_lo, byte_hi)`（`byte_hi == u64::MAX` 表示 `byte_lo..EOF`）
  pub const fn with_byte_range(mut self, byte_lo: u64, byte_hi: u64) -> Self {
    self.byte_range = Some((byte_lo, byte_hi));
    self
  }

  /// 导出归一化后的多选路径列表（空 `Vec` 代表整个分享组根目录）
  pub fn resolved_paths(&self) -> Vec<String> {
    if !self.paths.is_empty() {
      return self.paths.clone();
    }
    let mut out = Vec::new();
    extend_clean_paths_csv(&mut out, self.subpath);
    out
  }
}

/// 同步/拉取结果统计
#[derive(Debug, Default, Clone)]
pub struct SyncReport {
  pub root_hash: [u8; 16],
  pub total_blocks: u32,
  pub blocks_transferred: u32,
  pub bytes_transferred: u64,
  pub peers_used: usize,
}
