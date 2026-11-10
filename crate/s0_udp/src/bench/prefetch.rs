use std::{
  fs::File,
  mem::swap,
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering},
  },
  thread::{self, JoinHandle},
  time::Duration,
};

use parking_lot::{Condvar, Mutex, RwLock};
use s0_cdc::{FileAt, FileSpan, locate_span};
use s0_codec::{Codec, Detector, skip_name};
use s0_const::{MAX_BLOCK, ZSTD_DEFAULT_LEVEL};
use s0_fs::{RootBuf, SmallCache};

use crate::{
  ArgError, UdpError,
  bench::{EMPTY, manifest::Entry},
  now_us,
};

/// 后台预取与并行 Zstd 压缩工作线程数
const PREFETCH_WORKERS: usize = 4;
/// 表示关闭 Zstd 压缩的哨兵级别（区别于负数快速压缩级别 `-7`、`-3`）
pub(crate) const LVL_OFF: i32 = i32::MIN;
/// 前瞻预取窗口（128 块 = 16 MiB）
const PREFETCH_AHEAD: u32 = 128;
/// 预取环容量（必须为 2 的幂且 `>= 2 * PREFETCH_AHEAD`）
const PREFETCH_RING: usize = 256;

pub(crate) enum SrcKind {
  File(File),
  Dir(PathBuf, Arc<SmallCache>),
}

impl SrcKind {
  pub(crate) fn root_buf(&self) -> RootBuf {
    match self {
      Self::File(_) => RootBuf::new(PathBuf::new().as_path()),
      Self::Dir(root, _) => RootBuf::new(root),
    }
  }
}

/// 从虚拟流偏移 `off` 读取 `raw.len()` 字节；对连续的 `SmallCache` 小文件自动合并为单次读取
pub(crate) fn read_into(
  kind: &SrcKind,
  entries: &[Entry],
  spans: &[FileSpan],
  cursor: (&mut usize, &mut RootBuf, &mut Option<(usize, File)>),
  dst: (&mut [u8], u64),
) -> Result<(bool, bool), UdpError> {
  let (span_hint, root_buf, open) = cursor;
  let (raw, off) = dst;
  let len = raw.len();
  let end = off + len as u64;
  let mut done = 0usize;
  let mut all_skip_ext = matches!(kind, SrcKind::Dir(..));
  let mut entered_compressible = false;
  let start = locate_span(spans, span_hint, off);

  let mut pending_cache: Option<(u64, usize, usize)> = None;
  let flush_cache = |cache: &SmallCache,
                     raw: &mut [u8],
                     pending: &mut Option<(u64, usize, usize)>|
   -> Result<(), UdpError> {
    if let Some((c_off, r_start, r_end)) = pending.take() {
      cache.read_at(c_off, &mut raw[r_start..r_end])?;
    }
    Ok(())
  };

  for (i, sp) in spans[start..].iter().enumerate() {
    if sp.delta_off >= end || done >= len {
      break;
    }
    if sp.delta_off + sp.len <= end {
      *span_hint = start + i + 1;
    } else {
      *span_hint = start + i;
    }
    let s = off.max(sp.delta_off);
    let t = end.min(sp.delta_off + sp.len);
    if t > s {
      let chunk_len = (t - s) as usize;
      let file_off = sp.file_off + (s - sp.delta_off);
      match kind {
        SrcKind::File(f) => {
          all_skip_ext = false;
          f.read_exact_at_pos(&mut raw[done..done + chunk_len], file_off)?;
        }
        SrcKind::Dir(_, cache) => {
          let idx = sp.idx;
          let e = &entries[idx];
          if !skip_name(&e.path) {
            all_skip_ext = false;
            if file_off == 0 {
              entered_compressible = true;
            }
          }
          if let Some(base) = e.cached {
            let c_off = base + file_off;
            match &mut pending_cache {
              Some((base_c, r_start, r_end))
                if *r_end == done && *base_c + ((*r_end - *r_start) as u64) == c_off =>
              {
                *r_end = done + chunk_len;
              }
              _ => {
                flush_cache(cache, raw, &mut pending_cache)?;
                pending_cache = Some((c_off, done, done + chunk_len));
              }
            }
          } else {
            flush_cache(cache, raw, &mut pending_cache)?;
            let chunk = &mut raw[done..done + chunk_len];
            if let Some((cached_idx, f)) = open
              && *cached_idx == idx
            {
              f.read_exact_at_pos(chunk, file_off)?;
            } else {
              let f = File::open(root_buf.join(&e.path))?;
              f.read_exact_at_pos(chunk, file_off)?;
              if file_off + (t - s) < e.size {
                *open = Some((idx, f));
              }
            }
          }
        }
      }
      done += chunk_len;
    }
  }
  if let SrcKind::Dir(_, cache) = kind {
    flush_cache(cache, raw, &mut pending_cache)?;
  }
  if done != len {
    return Err(ArgError::StreamBounds.into());
  }
  Ok((all_skip_ext, entered_compressible))
}

struct Plan {
  epoch: u64,
  spans: Arc<Vec<FileSpan>>,
  total: u64,
  nblk: u32,
}

pub(crate) struct ReadyBlk {
  pub(crate) epoch: u64,
  pub(crate) blk: u32,
  pub(crate) zstd: bool,
  pub(crate) data: Vec<u8>,
  pub(crate) raw_len: usize,
  pub(crate) comp_us: u64,
  pub(crate) lvl: i32,
  pub(crate) attempted_zstd: bool,
}

impl Default for ReadyBlk {
  fn default() -> Self {
    Self {
      epoch: 0,
      blk: EMPTY,
      zstd: false,
      data: Vec::new(),
      raw_len: 0,
      comp_us: 0,
      lvl: 1,
      attempted_zstd: false,
    }
  }
}

impl ReadyBlk {
  #[inline]
  fn take_if_match(&mut self, epoch: u64, blk: u32, dst: &mut Vec<u8>) -> Option<Self> {
    if self.epoch != epoch || self.blk != blk {
      return None;
    }
    self.blk = EMPTY;
    let mut out = Self {
      epoch: self.epoch,
      blk,
      zstd: self.zstd,
      data: Vec::new(),
      raw_len: self.raw_len,
      comp_us: self.comp_us,
      lvl: self.lvl,
      attempted_zstd: self.attempted_zstd,
    };
    swap(dst, &mut self.data);
    swap(&mut out.data, dst);
    Some(out)
  }
}

struct SharedState {
  kind: Arc<SrcKind>,
  entries: Arc<Vec<Entry>>,
  skip_all: bool,
  check_magic: bool,
  plan: RwLock<Plan>,
  next_fetch_blk: AtomicU32,
  max_allowed_blk: AtomicU32,
  lvl: AtomicI32,
  stop: AtomicBool,
  work_mu: Mutex<()>,
  work_cv: Condvar,
  ring: Box<[Mutex<ReadyBlk>]>,
  ready_cv: Condvar,
}

pub(crate) struct Prefetcher {
  shared: Arc<SharedState>,
  workers: Vec<JoinHandle<()>>,
}

impl Prefetcher {
  pub(crate) fn new(
    kind: Arc<SrcKind>,
    entries: Arc<Vec<Entry>>,
    spans: Arc<Vec<FileSpan>>,
    total: u64,
    skip_all: bool,
    check_magic: bool,
  ) -> Self {
    let nblk = total.div_ceil(MAX_BLOCK as u64) as u32;
    let init_ahead = nblk.min(PREFETCH_AHEAD);
    let ring = (0..PREFETCH_RING)
      .map(|_| Mutex::new(ReadyBlk::default()))
      .collect::<Vec<_>>()
      .into_boxed_slice();
    let shared = Arc::new(SharedState {
      kind,
      entries,
      skip_all,
      check_magic,
      plan: RwLock::new(Plan {
        epoch: 1,
        spans,
        total,
        nblk,
      }),
      next_fetch_blk: AtomicU32::new(0),
      max_allowed_blk: AtomicU32::new(init_ahead),
      lvl: AtomicI32::new(if skip_all {
        LVL_OFF
      } else {
        ZSTD_DEFAULT_LEVEL
      }),
      stop: AtomicBool::new(false),
      work_mu: Mutex::new(()),
      work_cv: Condvar::new(),
      ring,
      ready_cv: Condvar::new(),
    });
    let n_workers = if nblk > 4 {
      (nblk as usize).div_ceil(8).clamp(1, PREFETCH_WORKERS)
    } else {
      0
    };
    let mut workers = Vec::with_capacity(n_workers);
    for _ in 0..n_workers {
      let st = Arc::clone(&shared);
      workers.push(thread::spawn(move || worker_loop(st)));
    }
    Self { shared, workers }
  }

  #[inline]
  pub(crate) fn worker_count(&self) -> u32 {
    self.workers.len().max(1) as u32
  }

  #[inline]
  pub(crate) fn set_level(&self, lvl: i32) {
    self.shared.lvl.store(lvl, Ordering::Relaxed);
  }

  pub(crate) fn reset_plan(&self, epoch: u64, spans: Arc<Vec<FileSpan>>, total: u64) {
    let nblk = total.div_ceil(MAX_BLOCK as u64) as u32;
    {
      let mut p = self.shared.plan.write();
      p.epoch = epoch;
      p.spans = spans;
      p.total = total;
      p.nblk = nblk;
    }
    self.shared.next_fetch_blk.store(0, Ordering::Release);
    self
      .shared
      .max_allowed_blk
      .store(nblk.min(PREFETCH_AHEAD), Ordering::Release);
    self.shared.work_cv.notify_all();
  }

  pub(crate) fn request_ahead(&self, blk: u32, nblk: u32) {
    if self.workers.is_empty() {
      return;
    }
    let want = blk.saturating_add(PREFETCH_AHEAD).min(nblk);
    let prev = self
      .shared
      .max_allowed_blk
      .fetch_max(want, Ordering::AcqRel);
    if want > prev {
      self.shared.work_cv.notify_all();
    }
  }

  pub(crate) fn take_ready(&self, epoch: u64, blk: u32, dst: &mut Vec<u8>) -> Option<ReadyBlk> {
    if self.workers.is_empty() {
      return None;
    }
    let r_idx = (blk as usize) & (PREFETCH_RING - 1);
    if let Some(out) = self.shared.ring[r_idx]
      .lock()
      .take_if_match(epoch, blk, dst)
    {
      return Some(out);
    }
    // 若尚未被后台线程领取，则由主线程认领并返回 None 直接内联计算，绝不空等
    loop {
      let cur = self.shared.next_fetch_blk.load(Ordering::Acquire);
      if blk < cur {
        break;
      }
      if self
        .shared
        .next_fetch_blk
        .compare_exchange_weak(cur, blk + 1, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
      {
        return None;
      }
    }
    // 已被后台工作线程认领，等待其完成
    let mut g = self.shared.ring[r_idx].lock();
    for _ in 0..200 {
      if let Some(out) = g.take_if_match(epoch, blk, dst) {
        return Some(out);
      }
      self
        .shared
        .ready_cv
        .wait_for(&mut g, Duration::from_micros(50));
    }
    None
  }
}

impl Drop for Prefetcher {
  fn drop(&mut self) {
    self.shared.stop.store(true, Ordering::Release);
    self.shared.work_cv.notify_all();
    for h in self.workers.drain(..) {
      let _ = h.join();
    }
  }
}

fn worker_loop(st: Arc<SharedState>) {
  let mut codec = (!st.skip_all)
    .then(|| Codec::new(ZSTD_DEFAULT_LEVEL).ok())
    .flatten();
  let mut detector = Detector::with_flags(st.skip_all, st.check_magic);
  let mut root_buf = st.kind.root_buf();
  let mut open_fd: Option<(usize, File)> = None;
  let mut span_hint = 0usize;
  let mut last_epoch = 0u64;
  let mut raw_buf = Vec::with_capacity(MAX_BLOCK);

  while !st.stop.load(Ordering::Acquire) {
    let cur = st.next_fetch_blk.load(Ordering::Relaxed);
    let max_blk = st.max_allowed_blk.load(Ordering::Acquire);
    if cur >= max_blk {
      let mut g = st.work_mu.lock();
      if st.next_fetch_blk.load(Ordering::Relaxed) >= st.max_allowed_blk.load(Ordering::Acquire)
        && !st.stop.load(Ordering::Acquire)
      {
        st.work_cv.wait(&mut g);
      }
      continue;
    }
    let blk = st.next_fetch_blk.fetch_add(1, Ordering::AcqRel);
    let (epoch, spans, total, nblk) = {
      let p = st.plan.read();
      (p.epoch, Arc::clone(&p.spans), p.total, p.nblk)
    };
    if blk >= nblk {
      continue;
    }
    if epoch != last_epoch {
      last_epoch = epoch;
      span_hint = 0;
    }
    let off = u64::from(blk) * MAX_BLOCK as u64;
    let len = total.saturating_sub(off).min(MAX_BLOCK as u64) as usize;
    raw_buf.resize(len, 0);
    let Ok((all_skip_ext, entered_compressible)) = read_into(
      &st.kind,
      &st.entries,
      &spans,
      (&mut span_hint, &mut root_buf, &mut open_fd),
      (&mut raw_buf, off),
    ) else {
      continue;
    };
    if entered_compressible && detector.skip() {
      detector.reset_file();
    }
    let lvl = st.lvl.load(Ordering::Relaxed);
    let mut zstd = false;
    let mut comp_us = 0u64;
    let mut attempted_zstd = false;

    let r_idx = (blk as usize) & (PREFETCH_RING - 1);
    let mut slot = st.ring[r_idx].lock();
    slot.data.clear();
    if !all_skip_ext
      && lvl != LVL_OFF
      && let Some(c) = codec.as_mut()
      && c.set_level(lvl).is_ok()
    {
      let was_skip = detector.skip();
      let t0 = now_us();
      if let Ok(Some(comp)) = detector.compress(&raw_buf, c) {
        comp_us = now_us().saturating_sub(t0).max(1);
        attempted_zstd = true;
        zstd = true;
        slot.data.extend_from_slice(comp);
      } else {
        if !was_skip {
          comp_us = now_us().saturating_sub(t0).max(1);
          attempted_zstd = true;
        }
        slot.data.extend_from_slice(&raw_buf);
      }
    } else {
      slot.data.extend_from_slice(&raw_buf);
    }
    slot.epoch = epoch;
    slot.blk = blk;
    slot.zstd = zstd;
    slot.raw_len = len;
    slot.comp_us = comp_us;
    slot.lvl = lvl;
    slot.attempted_zstd = attempted_zstd;
    drop(slot);
    st.ready_cv.notify_all();
  }
}
