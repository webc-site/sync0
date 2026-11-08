use std::{
  fs::File,
  path::{Path, PathBuf},
  sync::{
    Arc,
    atomic::{AtomicU16, Ordering},
    mpsc::{Receiver as MpscReceiver, Sender, SyncSender, channel, sync_channel},
  },
  thread::{self, JoinHandle},
};

use s0_cdc::{BasisIndex, CdcConfig, FileAt, hash128};

use super::{
  MAX_BLOCK, RootBuf, ZSTD_DEFAULT_LEVEL, manifest::Entry, now_us, par_chunk_plan, sink::Sink,
};
use crate::{
  Receiver, UdpError,
  codec::Codec,
  compio_io::flush_memory_slices_compio,
  fs::{create_trunc, open_nonempty_file, run_par_chunks, scan_files},
};

/// 目录内跨文件去重扫描的基准总字节上限（128 MiB）
const MAX_DIR_BASIS_BYTES: u64 = 128 << 20;

pub(super) type SmallWriteJob = (PathBuf, u32, u32);
type SmallBatch = (Vec<SmallWriteJob>, Vec<u8>);

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SinglePrecheck {
  Fallback,
  SameHash,
  NeedFull,
}

pub(super) struct TargetFile {
  pub(super) write_path: PathBuf,
  pub(super) size: u64,
  pub(super) rem: u64,
  pub(super) created: bool,
  pub(super) partial: Vec<u8>,
}

pub(super) fn check_single_chunk(
  basis_file: &File,
  basis_size: u64,
  e: &Entry,
  min_sz: u64,
  io_buf: &mut Vec<u8>,
) -> Result<SinglePrecheck, UdpError> {
  if basis_size == e.size {
    let n = e.size as usize;
    if io_buf.len() < n {
      io_buf.resize(n, 0);
    }
    basis_file.read_exact_at_pos(&mut io_buf[..n], 0)?;
    Ok(if hash128(&io_buf[..n]) == e.chunks[0].hash {
      SinglePrecheck::SameHash
    } else {
      SinglePrecheck::NeedFull
    })
  } else if basis_size < e.size || basis_size <= min_sz {
    Ok(SinglePrecheck::NeedFull)
  } else {
    Ok(SinglePrecheck::Fallback)
  }
}

pub(super) fn precheck_single_chunks(
  out: &Path,
  entries: &[Entry],
  min_sz: u64,
) -> Result<Vec<SinglePrecheck>, UdpError> {
  let (n_threads, chunk_len) = par_chunk_plan(entries.len());
  let res: Vec<Result<Vec<SinglePrecheck>, UdpError>> = thread::scope(|s| {
    let mut handles = Vec::with_capacity(n_threads);
    for slice in entries.chunks(chunk_len) {
      handles.push(s.spawn(move || {
        let mut root_buf = RootBuf::new(out);
        let mut io_buf = Vec::new();
        let mut v = Vec::with_capacity(slice.len());
        for e in slice {
          if e.del || e.size == 0 || e.chunks.len() != 1 {
            v.push(SinglePrecheck::Fallback);
            continue;
          }
          let target = root_buf.join(&e.path);
          let Some((basis_file, basis_size)) = open_nonempty_file(target) else {
            v.push(SinglePrecheck::Fallback);
            continue;
          };
          v.push(check_single_chunk(
            &basis_file,
            basis_size,
            e,
            min_sz,
            &mut io_buf,
          )?);
        }
        Ok(v)
      }));
    }
    handles
      .into_iter()
      .map(|h| h.join().unwrap_or(Err(UdpError::Corrupt)))
      .collect()
  });
  let mut out_vec = Vec::with_capacity(entries.len());
  for r in res {
    out_vec.extend(r?);
  }
  Ok(out_vec)
}

pub(super) fn flush_small_writes(writes: &[SmallWriteJob], buf: &[u8]) -> Result<(), UdpError> {
  Ok(run_par_chunks(writes, |slice| {
    if flush_memory_slices_compio(slice.len(), |i| {
      let (ref path, off, len) = slice[i];
      (path.as_path(), &buf[off as usize..(off + len) as usize])
    })
    .is_ok()
    {
      return Ok(());
    }
    for &(ref path, off, len) in slice {
      let data = &buf[off as usize..(off + len) as usize];
      create_trunc(path)?.write_all_at_pos(data, 0)?;
    }
    Ok(())
  })?)
}

/// 后台异步小文件批处理刷盘器：将 `Sink::write` 累积满 `8 MiB` 或 `2048` 个小文件的批次通过有界通道投递给后台线程并发 `compio` 落盘，
/// 使 UDP 主接收循环全程只写内存缓冲，彻底消除海量小文件同步落盘阻塞。
#[derive(Default)]
pub(super) struct SmallBatchFlusher {
  tx: Option<Sender<SmallBatch>>,
  handle: Option<JoinHandle<Result<(), UdpError>>>,
}

impl SmallBatchFlusher {
  pub(super) fn submit(
    &mut self,
    writes: Vec<SmallWriteJob>,
    buf: Vec<u8>,
  ) -> Result<(), UdpError> {
    let tx = match &self.tx {
      Some(tx) => tx,
      None => {
        let (tx, rx) = channel::<SmallBatch>();
        self.handle = Some(thread::spawn(move || {
          while let Ok((w, b)) = rx.recv() {
            flush_small_writes(&w, &b)?;
          }
          Ok(())
        }));
        self.tx.insert(tx)
      }
    };
    if tx.send((writes, buf)).is_err() {
      self.tx = None;
      if let Some(h) = self.handle.take() {
        return h.join().unwrap_or(Err(UdpError::Corrupt));
      }
    }
    Ok(())
  }

  pub(super) fn finish(
    &mut self,
    tail_writes: Vec<SmallWriteJob>,
    tail_buf: Vec<u8>,
  ) -> Result<(), UdpError> {
    drop(self.tx.take());
    let tail_res = if tail_writes.is_empty() {
      Ok(())
    } else {
      flush_small_writes(&tail_writes, &tail_buf)
    };
    if let Some(h) = self.handle.take() {
      h.join().unwrap_or(Err(UdpError::Corrupt))?;
    }
    tail_res
  }
}

impl Drop for SmallBatchFlusher {
  fn drop(&mut self) {
    drop(self.tx.take());
    if let Some(h) = self.handle.take() {
      let _ = h.join();
    }
  }
}

pub(super) fn build_dir_basis(
  out: &Path,
  entries: &[Entry],
  io_buf: &mut Vec<u8>,
) -> Result<(BasisIndex, Vec<PathBuf>), UdpError> {
  let Ok(existing) = scan_files(out) else {
    return Ok((BasisIndex::new(), Vec::new()));
  };
  let mut root = RootBuf::new(out);
  let mut index = BasisIndex::new();
  let mut paths = Vec::new();
  let mut total_indexed = 0u64;
  let mut entry_cur = 0usize;
  for sf in existing {
    if sf.size == 0 {
      continue;
    }
    while entry_cur < entries.len() && entries[entry_cur].path.as_str() < sf.path.as_str() {
      entry_cur += 1;
    }
    if entry_cur < entries.len() && entries[entry_cur].path == sf.path && !entries[entry_cur].del {
      continue;
    }
    if total_indexed + sf.size > MAX_DIR_BASIS_BYTES {
      break;
    }
    let p = root.join(&sf.path);
    if let Ok(f) = File::open(p) {
      let file_idx = paths.len() as u32;
      if index
        .insert_file_with_buf(file_idx, &f, sf.size, CdcConfig::DEFAULT, io_buf)
        .is_ok()
      {
        paths.push(p.to_path_buf());
        total_indexed += sf.size;
      }
    }
  }
  Ok((index, paths))
}

struct DecJob {
  blk: u32,
  zstd: bool,
  data: Vec<u8>,
}

fn dec_and_write_one(
  codec: &mut Option<Codec>,
  out: &mut Vec<u8>,
  sink: &mut Sink,
  blk: u32,
  zstd: bool,
  data: &[u8],
  total: u64,
) -> Result<Option<u16>, UdpError> {
  let off = u64::from(blk) * MAX_BLOCK as u64;
  if zstd {
    let raw_len = total.saturating_sub(off).min(MAX_BLOCK as u64) as usize;
    let c = match codec {
      Some(c) => c,
      None => codec.insert(Codec::new(ZSTD_DEFAULT_LEVEL)?),
    };
    out.resize(raw_len, 0);
    let t0 = now_us();
    if c.decompress_block(data, out)? != raw_len {
      return Err(UdpError::Corrupt);
    }
    let dt_us = now_us().saturating_sub(t0).max(1);
    let sample_mbps = ((raw_len as u64) / dt_us).clamp(1, u16::MAX as u64) as u16;
    sink.write(out, off)?;
    Ok(Some(sample_mbps))
  } else {
    sink.write(data, off)?;
    Ok(None)
  }
}

/// 异步块解压与写出器：大于 512 KiB 的传输由独立后台工作线程并行执行 Zstd 解压与 `Sink::write`，
/// UDP 接收主线程仅将收齐的块字节移入复用池缓冲并立即 `recv.release(blk)` 释放窗口，实现 0 解压与 0 磁盘阻塞。
pub(super) struct Dec {
  inline_sink: Option<(Sink, u64)>,
  codec: Option<Codec>,
  out: Vec<u8>,
  tx: Option<SyncSender<DecJob>>,
  recycle_rx: Option<MpscReceiver<Vec<u8>>>,
  handle: Option<JoinHandle<Result<(), UdpError>>>,
  dec_mbps: Arc<AtomicU16>,
}

impl Default for Dec {
  fn default() -> Self {
    Self {
      inline_sink: None,
      codec: None,
      out: Vec::new(),
      tx: None,
      recycle_rx: None,
      handle: None,
      dec_mbps: Arc::new(AtomicU16::new(2_000)),
    }
  }
}

impl Dec {
  #[inline]
  pub(super) fn is_active(&self) -> bool {
    self.inline_sink.is_some() || self.tx.is_some()
  }

  #[inline]
  pub(super) fn dec_mbps(&self) -> u16 {
    self.dec_mbps.load(Ordering::Relaxed)
  }

  pub(super) fn bind(&mut self, mut sink: Sink, total: u64) {
    if total <= 4 * (MAX_BLOCK as u64) {
      self.inline_sink = Some((sink, total));
      return;
    }
    let (tx, rx) = sync_channel::<DecJob>(128);
    let (recycle_tx, recycle_rx) = sync_channel::<Vec<u8>>(128);
    let dec_mbps = Arc::clone(&self.dec_mbps);
    self.handle = Some(thread::spawn(move || {
      let mut codec = None;
      let mut out = Vec::new();
      let mut ewma = 2_000u32;
      while let Ok(job) = rx.recv() {
        if let Some(s) = dec_and_write_one(
          &mut codec, &mut out, &mut sink, job.blk, job.zstd, &job.data, total,
        )? {
          ewma = (ewma * 3 + u32::from(s)) / 4;
          dec_mbps.store(ewma as u16, Ordering::Relaxed);
        }
        let _ = recycle_tx.try_send(job.data);
      }
      sink.finalize()
    }));
    self.tx = Some(tx);
    self.recycle_rx = Some(recycle_rx);
  }

  pub(super) fn write(
    &mut self,
    recv: &mut Receiver,
    blk: u32,
    zstd: bool,
  ) -> Result<(), UdpError> {
    let Some(data) = recv.data(blk) else {
      return Ok(());
    };
    if let Some((ref mut sink, total)) = self.inline_sink {
      if let Some(s) =
        dec_and_write_one(&mut self.codec, &mut self.out, sink, blk, zstd, data, total)?
      {
        let cur = u32::from(self.dec_mbps.load(Ordering::Relaxed));
        self
          .dec_mbps
          .store(((cur * 3 + u32::from(s)) / 4) as u16, Ordering::Relaxed);
      }
      return recv.release(blk);
    }
    let mut buf = self
      .recycle_rx
      .as_ref()
      .and_then(|r| r.try_recv().ok())
      .unwrap_or_else(|| Vec::with_capacity(data.len().max(32 * 1024)));
    buf.clear();
    buf.extend_from_slice(data);
    recv.release(blk)?;
    if let Some(ref tx) = self.tx
      && tx
        .send(DecJob {
          blk,
          zstd,
          data: buf,
        })
        .is_err()
    {
      self.tx = None;
      if let Some(h) = self.handle.take() {
        return h.join().unwrap_or(Err(UdpError::Corrupt));
      }
    }
    Ok(())
  }

  pub(super) fn finalize(&mut self) -> Result<(), UdpError> {
    if let Some((mut sink, _)) = self.inline_sink.take() {
      return sink.finalize();
    }
    drop(self.tx.take());
    self.recycle_rx = None;
    if let Some(h) = self.handle.take() {
      h.join().unwrap_or(Err(UdpError::Corrupt))?;
    }
    Ok(())
  }
}

impl Drop for Dec {
  fn drop(&mut self) {
    drop(self.tx.take());
    if let Some(h) = self.handle.take() {
      let _ = h.join();
    }
  }
}
