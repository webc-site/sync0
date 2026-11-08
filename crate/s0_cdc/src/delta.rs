use std::fs::File;

use gxhash::{HashMap, HashMapExt};

use crate::{
  CdcConfig, CdcError, ChunkSig, Chunks, FileAt, Hash128, file_io::IO_BUF_LEN,
  for_each_file_chunk_from_with_buf, get_bit_usize, hash128, set_bit_usize,
};

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
/// 1) 先按发送端已知切点 `(c.offset, c.len)` 顺序读取 `basis_file` 并做 SIMD `gxhash128` 原位比对；
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
  for sig in &chunks[mismatch_idx..] {
    if basis.get(&sig.hash, sig.len).is_none() {
      let clen = sig.len as usize;
      let end_off = sig.offset + u64::from(sig.len);
      if end_off <= basis_size && clen <= buf.len() {
        basis_file.read_exact_at_pos(&mut buf[..clen], sig.offset)?;
        if hash128(&buf[..clen]) == sig.hash {
          basis.insert_sig(0, sig);
        }
      }
    }
  }
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
