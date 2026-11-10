use std::cmp::Reverse;

use roaring::RoaringBitmap;
use s0_const::RoaringTag;

use crate::WireError;

/// 位图置位辅助函数（`usize` 索引）
#[inline]
pub fn set_bit_usize(bits: &mut [u8], bit_idx: usize) {
  if let Some(b) = bits.get_mut(bit_idx >> 3) {
    *b |= 1u8 << (bit_idx & 7);
  }
}

/// 位图读取辅助函数（`usize` 索引）
#[inline]
pub(crate) fn get_bit_usize(bits: &[u8], bit_idx: usize) -> bool {
  bits
    .get(bit_idx >> 3)
    .is_some_and(|&b| (b & (1u8 << (bit_idx & 7))) != 0)
}

/// 位图读取辅助函数（`u32` 索引）
#[inline]
pub fn get_bit(bits: &[u8], idx: u32) -> bool {
  get_bit_usize(bits, idx as usize)
}

#[inline]
fn word_at_bit(bits: &[u8], bit_pos: u32) -> u64 {
  let byte_idx = (bit_pos >> 3) as usize;
  let shift = bit_pos & 7;
  if let Some(chunk) = bits.get(byte_idx..).and_then(|s| s.first_chunk::<8>()) {
    u64::from_le_bytes(*chunk) >> shift
  } else if let Some(tail) = bits.get(byte_idx..) {
    let mut tmp = [0u8; 8];
    let n = tail.len().min(8);
    tmp[..n].copy_from_slice(&tail[..n]);
    u64::from_le_bytes(tmp) >> shift
  } else {
    0
  }
}

/// 从 `cur` 向后扫描（`< blk_hi`），返回首个未就绪块索引（若 `[cur, blk_hi)` 全部就绪则返回 `blk_hi`）
#[inline]
pub(crate) fn next_missing_bit(have_bits: &[u8], mut cur: u32, blk_hi: u32) -> u32 {
  while cur < blk_hi {
    let rem = (64 - (cur & 7)).min(blk_hi - cur);
    let ones = word_at_bit(have_bits, cur).trailing_ones().min(rem);
    cur += ones;
    if ones < rem {
      break;
    }
  }
  cur
}

/// 遍历位图在区间 `[blk_lo, blk_hi)` 内的所有连续缺失块区间 `(span_lo, span_hi)`（$O(N/64)$ 64 位字长指令扫描）
pub fn for_each_missing_span(
  have_bits: &[u8],
  blk_lo: u32,
  blk_hi: u32,
  mut on_span: impl FnMut(u32, u32),
) {
  let mut cur = blk_lo;
  while cur < blk_hi {
    cur = next_missing_bit(have_bits, cur, blk_hi);
    if cur >= blk_hi {
      break;
    }
    let start = cur;
    cur += 1;
    while cur < blk_hi {
      let rem = (64 - (cur & 7)).min(blk_hi - cur);
      let zeros = word_at_bit(have_bits, cur).trailing_zeros().min(rem);
      cur += zeros;
      if zeros < rem {
        break;
      }
    }
    on_span(start, cur);
  }
}

/// 遍历位图在区间 `[blk_lo, blk_hi)` 内的所有连续已就绪块区间 `(span_lo, span_hi)`（$O(N/64)$ 64 位字长指令扫描）
pub fn for_each_have_span(
  have_bits: &[u8],
  blk_lo: u32,
  blk_hi: u32,
  mut on_span: impl FnMut(u32, u32),
) {
  let mut cur = blk_lo;
  while cur < blk_hi {
    while cur < blk_hi {
      let rem = (64 - (cur & 7)).min(blk_hi - cur);
      let zeros = word_at_bit(have_bits, cur).trailing_zeros().min(rem);
      cur += zeros;
      if zeros < rem {
        break;
      }
    }
    if cur >= blk_hi {
      break;
    }
    let start = cur;
    cur = next_missing_bit(have_bits, cur, blk_hi);
    on_span(start, cur);
  }
}

/// 将定长位图 `have_bits` 转换为经 Run-Container 优化的 `RoaringBitmap`
pub fn bitset_to_roaring(have_bits: &[u8], total_blocks: u32) -> RoaringBitmap {
  let mut rb = RoaringBitmap::new();
  for_each_have_span(have_bits, 0, total_blocks, |lo, hi| {
    rb.insert_range(lo..hi);
  });
  rb.optimize();
  rb
}

/// 将 `RoaringBitmap` 还原为定长位图 `Vec<u8>`
pub fn roaring_to_bitset(rb: &RoaringBitmap, total_blocks: u32) -> Vec<u8> {
  let byte_len = (total_blocks as usize).div_ceil(8);
  let mut out = vec![0u8; byte_len];
  for idx in rb.iter() {
    if idx >= total_blocks {
      break;
    }
    set_bit_usize(&mut out, idx as usize);
  }
  out
}

/// 从 `RoaringBitmap` 中提取所有连续已就绪块区间 `[lo, hi)`（若超过 `max_runs` 段则保留跨度最大的前 `max_runs` 段并按起始块号升序排列）
pub fn roaring_runs(rb: &RoaringBitmap, max_runs: usize) -> Vec<(u32, u32)> {
  if rb.is_empty() || max_runs == 0 {
    return Vec::new();
  }
  let mut runs: Vec<(u32, u32)> = Vec::new();
  for v in rb.iter() {
    if let Some(last) = runs.last_mut()
      && last.1 == v
    {
      last.1 = v + 1;
    } else {
      runs.push((v, v + 1));
    }
  }
  if runs.len() > max_runs {
    runs.sort_unstable_by_key(|&(lo, hi)| Reverse(hi - lo));
    runs.truncate(max_runs);
    runs.sort_unstable_by_key(|&(lo, _)| lo);
  }
  runs
}

/// 判断咆哮位图编码帧是否为全 1 满位图（`RoaringTag::Full`）
#[inline]
pub fn is_roaring_full_frame(buf: &[u8]) -> bool {
  buf == [RoaringTag::Full as u8]
}

/// 咆哮位图（`roaring::RoaringBitmap`）自适应编码：
/// 在 `Empty(1B)`、`Full(1B)`、`Roaring(Run/Array/Bitmap 容器优化流)` 与 `Raw(原始定长位图)` 中自动选取字节数最小的表述
pub fn encode_roaring_bits(have_bits: &[u8], total_blocks: u32) -> Vec<u8> {
  if total_blocks == 0 || is_all_have(have_bits, total_blocks) {
    return vec![RoaringTag::Full as u8];
  }
  let rb = bitset_to_roaring(have_bits, total_blocks);
  if rb.is_empty() {
    return vec![RoaringTag::Empty as u8];
  }
  let byte_len = (total_blocks as usize).div_ceil(8);
  let roaring_size = rb.serialized_size();
  if roaring_size < byte_len {
    let mut out = Vec::with_capacity(1 + roaring_size);
    out.push(RoaringTag::Roaring as u8);
    if rb.serialize_into(&mut out).is_ok() {
      return out;
    }
  }
  let mut raw = Vec::with_capacity(1 + byte_len);
  raw.push(RoaringTag::Raw as u8);
  let n = have_bits.len().min(byte_len);
  raw.extend_from_slice(&have_bits[..n]);
  raw.resize(1 + byte_len, 0);
  raw
}

/// 解码咆哮位图帧为定长位图 `Vec<u8>`
pub fn decode_roaring_bits(buf: &[u8], total_blocks: u32) -> Result<Vec<u8>, WireError> {
  let byte_len = (total_blocks as usize).div_ceil(8);
  let Some((&tag_byte, body)) = buf.split_first() else {
    return Ok(vec![0u8; byte_len]);
  };
  let tag = RoaringTag::from_repr(tag_byte).ok_or(WireError::BadMagic)?;
  match tag {
    RoaringTag::Empty => Ok(vec![0u8; byte_len]),
    RoaringTag::Full => Ok(vec![0xFFu8; byte_len]),
    RoaringTag::Roaring => {
      let rb = RoaringBitmap::deserialize_from(body).map_err(|_| WireError::Truncated)?;
      Ok(roaring_to_bitset(&rb, total_blocks))
    }
    RoaringTag::Raw => {
      let mut out = vec![0u8; byte_len];
      let n = body.len().min(byte_len);
      out[..n].copy_from_slice(&body[..n]);
      Ok(out)
    }
  }
}

/// 判断位图前 `total_blocks` 位是否全部为 1（$O(N/64)$ 64 位字长扫描）
#[inline]
pub fn is_all_have(have_bits: &[u8], total_blocks: u32) -> bool {
  next_missing_bit(have_bits, 0, total_blocks) >= total_blocks
}

#[inline]
pub(crate) fn matches_sel(entry_path: &str, sel: &str) -> bool {
  entry_path
    .strip_prefix(sel)
    .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// 在升序去重的选择集合 `sorted_sels` 中以 $O(D \log M)$ 二分查找判断 `entry_path` 是否精确命中或命中任意父目录前缀
#[inline]
pub(crate) fn matches_sorted_sels(entry_path: &str, sorted_sels: &[&str]) -> bool {
  if sorted_sels.binary_search(&entry_path).is_ok() {
    return true;
  }
  for (idx, b) in entry_path.bytes().enumerate() {
    if b == b'/' && sorted_sels.binary_search(&&entry_path[..idx]).is_ok() {
      return true;
    }
  }
  false
}

/// 判断单条相对路径 `entry_path` 是否命中任意已选文件或目录集合 `sels`
#[inline]
pub fn matches_any_selected<S: AsRef<str>>(entry_path: &str, sels: &[S]) -> bool {
  sels.iter().any(|sel| matches_sel(entry_path, sel.as_ref()))
}
