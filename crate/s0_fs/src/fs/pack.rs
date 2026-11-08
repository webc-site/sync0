use std::{
  cell::UnsafeCell,
  fs::{self, File},
  io,
  path::{Path, PathBuf},
  ptr::copy_nonoverlapping,
  slice::from_raw_parts,
};

use s0_cdc::FileAt;

use super::{FsError, open_rw, run_par_chunks};
use crate::compio_io::{flush_memory_slices_compio, flush_pack_stream_compio};

/// 小文件内存聚合池上限（8 MiB）：超过该阈值自动切换为单文件流 `.s0_pack.tmp` 顺序/偏移写入，彻底杜绝内存爆炸
const MAX_MEM_PACK_BYTES: u64 = 8 << 20;
pub(super) const UNPACKED_OFFSET: u64 = u64::MAX;

/// 无锁不相交区间内存池（每个单块小文件拥有唯一、互不重叠的 `[off, off + len)` 切片，且由 `have_words` Release/Acquire 原子位同步）
pub(super) struct DisjointArena {
  buf: Box<[UnsafeCell<u8>]>,
}

// SAFETY: 每个 `entry_idx` 在 `prepare` 阶段被分配严格不相交的 `[pack_off, pack_off + size)` 区间，
// 写入由 `write_verified_block` 完成并经 `have_words.fetch_or(..., Release)` 发布，
// 读取前必须先通过 `has_block` (`Acquire`) 校验，因此不存在数据竞争。
unsafe impl Sync for DisjointArena {}
unsafe impl Send for DisjointArena {}

impl DisjointArena {
  fn new(len: usize) -> Self {
    let mut v = Vec::with_capacity(len);
    v.resize_with(len, || UnsafeCell::new(0));
    Self {
      buf: v.into_boxed_slice(),
    }
  }

  #[inline]
  pub(super) unsafe fn write_at(&self, off: usize, data: &[u8]) {
    let dst = unsafe { (self.buf.as_ptr() as *mut u8).add(off) };
    unsafe {
      copy_nonoverlapping(data.as_ptr(), dst, data.len());
    }
  }

  #[inline]
  pub(super) unsafe fn slice_at(&self, off: usize, len: usize) -> &[u8] {
    let ptr = unsafe { (self.buf.as_ptr() as *const u8).add(off) };
    unsafe { from_raw_parts(ptr, len) }
  }
}

/// 小文件聚合流存储后端：
/// - `<= 8 MiB`：单次分配定长连续内存池 (`Mem`)，零碎片；
/// - `> 8 MiB`：单文件流 `.s0_pack.tmp` (`StreamFile`)，`pwrite`/`pread` 直接落盘，内存占用 $O(1)$。
pub(super) enum SmallPack {
  Empty,
  Mem(DisjointArena),
  StreamFile { file: File, path: PathBuf },
}

impl SmallPack {
  pub(super) fn create(root: &Path, total_bytes: u64) -> io::Result<Self> {
    if total_bytes == 0 {
      return Ok(Self::Empty);
    }
    if total_bytes <= MAX_MEM_PACK_BYTES {
      return Ok(Self::Mem(DisjointArena::new(total_bytes as usize)));
    }
    let path = root.join(".s0_pack.tmp");
    let file = open_rw(&path, true, total_bytes)?;
    Ok(Self::StreamFile { file, path })
  }

  #[inline]
  pub(super) fn write_at(&self, off: u64, data: &[u8]) -> io::Result<()> {
    match self {
      Self::Empty => Ok(()),
      Self::Mem(arena) => {
        unsafe { arena.write_at(off as usize, data) };
        Ok(())
      }
      Self::StreamFile { file, .. } => file.write_all_at_pos(data, off),
    }
  }

  #[inline]
  pub(super) fn read_at(&self, off: u64, dst: &mut [u8]) -> io::Result<()> {
    match self {
      Self::Empty => Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
      Self::Mem(arena) => {
        let src = unsafe { arena.slice_at(off as usize, dst.len()) };
        dst.copy_from_slice(src);
        Ok(())
      }
      Self::StreamFile { file, .. } => file.read_exact_at_pos(dst, off),
    }
  }

  pub(super) fn flush_jobs(&self, pack_jobs: &[(PathBuf, u64, u32)]) -> Result<(), FsError> {
    if pack_jobs.is_empty() {
      return Ok(());
    }
    match self {
      Self::Empty => {}
      Self::Mem(arena) => {
        run_par_chunks(pack_jobs, |slice| {
          if flush_memory_slices_compio(slice.len(), |i| {
            let (ref path, off, len) = slice[i];
            (path.as_path(), unsafe {
              arena.slice_at(off as usize, len as usize)
            })
          })
          .is_ok()
          {
            return Ok(());
          }
          for &(ref path, off, len) in slice {
            fs::write(path, unsafe { arena.slice_at(off as usize, len as usize) })?;
          }
          Ok(())
        })?;
      }
      Self::StreamFile { file, path } => {
        run_par_chunks(pack_jobs, |slice| {
          if flush_pack_stream_compio(path, slice).is_ok() {
            return Ok(());
          }
          let mut buf = Vec::with_capacity(64 * 1024);
          for &(ref dst, off, len) in slice {
            buf.resize(len as usize, 0);
            file.read_exact_at_pos(&mut buf, off)?;
            fs::write(dst, &buf)?;
          }
          Ok(())
        })?;
      }
    }
    self.cleanup();
    Ok(())
  }

  pub(super) fn cleanup(&self) {
    if let Self::StreamFile { path, .. } = self {
      let _ = fs::remove_file(path);
    }
  }
}
