use s0_const::{DATA_HDR_LEN, PKT_ACK, PKT_NACK, PKT_PUNCH};

use crate::{Rd, WireError, Wr};

/// 20 字节紧凑原生 UDP 数据包头
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataHeader {
  pub session_id: u32,
  pub block_idx: u32,
  pub seg_idx: u16,
  pub seg_cnt: u16,
  pub offset_in_block: u32,
  pub payload_len: u16,
}

impl DataHeader {
  #[inline]
  fn calc_crc16(hdr18: &[u8; 18]) -> u16 {
    let mut acc: u32 = 0x5A3C;
    for chunk in hdr18.as_chunks::<2>().0 {
      let w = u16::from_le_bytes([chunk[0], chunk[1]]) as u32;
      acc = acc.rotate_left(5) ^ w.wrapping_mul(0x9E37);
    }
    (acc ^ (acc >> 16)) as u16
  }

  #[inline]
  pub fn encode(&self, out: &mut [u8; DATA_HDR_LEN]) {
    // 首字节固定为 0x00，高 3 字节存放 24-bit session_id，与控制包 (0x81/0x82/0x83) 零歧义区分
    let word0 = (self.session_id & 0x00FF_FFFF) << 8;
    out[0..4].copy_from_slice(&word0.to_le_bytes());
    out[4..8].copy_from_slice(&self.block_idx.to_le_bytes());
    out[8..10].copy_from_slice(&self.seg_idx.to_le_bytes());
    out[10..12].copy_from_slice(&self.seg_cnt.to_le_bytes());
    out[12..16].copy_from_slice(&self.offset_in_block.to_le_bytes());
    out[16..18].copy_from_slice(&self.payload_len.to_le_bytes());
    let hdr18: &[u8; 18] = out[0..18].try_into().unwrap();
    let crc = Self::calc_crc16(hdr18);
    out[18..20].copy_from_slice(&crc.to_le_bytes());
  }

  #[inline]
  pub fn decode(buf: &[u8]) -> Result<Self, WireError> {
    if buf.len() < DATA_HDR_LEN {
      return Err(WireError::Truncated);
    }
    if buf[0] != 0 {
      return Err(WireError::BadMagic);
    }
    let hdr18: &[u8; 18] = buf[0..18].try_into().unwrap();
    let expected = Self::calc_crc16(hdr18);
    let actual = u16::from_le_bytes([buf[18], buf[19]]);
    if expected != actual {
      return Err(WireError::CrcMismatch);
    }
    let word0 = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    Ok(Self {
      session_id: word0 >> 8,
      block_idx: u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]),
      seg_idx: u16::from_le_bytes([buf[8], buf[9]]),
      seg_cnt: u16::from_le_bytes([buf[10], buf[11]]),
      offset_in_block: u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
      payload_len: u16::from_le_bytes([buf[16], buf[17]]),
    })
  }
}

/// 原生 UDP 缺失块区间 NACK 包（游程编码：`[(blk_lo, blk_hi)]`）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NackPkt {
  pub session_id: u32,
  pub spans: Vec<(u32, u32)>,
}

impl NackPkt {
  pub fn encode(&self, out: &mut Vec<u8>) -> Result<(), WireError> {
    let mut w = Wr::new(out);
    w.u8(PKT_NACK);
    w.u32(self.session_id & 0x00FF_FFFF);
    let cnt = self.spans.len().min(128);
    w.u15(cnt)?;
    for &(lo, hi) in &self.spans[..cnt] {
      w.u32(lo);
      w.u32(hi);
    }
    Ok(())
  }

  pub fn decode(buf: &[u8]) -> Result<Self, WireError> {
    let mut r = Rd::new(buf);
    if r.u8()? != PKT_NACK {
      return Err(WireError::BadMagic);
    }
    let session_id = r.u32()? & 0x00FF_FFFF;
    let cnt = r.u15()? as usize;
    let mut spans = Vec::with_capacity(cnt.min(128));
    for _ in 0..cnt {
      let lo = r.u32()?;
      let hi = r.u32()?;
      if lo < hi {
        spans.push((lo, hi));
      }
    }
    Ok(Self { session_id, spans })
  }
}

/// 原生 UDP 传输完成 ACK 包
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AckPkt {
  pub session_id: u32,
  pub blk_lo: u32,
  pub blk_hi: u32,
}

impl AckPkt {
  pub fn encode(&self, out: &mut [u8; 13]) {
    out[0] = PKT_ACK;
    out[1..5].copy_from_slice(&(self.session_id & 0x00FF_FFFF).to_le_bytes());
    out[5..9].copy_from_slice(&self.blk_lo.to_le_bytes());
    out[9..13].copy_from_slice(&self.blk_hi.to_le_bytes());
  }

  pub fn decode(buf: &[u8]) -> Result<Self, WireError> {
    if buf.len() < 13 {
      return Err(WireError::Truncated);
    }
    if buf[0] != PKT_ACK {
      return Err(WireError::BadMagic);
    }
    Ok(Self {
      session_id: u32::from_le_bytes([buf[1], buf[2], buf[3], buf[4]]) & 0x00FF_FFFF,
      blk_lo: u32::from_le_bytes([buf[5], buf[6], buf[7], buf[8]]),
      blk_hi: u32::from_le_bytes([buf[9], buf[10], buf[11], buf[12]]),
    })
  }
}

/// 构造 5 字节 UDP 打洞探测包
#[inline]
pub fn encode_punch(session_id: u32) -> [u8; 5] {
  let mut out = [0u8; 5];
  out[0] = PKT_PUNCH;
  out[1..5].copy_from_slice(&(session_id & 0x00FF_FFFF).to_le_bytes());
  out
}
