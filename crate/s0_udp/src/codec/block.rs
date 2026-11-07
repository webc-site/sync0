use mzstd::{Compressor, Decompressor, EncoderConfig, Level};

use super::{Error, FrameError, Result};
pub use crate::s0_const::MAX_BLOCK;

/// 压缩块前缀：`u32 clen`
pub const HEADER: usize = 4;
/// 压缩输出缓冲需要的额外余量
pub const SLACK: usize = 4096;
/// 压缩后达到原长 97% 即回退原文
pub const FALLBACK_PCT: usize = 97;

/// 压缩结果
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Encoded {
  /// 写入 `dst` 的字节数
  pub len: usize,
  /// 是否为压缩帧（false = 原文）
  pub compressed: bool,
}

impl Encoded {
  pub const fn ratio_pct(&self, raw_len: usize) -> usize {
    if raw_len == 0 {
      return 0;
    }
    self.len * 100 / raw_len
  }
}

/// 每线程复用的 zstd 编解码器
pub struct Codec {
  c: Compressor,
  d: Decompressor,
  level: i32,
}

impl Codec {
  pub fn new(level: i32) -> Result<Self> {
    Ok(Self {
      c: Compressor::new(config(level)?)?,
      d: Decompressor::default(),
      level,
    })
  }

  pub const fn level(&self) -> i32 {
    self.level
  }

  pub fn set_level(&mut self, level: i32) -> Result<()> {
    if level != self.level {
      self.c.reconfigure(config(level)?)?;
      self.level = level;
    }
    Ok(())
  }

  /// 尝试压缩到 `dst`（需 ≥ `raw.len() + SLACK`）；压缩收益不足时返回 `Ok(None)`，避免无谓拷贝原文
  pub fn try_compress_block(&mut self, raw: &[u8], dst: &mut [u8]) -> Result<Option<usize>> {
    if dst.len() < raw.len().saturating_add(SLACK) {
      return Err(Error::Overflow);
    }
    if raw.is_empty() {
      return Ok(None);
    }
    let n = self.c.compress_to_slice(raw, &mut dst[HEADER..])?;
    let total = n + HEADER;
    if total * 100 >= raw.len() * FALLBACK_PCT {
      return Ok(None);
    }
    dst[..HEADER].copy_from_slice(&(n as u32).to_le_bytes());
    Ok(Some(total))
  }

  /// `dst` 需 ≥ `raw.len() + SLACK`；压缩收益不足时写回原文
  pub fn compress_block(&mut self, raw: &[u8], dst: &mut [u8]) -> Result<Encoded> {
    if let Some(len) = self.try_compress_block(raw, dst)? {
      Ok(Encoded {
        len,
        compressed: true,
      })
    } else {
      dst[..raw.len()].copy_from_slice(raw);
      Ok(Encoded {
        len: raw.len(),
        compressed: false,
      })
    }
  }

  /// 解压 `u32 clen ‖ frame`，返回写出的字节数
  pub fn decompress_block(&mut self, src: &[u8], dst: &mut [u8]) -> Result<usize> {
    let (head, body) = src
      .split_first_chunk::<HEADER>()
      .ok_or(Error::Frame(FrameError::ShortPrefix))?;
    let frame = body
      .get(..u32::from_le_bytes(*head) as usize)
      .ok_or(Error::Frame(FrameError::LenOverflow))?;
    Ok(self.d.decompress_to_slice(frame, dst)?)
  }
}

fn config(level: i32) -> Result<EncoderConfig> {
  let level = Level::try_from(level)?;
  Ok(EncoderConfig::default().with_level(level))
}
