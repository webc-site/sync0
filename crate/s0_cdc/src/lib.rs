use std::io::Error as IoError;

use thiserror::Error;

mod cursor;
mod delta;
mod fastcdc;
mod file_io;
mod manifest;

pub use cursor::{Rd, SliceWr, WireError, Wr};
pub use delta::{
  BasisIndex, CopySpan, DataSpan, FileDelta, for_each_missing_data_span, match_basis_file,
  match_chunks,
};
pub use fastcdc::{CdcConfig, find_boundary};
pub use file_io::{
  FileAt, Hash128, chunk_file, chunk_file_with_buf, copy_file_span,
  for_each_file_chunk_from_with_buf,
};
pub use manifest::{
  BlockLoc, ChunkSig, FLAG_DELETE, FLAG_PARTIAL, MANIFEST_MAGIC, Manifest, ManifestEntry,
  chunk_slice, for_each_missing_span, get_bit, get_bit_usize, hash128, is_all_have,
  matches_any_selected, set_bit, set_bit_usize,
};

#[derive(Debug, Error)]
pub enum CdcError {
  #[error("IO 错误: {0}")]
  Io(#[from] IoError),
  #[error("线协议编解码错误: {0}")]
  Wire(#[from] WireError),
}
