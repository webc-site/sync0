use core::str::from_utf8;

use super::WireError;
use crate::s0_const::U15_MAX;

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
  pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], WireError> {
    if self.rem() < n {
      return Err(WireError::Truncated);
    }
    let s = unsafe { self.buf.get_unchecked(self.pos..self.pos + n) };
    self.pos += n;
    Ok(s)
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
  pub const fn is_empty(&self) -> bool {
    self.pos >= self.buf.len()
  }

  #[inline]
  pub fn rest(&self) -> &'a [u8] {
    &self.buf[self.pos..]
  }

  #[inline]
  pub fn take(&mut self, n: usize) -> Result<&'a [u8], WireError> {
    self.bytes(n)
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
  pub fn u64(&mut self) -> Result<u64, WireError> {
    Ok(u64::from_le_bytes(self.array::<8>()?))
  }

  #[inline]
  pub fn u128(&mut self) -> Result<u128, WireError> {
    Ok(u128::from_le_bytes(self.array::<16>()?))
  }

  /// 读取 `u15` 变长整数（`<128` 占 1B，`128..=32767` 占 2B）
  #[inline]
  pub fn u15(&mut self) -> Result<u16, WireError> {
    let b0 = self.u8()?;
    if b0 & 0x80 == 0 {
      Ok(u16::from(b0))
    } else {
      let b1 = self.u8()?;
      Ok((u16::from(b0 & 0x7F) << 8) | u16::from(b1))
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
  pub fn u64(&mut self, v: u64) {
    self.buf.extend_from_slice(&v.to_le_bytes());
  }

  #[inline]
  pub fn bytes(&mut self, s: &[u8]) {
    self.buf.extend_from_slice(s);
  }

  /// 写入 `u15` 变长整数
  #[inline]
  pub fn u15(&mut self, v: usize) -> Result<(), WireError> {
    if v > U15_MAX as usize {
      return Err(WireError::Overflow);
    }
    if v < 0x80 {
      self.buf.push(v as u8);
    } else {
      self.buf.push(0x80 | ((v >> 8) as u8));
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
