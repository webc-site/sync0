use s0_cdc::{Rd, WireError, Wr};

use crate::consts::{
  ACK_PKT_LEN, DATA_CRC_INPUT_LEN, DATA_CRC_MUL, DATA_CRC_SEED, DATA_HDR_LEN, DEFAULT_PKT,
  NACK_MAX_PARTIALS, NACK_MAX_SPANS, PKT_ACK, PKT_NACK, PKT_PUNCH, PUNCH_PKT_LEN, SID24_MASK,
  SID24_SHIFT,
};

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
  fn calc_crc16(hdr18: &[u8; DATA_CRC_INPUT_LEN]) -> u16 {
    let mut acc: u32 = DATA_CRC_SEED;
    for chunk in hdr18.as_chunks::<2>().0 {
      let w = u16::from_le_bytes([chunk[0], chunk[1]]) as u32;
      acc = acc.rotate_left(5) ^ w.wrapping_mul(DATA_CRC_MUL);
    }
    (acc ^ (acc >> 16)) as u16
  }

  #[inline]
  pub fn encode(&self, out: &mut [u8; DATA_HDR_LEN]) {
    // 首字节固定为 0x00，高 3 字节存放 24-bit session_id，与控制包 (0x81/0x82/0x83) 零歧义区分
    let word0 = (self.session_id & SID24_MASK) << SID24_SHIFT;
    out[0..4].copy_from_slice(&word0.to_le_bytes());
    out[4..8].copy_from_slice(&self.block_idx.to_le_bytes());
    out[8..10].copy_from_slice(&self.seg_idx.to_le_bytes());
    out[10..12].copy_from_slice(&self.seg_cnt.to_le_bytes());
    out[12..16].copy_from_slice(&self.offset_in_block.to_le_bytes());
    out[16..18].copy_from_slice(&self.payload_len.to_le_bytes());
    let hdr18: &[u8; DATA_CRC_INPUT_LEN] = out[0..DATA_CRC_INPUT_LEN].try_into().unwrap();
    let crc = Self::calc_crc16(hdr18);
    out[DATA_CRC_INPUT_LEN..DATA_HDR_LEN].copy_from_slice(&crc.to_le_bytes());
  }

  #[inline]
  pub fn decode(buf: &[u8]) -> Result<Self, WireError> {
    if buf.len() < DATA_HDR_LEN {
      return Err(WireError::Truncated);
    }
    if buf[0] != 0 {
      return Err(WireError::BadMagic);
    }
    let hdr18: &[u8; DATA_CRC_INPUT_LEN] = buf[0..DATA_CRC_INPUT_LEN].try_into().unwrap();
    let expected = Self::calc_crc16(hdr18);
    let actual = u16::from_le_bytes([buf[18], buf[19]]);
    if expected != actual {
      return Err(WireError::CrcMismatch);
    }
    let word0 = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    Ok(Self {
      session_id: word0 >> SID24_SHIFT,
      block_idx: u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]),
      seg_idx: u16::from_le_bytes([buf[8], buf[9]]),
      seg_cnt: u16::from_le_bytes([buf[10], buf[11]]),
      offset_in_block: u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
      payload_len: u16::from_le_bytes([buf[16], buf[17]]),
    })
  }
}

/// 原生 UDP 缺失块区间 NACK 包（游程编码：`[(blk_lo, blk_hi)]` + 部分到达块的分片位图 `[(blk_idx, seg_mask)]`）
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NackPkt {
  pub session_id: u32,
  pub spans: Vec<(u32, u32)>,
  pub partials: Vec<(u32, u64)>,
}

impl NackPkt {
  #[inline]
  pub fn encode(&self, out: &mut Vec<u8>) -> Result<(), WireError> {
    Self::encode_parts(self.session_id, &self.spans, &self.partials, out)
  }

  /// 零克隆切片编码 NACK 帧（供接收端热路径直接按切片块编码，免除 `Vec` 临时分配，采用差分 + `uvar` 压缩）
  pub fn encode_parts(
    session_id: u32,
    spans: &[(u32, u32)],
    partials: &[(u32, u64)],
    out: &mut Vec<u8>,
  ) -> Result<(), WireError> {
    let mut w = Wr::new(out);
    w.u32(u32::from(PKT_NACK) | ((session_id & SID24_MASK) << SID24_SHIFT));
    let cnt = spans.len().min(NACK_MAX_SPANS);
    w.uvar(cnt as u64);
    let mut prev_hi = 0u32;
    for &(lo, hi) in &spans[..cnt] {
      let gap = lo.saturating_sub(prev_hi);
      let len = hi.saturating_sub(lo);
      w.uvar(u64::from(gap));
      w.uvar(u64::from(len));
      prev_hi = hi;
    }
    let pcnt = partials.len().min(NACK_MAX_PARTIALS);
    w.uvar(pcnt as u64);
    let mut prev_blk = 0u32;
    for &(blk, mask) in &partials[..pcnt] {
      w.uvar(u64::from(blk.saturating_sub(prev_blk)));
      w.uvar(mask);
      prev_blk = blk;
    }
    Ok(())
  }

  pub fn decode(buf: &[u8]) -> Result<Self, WireError> {
    let mut r = Rd::new(buf);
    let word0 = r.u32()?;
    if (word0 as u8) != PKT_NACK {
      return Err(WireError::BadMagic);
    }
    let session_id = word0 >> SID24_SHIFT;
    let cnt = r.uvar_usize()?.min(NACK_MAX_SPANS);
    let mut spans = Vec::with_capacity(cnt);
    let mut prev_hi = 0u32;
    for _ in 0..cnt {
      let gap = r.uvar_u32()?;
      let len = r.uvar_u32()?;
      let lo = prev_hi.saturating_add(gap);
      let hi = lo.saturating_add(len);
      if lo < hi {
        spans.push((lo, hi));
      }
      prev_hi = hi;
    }
    let mut partials = Vec::new();
    if !r.is_empty()
      && let Ok(pcnt) = r.uvar_usize()
    {
      let pcnt = pcnt.min(NACK_MAX_PARTIALS);
      partials.reserve(pcnt);
      let mut prev_blk = 0u32;
      for _ in 0..pcnt {
        if let (Ok(d_blk), Ok(mask)) = (r.uvar_u32(), r.uvar()) {
          let blk = prev_blk.saturating_add(d_blk);
          partials.push((blk, mask));
          prev_blk = blk;
        } else {
          break;
        }
      }
    }
    Ok(Self {
      session_id,
      spans,
      partials,
    })
  }
}

/// 原生 UDP 传输完成 12 字节对齐 ACK 包
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AckPkt {
  pub session_id: u32,
  pub blk_lo: u32,
  pub blk_hi: u32,
}

impl AckPkt {
  #[inline]
  pub fn encode(&self, out: &mut [u8; ACK_PKT_LEN]) {
    let word0 = u32::from(PKT_ACK) | ((self.session_id & SID24_MASK) << SID24_SHIFT);
    out[0..4].copy_from_slice(&word0.to_le_bytes());
    out[4..8].copy_from_slice(&self.blk_lo.to_le_bytes());
    out[8..ACK_PKT_LEN].copy_from_slice(&self.blk_hi.to_le_bytes());
  }

  #[inline]
  pub fn decode(buf: &[u8]) -> Result<Self, WireError> {
    if buf.len() < ACK_PKT_LEN {
      return Err(WireError::Truncated);
    }
    if buf[0] != PKT_ACK {
      return Err(WireError::BadMagic);
    }
    let word0 = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    Ok(Self {
      session_id: word0 >> SID24_SHIFT,
      blk_lo: u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]),
      blk_hi: u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]),
    })
  }
}

/// 构造 4 字节对齐 UDP 打洞探测包（`PKT_PUNCH | (session_id << 8)`）
#[inline]
pub const fn encode_punch(session_id: u32) -> [u8; PUNCH_PKT_LEN] {
  (u32::from(PKT_PUNCH) | ((session_id & SID24_MASK) << SID24_SHIFT)).to_le_bytes()
}

/// 构造填充至 `DEFAULT_PKT` (1400 B) 的 Path MTU 探测打洞包（RFC 8899 DPLPMTUD）
#[inline]
pub fn encode_punch_padded(session_id: u32) -> [u8; DEFAULT_PKT] {
  let mut out = [0u8; DEFAULT_PKT];
  out[0..PUNCH_PKT_LEN].copy_from_slice(&encode_punch(session_id));
  out
}

/// 解码 UDP 打洞探测包（兼容 4B 保底包与 1400B PMTUD 填充探测包）并返回 24-bit `session_id`
#[inline]
pub fn decode_punch(buf: &[u8]) -> Result<u32, WireError> {
  if buf.len() < PUNCH_PKT_LEN {
    return Err(WireError::Truncated);
  }
  if buf[0] != PKT_PUNCH {
    return Err(WireError::BadMagic);
  }
  Ok(u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) >> SID24_SHIFT)
}
