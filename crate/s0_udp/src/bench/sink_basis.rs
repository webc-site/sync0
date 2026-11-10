use std::{
  sync::{
    Arc,
    atomic::{AtomicU16, Ordering},
    mpsc::{Receiver as MpscReceiver, SyncSender, sync_channel},
  },
  thread::{self, JoinHandle},
};

use s0_codec::Codec;
use s0_const::{MAX_BLOCK, ZSTD_DEFAULT_LEVEL};

use crate::{Receiver, UdpError, bench::sink::Sink, now_us};

/// 初始预估解压吞吐率（2000 MB/s）
pub(crate) const INIT_DEC_MBPS: u16 = 2_000;
/// 启用后台双阶段异步解压与刷盘的最小传输字节数（4 × `MAX_BLOCK` = 512 KiB）
const ASYNC_DEC_MIN_BYTES: u64 = 4 * (MAX_BLOCK as u64);
/// 后台块解压通道深度与回收缓冲池深度
const DEC_CHAN_CAP: usize = 256;
/// 后台块落盘写出通道深度
const WRITE_CHAN_CAP: usize = 64;
/// 接收端块缓冲最小初始容量（32 KiB）
const MIN_REC_BUF_CAP: usize = 32 * 1024;

struct DecJob {
  blk: u32,
  zstd: bool,
  data: Vec<u8>,
}

/// 异步块解压线程内复用同一套 `decompress_block` + MB/s 测量逻辑
fn decompress_and_measure(
  codec: &mut Option<Codec>,
  data: &[u8],
  out: &mut Vec<u8>,
  raw_len: usize,
) -> Result<u16, UdpError> {
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
  Ok(((raw_len as u64) / dt_us).clamp(1, u16::MAX as u64) as u16)
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
    let sample_mbps = decompress_and_measure(codec, data, out, raw_len)?;
    sink.write(out, off)?;
    Ok(Some(sample_mbps))
  } else {
    sink.write(data, off)?;
    Ok(None)
  }
}

/// 异步块解压与写出器：大于 512 KiB 的传输由独立后台工作线程并行执行 Zstd 解压与 `Sink::write`，
/// UDP 接收主线程仅将收齐的块字节移入复用池缓冲并立即 `recv.release(blk)` 释放窗口，实现 0 解压与 0 磁盘阻塞。
pub(crate) struct Dec {
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
      dec_mbps: Arc::new(AtomicU16::new(INIT_DEC_MBPS)),
    }
  }
}

impl Dec {
  #[inline]
  pub(crate) fn is_active(&self) -> bool {
    self.inline_sink.is_some() || self.tx.is_some()
  }

  #[inline]
  pub(crate) fn dec_mbps(&self) -> u16 {
    self.dec_mbps.load(Ordering::Relaxed)
  }

  pub(crate) fn bind(&mut self, mut sink: Sink, total: u64) {
    if total <= ASYNC_DEC_MIN_BYTES {
      self.inline_sink = Some((sink, total));
      return;
    }
    let (tx, rx) = sync_channel::<DecJob>(DEC_CHAN_CAP);
    let (recycle_tx, recycle_rx) = sync_channel::<Vec<u8>>(DEC_CHAN_CAP);
    let dec_mbps = Arc::clone(&self.dec_mbps);
    let writer_recycle = recycle_tx.clone();
    self.handle = Some(thread::spawn(move || {
      let (write_tx, write_rx) = sync_channel::<(u64, Vec<u8>, bool)>(WRITE_CHAN_CAP);
      let (raw_tx, raw_rx) = sync_channel::<Vec<u8>>(WRITE_CHAN_CAP);
      let writer = thread::spawn(move || -> Result<(), UdpError> {
        while let Ok((off, buf, is_raw)) = write_rx.recv() {
          sink.write(&buf, off)?;
          if is_raw {
            let _ = raw_tx.try_send(buf);
          } else {
            let _ = writer_recycle.try_send(buf);
          }
        }
        sink.finalize()
      });
      let mut codec = None;
      let mut ewma = u32::from(INIT_DEC_MBPS);
      let mut dec_res = Ok(());
      while let Ok(job) = rx.recv() {
        let off = u64::from(job.blk) * (MAX_BLOCK as u64);
        if job.zstd {
          let raw_len = total.saturating_sub(off).min(MAX_BLOCK as u64) as usize;
          let mut out = raw_rx
            .try_recv()
            .unwrap_or_else(|_| Vec::with_capacity(MAX_BLOCK));
          let sample_mbps = match decompress_and_measure(&mut codec, &job.data, &mut out, raw_len) {
            Ok(s) => s,
            Err(e) => {
              dec_res = Err(e);
              break;
            }
          };
          ewma = (ewma * 3 + u32::from(sample_mbps)) / 4;
          dec_mbps.store(ewma as u16, Ordering::Relaxed);
          let _ = recycle_tx.try_send(job.data);
          if write_tx.send((off, out, true)).is_err() {
            break;
          }
        } else if write_tx.send((off, job.data, false)).is_err() {
          break;
        }
      }
      drop(write_tx);
      let write_res = writer.join().unwrap_or(Err(UdpError::Corrupt));
      dec_res.and(write_res)
    }));
    self.tx = Some(tx);
    self.recycle_rx = Some(recycle_rx);
  }

  pub(crate) fn write(
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
      .unwrap_or_else(|| Vec::with_capacity(data.len().max(MIN_REC_BUF_CAP)));
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

  pub(crate) fn finalize(&mut self) -> Result<(), UdpError> {
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
