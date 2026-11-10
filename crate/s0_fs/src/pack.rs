use std::{
  cell::UnsafeCell,
  fs::{File, remove_file},
  io,
  path::{Path, PathBuf},
  ptr::copy_nonoverlapping,
  slice::from_raw_parts,
  sync::atomic::{AtomicU64, Ordering},
};

use s0_cdc::FileAt;

use crate::{
  FsError,
  compio_io::{flush_memory_slices, flush_pack_stream},
  filter::PACK_TMP,
  open_rw, run_par_chunks,
};

/// 小文件内存聚合池上限（64 MiB，与发送端 `DEFAULT_SMALL_ARENA` 对齐）：超过该阈值自动切换为单文件流 `.s0_pack.tmp` 顺序/偏移写入，彻底杜绝内存爆炸
const MAX_MEM_PACK_BYTES: u64 = 64 << 20;
pub(crate) const UNPACKED_OFFSET: u64 = u64::MAX;

/// 无锁不相交区间内存池（每个单块小文件拥有唯一、互不重叠的 `[off, off + len)` 切片，且由 `have_words` Release/Acquire 原子位同步）
pub struct DisjointArena {
  buf: Box<[UnsafeCell<u8>]>,
}

// SAFETY: 每个 `entry_idx` 在 `prepare` 阶段被分配严格不相交的 `[pack_off, pack_off + size)` 区间，
// 写入由 `write_verified_block` 完成并经 `have_words.fetch_or(..., Release)` 发布，
// 读取前必须先通过 `has_block` (`Acquire`) 校验，因此不存在数据竞争。
unsafe impl Sync for DisjointArena {}
unsafe impl Send for DisjointArena {}

impl DisjointArena {
  pub fn new(len: usize) -> Self {
    let mut v = Vec::with_capacity(len);
    v.resize_with(len, || UnsafeCell::new(0));
    Self {
      buf: v.into_boxed_slice(),
    }
  }

  /// # Safety
  /// 调用方必须确保并发写入的 `[off, off + data.len())` 区间互不重叠。
  #[inline]
  pub unsafe fn write_at(&self, off: usize, data: &[u8]) {
    let sub = &self.buf[off..off + data.len()];
    unsafe {
      copy_nonoverlapping(
        data.as_ptr(),
        sub.as_ptr().cast::<u8>().cast_mut(),
        data.len(),
      );
    }
  }

  /// # Safety
  /// 调用方必须确保 `[off, off + len)` 已完成写入且读取期间无并发修改。
  #[inline]
  pub unsafe fn slice_at(&self, off: usize, len: usize) -> &[u8] {
    let sub = &self.buf[off..off + len];
    unsafe { from_raw_parts(sub.as_ptr().cast::<u8>(), len) }
  }
}

/// 小文件聚合流存储后端：
/// - `<= 64 MiB`：单次分配定长连续内存池 (`Mem`)，零碎片；
/// - `> 64 MiB`：单文件流 `.s0_pack.tmp` (`StreamFile`)，`pwrite`/`pread` 直接落盘，内存占用 $O(1)$。
pub(crate) enum SmallPack {
  Empty,
  Mem(DisjointArena),
  StreamFile { file: File, path: PathBuf },
}

impl SmallPack {
  pub(crate) fn create(root: &Path, total_bytes: u64) -> io::Result<Self> {
    if total_bytes == 0 {
      return Ok(Self::Empty);
    }
    if total_bytes <= MAX_MEM_PACK_BYTES {
      return Ok(Self::Mem(DisjointArena::new(total_bytes as usize)));
    }
    let path = root.join(PACK_TMP);
    let file = open_rw(&path, true, total_bytes)?;
    Ok(Self::StreamFile { file, path })
  }

  #[inline]
  pub(crate) fn write_at(&self, off: u64, data: &[u8]) -> io::Result<()> {
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
  pub(crate) fn read_at(&self, off: u64, dst: &mut [u8]) -> io::Result<()> {
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

  pub(crate) fn flush_jobs(&self, pack_jobs: &[(PathBuf, u64, u32)]) -> Result<(), FsError> {
    if pack_jobs.is_empty() {
      return Ok(());
    }
    match self {
      Self::Empty => {}
      Self::Mem(arena) => {
        run_par_chunks(pack_jobs, |slice| {
          flush_memory_slices(slice.len(), |i| {
            let (ref path, off, len) = slice[i];
            (path.as_path(), unsafe {
              arena.slice_at(off as usize, len as usize)
            })
          })
        })?;
      }
      Self::StreamFile { file, path } => {
        run_par_chunks(pack_jobs, |slice| flush_pack_stream(file, path, slice))?;
      }
    }
    self.cleanup();
    Ok(())
  }

  pub(crate) fn cleanup(&self) {
    if let Self::StreamFile { path, .. } = self {
      let _ = remove_file(path);
    }
  }
}

impl Drop for SmallPack {
  fn drop(&mut self) {
    self.cleanup();
  }
}

/// 64 位无锁原子块就绪位图
pub struct AtomicBitset {
  words: Vec<AtomicU64>,
  total_bits: u32,
}

impl AtomicBitset {
  pub fn from_bits(initial_bits: &[u8], total_bits: u32) -> Self {
    let words = (0..(total_bits as usize).div_ceil(64))
      .map(|w_idx| {
        let (mut bytes, sub) = ([0u8; 8], &initial_bits[w_idx * 8..]);
        let n = sub.len().min(8);
        bytes[..n].copy_from_slice(&sub[..n]);
        AtomicU64::new(u64::from_le_bytes(bytes))
      })
      .collect();
    Self { words, total_bits }
  }

  pub fn snapshot(&self) -> Vec<u8> {
    let mut out = vec![0u8; (self.total_bits as usize).div_ceil(8)];
    for (wi, atom) in self.words.iter().enumerate() {
      let bytes = atom.load(Ordering::Acquire).to_le_bytes();
      if let Some(dst) = out.get_mut(wi * 8..) {
        let n = dst.len().min(8);
        dst[..n].copy_from_slice(&bytes[..n]);
      }
    }
    out
  }

  #[inline]
  fn shifted_word(&self, cur: u32) -> u64 {
    self
      .words
      .get((cur >> 6) as usize)
      .map_or(0, |w| w.load(Ordering::Acquire) >> (cur & 63))
  }

  #[inline]
  pub fn has(&self, idx: u32) -> bool {
    (self.shifted_word(idx) & 1) != 0
  }

  #[inline]
  pub fn mark(&self, idx: u32) -> bool {
    let bit = 1u64 << (idx & 63);
    (self.words[(idx >> 6) as usize].fetch_or(bit, Ordering::Release) & bit) == 0
  }

  /// 从 `cur` 向后扫描（`< hi`），返回首个未就绪块索引（若 `[cur, hi)` 全部就绪则返回 `hi`）
  #[inline]
  pub fn next_missing(&self, mut cur: u32, hi: u32) -> u32 {
    let end = hi.min(self.total_bits);
    while cur < end {
      let rem = (64 - (cur & 63)).min(end - cur);
      let ones = self.shifted_word(cur).trailing_ones().min(rem);
      cur += ones;
      if ones < rem {
        break;
      }
    }
    cur.min(hi)
  }

  #[inline]
  pub fn is_range_complete(&self, blk_lo: u32, blk_hi: u32) -> bool {
    let hi = blk_hi.min(self.total_bits);
    self.next_missing(blk_lo.min(hi), hi) >= hi
  }

  /// 遍历 `[blk_lo, blk_hi)` 内的所有连续缺失块区间 `(span_lo, span_hi)`（零堆分配，$O(N/64)$ 64 位原子字指令扫描）
  pub fn for_each_missing_span(&self, blk_lo: u32, blk_hi: u32, mut on_span: impl FnMut(u32, u32)) {
    let hi = blk_hi.min(self.total_bits);
    let mut cur = blk_lo.min(hi);
    while cur < hi {
      cur = self.next_missing(cur, hi);
      if cur >= hi {
        break;
      }
      let start = cur;
      cur += 1;
      while cur < hi {
        let rem = (64 - (cur & 63)).min(hi - cur);
        let zeros = self.shifted_word(cur).trailing_zeros().min(rem);
        cur += zeros;
        if zeros < rem {
          break;
        }
      }
      on_span(start, cur);
    }
  }

  /// 裁剪 `[blk_lo, blk_hi)` 首尾已落盘的块，返回首个缺失块到末个缺失块+1 的紧凑区间；若区间内全部就绪则返回 `None`
  pub fn trim_missing_range(&self, blk_lo: u32, blk_hi: u32) -> Option<(u32, u32)> {
    let hi = blk_hi.min(self.total_bits);
    let (mut min_lo, mut max_hi) = (hi, blk_lo.min(hi));
    self.for_each_missing_span(max_hi, hi, |m_lo, m_hi| {
      min_lo = min_lo.min(m_lo);
      max_hi = max_hi.max(m_hi);
    });
    (min_lo < max_hi).then_some((min_lo, max_hi))
  }

  /// 扫描 64 位原子字，返回当前最长连续已就绪块区间 `[best_lo, best_hi)`（供 BT 式边下边传上报 `Op::Have`）
  pub fn longest_have_span(&self) -> (u32, u32) {
    let total = self.total_bits;
    let (mut best_lo, mut best_hi, mut cur) = (0u32, 0u32, 0u32);
    while cur < total {
      let rem = (64 - (cur & 63)).min(total - cur);
      let zeros = self.shifted_word(cur).trailing_zeros().min(rem);
      cur += zeros;
      if zeros == rem {
        continue;
      }
      let start = cur;
      cur = self.next_missing(cur, total);
      if cur - start > best_hi - best_lo {
        (best_lo, best_hi) = (start, cur);
      }
    }
    (best_lo, best_hi)
  }
}
