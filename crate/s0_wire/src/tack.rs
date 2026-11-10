use std::marker::PhantomData;

use s0_cdc::{Rd, SliceWr, WireError};
use s0_const::{ACK_DELAY_US, DATA_HLEN, MAX_CNT, MAX_RANGES, PHASE, SESSION_HLEN, ZSTD};
use strum::FromRepr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromRepr)]
#[repr(u8)]
pub enum Kind {
  Data = 0x10,
  Ack = 0x11,
}

impl Kind {
  #[inline]
  pub fn parse(v: u8) -> Result<Self, WireError> {
    Self::from_repr(v).ok_or(WireError::BadOp)
  }

  #[inline]
  pub const fn code(self) -> u8 {
    self as u8
  }

  #[inline]
  const fn is_session(self) -> bool {
    (self as u8) > 0x10
  }

  #[inline]
  const fn expect(self, want: Self) -> Result<(), WireError> {
    if self as u8 == want as u8 {
      Ok(())
    } else {
      Err(WireError::BadOp)
    }
  }
}

/// 会话头 12 B：`kind ‖ flag ‖ x ‖ sid ‖ pn`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionHeader {
  pub kind: Kind,
  pub flag: u8,
  pub x: u16,
  pub sid: u32,
  pub pn: u32,
}

impl SessionHeader {
  pub const LEN: usize = SESSION_HLEN;

  #[inline]
  pub fn parse(b: &[u8]) -> Result<(Self, &[u8]), WireError> {
    let (h, rest) = b
      .split_first_chunk::<{ Self::LEN }>()
      .ok_or(WireError::Truncated)?;
    let kind = Kind::parse(h[0])?;
    if !kind.is_session() {
      return Err(WireError::BadOp);
    }
    let flag = h[1];
    if flag & !PHASE != 0 {
      return Err(WireError::BadVersion);
    }
    Ok((
      Self {
        kind,
        flag,
        x: u16::from_le_bytes([h[2], h[3]]),
        sid: u32::from_le_bytes([h[4], h[5], h[6], h[7]]),
        pn: u32::from_le_bytes([h[8], h[9], h[10], h[11]]),
      },
      rest,
    ))
  }

  #[inline]
  pub fn write(&self, w: &mut SliceWr<'_>) -> Result<(), WireError> {
    let [x0, x1] = self.x.to_le_bytes();
    let [s0, s1, s2, s3] = self.sid.to_le_bytes();
    let [p0, p1, p2, p3] = self.pn.to_le_bytes();
    w.bytes(&[
      self.kind.code(),
      self.flag,
      x0,
      x1,
      s0,
      s1,
      s2,
      s3,
      p0,
      p1,
      p2,
      p3,
    ])
  }
}

/// 高速流 DATA 头 20 B：`kind ‖ flag ‖ idx ‖ cnt ‖ sid ‖ pn ‖ blk ‖ ts`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamDataHeader {
  pub flag: u8,
  pub idx: u8,
  pub cnt: u8,
  pub sid: u32,
  pub pn: u32,
  pub blk: u32,
  pub ts: u32,
}

impl StreamDataHeader {
  pub const LEN: usize = DATA_HLEN;
  pub const MAX_CNT: u8 = MAX_CNT as u8;

  #[inline]
  pub fn parse(b: &[u8]) -> Result<(Self, &[u8]), WireError> {
    let (h, rest) = b
      .split_first_chunk::<{ Self::LEN }>()
      .ok_or(WireError::Truncated)?;
    Kind::parse(h[0])?.expect(Kind::Data)?;
    let hdr = Self {
      flag: h[1],
      idx: h[2],
      cnt: h[3],
      sid: u32::from_le_bytes([h[4], h[5], h[6], h[7]]),
      pn: u32::from_le_bytes([h[8], h[9], h[10], h[11]]),
      blk: u32::from_le_bytes([h[12], h[13], h[14], h[15]]),
      ts: u32::from_le_bytes([h[16], h[17], h[18], h[19]]),
    };
    if hdr.cnt == 0 || hdr.cnt > Self::MAX_CNT || hdr.idx >= hdr.cnt {
      return Err(WireError::Overflow);
    }
    Ok((hdr, rest))
  }

  #[inline]
  pub const fn is_zstd(&self) -> bool {
    self.flag & ZSTD != 0
  }

  #[inline]
  pub fn write(&self, w: &mut SliceWr<'_>, payload: &[u8]) -> Result<(), WireError> {
    let [s0, s1, s2, s3] = self.sid.to_le_bytes();
    let [p0, p1, p2, p3] = self.pn.to_le_bytes();
    let [b0, b1, b2, b3] = self.blk.to_le_bytes();
    let [t0, t1, t2, t3] = self.ts.to_le_bytes();
    w.bytes(&[
      Kind::Data.code(),
      self.flag,
      self.idx,
      self.cnt,
      s0,
      s1,
      s2,
      s3,
      p0,
      p1,
      p2,
      p3,
      b0,
      b1,
      b2,
      b3,
      t0,
      t1,
      t2,
      t3,
    ])?;
    w.bytes(payload)
  }
}

/// RFC 9000 §A.3：由已收最大 64 位包号 `largest` 与线上 32 位截断值还原完整 64 位包号
#[inline]
pub const fn expand_pn(largest: u64, truncated: u32) -> u64 {
  const WIN: u64 = 1u64 << 32;
  const HALF: u64 = WIN >> 1;
  let expected = largest.wrapping_add(1);
  let candidate = (expected & !(WIN - 1)) | (truncated as u64);
  if expected >= HALF && candidate <= expected - HALF && candidate < (1u64 << 62) - WIN {
    candidate + WIN
  } else if candidate > expected.saturating_add(HALF) && candidate >= WIN {
    candidate - WIN
  } else {
    candidate
  }
}

pub trait Item<'a>: Sized {
  fn read(rd: &mut Rd<'a>) -> Result<Self, WireError>;
}

#[derive(Clone, Copy)]
pub struct Items<'a, T> {
  rd: Rd<'a>,
  left: u16,
  _t: PhantomData<fn() -> T>,
}

impl<'a, T> Items<'a, T> {
  #[inline]
  pub const fn new(b: &'a [u8], n: u16) -> Self {
    Self {
      rd: Rd::new(b),
      left: n,
      _t: PhantomData,
    }
  }
}

impl<'a, T: Item<'a>> Iterator for Items<'a, T> {
  type Item = T;

  #[inline]
  fn next(&mut self) -> Option<T> {
    if self.left == 0 {
      return None;
    }
    self.left -= 1;
    T::read(&mut self.rd).ok()
  }

  #[inline]
  fn size_hint(&self) -> (usize, Option<usize>) {
    let n = usize::from(self.left);
    (n, Some(n))
  }
}

impl<'a, T: Item<'a>> ExactSizeIterator for Items<'a, T> {}

const RANGE_LEN: usize = 4;
const BLOCK_LEN: usize = 20;

impl Item<'_> for (u16, u16) {
  #[inline]
  fn read(rd: &mut Rd<'_>) -> Result<Self, WireError> {
    Ok((rd.u16()?, rd.u16()?))
  }
}

impl Item<'_> for (u32, u128) {
  #[inline]
  fn read(rd: &mut Rd<'_>) -> Result<Self, WireError> {
    Ok((rd.u32()?, rd.u128()?))
  }
}

type Ranges<'a> = Items<'a, (u16, u16)>;
type Blocks<'a> = Items<'a, (u32, u128)>;

/// TACK 双层确认包（包号连续区间 + 卡住块 128 位分片位图）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ack<'a> {
  pub largest: u32,
  pub delay: u16,
  pub ts_echo: u32,
  pub rx_pkts: u32,
  pub credit: u32,
  pub done_lo: u32,
  pub dec: u16,
  pub flag: u8,
  pub nr: u8,
  pub nb: u8,
  pub reach: u32,
  ranges: &'a [u8],
  blocks: &'a [u8],
}

impl<'a> Ack<'a> {
  pub const DUP: u8 = 1;
  pub const DELAY_US: u64 = ACK_DELAY_US;

  pub fn parse(hdr: &SessionHeader, body: &'a [u8]) -> Result<Self, WireError> {
    hdr.kind.expect(Kind::Ack)?;
    let mut rd = Rd::new(body);
    let largest = rd.u32()?;
    let delay = rd.u16()?;
    let ts_echo = rd.u32()?;
    let rx_pkts = rd.u32()?;
    let credit = rd.u32()?;
    let done_lo = rd.u32()?;
    let dec = rd.u16()?;
    let flag = rd.u8()?;
    let nr = rd.u8()?;
    let ranges = rd.bytes(usize::from(nr) * RANGE_LEN)?;
    let nb = rd.u8()?;
    let blocks = rd.take_end(usize::from(nb) * BLOCK_LEN)?;
    let reach = check_ranges(Items::new(ranges, u16::from(nr)))?;
    Ok(Self {
      largest,
      delay,
      ts_echo,
      rx_pkts,
      credit,
      done_lo,
      dec,
      flag,
      nr,
      nb,
      reach,
      ranges,
      blocks,
    })
  }

  #[inline]
  pub const fn ranges(&self) -> Ranges<'a> {
    Items::new(self.ranges, self.nr as u16)
  }

  #[inline]
  pub const fn blocks(&self) -> Blocks<'a> {
    Items::new(self.blocks, self.nb as u16)
  }

  #[inline]
  pub const fn span(&self, gap: u16, len: u16, cursor: &mut u64) -> Option<(u64, u64)> {
    let Some(c) = cursor.checked_sub(gap as u64 + len as u64) else {
      return None;
    };
    *cursor = c;
    Some((c, c + len as u64))
  }
}

fn check_ranges(ranges: impl Iterator<Item = (u16, u16)>) -> Result<u32, WireError> {
  let mut reach = 0u32;
  for (i, (gap, len)) in ranges.enumerate() {
    if i == 0 && gap != 0 {
      return Err(WireError::Overflow);
    }
    if len == 0 {
      return Err(WireError::Overflow);
    }
    reach += u32::from(gap) + u32::from(len);
  }
  Ok(reach)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AckOut<'a> {
  pub largest: u32,
  pub delay: u16,
  pub ts_echo: u32,
  pub rx_pkts: u32,
  pub credit: u32,
  pub done_lo: u32,
  pub dec: u16,
  pub flag: u8,
  pub ranges: &'a [(u16, u16)],
  pub blocks: &'a [(u32, u128)],
}

pub fn encode_ack(
  w: &mut SliceWr<'_>,
  hdr: &SessionHeader,
  a: &AckOut<'_>,
) -> Result<(), WireError> {
  hdr.kind.expect(Kind::Ack)?;
  if a.ranges.len() > MAX_RANGES {
    return Err(WireError::Overflow);
  }
  let nb = u8::try_from(a.blocks.len()).map_err(|_| WireError::Overflow)?;
  check_ranges(a.ranges.iter().copied())?;
  hdr.write(w)?;
  w.u32(a.largest)?;
  w.u16(a.delay)?;
  w.u32(a.ts_echo)?;
  w.u32(a.rx_pkts)?;
  w.u32(a.credit)?;
  w.u32(a.done_lo)?;
  w.u16(a.dec)?;
  w.u8(a.flag)?;
  w.u8(a.ranges.len() as u8)?;
  for &(gap, len) in a.ranges {
    w.u16(gap)?;
    w.u16(len)?;
  }
  w.u8(nb)?;
  for &(blk, bits) in a.blocks {
    w.u32(blk)?;
    w.u128(bits)?;
  }
  Ok(())
}
