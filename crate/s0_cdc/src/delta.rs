use std::{fs::File, io, slice::from_ref, thread};

use gxhash::{HashMap, HashMapExt};

use crate::{
  CdcConfig, ChunkSig, FileAt, Hash128, chunk_file_range_with_buf,
  file_io::{IO_BUF_LEN, PAR_FILE_MIN_BYTES},
  for_each_file_chunk_from_with_buf, get_bit_usize, hash128, join_io, max_par_threads,
  set_bit_usize,
};

/// 启用双阶段多线程原位偏移校验的最小目标块数
const PAR_VERIFY_MIN_CHUNKS: usize = 256;

/// 基准池（Basis）中某个已存在 CDC 块的位置
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BasisLoc {
  file_idx: u32,
  off: u64,
  len: u32,
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

  #[inline]
  pub fn is_empty(&self) -> bool {
    self.map.is_empty()
  }

  #[inline]
  fn clear(&mut self) {
    self.map.clear();
  }

  #[inline]
  fn insert_sig(&mut self, file_idx: u32, sig: &ChunkSig) {
    self.map.entry(sig.hash).or_insert(BasisLoc {
      file_idx,
      off: sig.offset,
      len: sig.len,
    });
  }

  pub fn insert_file_with_buf(
    &mut self,
    file_idx: u32,
    file: &File,
    size: u64,
    cfg: CdcConfig,
    buf: &mut Vec<u8>,
  ) -> io::Result<()> {
    self.insert_file_from_with_buf(file_idx, file, 0, size, cfg, buf)
  }

  fn insert_file_from_with_buf(
    &mut self,
    file_idx: u32,
    file: &File,
    start_off: u64,
    size: u64,
    cfg: CdcConfig,
    buf: &mut Vec<u8>,
  ) -> io::Result<()> {
    if size.saturating_sub(start_off) >= PAR_FILE_MIN_BYTES {
      for sig in chunk_file_range_with_buf(file, start_off, size, cfg, buf)? {
        self.insert_sig(file_idx, &sig);
      }
      return Ok(());
    }
    for_each_file_chunk_from_with_buf(file, start_off, size, cfg, buf, |sig| {
      self.insert_sig(file_idx, &sig);
    })
  }

  #[inline]
  fn get(&self, hash: &Hash128, len: u32) -> Option<BasisLoc> {
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

/// 虚拟增量流区间与目标文件/条目下标的映射
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileSpan {
  pub idx: usize,
  pub file_off: u64,
  pub delta_off: u64,
  pub len: u64,
}

impl FileSpan {
  #[inline]
  pub const fn from_data(idx: usize, s: DataSpan) -> Self {
    Self {
      idx,
      file_off: s.file_off,
      delta_off: s.delta_off,
      len: s.len,
    }
  }
}

/// 利用单调游标提示 `hint` 在区间表中定位包含或紧接 `off` 的起始下标（顺序访问 $O(1)$，乱序访问回退二分 $O(\log N)$）
#[inline]
pub fn locate_span(spans: &[FileSpan], hint: &mut usize, off: u64) -> usize {
  let start = if let Some(sp) = spans.get(*hint)
    && sp.delta_off <= off
    && (*hint == 0 || spans[*hint - 1].delta_off + spans[*hint - 1].len <= off)
  {
    let mut idx = *hint;
    while idx < spans.len() && spans[idx].delta_off + spans[idx].len <= off {
      idx += 1;
    }
    idx
  } else {
    spans.partition_point(|sp| sp.delta_off + sp.len <= off)
  };
  *hint = start;
  start
}

/// 单文件的 CDC 增量还原计划
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileDelta {
  pub copy_spans: Vec<CopySpan>,
  pub data_spans: Vec<DataSpan>,
  pub delta_bytes: u64,
  pub reused_bytes: u64,
}

impl FileDelta {
  /// 是否与大小为 `size` 的基准文件原位 100% 相同（零网络增量、单段从 0 完整覆盖）
  #[inline]
  pub const fn is_identical_to(&self, size: u64) -> bool {
    self.delta_bytes == 0
      && matches!(
        self.copy_spans.as_slice(),
        [s] if s.src_off == 0 && s.dst_off == 0 && s.len == size
      )
  }

  /// 所有可复用块是否均位于本地基准文件 0 的相同字节偏移处（支持零拷贝原位补丁）
  #[inline]
  pub fn is_in_place(&self) -> bool {
    !self.copy_spans.is_empty()
      && self
        .copy_spans
        .iter()
        .all(|s| s.src_file == 0 && s.dst_off == s.src_off)
  }
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
    if let Some(loc) = basis.get(&c.hash, c.len) {
      record_copy_hit(c, loc.file_idx, loc.off, have_bits, bit_cursor, plan);
    } else {
      *bit_cursor += 1;
      let len = u64::from(c.len);
      plan.delta_bytes += len;
      push_data_span(&mut plan.data_spans, c.offset, delta_cursor, len);
    }
  }
}

#[inline]
fn record_copy_hit(
  c: &ChunkSig,
  src_file: u32,
  src_off: u64,
  have_bits: &mut [u8],
  bit_cursor: &mut usize,
  plan: &mut FileDelta,
) {
  set_bit_usize(have_bits, *bit_cursor);
  *bit_cursor += 1;
  let len = u64::from(c.len);
  plan.reused_bytes += len;
  push_copy_span(&mut plan.copy_spans, c.offset, src_file, src_off, len);
}

#[inline]
fn record_in_place_hit(
  c: &ChunkSig,
  have_bits: &mut [u8],
  bit_cursor: &mut usize,
  plan: &mut FileDelta,
) {
  record_copy_hit(c, 0, c.offset, have_bits, bit_cursor, plan);
}

#[inline]
fn verify_chunk_win(
  c: &ChunkSig,
  (basis_file, basis_size): (&File, u64),
  buf: &mut [u8],
  (win_off, win_len): &mut (u64, usize),
) -> io::Result<bool> {
  let clen = c.len as usize;
  let end_off = c.offset + u64::from(c.len);
  if end_off > basis_size || clen > buf.len() {
    return Ok(false);
  }
  if *win_len == 0 || c.offset < *win_off || end_off > *win_off + (*win_len as u64) {
    *win_off = c.offset;
    *win_len = ((buf.len() as u64).min(basis_size - *win_off)) as usize;
    basis_file.read_exact_at_pos(&mut buf[..*win_len], *win_off)?;
  }
  let rel = (c.offset - *win_off) as usize;
  Ok(hash128(&buf[rel..rel + clen]) == c.hash)
}

/// 接收端同名基准文件高速匹配：
/// - 大文件（`>= 8 MiB` 且块数 `>= 256`）启用双阶段并发引擎：阶段 1 多线程 `pread` + SIMD `gxhash128` 并发校验原位切点，阶段 2 仅对最小失配区间执行多线程收敛并行 FastCDC；
/// - 中小文件保持单次流式原位比对与后缀 FastCDC 回退。
pub fn match_basis_file(
  chunks: &[ChunkSig],
  (basis_file, basis_size): (&File, u64),
  cfg: CdcConfig,
  (basis, buf): (&mut BasisIndex, &mut Vec<u8>),
  have_bits: &mut [u8],
  bit_cursor: &mut usize,
  delta_cursor: &mut u64,
) -> io::Result<FileDelta> {
  let mut plan = FileDelta::default();
  if chunks.is_empty() {
    return Ok(plan);
  }
  if basis_size >= PAR_FILE_MIN_BYTES && chunks.len() >= PAR_VERIFY_MIN_CHUNKS {
    return match_basis_file_par(
      chunks,
      (basis_file, basis_size),
      cfg,
      (basis, buf),
      have_bits,
      bit_cursor,
      delta_cursor,
    );
  }
  let win_cap = IO_BUF_LEN.max(cfg.max_size * 2);
  let buf_cap = usize::try_from(basis_size).map_or(win_cap, |s| s.min(win_cap));
  if buf.len() < buf_cap {
    buf.resize(buf_cap, 0);
  }
  let mut win = (0u64, 0usize);
  let mut mismatch_idx = chunks.len();

  for (idx, c) in chunks.iter().enumerate() {
    if !verify_chunk_win(c, (basis_file, basis_size), &mut buf[..buf_cap], &mut win)? {
      mismatch_idx = idx;
      break;
    }
    record_in_place_hit(c, have_bits, bit_cursor, &mut plan);
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
  win = (0, 0);
  for sig in &chunks[mismatch_idx..] {
    if basis.get(&sig.hash, sig.len).is_none()
      && verify_chunk_win(sig, (basis_file, basis_size), &mut buf[..buf_cap], &mut win)?
    {
      basis.insert_sig(0, sig);
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

fn match_basis_file_par(
  chunks: &[ChunkSig],
  (basis_file, basis_size): (&File, u64),
  cfg: CdcConfig,
  (basis, buf): (&mut BasisIndex, &mut Vec<u8>),
  have_bits: &mut [u8],
  bit_cursor: &mut usize,
  delta_cursor: &mut u64,
) -> io::Result<FileDelta> {
  let mut exact_hit = vec![false; chunks.len()];
  let n_threads = max_par_threads().min(chunks.len() / 64).max(1);
  let step = chunks.len().div_ceil(n_threads);
  let win_cap = IO_BUF_LEN.max(cfg.max_size * 2);

  thread::scope(|s| -> io::Result<()> {
    let mut handles = Vec::with_capacity(n_threads);
    for (c_slice, hit_slice) in chunks.chunks(step).zip(exact_hit.chunks_mut(step)) {
      handles.push(s.spawn(move || -> io::Result<()> {
        let mut local_buf = vec![0u8; win_cap];
        let mut win = (0u64, 0usize);
        for (c, hit) in c_slice.iter().zip(hit_slice.iter_mut()) {
          *hit = verify_chunk_win(c, (basis_file, basis_size), &mut local_buf, &mut win)?;
        }
        Ok(())
      }));
    }
    for h in handles {
      join_io(h.join())?;
    }
    Ok(())
  })?;

  let mut plan = FileDelta::default();
  let Some(first_miss) = exact_hit.iter().position(|&h| !h) else {
    for c in chunks {
      record_in_place_hit(c, have_bits, bit_cursor, &mut plan);
    }
    return Ok(plan);
  };

  let last_miss = exact_hit.iter().rposition(|&h| !h).unwrap_or(first_miss);
  let target_size = chunks.last().map_or(0, |c| c.offset + u64::from(c.len));
  let scan_lo = chunks[first_miss].offset.min(basis_size);
  let scan_hi = if target_size != basis_size || last_miss + 1 == chunks.len() {
    basis_size
  } else {
    (chunks[last_miss].offset + u64::from(chunks[last_miss].len) + (cfg.max_size as u64))
      .min(basis_size)
  };

  basis.clear();
  for (c, &hit) in chunks.iter().zip(&exact_hit) {
    if hit {
      basis.insert_sig(0, c);
    }
  }
  for sig in chunk_file_range_with_buf(basis_file, scan_lo, scan_hi, cfg, buf)? {
    basis.insert_sig(0, &sig);
  }

  for (c, &hit) in chunks.iter().zip(&exact_hit) {
    if hit {
      record_in_place_hit(c, have_bits, bit_cursor, &mut plan);
    } else {
      match_chunks_into(
        from_ref(c),
        basis,
        have_bits,
        bit_cursor,
        delta_cursor,
        &mut plan,
      );
    }
  }
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
