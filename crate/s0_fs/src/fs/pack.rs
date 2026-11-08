use std::{
  cell::UnsafeCell,
  fs::{self, File},
  io,
  mem::take,
  path::{Path, PathBuf},
  ptr::copy_nonoverlapping,
  slice::from_raw_parts,
  sync::mpsc::{Sender, channel},
  thread::{self, JoinHandle},
};

use parking_lot::Mutex;
use s0_cdc::FileAt;

use super::{FsError, create_trunc, run_par_chunks};
use crate::compio_io::flush_memory_slices_compio;

/// 小文件直接内存池上限（8 MiB）：超过该阈值自动切换为后台流式并发解包池 (`Stream`)，边收网络包边并发落盘
const MAX_MEM_PACK_BYTES: u64 = 8 << 20;
/// 流式解包单批聚合字节阈值（32 MiB）
const STREAM_BATCH_BYTES: usize = 32 << 20;
/// 流式解包单批聚合小文件数阈值
const STREAM_BATCH_FILES: usize = 8192;
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

#[derive(Default)]
struct StreamBatch {
  jobs: Vec<(PathBuf, u32, u32)>,
  buf: Vec<u8>,
}

fn flush_stream_batch(batch: &StreamBatch) -> io::Result<()> {
  run_par_chunks(&batch.jobs, |slice| {
    if flush_memory_slices_compio(slice.len(), |i| {
      let (ref path, off, len) = slice[i];
      (
        path.as_path(),
        &batch.buf[off as usize..(off + len) as usize],
      )
    })
    .is_ok()
    {
      return Ok(());
    }
    for &(ref path, off, len) in slice {
      let data = &batch.buf[off as usize..(off + len) as usize];
      create_trunc(path)?.write_all_at_pos(data, 0)?;
    }
    Ok(())
  })
}

#[derive(Default)]
struct StreamState {
  batch: StreamBatch,
  tx: Option<Sender<StreamBatch>>,
  worker: Option<JoinHandle<io::Result<()>>>,
}

pub(super) struct StreamPack {
  state: Mutex<StreamState>,
}

impl StreamPack {
  fn new() -> Self {
    Self {
      state: Mutex::new(StreamState::default()),
    }
  }

  fn write_small(&self, final_path: PathBuf, data: &[u8]) -> io::Result<()> {
    let to_send = {
      let mut st = self.state.lock();
      if st.batch.buf.is_empty() {
        st.batch.buf.reserve(STREAM_BATCH_BYTES.min(8 << 20));
        st.batch.jobs.reserve(1024);
      }
      let off = st.batch.buf.len() as u32;
      st.batch.buf.extend_from_slice(data);
      st.batch.jobs.push((final_path, off, data.len() as u32));
      if st.batch.buf.len() >= STREAM_BATCH_BYTES || st.batch.jobs.len() >= STREAM_BATCH_FILES {
        let ready = StreamBatch {
          jobs: take(&mut st.batch.jobs),
          buf: take(&mut st.batch.buf),
        };
        let tx = match &st.tx {
          Some(tx) => tx.clone(),
          None => {
            let (tx, rx) = channel::<StreamBatch>();
            st.worker = Some(thread::spawn(move || {
              while let Ok(b) = rx.recv() {
                flush_stream_batch(&b)?;
              }
              Ok(())
            }));
            st.tx.insert(tx).clone()
          }
        };
        Some((tx, ready))
      } else {
        None
      }
    };
    if let Some((tx, ready)) = to_send
      && tx.send(ready).is_err()
    {
      return self.join_worker();
    }
    Ok(())
  }

  fn read_small(&self, final_path: &Path, dst: &mut [u8]) -> io::Result<()> {
    {
      let st = self.state.lock();
      if let Some(&(_, off, len)) = st.batch.jobs.iter().rev().find(|(p, ..)| p == final_path)
        && (len as usize) == dst.len()
      {
        dst.copy_from_slice(&st.batch.buf[off as usize..(off + len) as usize]);
        return Ok(());
      }
    }
    if let Ok(f) = File::open(final_path)
      && f.read_exact_at_pos(dst, 0).is_ok()
    {
      return Ok(());
    }
    self.flush_all()?;
    File::open(final_path)?.read_exact_at_pos(dst, 0)
  }

  fn join_worker(&self) -> io::Result<()> {
    let worker = {
      let mut st = self.state.lock();
      st.tx = None;
      st.worker.take()
    };
    if let Some(h) = worker {
      h.join()
        .unwrap_or_else(|_| Err(io::Error::other("stream pack worker panic")))?;
    }
    Ok(())
  }

  fn flush_all(&self) -> io::Result<()> {
    let (tail, tx, worker) = {
      let mut st = self.state.lock();
      let tail = StreamBatch {
        jobs: take(&mut st.batch.jobs),
        buf: take(&mut st.batch.buf),
      };
      (tail, st.tx.take(), st.worker.take())
    };
    drop(tx);
    let mut tail_res = Ok(());
    if !tail.jobs.is_empty() {
      tail_res = flush_stream_batch(&tail);
    }
    if let Some(h) = worker {
      h.join()
        .unwrap_or_else(|_| Err(io::Error::other("stream pack worker panic")))?;
    }
    tail_res
  }
}

/// 小文件聚合流存储后端：
/// - `<= 8 MiB`：单次分配定长连续内存池 (`Mem`)，零碎片；
/// - `> 8 MiB`：后台流式并发解包池 (`Stream`)，边收网络包边并发调用 `compio` 写入最终目标文件，省去 `.s0_pack.tmp` 双倍磁盘 I/O。
pub(super) enum SmallPack {
  Empty,
  Mem(DisjointArena),
  Stream(StreamPack),
}

impl SmallPack {
  pub(super) fn create(_root: &Path, total_bytes: u64) -> io::Result<Self> {
    if total_bytes == 0 {
      return Ok(Self::Empty);
    }
    if total_bytes <= MAX_MEM_PACK_BYTES {
      return Ok(Self::Mem(DisjointArena::new(total_bytes as usize)));
    }
    Ok(Self::Stream(StreamPack::new()))
  }

  #[inline]
  pub(super) const fn is_stream(&self) -> bool {
    matches!(self, Self::Stream(_))
  }

  #[inline]
  pub(super) fn write_at(
    &self,
    off: u64,
    make_path: impl FnOnce() -> PathBuf,
    data: &[u8],
  ) -> io::Result<()> {
    match self {
      Self::Empty => Ok(()),
      Self::Mem(arena) => {
        unsafe { arena.write_at(off as usize, data) };
        Ok(())
      }
      Self::Stream(sp) => sp.write_small(make_path(), data),
    }
  }

  #[inline]
  pub(super) fn read_at(
    &self,
    off: u64,
    make_path: impl FnOnce() -> PathBuf,
    dst: &mut [u8],
  ) -> io::Result<()> {
    match self {
      Self::Empty => Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
      Self::Mem(arena) => {
        let src = unsafe { arena.slice_at(off as usize, dst.len()) };
        dst.copy_from_slice(src);
        Ok(())
      }
      Self::Stream(sp) => sp.read_small(&make_path(), dst),
    }
  }

  pub(super) fn flush_jobs(&self, pack_jobs: &[(PathBuf, u64, u32)]) -> Result<(), FsError> {
    match self {
      Self::Empty => {}
      Self::Mem(arena) => {
        if !pack_jobs.is_empty() {
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
      }
      Self::Stream(sp) => sp.flush_all()?,
    }
    Ok(())
  }

  pub(super) fn cleanup(&self) {
    if let Self::Stream(sp) = self {
      let _ = sp.join_worker();
    }
  }
}
