use std::io::Error as IoError;

use nested_text::Error as NtError;
use thiserror::Error;

/// 配置与格式解析错误
#[derive(Debug, Error)]
pub enum ConfError {
  #[error(transparent)]
  Io(#[from] IoError),
  #[error(transparent)]
  Nt(#[from] NtError),
  #[error("invalid rate format")]
  BadRate,
}
