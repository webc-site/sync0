use std::result::Result as StdResult;

use mzstd::{ConfigError, DecodeError, EncodeError};
use thiserror::Error;

/// 压缩帧头错误编码（1 字节数值枚举，替代字符串胖指针）
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
#[repr(u8)]
pub enum FrameError {
  /// 块前缀不足 4 字节
  #[error("块前缀不足")]
  ShortPrefix = 1,
  /// `clen` 超过载荷剩余长度
  #[error("clen 超过载荷")]
  LenOverflow = 2,
}

#[derive(Error, Debug)]
pub enum Error {
  #[error(transparent)]
  Encode(#[from] EncodeError),

  #[error(transparent)]
  Decode(#[from] DecodeError),

  #[error(transparent)]
  Config(#[from] ConfigError),

  #[error("帧格式非法: {0}")]
  Frame(#[from] FrameError),

  #[error("输出缓冲不足")]
  Overflow,
}

pub type Result<T> = StdResult<T, Error>;
