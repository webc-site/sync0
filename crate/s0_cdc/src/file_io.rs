use std::{fs::File, io};

use crate::{CdcConfig, CdcError, ChunkSig, find_boundary, hash128};

/// 128 位 `gxhash128` 内容指纹（16 字节，100 GB 数据下碰撞率 < 10^-24）
pub type Hash128 = [u8; 16];

/// 流式文件读取缓冲大小（1 MiB = 16 × 默认最大块，减少大文件 `pread` 系统调用与滑窗 `memmove` 频次）
pub(crate) const IO_BUF_LEN: usize = 1024 * 1024;

/// 跨平台无锁原位随机读写扩展 (`pread` / `pwrite`)
pub trait FileAt {
  fn read_exact_at_pos(&self, buf: &mut [u8], offset: u64) -> io::Result<()>;
  fn write_all_at_pos(&self, buf: &[u8], offset: u64) -> io::Result<()>;
}

#[cfg(unix)]
use std::os::unix::fs::FileExt as UnixFileExt;
#[cfg(windows)]
use std::os::windows::fs::FileExt as WinFileExt;

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
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "eof"));
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
        return Err(io::Error::new(io::ErrorKind::WriteZero, "write zero"));
      }
      buf = &buf[n..];
      offset += n as u64;
    }
    Ok(())
  }
}

/// 跨平台流式对文件执行 FastCDC 分块与 `gxhash128` 签名（基于 `FileAt`，固定缓冲零拷贝滑窗）
pub fn chunk_file(file: &File, size: u64, cfg: CdcConfig) -> Result<Vec<ChunkSig>, CdcError> {
  let mut buf = Vec::new();
  chunk_file_with_buf(file, size, cfg, &mut buf)
}

/// 复用调用方提供的读取缓冲 `buf` 对文件执行 FastCDC 分块与 `gxhash128` 签名（适合海量文件批处理）
pub fn chunk_file_with_buf(
  file: &File,
  size: u64,
  cfg: CdcConfig,
  buf: &mut Vec<u8>,
) -> Result<Vec<ChunkSig>, CdcError> {
  if size == 0 {
    return Ok(Vec::new());
  }
  if size <= cfg.min_size as u64 {
    let n = size as usize;
    if buf.len() < n {
      buf.resize(n, 0);
    }
    file.read_exact_at_pos(&mut buf[..n], 0)?;
    return Ok(vec![ChunkSig {
      offset: 0,
      len: size as u32,
      hash: hash128(&buf[..n]),
    }]);
  }
  let est = usize::try_from(size / (cfg.avg_size.max(1) as u64)).unwrap_or(0) + 2;
  let mut out = Vec::with_capacity(est);
  for_each_file_chunk_from_with_buf(file, 0, size, cfg, buf, |sig| out.push(sig))?;
  Ok(out)
}

/// 从指定起始偏移 `start_off` 流式对文件后缀 `[start_off, size)` 执行 FastCDC 分块与签名
pub fn for_each_file_chunk_from_with_buf(
  file: &File,
  start_off: u64,
  size: u64,
  cfg: CdcConfig,
  buf: &mut Vec<u8>,
  mut on_sig: impl FnMut(ChunkSig),
) -> Result<(), CdcError> {
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
    on_sig(ChunkSig {
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
      on_sig(ChunkSig {
        offset: chunk_off,
        len: cut as u32,
        hash: hash128(slice),
      });
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

/// 跨平台从 `src` 的 `src_off` 拷贝 `len` 字节到 `dst` 的 `dst_off`
pub fn copy_file_span(
  src: &File,
  mut src_off: u64,
  dst: &File,
  mut dst_off: u64,
  mut len: u64,
  buf: &mut [u8],
) -> Result<(), CdcError> {
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
