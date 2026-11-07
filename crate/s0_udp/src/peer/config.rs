use std::{fs, path::PathBuf};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use gxhash::HashMap;
use s0_cdc::hash128;

use super::PeerError;
use crate::{
  s0_const::Perm,
  s0_profile::{ProfileDraft, SecretKey, SignedProfile},
  s0_region::Region,
};

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

/// 将 32 字节公钥/私钥编码为 43 字符 URL-safe Base64
#[inline]
pub fn encode_id_b64(bytes: &[u8; 32]) -> String {
  URL_SAFE_NO_PAD.encode(bytes)
}

/// 将 43 字符 URL-safe Base64 解码为 32 字节数组
#[inline]
pub fn decode_id_b64(s: &str) -> Option<[u8; 32]> {
  let clean = s.trim().strip_prefix("iroh:").unwrap_or(s.trim());
  let raw = URL_SAFE_NO_PAD.decode(clean).ok()?;
  raw.try_into().ok()
}

/// 从分享名称或群组名称派生 128-bit `ShareId`
#[inline]
pub fn derive_share_id(name: &str) -> [u8; 16] {
  hash128(name.as_bytes())
}

/// 单对等节点专属访问配额
#[derive(Debug, Clone, Copy)]
pub struct PeerQuota {
  pub perm: Perm,
  /// 该节点专属带宽限额（字节/秒，`0` 表示不限制）
  pub max_rate: u64,
}

/// 有效访问决议结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedAccess {
  pub perm: Perm,
  pub eff_rate: u64,
}

/// 单个分享文件夹的访问与限速配置 (`ShareConfig`)
#[derive(Debug, Clone)]
pub struct ShareConfig {
  pub share_id: [u8; 16],
  pub root: PathBuf,
  /// 是否开启白名单/令牌认证（默认 `false` 匿名公开访问）
  pub auth_required: bool,
  /// 匿名或未单独配置配额节点的默认权限（默认 `Perm::Ro`）
  pub default_perm: Perm,
  /// 默认带宽限额（字节/秒，`0` 表示不限速）
  pub default_max_rate: u64,
  /// 可选读写能力密钥（Resilio Sync 风格 Read-Write Secret）
  pub rw_token: Option<[u8; 16]>,
  /// 基于客户端 `iroh` `EndpointId` (32B 公钥) 的白名单与独立带宽限额
  pub peer_quotas: HashMap<[u8; 32], PeerQuota>,
}

impl ShareConfig {
  /// 创建默认公开只读分享（无需用户名密码）
  pub fn public_ro(name: &str, root: impl Into<PathBuf>, default_max_rate: u64) -> Self {
    Self {
      share_id: derive_share_id(name),
      root: root.into(),
      auth_required: false,
      default_perm: Perm::Ro,
      default_max_rate,
      rw_token: None,
      peer_quotas: HashMap::default(),
    }
  }

  /// 创建默认公开读写分享（用于对等群组或开放协作目录）
  pub fn public_rw(name: &str, root: impl Into<PathBuf>, default_max_rate: u64) -> Self {
    Self {
      share_id: derive_share_id(name),
      root: root.into(),
      auth_required: false,
      default_perm: Perm::Rw,
      default_max_rate,
      rw_token: None,
      peer_quotas: HashMap::default(),
    }
  }

  pub fn with_auth(mut self, auth_required: bool) -> Self {
    self.auth_required = auth_required;
    self
  }

  pub fn with_rw_token(mut self, token: [u8; 16]) -> Self {
    self.rw_token = Some(token);
    self
  }

  pub fn grant_peer(mut self, endpoint_id: [u8; 32], perm: Perm, max_rate: u64) -> Self {
    self
      .peer_quotas
      .insert(endpoint_id, PeerQuota { perm, max_rate });
    self
  }

  /// 根据对端 `EndpointId`、可选令牌与请求速率计算有效权限与协商速率
  pub fn resolve_access(
    &self,
    remote_id: &[u8; 32],
    req_rate: u64,
    token: Option<[u8; 16]>,
  ) -> Option<ResolvedAccess> {
    let has_valid_rw_token = self.rw_token.is_some() && self.rw_token == token;

    if let Some(q) = self.peer_quotas.get(remote_id) {
      let perm = if has_valid_rw_token { Perm::Rw } else { q.perm };
      let peer_or_def = min_nonzero(q.max_rate, self.default_max_rate);
      return Some(ResolvedAccess {
        perm,
        eff_rate: min_nonzero(req_rate, peer_or_def),
      });
    }

    if self.auth_required && !has_valid_rw_token {
      return None;
    }

    let perm = if has_valid_rw_token {
      Perm::Rw
    } else {
      self.default_perm
    };
    Some(ResolvedAccess {
      perm,
      eff_rate: min_nonzero(req_rate, self.default_max_rate),
    })
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
  /// 是否启用 `iroh` 广域网/内网 QUIC 端点（测试或纯局域网可按需开启）
  pub enable_iroh: bool,
}

impl Default for NodeConfig {
  fn default() -> Self {
    Self {
      key_mode: KeyMode::Ephemeral,
      profile: ProfileDraft::new("peer", Region::Unknown),
      enable_iroh: false,
    }
  }
}

impl NodeConfig {
  pub fn sign_profile(&self, sk: &SecretKey) -> Result<SignedProfile, PeerError> {
    Ok(self.profile.clone().sign(sk)?)
  }
}
