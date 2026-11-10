use core::str::from_utf8;

use s0_const::{U15_MAX, UVAR_MAX_SHIFT, UVAR_STEP_BITS, VARINT_CONT_BIT, VARINT_VAL_MASK};
use thiserror::Error;

/// 紧凑 1 字节线协议错误类型
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum WireError {
  #[error("帧数据截断")]
  Truncated = 1,
  #[error("魔数不匹配")]
  BadMagic = 2,
  #[error("协议版本不支持")]
  BadVersion = 3,
  #[error("未知操作码")]
  BadOp = 4,
  #[error("未知状态码")]
  BadStatus = 5,
  #[error("非法 UTF-8 编码")]
  InvalidUtf8 = 6,
  #[error("字段长度超出 u15 上限")]
  Overflow = 7,
  #[error("包头 CRC16 校验失败")]
  CrcMismatch = 8,
}

/// 零拷贝二进制读取游标
#[derive(Clone, Copy)]
pub struct Rd<'a> {
  pub buf: &'a [u8],
  pub pos: usize,
}

impl<'a> Rd<'a> {
  #[inline]
  pub const fn new(buf: &'a [u8]) -> Self {
    Self { buf, pos: 0 }
  }

  #[inline]
  pub const fn rem(&self) -> usize {
    self.buf.len().saturating_sub(self.pos)
  }

  #[inline]
  pub const fn is_empty(&self) -> bool {
    self.pos >= self.buf.len()
  }

  #[inline]
  pub fn rest(&self) -> &'a [u8] {
    &self.buf[self.pos..]
  }

  #[inline]
  pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], WireError> {
    if self.rem() < n {
      return Err(WireError::Truncated);
    }
    let s = unsafe { self.buf.get_unchecked(self.pos..self.pos + n) };
    self.pos += n;
    Ok(s)
  }

  #[inline]
  pub fn take_end(&mut self, n: usize) -> Result<&'a [u8], WireError> {
    let s = self.bytes(n)?;
    self.expect_end()?;
    Ok(s)
  }

  #[inline]
  pub const fn expect_end(&self) -> Result<(), WireError> {
    if self.pos == self.buf.len() {
      Ok(())
    } else {
      Err(WireError::Overflow)
    }
  }

  #[inline]
  pub fn array<const N: usize>(&mut self) -> Result<[u8; N], WireError> {
    let s = self.bytes(N)?;
    let mut out = [0u8; N];
    out.copy_from_slice(s);
    Ok(out)
  }

  #[inline]
  pub fn u8(&mut self) -> Result<u8, WireError> {
    Ok(self.array::<1>()?[0])
  }

  #[inline]
  pub fn u16(&mut self) -> Result<u16, WireError> {
    Ok(u16::from_le_bytes(self.array::<2>()?))
  }

  #[inline]
  pub fn u32(&mut self) -> Result<u32, WireError> {
    Ok(u32::from_le_bytes(self.array::<4>()?))
  }

  #[inline]
  pub fn u128(&mut self) -> Result<u128, WireError> {
    Ok(u128::from_le_bytes(self.array::<16>()?))
  }

  /// 读取 `u15` 变长整数（`<128` 占 1B，`128..=32767` 占 2B）
  #[inline]
  pub fn u15(&mut self) -> Result<u16, WireError> {
    let b0 = self.u8()?;
    if b0 & VARINT_CONT_BIT == 0 {
      Ok(u16::from(b0))
    } else {
      let b1 = self.u8()?;
      Ok((u16::from(b0 & VARINT_VAL_MASK) << 8) | u16::from(b1))
    }
  }

  #[inline]
  pub fn str_u15(&mut self) -> Result<&'a str, WireError> {
    let n = self.u15()? as usize;
    let raw = self.bytes(n)?;
    from_utf8(raw).map_err(|_| WireError::InvalidUtf8)
  }

  #[inline]
  pub fn vec_u15(&mut self) -> Result<&'a [u8], WireError> {
    let n = self.u15()? as usize;
    self.bytes(n)
  }

  /// 读取 ULEB128 变长无符号 64 位整数（`<128` 占 1B）
  #[inline]
  pub fn uvar(&mut self) -> Result<u64, WireError> {
    let mut acc = 0u64;
    let mut shift = 0u32;
    loop {
      let b = self.u8()?;
      if shift == UVAR_MAX_SHIFT && b > 1 {
        return Err(WireError::Overflow);
      }
      acc |= u64::from(b & VARINT_VAL_MASK) << shift;
      if b & VARINT_CONT_BIT == 0 {
        return Ok(acc);
      }
      shift += UVAR_STEP_BITS;
      if shift > UVAR_MAX_SHIFT {
        return Err(WireError::Overflow);
      }
    }
  }

  #[inline]
  pub fn uvar_u32(&mut self) -> Result<u32, WireError> {
    u32::try_from(self.uvar()?).map_err(|_| WireError::Overflow)
  }

  #[inline]
  pub fn uvar_usize(&mut self) -> Result<usize, WireError> {
    usize::try_from(self.uvar()?).map_err(|_| WireError::Overflow)
  }

  /// 读取单条前缀差分压缩（Front-Coding）字符串并更新 `prev`
  #[inline]
  pub fn front_str(&mut self, first: bool, prev: &mut Vec<u8>) -> Result<String, WireError> {
    let shared = if first { 0 } else { self.u15()? as usize };
    let suffix = self.vec_u15()?;
    if shared > prev.len() {
      return Err(WireError::Truncated);
    }
    prev.truncate(shared);
    prev.extend_from_slice(suffix);
    Ok(
      from_utf8(prev)
        .map_err(|_| WireError::InvalidUtf8)?
        .to_owned(),
    )
  }

  /// 读取前缀差分压缩（Front-Coding）的字符串列表（条数采用 `uvar` 64 位无上限编码，按帧剩余字节数防截断）
  pub fn strs_front(&mut self) -> Result<Vec<String>, WireError> {
    let cnt = self.uvar_usize()?;
    if cnt > self.rem() {
      return Err(WireError::Truncated);
    }
    let mut out = Vec::with_capacity(cnt.min(1024));
    let mut prev = Vec::<u8>::new();
    for i in 0..cnt {
      out.push(self.front_str(i == 0, &mut prev)?);
    }
    Ok(out)
  }
}

/// 紧凑二进制写入辅助器
pub struct Wr<'a> {
  pub buf: &'a mut Vec<u8>,
}

impl<'a> Wr<'a> {
  #[inline]
  pub fn new(buf: &'a mut Vec<u8>) -> Self {
    Self { buf }
  }

  #[inline]
  pub fn u8(&mut self, v: u8) {
    self.buf.push(v);
  }

  #[inline]
  pub fn u16(&mut self, v: u16) {
    self.buf.extend_from_slice(&v.to_le_bytes());
  }

  #[inline]
  pub fn u32(&mut self, v: u32) {
    self.buf.extend_from_slice(&v.to_le_bytes());
  }

  #[inline]
  pub fn bytes(&mut self, s: &[u8]) {
    self.buf.extend_from_slice(s);
  }

  /// 写入 `u15` 变长整数
  #[inline]
  pub fn u15(&mut self, v: usize) -> Result<(), WireError> {
    if v > U15_MAX {
      return Err(WireError::Overflow);
    }
    if v < usize::from(VARINT_CONT_BIT) {
      self.buf.push(v as u8);
    } else {
      self.buf.push(VARINT_CONT_BIT | ((v >> 8) as u8));
      self.buf.push((v & 0xFF) as u8);
    }
    Ok(())
  }

  #[inline]
  pub fn slice_u15(&mut self, s: &[u8]) -> Result<(), WireError> {
    self.u15(s.len())?;
    self.bytes(s);
    Ok(())
  }

  /// 写入 ULEB128 变长无符号 64 位整数（`<128` 占 1B）
  #[inline]
  pub fn uvar(&mut self, mut v: u64) {
    while v >= u64::from(VARINT_CONT_BIT) {
      self.buf.push((v as u8) | VARINT_CONT_BIT);
      v >>= UVAR_STEP_BITS;
    }
    self.buf.push(v as u8);
  }

  /// 写入单条前缀差分压缩（Front-Coding）字节切片并更新 `prev`
  #[inline]
  pub fn front_str<'b>(
    &mut self,
    first: bool,
    prev: &mut &'b [u8],
    cur: &'b [u8],
  ) -> Result<(), WireError> {
    if first {
      self.slice_u15(cur)?;
    } else {
      let shared = prev
        .iter()
        .zip(cur.iter())
        .take_while(|(a, b)| a == b)
        .count()
        .min(U15_MAX);
      self.u15(shared)?;
      self.slice_u15(&cur[shared..])?;
    }
    *prev = cur;
    Ok(())
  }

  /// 写入前缀差分压缩（Front-Coding）的字符串切片列表
  pub fn strs_front<S: AsRef<str>>(&mut self, list: &[S]) -> Result<(), WireError> {
    self.uvar(list.len() as u64);
    let mut prev: &[u8] = &[];
    for (i, item) in list.iter().enumerate() {
      self.front_str(i == 0, &mut prev, item.as_ref().as_bytes())?;
    }
    Ok(())
  }
}

/// 面向栈上固定切片 `&mut [u8]` 的零分配写入游标（用于热路径 UDP 发包）
pub struct SliceWr<'a> {
  buf: &'a mut [u8],
  pos: usize,
}

impl<'a> SliceWr<'a> {
  #[inline]
  pub const fn new(buf: &'a mut [u8]) -> Self {
    Self { buf, pos: 0 }
  }

  #[inline]
  pub const fn pos(&self) -> usize {
    self.pos
  }

  #[inline]
  pub fn bytes(&mut self, s: &[u8]) -> Result<(), WireError> {
    let dst = self
      .buf
      .get_mut(self.pos..self.pos + s.len())
      .ok_or(WireError::Overflow)?;
    dst.copy_from_slice(s);
    self.pos += s.len();
    Ok(())
  }

  #[inline]
  pub fn u8(&mut self, v: u8) -> Result<(), WireError> {
    self.bytes(&[v])
  }

  #[inline]
  pub fn u16(&mut self, v: u16) -> Result<(), WireError> {
    self.bytes(&v.to_le_bytes())
  }

  #[inline]
  pub fn u32(&mut self, v: u32) -> Result<(), WireError> {
    self.bytes(&v.to_le_bytes())
  }

  #[inline]
  pub fn u128(&mut self, v: u128) -> Result<(), WireError> {
    self.bytes(&v.to_le_bytes())
  }
}
