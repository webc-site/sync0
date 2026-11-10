use std::{
  array::from_fn,
  cell::{Cell, OnceCell, RefCell},
  fs::File,
  future::{Future, poll_fn},
  io,
  path::{Path, PathBuf},
  pin::Pin,
  sync::{
    Arc,
    mpsc::{SyncSender, sync_channel},
  },
  task::Poll,
  thread::{self, JoinHandle},
};

use compio::{
  BufResult,
  buf::{IntoInner, IoBuf},
  fs::OpenOptions,
  io::{AsyncReadAtExt, AsyncWriteAtExt},
  runtime::Runtime,
};
use s0_cdc::{FileAt, join_io};

use crate::{RootBuf, create_trunc, open_nonempty_file, par_chunk_plan, run_par_chunks};

/// 单线程 `compio` Runtime 内部并发投递的异步文件流水线槽位数
const CONCURRENCY: usize = 8;
/// 单槽位初始复用缓冲容量（16 KiB，驻留线程局部 tcache 避免频繁 mmap/munmap）
const INIT_BUF_CAP: usize = 16 * 1024;
/// 同步回退路径单次读取缓冲容量（64 KiB）
const FALLBACK_BUF_CAP: usize = 64 * 1024;
/// 后台小文件刷盘每线程通道深度
const FLUSH_CHAN_CAP: usize = 8;

/// 小文件写出任务 `(目标路径, 批次缓冲字节偏移, 字节长度)`
pub type SmallWriteJob = (PathBuf, u32, u32);
type SmallTask = (Arc<Vec<SmallWriteJob>>, Arc<Vec<u8>>, usize, usize);

thread_local! {
  static LOCAL_RT: OnceCell<Runtime> = const { OnceCell::new() };
}

/// 在线程局部 `compio` Runtime 上执行异步 I/O 任务（每线程仅创建一次 `io_uring` / `kqueue` / `IOCP` 实例）
fn run_compio<T, F: Future<Output = io::Result<T>>>(fut: F) -> io::Result<T> {
  LOCAL_RT.with(|cell| {
    if let Some(rt) = cell.get() {
      return rt.block_on(fut);
    }
    let rt = Runtime::new()?;
    cell.get_or_init(|| rt).block_on(fut)
  })
}

#[inline]
fn take_next_idx(next_idx: &Cell<usize>, count: usize) -> Option<usize> {
  let i = next_idx.get();
  (i < count).then(|| {
    next_idx.set(i + 1);
    i
  })
}

/// 在当前线程局部 `compio` Runtime 上并发驱动 `n_slots` 个异步工作槽位（栈上定长槽位数组零堆分配，实现单线程多 SQE 批量提交）
async fn drive_slots<'a, Fut>(
  n_slots: usize,
  mut make_slot: impl FnMut() -> Pin<Box<Fut>>,
) -> io::Result<()>
where
  Fut: Future<Output = io::Result<()>> + 'a,
{
  let mut active: [Option<Pin<Box<Fut>>>; CONCURRENCY] =
    from_fn(|i| (i < n_slots).then(&mut make_slot));
  poll_fn(|cx| {
    let mut any_pending = false;
    for slot in &mut active[..n_slots] {
      if let Some(f) = slot {
        match f.as_mut().poll(cx) {
          Poll::Ready(Ok(())) => *slot = None,
          Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
          Poll::Pending => any_pending = true,
        }
      }
    }
    if any_pending {
      Poll::Pending
    } else {
      Poll::Ready(Ok(()))
    }
  })
  .await
}

/// 异步打开只读文件并从偏移 0 读取指定长度 `len` 字节到 `buf`，归还缓冲供下一文件零分配复用
async fn read_file_exact(path: &Path, mut buf: Vec<u8>, len: usize) -> BufResult<(), Vec<u8>> {
  buf.clear();
  buf.reserve(len);
  match OpenOptions::new().read(true).open(path).await {
    Ok(file) => {
      let BufResult(res, slice_buf) = file.read_exact_at(buf.slice(..len), 0).await;
      let returned = slice_buf.into_inner();
      let _ = file.close().await;
      BufResult(res, returned)
    }
    Err(e) => BufResult(Err(e), buf),
  }
}

/// 异步探测本地文件是否存在及字节长度，当文件大小等于 `expected_len` 时一并读取内容到 `buf`
async fn probe_file_exact(
  path: &Path,
  mut buf: Vec<u8>,
  expected_len: usize,
) -> (Option<(u64, bool)>, Vec<u8>) {
  let Ok(file) = OpenOptions::new().read(true).open(path).await else {
    return (None, buf);
  };
  let Ok(meta) = file.metadata().await else {
    let _ = file.close().await;
    return (None, buf);
  };
  let size = meta.len();
  if !meta.is_file() || size == 0 {
    let _ = file.close().await;
    return (None, buf);
  }
  if size != expected_len as u64 {
    let _ = file.close().await;
    return (Some((size, false)), buf);
  }
  buf.clear();
  buf.reserve(expected_len);
  let BufResult(res, slice_buf) = file.read_exact_at(buf.slice(..expected_len), 0).await;
  let returned = slice_buf.into_inner();
  let _ = file.close().await;
  if res.is_ok() {
    (Some((size, true)), returned)
  } else {
    (None, returned)
  }
}

/// 利用 `compio` 跨平台完成式异步 I/O (`io_uring` / `kqueue` / `IOCP`) 批量并发打开并读取 (`ReadAt`) 小文件；
/// 若平台不支持则自动回退至同步 `File::open` + `read_exact_at_pos`
pub(crate) fn read_batch_files<'a, G, C>(
  root: &Path,
  count: usize,
  get_job: G,
  mut on_data: C,
) -> io::Result<()>
where
  G: Fn(usize) -> Option<(&'a str, usize)>,
  C: FnMut(usize, &[u8]),
{
  if count == 0 {
    return Ok(());
  }
  let n_slots = count.min(CONCURRENCY);
  let next_idx = Cell::new(0usize);
  let get_ref = &get_job;
  let idx_ref = &next_idx;
  let on_data_cell = RefCell::new(&mut on_data);
  let on_data_ref = &on_data_cell;
  if run_compio(drive_slots(n_slots, || {
    Box::pin(async move {
      let mut root_buf = RootBuf::new(root);
      let mut reusable_buf = Vec::with_capacity(INIT_BUF_CAP);
      while let Some(i) = take_next_idx(idx_ref, count) {
        let Some((rel_path, len)) = get_ref(i) else {
          continue;
        };
        let BufResult(res, returned_buf) =
          read_file_exact(root_buf.join(rel_path), reusable_buf, len).await;
        res?;
        on_data_ref.borrow_mut()(i, &returned_buf[..len]);
        reusable_buf = returned_buf;
      }
      Ok(())
    })
  }))
  .is_ok()
  {
    return Ok(());
  }
  let mut root_buf = RootBuf::new(root);
  let mut buf = Vec::with_capacity(FALLBACK_BUF_CAP);
  for i in 0..count {
    if let Some((rel_path, len)) = get_job(i) {
      buf.resize(len, 0);
      File::open(root_buf.join(rel_path))?.read_exact_at_pos(&mut buf, 0)?;
      on_data(i, &buf);
    }
  }
  Ok(())
}

/// 利用 `compio` 批量并发预检本地已有单块文件（大小等于 `expected_len` 时一并异步读取内容）
pub(crate) fn probe_batch_files<'a, G, C>(root: &Path, count: usize, get_job: G, mut on_probe: C)
where
  G: Fn(usize) -> Option<(&'a str, usize)>,
  C: FnMut(usize, Option<(u64, Option<&[u8]>)>),
{
  if count == 0 {
    return;
  }
  let n_slots = count.min(CONCURRENCY);
  let next_idx = Cell::new(0usize);
  let get_ref = &get_job;
  let idx_ref = &next_idx;
  let on_probe_cell = RefCell::new(&mut on_probe);
  let on_probe_ref = &on_probe_cell;
  if run_compio(drive_slots(n_slots, || {
    Box::pin(async move {
      let mut root_buf = RootBuf::new(root);
      let mut reusable_buf = Vec::with_capacity(INIT_BUF_CAP);
      while let Some(i) = take_next_idx(idx_ref, count) {
        let Some((rel_path, len)) = get_ref(i) else {
          continue;
        };
        let (res, returned_buf) =
          probe_file_exact(root_buf.join(rel_path), reusable_buf, len).await;
        on_probe_ref.borrow_mut()(
          i,
          res.map(|(sz, has_data)| (sz, has_data.then(|| &returned_buf[..len]))),
        );
        reusable_buf = returned_buf;
      }
      Ok(())
    })
  }))
  .is_ok()
  {
    return;
  }
  let mut root_buf = RootBuf::new(root);
  let mut buf = Vec::with_capacity(FALLBACK_BUF_CAP);
  for i in 0..count {
    let Some((rel_path, len)) = get_job(i) else {
      continue;
    };
    let res = open_nonempty_file(root_buf.join(rel_path)).and_then(|(bf, bs)| {
      if bs == len as u64 {
        buf.resize(len, 0);
        bf.read_exact_at_pos(&mut buf, 0).ok().map(|()| (bs, true))
      } else {
        Some((bs, false))
      }
    });
    on_probe(
      i,
      res.map(|(sz, has_data)| (sz, has_data.then(|| &buf[..len]))),
    );
  }
}

/// 异步创建/截断目标文件并将 `buf` 从偏移 0 全量落盘，归还 `Vec<u8>` 供下一文件零分配复用
async fn write_new_file(path: &Path, buf: Vec<u8>) -> BufResult<(), Vec<u8>> {
  match OpenOptions::new()
    .create(true)
    .truncate(true)
    .write(true)
    .open(path)
    .await
  {
    Ok(mut file) => {
      let res = file.write_all_at(buf, 0).await;
      let _ = file.close().await;
      res
    }
    Err(e) => BufResult(Err(e), buf),
  }
}

/// 利用 `compio` 跨平台完成式异步 I/O (`io_uring` / `kqueue` / `IOCP`) 批量并发创建并定位写入 (`WriteAt`) 小文件切片；
/// 若平台不支持则自动回退至同步 `create_trunc` + `write_all_at_pos`
pub fn flush_memory_slices<'a, F>(count: usize, get_item: F) -> io::Result<()>
where
  F: Fn(usize) -> (&'a Path, &'a [u8]),
{
  if count == 0 {
    return Ok(());
  }
  let n_slots = count.min(CONCURRENCY);
  let next_idx = Cell::new(0usize);
  let get_ref = &get_item;
  let idx_ref = &next_idx;
  if run_compio(drive_slots(n_slots, || {
    Box::pin(async move {
      let mut reusable_buf = Vec::with_capacity(INIT_BUF_CAP);
      while let Some(i) = take_next_idx(idx_ref, count) {
        let (path, data) = get_ref(i);
        reusable_buf.clear();
        reusable_buf.extend_from_slice(data);
        let BufResult(res, returned_buf) = write_new_file(path, reusable_buf).await;
        res?;
        reusable_buf = returned_buf;
      }
      Ok(())
    })
  }))
  .is_ok()
  {
    return Ok(());
  }
  for i in 0..count {
    let (path, data) = get_item(i);
    create_trunc(path)?.write_all_at_pos(data, 0)?;
  }
  Ok(())
}

/// 利用 `compio` 异步 `ReadAt` + `WriteAt` 从聚合流文件 `.s0_pack.tmp` 按偏移并发读取并批量解包落盘；
/// 若平台不支持则自动回退至同步 `read_exact_at_pos` + `write_all_at_pos`
pub(crate) fn flush_pack_stream(
  pack_file: &File,
  pack_path: &Path,
  jobs: &[(PathBuf, u64, u32)],
) -> io::Result<()> {
  if jobs.is_empty() {
    return Ok(());
  }
  let n_slots = jobs.len().min(CONCURRENCY);
  let next_idx = Cell::new(0usize);
  let idx_ref = &next_idx;
  if run_compio(async move {
    let async_pack = OpenOptions::new().read(true).open(pack_path).await?;
    let pack_ref = &async_pack;
    drive_slots(n_slots, || {
      Box::pin(async move {
        let mut reusable_buf = Vec::with_capacity(INIT_BUF_CAP);
        while let Some(i) = take_next_idx(idx_ref, jobs.len()) {
          let (ref dst_path, off, len) = jobs[i];
          let n = len as usize;
          reusable_buf.clear();
          reusable_buf.reserve(n);
          let BufResult(read_res, slice_buf) =
            pack_ref.read_exact_at(reusable_buf.slice(..n), off).await;
          read_res?;
          let buf = slice_buf.into_inner();
          let BufResult(write_res, returned_buf) = write_new_file(dst_path, buf).await;
          write_res?;
          reusable_buf = returned_buf;
        }
        Ok(())
      })
    })
    .await
  })
  .is_ok()
  {
    return Ok(());
  }
  let mut buf = Vec::with_capacity(FALLBACK_BUF_CAP);
  for &(ref dst, off, len) in jobs {
    buf.resize(len as usize, 0);
    pack_file.read_exact_at_pos(&mut buf, off)?;
    create_trunc(dst)?.write_all_at_pos(&buf, 0)?;
  }
  Ok(())
}

fn flush_small_slice(slice: &[SmallWriteJob], buf: &[u8]) -> io::Result<()> {
  flush_memory_slices(slice.len(), |i| {
    let (ref path, off, len) = slice[i];
    (path.as_path(), &buf[off as usize..(off + len) as usize])
  })
}

/// 后台异步小文件批处理刷盘器：启动常驻多线程 `compio` 工作池（每线程复用单实例 `io_uring` + 并发 SQE），
/// 使主接收循环与解压线程全程只写内存缓冲，彻底消除海量小文件落盘与线程频繁启停开销。
#[derive(Default)]
pub struct SmallBatchFlusher {
  txs: Vec<SyncSender<SmallTask>>,
  handles: Vec<JoinHandle<io::Result<()>>>,
  rr: usize,
}

impl SmallBatchFlusher {
  fn ensure_workers(&mut self) {
    if !self.txs.is_empty() {
      return;
    }
    let (n_workers, _) = par_chunk_plan(usize::MAX);
    self.txs.reserve(n_workers);
    self.handles.reserve(n_workers);
    for _ in 0..n_workers {
      let (tx, rx) = sync_channel::<SmallTask>(FLUSH_CHAN_CAP);
      self.txs.push(tx);
      self.handles.push(thread::spawn(move || {
        while let Ok((w, b, lo, hi)) = rx.recv() {
          flush_small_slice(&w[lo..hi], &b)?;
        }
        Ok(())
      }));
    }
  }

  fn join_workers(&mut self) -> io::Result<()> {
    self.txs.clear();
    for h in self.handles.drain(..) {
      join_io(h.join())?;
    }
    Ok(())
  }

  pub fn submit(&mut self, writes: Vec<SmallWriteJob>, buf: Vec<u8>) -> io::Result<()> {
    if writes.is_empty() {
      return Ok(());
    }
    self.ensure_workers();
    let len = writes.len();
    let w_arc = Arc::new(writes);
    let b_arc = Arc::new(buf);
    let idx = self.rr % self.txs.len();
    self.rr = self.rr.wrapping_add(1);
    if self.txs[idx].send((w_arc, b_arc, 0, len)).is_err() {
      self.join_workers()?;
    }
    Ok(())
  }

  pub fn finish(&mut self, tail_writes: Vec<SmallWriteJob>, tail_buf: Vec<u8>) -> io::Result<()> {
    if self.txs.is_empty() {
      return if tail_writes.is_empty() {
        Ok(())
      } else {
        run_par_chunks(&tail_writes, |slice| flush_small_slice(slice, &tail_buf))
      };
    }
    if !tail_writes.is_empty() {
      let n = self.txs.len();
      let chunk = tail_writes.len().div_ceil(n).max(1);
      let w_arc = Arc::new(tail_writes);
      let b_arc = Arc::new(tail_buf);
      for (i, tx) in self.txs.iter().enumerate() {
        let lo = (i * chunk).min(w_arc.len());
        let hi = ((i + 1) * chunk).min(w_arc.len());
        if lo < hi {
          let _ = tx.send((Arc::clone(&w_arc), Arc::clone(&b_arc), lo, hi));
        }
      }
    }
    self.join_workers()
  }
}

impl Drop for SmallBatchFlusher {
  fn drop(&mut self) {
    self.txs.clear();
    for h in self.handles.drain(..) {
      let _ = h.join();
    }
  }
}
