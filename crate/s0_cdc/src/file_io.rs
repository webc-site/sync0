#[cfg(target_os = "linux")]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::fs::FileExt as UnixFileExt;
#[cfg(windows)]
use std::os::windows::fs::FileExt as WinFileExt;
use std::{fs::File, io, thread};

use s0_const::HASH_LEN;

use crate::{CdcConfig, ChunkSig, CopySpan, find_boundary, hash128};

/// 128 位 `gxhash128` 内容指纹（16 字节，100 GB 数据下碰撞率 < 10^-24）
pub type Hash128 = [u8; HASH_LEN];

/// 流式文件读取缓冲大小（1 MiB = 16 × 默认最大块，减少大文件 `pread` 系统调用与滑窗 `memmove` 频次）
pub(crate) const IO_BUF_LEN: usize = 1024 * 1024;
/// 启用多线程收敛并行 FastCDC 的单文件字节阈值（8 MiB）
pub(crate) const PAR_FILE_MIN_BYTES: u64 = 8 << 20;
/// 每个并行 FastCDC 分片最小字节数（4 MiB）
const PAR_STRIPE_MIN_BYTES: u64 = 4 << 20;
/// 并行条带边界的重叠收敛窗口块数（16 × `max_size` = 1 MiB，确保无状态 Gear 滚哈 100% 收敛到完全相同的切分点）
const PAR_OVERLAP_BLOCKS: usize = 16;
/// 本地基准块拷贝最小缓冲容量（64 KiB）
const COPY_SPAN_BUF_CAP: usize = 64 * 1024;
/// 并行文件处理最大工作线程数
pub const MAX_PAR_THREADS: usize = 8;

/// 等待工作线程返回 `io::Result<T>`，线程 panic 时统一映射为 `io::ErrorKind::Other`
#[inline]
pub fn join_io<T>(res: thread::Result<io::Result<T>>) -> io::Result<T> {
  res.unwrap_or_else(|_| Err(io::Error::from(io::ErrorKind::Other)))
}

/// 获取当前 CPU 并发工作线程数 `[1, MAX_PAR_THREADS]`
#[inline]
pub fn max_par_threads() -> usize {
  thread::available_parallelism()
    .map_or(MAX_PAR_THREADS / 2, |n| n.get())
    .clamp(1, MAX_PAR_THREADS)
}

/// 跨平台无锁原位随机读写扩展 (`pread` / `pwrite`)
pub trait FileAt {
  fn read_exact_at_pos(&self, buf: &mut [u8], offset: u64) -> io::Result<()>;
  fn write_all_at_pos(&self, buf: &[u8], offset: u64) -> io::Result<()>;
}

#[cfg(unix)]
impl FileAt for File {
  #[inline]
  fn read_exact_at_pos(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
    UnixFileExt::read_exact_at(self, buf, offset)
  }

  #[inline]
  fn write_all_at_pos(&self, buf: &[u8], offset: u64) -> io::Result<()> {
    UnixFileExt::write_all_at(self, buf, offset)
  }
}

#[cfg(windows)]
impl FileAt for File {
  #[inline]
  fn read_exact_at_pos(&self, mut buf: &mut [u8], mut offset: u64) -> io::Result<()> {
    while !buf.is_empty() {
      let n = WinFileExt::seek_read(self, buf, offset)?;
      if n == 0 {
        return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
      }
      buf = &mut buf[n..];
      offset += n as u64;
    }
    Ok(())
  }

  #[inline]
  fn write_all_at_pos(&self, mut buf: &[u8], mut offset: u64) -> io::Result<()> {
    while !buf.is_empty() {
      let n = WinFileExt::seek_write(self, buf, offset)?;
      if n == 0 {
        return Err(io::Error::from(io::ErrorKind::WriteZero));
      }
      buf = &buf[n..];
      offset += n as u64;
    }
    Ok(())
  }
}

/// 跨平台流式对文件执行 FastCDC 分块与 `gxhash128` 签名（基于 `FileAt`，固定缓冲零拷贝滑窗）
pub fn chunk_file(file: &File, size: u64, cfg: CdcConfig) -> io::Result<Vec<ChunkSig>> {
  let mut buf = Vec::new();
  chunk_file_range_with_buf(file, 0, size, cfg, &mut buf)
}

/// 对文件任意半开区间 `[start_off, end_off)` 执行多线程收敛并行 FastCDC 分块与 `gxhash128` 签名
pub fn chunk_file_range_with_buf(
  file: &File,
  start_off: u64,
  end_off: u64,
  cfg: CdcConfig,
  buf: &mut Vec<u8>,
) -> io::Result<Vec<ChunkSig>> {
  if start_off >= end_off {
    return Ok(Vec::new());
  }
  let span = end_off - start_off;
  if span <= cfg.min_size as u64 {
    let n = span as usize;
    if buf.len() < n {
      buf.resize(n, 0);
    }
    file.read_exact_at_pos(&mut buf[..n], start_off)?;
    return Ok(vec![ChunkSig {
      offset: start_off,
      len: span as u32,
      hash: hash128(&buf[..n]),
    }]);
  }
  let est = usize::try_from(span / (cfg.avg_size.max(1) as u64)).unwrap_or(0) + 2;
  let n_threads = if span >= PAR_FILE_MIN_BYTES {
    max_par_threads().min((span / PAR_STRIPE_MIN_BYTES) as usize)
  } else {
    1
  };
  if n_threads <= 1 {
    let mut out = Vec::with_capacity(est);
    for_each_file_chunk_from_with_buf(file, start_off, end_off, cfg, buf, |sig| out.push(sig))?;
    return Ok(out);
  }

  let overlap = (cfg.max_size.saturating_mul(PAR_OVERLAP_BLOCKS)) as u64;
  let parts: Vec<io::Result<(u64, Vec<ChunkSig>)>> = thread::scope(|s| {
    let mut handles = Vec::with_capacity(n_threads);
    for i in 0..n_threads {
      let start = start_off + (span * (i as u64)) / (n_threads as u64);
      let next_start = start_off + (span * ((i + 1) as u64)) / (n_threads as u64);
      let limit = if i + 1 == n_threads {
        end_off
      } else {
        next_start.saturating_add(overlap).min(end_off)
      };
      handles.push(s.spawn(move || {
        let mut local_buf = Vec::new();
        let cap = usize::try_from((limit - start) / (cfg.avg_size.max(1) as u64)).unwrap_or(0) + 2;
        let mut v = Vec::with_capacity(cap);
        for_each_file_chunk_from_with_buf(file, start, limit, cfg, &mut local_buf, |sig| {
          v.push(sig)
        })?;
        Ok((start, v))
      }));
    }
    handles.into_iter().map(|h| join_io(h.join())).collect()
  });

  let mut out = Vec::with_capacity(est);
  for (i, res) in parts.into_iter().enumerate() {
    let (boundary, part) = res?;
    if i == 0 {
      out = part;
      continue;
    }
    let tail_start = out.partition_point(|c| c.offset < boundary);
    if let Some((idx, p_idx)) = (tail_start..out.len()).find_map(|idx| {
      part
        .binary_search_by_key(&out[idx].offset, |c| c.offset)
        .ok()
        .map(|p_idx| (idx, p_idx))
    }) {
      out.truncate(idx);
      out.extend_from_slice(&part[p_idx..]);
      continue;
    }
    let cur_end = out
      .last()
      .map_or(start_off, |c| c.offset + u64::from(c.len));
    if let Ok(p_idx) = part.binary_search_by_key(&cur_end, |c| c.offset) {
      out.extend_from_slice(&part[p_idx..]);
      continue;
    }
    let mut splice_idx = None;
    for_each_file_chunk_until(file, cur_end, end_off, cfg, buf, |sig| {
      if let Ok(p_idx) = part.binary_search_by_key(&sig.offset, |c| c.offset) {
        splice_idx = Some(p_idx);
        false
      } else {
        out.push(sig);
        true
      }
    })?;
    if let Some(p_idx) = splice_idx {
      out.extend_from_slice(&part[p_idx..]);
    }
  }
  Ok(out)
}

/// 从指定起始偏移 `start_off` 流式对文件后缀 `[start_off, size)` 执行 FastCDC 分块与签名
pub(crate) fn for_each_file_chunk_from_with_buf(
  file: &File,
  start_off: u64,
  size: u64,
  cfg: CdcConfig,
  buf: &mut Vec<u8>,
  mut on_sig: impl FnMut(ChunkSig),
) -> io::Result<()> {
  for_each_file_chunk_until(file, start_off, size, cfg, buf, |sig| {
    on_sig(sig);
    true
  })
}

fn for_each_file_chunk_until(
  file: &File,
  start_off: u64,
  size: u64,
  cfg: CdcConfig,
  buf: &mut Vec<u8>,
  mut on_sig: impl FnMut(ChunkSig) -> bool,
) -> io::Result<()> {
  if start_off >= size {
    return Ok(());
  }
  let rem_size = size - start_off;
  if rem_size <= cfg.min_size as u64 {
    let n = rem_size as usize;
    if buf.len() < n {
      buf.resize(n, 0);
    }
    file.read_exact_at_pos(&mut buf[..n], start_off)?;
    let _ = on_sig(ChunkSig {
      offset: start_off,
      len: rem_size as u32,
      hash: hash128(&buf[..n]),
    });
    return Ok(());
  }
  let max_cap = IO_BUF_LEN.max(cfg.max_size * 2);
  let buf_cap = usize::try_from(rem_size).map_or(max_cap, |s| s.min(max_cap));
  if buf.len() < buf_cap {
    buf.resize(buf_cap, 0);
  }
  let buf = &mut buf[..buf_cap];
  let mut filled = 0usize;
  let mut file_pos = start_off;
  let mut chunk_off = start_off;

  loop {
    if filled < cfg.max_size && file_pos < size {
      let to_read = ((buf_cap - filled) as u64).min(size - file_pos) as usize;
      file.read_exact_at_pos(&mut buf[filled..filled + to_read], file_pos)?;
      filled += to_read;
      file_pos += to_read as u64;
    }
    if filled == 0 {
      break;
    }
    let eof = file_pos >= size;
    let mut pos = 0usize;
    while filled - pos >= cfg.max_size || (eof && pos < filled) {
      let cut = find_boundary(&buf[pos..filled], cfg);
      let slice = &buf[pos..pos + cut];
      if !on_sig(ChunkSig {
        offset: chunk_off,
        len: cut as u32,
        hash: hash128(slice),
      }) {
        return Ok(());
      }
      chunk_off += cut as u64;
      pos += cut;
    }
    if pos > 0 {
      buf.copy_within(pos..filled, 0);
      filled -= pos;
    }
  }
  Ok(())
}

/// 跨平台从 `src` 的 `src_off` 拷贝 `len` 字节到 `dst` 的 `dst_off`（Linux 优先走 `copy_file_range` 内核零拷贝，不支持时回退用户态缓冲）
pub fn copy_file_span(
  src: &File,
  mut src_off: u64,
  dst: &File,
  mut dst_off: u64,
  mut len: u64,
  buf: &mut [u8],
) -> io::Result<()> {
  #[cfg(target_os = "linux")]
  {
    unsafe extern "C" {
      fn copy_file_range(
        fd_in: i32,
        off_in: *mut i64,
        fd_out: i32,
        off_out: *mut i64,
        len: usize,
        flags: u32,
      ) -> isize;
    }
    while len > 0 {
      let (Ok(mut s_off), Ok(mut d_off)) = (i64::try_from(src_off), i64::try_from(dst_off)) else {
        break;
      };
      let step = len.min(1 << 30) as usize;
      let n = unsafe {
        copy_file_range(
          src.as_raw_fd(),
          &mut s_off,
          dst.as_raw_fd(),
          &mut d_off,
          step,
          0,
        )
      };
      if n > 0 {
        let copied = n as u64;
        src_off += copied;
        dst_off += copied;
        len -= copied;
      } else {
        break;
      }
    }
  }
  let cap = buf.len().max(1);
  while len > 0 {
    let n = (cap as u64).min(len) as usize;
    src.read_exact_at_pos(&mut buf[..n], src_off)?;
    dst.write_all_at_pos(&buf[..n], dst_off)?;
    src_off += n as u64;
    dst_off += n as u64;
    len -= n as u64;
  }
  Ok(())
}

/// 批量按偏移区间拷贝本地基准文件块到目标暂存文件
pub fn copy_file_spans(
  src: &File,
  dst: &File,
  spans: &[CopySpan],
  buf: &mut Vec<u8>,
) -> io::Result<()> {
  if spans.is_empty() {
    return Ok(());
  }
  buf.resize(buf.len().max(COPY_SPAN_BUF_CAP), 0);
  for sp in spans {
    copy_file_span(src, sp.src_off, dst, sp.dst_off, sp.len, buf)?;
  }
  Ok(())
}
