use std::fmt;

use coarsetime::Clock;
use ed25519_dalek::{
  Signature as Ed25519DalekSignature, Signer as _, SigningKey, Verifier as _, VerifyingKey,
};
use s0_cdc::{Rd, WireError, Wr};
use thiserror::Error;

use crate::{
  consts::{
    AttrKey, MAX_BIO_CHARS, MAX_EMAIL_LEN, MAX_NAME_CHARS, MAX_URL_LEN, PROFILE_MAGIC,
    PROFILE_SIG_DOMAIN,
  },
  region::Region,
};

/// 与 `iroh::SecretKey` 二进制完全兼容的 32B Ed25519 节点私钥（无 `tokio` 传递依赖）
#[derive(Debug, Clone)]
pub struct SecretKey(SigningKey);

impl SecretKey {
  #[inline]
  pub fn from_bytes(bytes: &[u8; 32]) -> Self {
    Self(SigningKey::from_bytes(bytes))
  }

  #[inline]
  pub fn to_bytes(&self) -> [u8; 32] {
    self.0.to_bytes()
  }

  #[inline]
  pub fn public(&self) -> PublicKey {
    PublicKey(self.0.verifying_key())
  }

  #[inline]
  pub fn sign(&self, msg: &[u8]) -> Signature {
    Signature(self.0.sign(msg))
  }
}

/// 与 `iroh::PublicKey` (`EndpointId`) 二进制完全兼容的 32B Ed25519 节点公钥
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicKey(VerifyingKey);

impl PublicKey {
  #[inline]
  pub fn from_bytes(bytes: &[u8; 32]) -> Result<Self, ProfileError> {
    VerifyingKey::from_bytes(bytes)
      .map(Self)
      .map_err(|_| ProfileError::BadPublicKey)
  }

  #[inline]
  pub fn as_bytes(&self) -> &[u8; 32] {
    self.0.as_bytes()
  }

  #[inline]
  fn verify(&self, msg: &[u8], sig: &Signature) -> Result<(), ProfileError> {
    self
      .0
      .verify(msg, &sig.0)
      .map_err(|_| ProfileError::BadSignature)
  }
}

/// 64B Ed25519 签名封装
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signature(Ed25519DalekSignature);

impl Signature {
  #[inline]
  pub fn from_bytes(bytes: &[u8; 64]) -> Self {
    Self(Ed25519DalekSignature::from_bytes(bytes))
  }

  #[inline]
  pub fn to_bytes(&self) -> [u8; 64] {
    self.0.to_bytes()
  }
}

/// 节点自签名名片错误定义
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProfileError {
  #[error("线协议编解码错误: {0}")]
  Wire(#[from] WireError),
  #[error("用户名超过最大 64 字符限制 (当前 {0} 字符)")]
  NameTooLong(usize),
  #[error("邮箱长度超过 128 字节上限")]
  EmailTooLong,
  #[error("网址长度超过 256 字节上限")]
  UrlTooLong,
  #[error("个性签名超过最大 256 字符限制 (当前 {0} 字符)")]
  BioTooLong(usize),
  #[error("未知或乱序的数字属性键: {0}")]
  BadAttrKey(u8),
  #[error("非法的 iroh Ed25519 公钥")]
  BadPublicKey,
  #[error("Ed25519 签名验证失败")]
  BadSignature,
}

/// 基于 `iroh` Ed25519 公钥自签名的对等节点名片
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedProfile {
  pub pubkey: [u8; 32],
  pub ts_sec: u64,
  pub name: String,
  pub email: String,
  pub url: String,
  pub region: Region,
  pub bio: String,
  pub sig: [u8; 64],
}

/// 待签名节点资料构建器
#[derive(Debug, Default, Clone)]
pub struct ProfileDraft {
  pub name: String,
  pub email: String,
  pub url: String,
  pub region: Region,
  pub bio: String,
}

impl ProfileDraft {
  pub fn new(name: impl Into<String>, region: Region) -> Self {
    Self {
      name: name.into(),
      email: String::new(),
      url: String::new(),
      region,
      bio: String::new(),
    }
  }

  pub fn with_email(mut self, email: impl Into<String>) -> Self {
    self.email = email.into();
    self
  }

  pub fn with_url(mut self, url: impl Into<String>) -> Self {
    self.url = url.into();
    self
  }

  pub fn with_bio(mut self, bio: impl Into<String>) -> Self {
    self.bio = bio.into();
    self
  }

  pub fn with_motto(self, motto: impl Into<String>) -> Self {
    self.with_bio(motto)
  }

  fn validate(&self) -> Result<(), ProfileError> {
    let name_chars = self.name.chars().count();
    if name_chars > MAX_NAME_CHARS {
      return Err(ProfileError::NameTooLong(name_chars));
    }
    if self.email.len() > MAX_EMAIL_LEN {
      return Err(ProfileError::EmailTooLong);
    }
    if self.url.len() > MAX_URL_LEN {
      return Err(ProfileError::UrlTooLong);
    }
    let bio_chars = self.bio.chars().count();
    if bio_chars > MAX_BIO_CHARS {
      return Err(ProfileError::BioTooLong(bio_chars));
    }
    Ok(())
  }

  /// 使用节点 `iroh::SecretKey` 对数字常量属性表进行规范化 Ed25519 签名
  pub fn sign(self, sk: &SecretKey) -> Result<SignedProfile, ProfileError> {
    self.validate()?;
    let pubkey = *sk.public().as_bytes();
    let ts_sec = Clock::now_since_epoch().as_secs();
    let mut profile = SignedProfile {
      pubkey,
      ts_sec,
      name: self.name,
      email: self.email,
      url: self.url,
      region: self.region,
      bio: self.bio,
      sig: [0u8; 64],
    };
    let msg = profile.signing_message()?;
    let sig = sk.sign(&msg);
    profile.sig = sig.to_bytes();
    Ok(profile)
  }
}

impl SignedProfile {
  /// 编码除 64B 签名外的规范化 TLV 正文（按数值 `AttrKey` 严格升序排列）
  fn encode_body(&self, out: &mut Vec<u8>) -> Result<(), ProfileError> {
    let mut w = Wr::new(out);
    w.u32(PROFILE_MAGIC);
    w.bytes(&self.pubkey);
    w.u64(self.ts_sec);

    let mut cnt = 0u8;
    if !self.name.is_empty() {
      cnt += 1;
    }
    if !self.email.is_empty() {
      cnt += 1;
    }
    if !self.url.is_empty() {
      cnt += 1;
    }
    if self.region != Region::Unknown {
      cnt += 1;
    }
    if !self.bio.is_empty() {
      cnt += 1;
    }
    w.u8(cnt);

    if !self.name.is_empty() {
      w.u8(AttrKey::Name as u8);
      w.slice_u15(self.name.as_bytes())?;
    }
    if !self.email.is_empty() {
      w.u8(AttrKey::Email as u8);
      w.slice_u15(self.email.as_bytes())?;
    }
    if !self.url.is_empty() {
      w.u8(AttrKey::Url as u8);
      w.slice_u15(self.url.as_bytes())?;
    }
    if self.region != Region::Unknown {
      w.u8(AttrKey::Region as u8);
      w.u15(2)?;
      w.u16(self.region.as_u16());
    }
    if !self.bio.is_empty() {
      w.u8(AttrKey::Bio as u8);
      w.slice_u15(self.bio.as_bytes())?;
    }
    Ok(())
  }

  fn signing_message(&self) -> Result<Vec<u8>, ProfileError> {
    let mut msg = Vec::with_capacity(128);
    msg.extend_from_slice(PROFILE_SIG_DOMAIN);
    self.encode_body(&mut msg)?;
    Ok(msg)
  }

  /// 序列化为紧凑二进制名片帧
  pub fn encode(&self) -> Result<Vec<u8>, ProfileError> {
    let mut out = Vec::with_capacity(160);
    self.encode_body(&mut out)?;
    out.extend_from_slice(&self.sig);
    Ok(out)
  }

  /// 解码并自动执行 Ed25519 公钥验签与字段约束校验
  pub fn decode_and_verify(buf: &[u8]) -> Result<Self, ProfileError> {
    if buf.len() < 4 + 32 + 8 + 1 + 64 {
      return Err(ProfileError::Wire(WireError::Truncated));
    }
    let body_end = buf.len() - 64;
    let body = &buf[..body_end];
    let mut sig = [0u8; 64];
    sig.copy_from_slice(&buf[body_end..]);

    let mut r = Rd::new(body);
    if r.u32()? != PROFILE_MAGIC {
      return Err(ProfileError::Wire(WireError::BadMagic));
    }
    let pubkey = r.array::<32>()?;
    let ts_sec = r.u64()?;
    let attr_cnt = r.u8()? as usize;

    let mut name = String::new();
    let mut email = String::new();
    let mut url = String::new();
    let mut region = Region::Unknown;
    let mut bio = String::new();
    let mut prev_key = 0u8;

    for _ in 0..attr_cnt {
      let raw_k = r.u8()?;
      if raw_k <= prev_key {
        return Err(ProfileError::BadAttrKey(raw_k));
      }
      prev_key = raw_k;
      let key = AttrKey::from_repr(raw_k).ok_or(ProfileError::BadAttrKey(raw_k))?;
      match key {
        AttrKey::Name => {
          let s = r.str_u15()?;
          let chars = s.chars().count();
          if chars > MAX_NAME_CHARS {
            return Err(ProfileError::NameTooLong(chars));
          }
          name = s.to_owned();
        }
        AttrKey::Email => {
          let s = r.str_u15()?;
          if s.len() > MAX_EMAIL_LEN {
            return Err(ProfileError::EmailTooLong);
          }
          email = s.to_owned();
        }
        AttrKey::Url => {
          let s = r.str_u15()?;
          if s.len() > MAX_URL_LEN {
            return Err(ProfileError::UrlTooLong);
          }
          url = s.to_owned();
        }
        AttrKey::Region => {
          let val_bytes = r.vec_u15()?;
          if val_bytes.len() != 2 {
            return Err(ProfileError::Wire(WireError::Truncated));
          }
          let id = u16::from_le_bytes([val_bytes[0], val_bytes[1]]);
          region = Region::from_u16(id);
        }
        AttrKey::Bio => {
          let s = r.str_u15()?;
          let chars = s.chars().count();
          if chars > MAX_BIO_CHARS {
            return Err(ProfileError::BioTooLong(chars));
          }
          bio = s.to_owned();
        }
      }
    }

    let pk = PublicKey::from_bytes(&pubkey).map_err(|_| ProfileError::BadPublicKey)?;
    let mut msg = Vec::with_capacity(PROFILE_SIG_DOMAIN.len() + body.len());
    msg.extend_from_slice(PROFILE_SIG_DOMAIN);
    msg.extend_from_slice(body);
    let signature = Signature::from_bytes(&sig);
    pk.verify(&msg, &signature)
      .map_err(|_| ProfileError::BadSignature)?;

    Ok(Self {
      pubkey,
      ts_sec,
      name,
      email,
      url,
      region,
      bio,
      sig,
    })
  }

  /// 校验该名片是否由指定的 `EndpointId` 签名
  #[inline]
  pub fn matches_endpoint(&self, endpoint_id: &[u8; 32]) -> bool {
    &self.pubkey == endpoint_id
  }
}

impl fmt::Display for SignedProfile {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let disp_name = if self.name.is_empty() {
      "anonymous"
    } else {
      &self.name
    };
    write!(
      f,
      "{} [{}/{}]",
      disp_name,
      self.region.name_zh(),
      self.region.code()
    )?;
    if !self.email.is_empty() {
      write!(f, " <{}>", self.email)?;
    }
    if !self.url.is_empty() {
      write!(f, " ({})", self.url)?;
    }
    if !self.bio.is_empty() {
      write!(f, " \"{}\"", self.bio)?;
    }
    write!(
      f,
      " #{:02x}{:02x}..{:02x}{:02x}",
      self.pubkey[0], self.pubkey[1], self.pubkey[30], self.pubkey[31]
    )
  }
}
