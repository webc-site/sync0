use std::{fs::File, io};

use gxhash::{HashMap, HashMapExt};

use crate::{
  CdcConfig, CdcError, ChunkSig, Chunks, find_boundary, get_bit_usize, hash128, set_bit_usize,
};

/// 128 位 BLAKE3 内容指纹（16 字节，100 GB 数据下碰撞率 < 10^-24）
pub type Hash128 = [u8; 16];

/// 流式文件读取缓冲大小（1 MiB = 16 × 默认最大块，减少大文件 `pread` 系统调用与滑窗 `memmove` 频次）
const IO_BUF_LEN: usize = 1024 * 1024;

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

/// 跨平台流式对文件执行 FastCDC 分块与 BLAKE3 签名（基于 `FileAt`，固定缓冲零拷贝滑窗）
pub fn chunk_file(file: &File, size: u64, cfg: CdcConfig) -> Result<Vec<ChunkSig>, CdcError> {
  let mut buf = Vec::new();
  chunk_file_with_buf(file, size, cfg, &mut buf)
}

/// 复用调用方提供的读取缓冲 `buf` 对文件执行 FastCDC 分块与 BLAKE3 签名（适合海量文件批处理）
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
  for_each_file_chunk_with_buf(file, size, cfg, buf, |sig| out.push(sig))?;
  Ok(out)
}

/// 复用调用方缓冲 `buf` 遍历文件的每个 CDC 块签名（消除多文件循环中的重复堆分配）
pub fn for_each_file_chunk_with_buf(
  file: &File,
  size: u64,
  cfg: CdcConfig,
  buf: &mut Vec<u8>,
  on_sig: impl FnMut(ChunkSig),
) -> Result<(), CdcError> {
  for_each_file_chunk_from_with_buf(file, 0, size, cfg, buf, on_sig)
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

/// 基准池（Basis）中某个已存在 CDC 块的位置
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BasisLoc {
  pub file_idx: u32,
  pub off: u64,
  pub len: u32,
}

/// 接收端已存在文件的 CDC 块索引表（O(1) `gxhash` 查找）
#[derive(Default, Debug)]
pub struct BasisIndex {
  map: HashMap<Hash128, BasisLoc>,
}

impl BasisIndex {
  pub fn new() -> Self {
    Self {
      map: HashMap::new(),
    }
  }

  pub fn with_capacity(cap: usize) -> Self {
    Self {
      map: HashMap::with_capacity(cap),
    }
  }

  #[inline]
  pub fn is_empty(&self) -> bool {
    self.map.is_empty()
  }

  #[inline]
  pub fn len(&self) -> usize {
    self.map.len()
  }

  #[inline]
  pub fn clear(&mut self) {
    self.map.clear();
  }

  #[inline]
  pub fn insert_sig(&mut self, file_idx: u32, sig: &ChunkSig) {
    self.map.entry(sig.hash).or_insert(BasisLoc {
      file_idx,
      off: sig.offset,
      len: sig.len,
    });
  }

  pub fn insert_slice(&mut self, file_idx: u32, data: &[u8], cfg: CdcConfig) {
    for c in Chunks::new(data, cfg) {
      let len = c.data.len() as u32;
      self.map.entry(hash128(c.data)).or_insert(BasisLoc {
        file_idx,
        off: c.off,
        len,
      });
    }
  }

  pub fn insert_file(
    &mut self,
    file_idx: u32,
    file: &File,
    size: u64,
    cfg: CdcConfig,
  ) -> Result<(), CdcError> {
    let mut buf = Vec::new();
    self.insert_file_with_buf(file_idx, file, size, cfg, &mut buf)
  }

  pub fn insert_file_with_buf(
    &mut self,
    file_idx: u32,
    file: &File,
    size: u64,
    cfg: CdcConfig,
    buf: &mut Vec<u8>,
  ) -> Result<(), CdcError> {
    self.insert_file_from_with_buf(file_idx, file, 0, size, cfg, buf)
  }

  pub fn insert_file_from_with_buf(
    &mut self,
    file_idx: u32,
    file: &File,
    start_off: u64,
    size: u64,
    cfg: CdcConfig,
    buf: &mut Vec<u8>,
  ) -> Result<(), CdcError> {
    for_each_file_chunk_from_with_buf(file, start_off, size, cfg, buf, |sig| {
      self.insert_sig(file_idx, &sig);
    })
  }

  #[inline]
  pub fn get(&self, hash: &Hash128, len: u32) -> Option<BasisLoc> {
    self.map.get(hash).copied().filter(|loc| loc.len == len)
  }
}

/// 从本地基准文件复用的连续区间（相邻块自动合并，减少系统调用）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CopySpan {
  pub dst_off: u64,
  pub src_file: u32,
  pub src_off: u64,
  pub len: u64,
}

/// 需从网络增量虚拟流接收并写入目标文件的连续区间（相邻缺失块自动合并）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DataSpan {
  pub file_off: u64,
  pub delta_off: u64,
  pub len: u64,
}

/// 单文件的 CDC 增量还原计划
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileDelta {
  pub copy_spans: Vec<CopySpan>,
  pub data_spans: Vec<DataSpan>,
  pub delta_bytes: u64,
  pub reused_bytes: u64,
}

#[inline]
fn push_copy_span(spans: &mut Vec<CopySpan>, dst_off: u64, src_file: u32, src_off: u64, len: u64) {
  if let Some(last) = spans.last_mut()
    && last.src_file == src_file
    && last.src_off + last.len == src_off
    && last.dst_off + last.len == dst_off
  {
    last.len += len;
  } else {
    spans.push(CopySpan {
      dst_off,
      src_file,
      src_off,
      len,
    });
  }
}

#[inline]
fn push_data_span(spans: &mut Vec<DataSpan>, file_off: u64, delta_cursor: &mut u64, len: u64) {
  let d_off = *delta_cursor;
  *delta_cursor += len;
  if let Some(last) = spans.last_mut()
    && last.file_off + last.len == file_off
    && last.delta_off + last.len == d_off
  {
    last.len += len;
  } else {
    spans.push(DataSpan {
      file_off,
      delta_off: d_off,
      len,
    });
  }
}

/// 接收端：将目标文件块列表 `chunks` 与 `basis` 匹配，置位 `have_bits` 并生成合并后的 `CopySpan` 与 `DataSpan`
pub fn match_chunks(
  chunks: &[ChunkSig],
  basis: &BasisIndex,
  have_bits: &mut [u8],
  bit_cursor: &mut usize,
  delta_cursor: &mut u64,
) -> FileDelta {
  let mut plan = FileDelta::default();
  match_chunks_into(
    chunks,
    basis,
    have_bits,
    bit_cursor,
    delta_cursor,
    &mut plan,
  );
  plan
}

fn match_chunks_into(
  chunks: &[ChunkSig],
  basis: &BasisIndex,
  have_bits: &mut [u8],
  bit_cursor: &mut usize,
  delta_cursor: &mut u64,
  plan: &mut FileDelta,
) {
  for c in chunks {
    let bit_idx = *bit_cursor;
    *bit_cursor += 1;
    let len = u64::from(c.len);
    if let Some(loc) = basis.get(&c.hash, c.len) {
      set_bit_usize(have_bits, bit_idx);
      plan.reused_bytes += len;
      push_copy_span(&mut plan.copy_spans, c.offset, loc.file_idx, loc.off, len);
    } else {
      plan.delta_bytes += len;
      push_data_span(&mut plan.data_spans, c.offset, delta_cursor, len);
    }
  }
}

/// 接收端同名基准文件高速匹配：
/// 1) 先按发送端已知切点 `(c.offset, c.len)` 顺序读取 `basis_file` 并做 SIMD BLAKE3 原位比对；
/// 2) 一旦在 `chunks[idx]` 遇到首个失配块，将已验证的前缀签名直接记入 `basis`，仅对剩余后缀执行 FastCDC 滚动分块。
pub fn match_basis_file(
  chunks: &[ChunkSig],
  (basis_file, basis_size): (&File, u64),
  cfg: CdcConfig,
  (basis, buf): (&mut BasisIndex, &mut Vec<u8>),
  have_bits: &mut [u8],
  bit_cursor: &mut usize,
  delta_cursor: &mut u64,
) -> Result<FileDelta, CdcError> {
  let mut plan = FileDelta::default();
  if chunks.is_empty() {
    return Ok(plan);
  }
  let win_cap = IO_BUF_LEN.max(cfg.max_size * 2);
  let buf_cap = usize::try_from(basis_size).map_or(win_cap, |s| s.min(win_cap));
  if buf.len() < buf_cap {
    buf.resize(buf_cap, 0);
  }
  let mut win_off = 0u64;
  let mut win_len = 0usize;
  let mut mismatch_idx = chunks.len();

  for (idx, c) in chunks.iter().enumerate() {
    let clen = c.len as usize;
    let end_off = c.offset + u64::from(c.len);
    if end_off > basis_size || clen > buf_cap {
      mismatch_idx = idx;
      break;
    }
    if c.offset < win_off || end_off > win_off + (win_len as u64) {
      win_off = c.offset;
      win_len = ((buf_cap as u64).min(basis_size - win_off)) as usize;
      basis_file.read_exact_at_pos(&mut buf[..win_len], win_off)?;
    }
    let rel = (c.offset - win_off) as usize;
    if hash128(&buf[rel..rel + clen]) != c.hash {
      mismatch_idx = idx;
      break;
    }
    set_bit_usize(have_bits, *bit_cursor);
    *bit_cursor += 1;
    let len64 = u64::from(c.len);
    plan.reused_bytes += len64;
    push_copy_span(&mut plan.copy_spans, c.offset, 0, c.offset, len64);
  }

  if mismatch_idx == chunks.len() {
    return Ok(plan);
  }

  basis.clear();
  for sig in &chunks[..mismatch_idx] {
    basis.insert_sig(0, sig);
  }
  let scan_from = chunks[mismatch_idx].offset;
  basis.insert_file_from_with_buf(0, basis_file, scan_from, basis_size, cfg, buf)?;
  match_chunks_into(
    &chunks[mismatch_idx..],
    basis,
    have_bits,
    bit_cursor,
    delta_cursor,
    &mut plan,
  );
  Ok(plan)
}

/// 发送端：遍历接收端未命中的缺失块区间并调用 `on_span`（零中间 `Vec` 分配）
pub fn for_each_missing_data_span(
  chunks: &[ChunkSig],
  have_bits: &[u8],
  bit_cursor: &mut usize,
  delta_cursor: &mut u64,
  mut on_span: impl FnMut(DataSpan),
) {
  let mut pending: Option<DataSpan> = None;
  for c in chunks {
    let have = get_bit_usize(have_bits, *bit_cursor);
    *bit_cursor += 1;
    if !have {
      let len = u64::from(c.len);
      let d_off = *delta_cursor;
      *delta_cursor += len;
      if let Some(last) = &mut pending
        && last.file_off + last.len == c.offset
        && last.delta_off + last.len == d_off
      {
        last.len += len;
      } else if let Some(prev) = pending.replace(DataSpan {
        file_off: c.offset,
        delta_off: d_off,
        len,
      }) {
        on_span(prev);
      }
    }
  }
  if let Some(last) = pending {
    on_span(last);
  }
}

/// 发送端：根据接收端回传的 `have_bits` 构建缺失块的连续发送区间 `DataSpan`
pub fn plan_missing_spans(
  chunks: &[ChunkSig],
  have_bits: &[u8],
  bit_cursor: &mut usize,
  delta_cursor: &mut u64,
) -> Vec<DataSpan> {
  let mut spans = Vec::new();
  for_each_missing_data_span(chunks, have_bits, bit_cursor, delta_cursor, |sp| {
    spans.push(sp);
  });
  spans
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
