use std::io;

use thiserror::Error;

/// 配置与格式解析错误
#[derive(Debug, Error)]
pub enum ConfError {
  #[error(transparent)]
  Io(#[from] io::Error),
  #[error(transparent)]
  Nt(#[from] nested_text::Error),
  #[error("invalid rate format")]
  BadRate,
}
