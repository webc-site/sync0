mod bitmap;
mod change;
mod cursor;
mod delta;
mod fastcdc;
mod file_io;
mod manifest;

use bitmap::get_bit_usize;
pub use bitmap::{
  bitset_to_roaring, decode_roaring_bits, encode_roaring_bits, for_each_have_span,
  for_each_missing_span, get_bit, is_all_have, is_roaring_full_frame, matches_any_selected,
  roaring_runs, roaring_to_bitset, set_bit_usize,
};
pub use change::PathChange;
pub use cursor::{Rd, SliceWr, WireError, Wr};
pub use delta::{
  BasisIndex, CopySpan, DataSpan, FileDelta, FileSpan, for_each_missing_data_span, locate_span,
  match_basis_file, match_chunks,
};
use fastcdc::find_boundary;
pub use fastcdc::{CdcConfig, ChunkSig, chunk_slice, hash128};
use file_io::for_each_file_chunk_from_with_buf;
pub use file_io::{
  FileAt, Hash128, MAX_PAR_THREADS, chunk_file, chunk_file_range_with_buf, copy_file_span,
  copy_file_spans, join_io, max_par_threads,
};
pub use manifest::{BlockLoc, Manifest, ManifestEntry, norm_mode};
