use core::str::from_utf8;

use crate::{CdcConfig, CdcError, find_boundary};

/// 内容寻址清单魔数 `"S0M2"`
pub const MANIFEST_MAGIC: [u8; 4] = *b"S0M2";
/// 请求/推送标志位：镜像清理多余文件
pub const FLAG_DELETE: u8 = 1 << 0;
/// 请求/推送标志位：增量路径子集更新
pub const FLAG_PARTIAL: u8 = 1 << 1;
/// `u15` 变长整数最大值 (`32767`)
pub const U15_MAX: u16 = 0x7FFF;

/// 紧凑 1 字节线协议错误类型
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WireError {
  #[error("帧数据截断")]
  Truncated = 1,
  #[error("魔数不匹配")]
  BadMagic = 2,
  #[error("协议版本不支持")]
  BadVersion = 3,
  #[error("未知操作码")]
  BadOp = 4,
  #[error("未知状态码")]
  BadStatus = 5,
  #[error("非法 UTF-8 编码")]
  InvalidUtf8 = 6,
  #[error("字段长度超出 u15 上限")]
  Overflow = 7,
  #[error("包头 CRC16 校验失败")]
  CrcMismatch = 8,
}

/// 零拷贝二进制读取游标
#[derive(Clone, Copy)]
pub struct Rd<'a> {
  pub buf: &'a [u8],
  pub pos: usize,
}

impl<'a> Rd<'a> {
  #[inline]
  pub const fn new(buf: &'a [u8]) -> Self {
    Self { buf, pos: 0 }
  }

  #[inline]
  pub const fn rem(&self) -> usize {
    self.buf.len().saturating_sub(self.pos)
  }

  #[inline]
  pub const fn is_empty(&self) -> bool {
    self.pos >= self.buf.len()
  }

  #[inline]
  pub fn rest(&self) -> &'a [u8] {
    &self.buf[self.pos..]
  }

  #[inline]
  pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], WireError> {
    if self.rem() < n {
      return Err(WireError::Truncated);
    }
    let s = unsafe { self.buf.get_unchecked(self.pos..self.pos + n) };
    self.pos += n;
    Ok(s)
  }

  #[inline]
  pub fn take(&mut self, n: usize) -> Result<&'a [u8], WireError> {
    self.bytes(n)
  }

  #[inline]
  pub fn take_end(&mut self, n: usize) -> Result<&'a [u8], WireError> {
    let s = self.bytes(n)?;
    self.expect_end()?;
    Ok(s)
  }

  #[inline]
  pub const fn expect_end(&self) -> Result<(), WireError> {
    if self.pos == self.buf.len() {
      Ok(())
    } else {
      Err(WireError::Overflow)
    }
  }

  #[inline]
  pub fn array<const N: usize>(&mut self) -> Result<[u8; N], WireError> {
    let s = self.bytes(N)?;
    let mut out = [0u8; N];
    out.copy_from_slice(s);
    Ok(out)
  }

  #[inline]
  pub fn u8(&mut self) -> Result<u8, WireError> {
    Ok(self.array::<1>()?[0])
  }

  #[inline]
  pub fn u16(&mut self) -> Result<u16, WireError> {
    Ok(u16::from_le_bytes(self.array::<2>()?))
  }

  #[inline]
  pub fn u32(&mut self) -> Result<u32, WireError> {
    Ok(u32::from_le_bytes(self.array::<4>()?))
  }

  #[inline]
  pub fn u64(&mut self) -> Result<u64, WireError> {
    Ok(u64::from_le_bytes(self.array::<8>()?))
  }

  #[inline]
  pub fn u128(&mut self) -> Result<u128, WireError> {
    Ok(u128::from_le_bytes(self.array::<16>()?))
  }

  /// 读取 `u15` 变长整数（`<128` 占 1B，`128..=32767` 占 2B）
  #[inline]
  pub fn u15(&mut self) -> Result<u16, WireError> {
    let b0 = self.u8()?;
    if b0 & 0x80 == 0 {
      Ok(u16::from(b0))
    } else {
      let b1 = self.u8()?;
      Ok((u16::from(b0 & 0x7F) << 8) | u16::from(b1))
    }
  }

  #[inline]
  pub fn str_u15(&mut self) -> Result<&'a str, WireError> {
    let n = self.u15()? as usize;
    let raw = self.bytes(n)?;
    from_utf8(raw).map_err(|_| WireError::InvalidUtf8)
  }

  #[inline]
  pub fn vec_u15(&mut self) -> Result<&'a [u8], WireError> {
    let n = self.u15()? as usize;
    self.bytes(n)
  }
}

/// 紧凑二进制写入辅助器
pub struct Wr<'a> {
  pub buf: &'a mut Vec<u8>,
}

impl<'a> Wr<'a> {
  #[inline]
  pub fn new(buf: &'a mut Vec<u8>) -> Self {
    Self { buf }
  }

  #[inline]
  pub fn u8(&mut self, v: u8) {
    self.buf.push(v);
  }

  #[inline]
  pub fn u16(&mut self, v: u16) {
    self.buf.extend_from_slice(&v.to_le_bytes());
  }

  #[inline]
  pub fn u32(&mut self, v: u32) {
    self.buf.extend_from_slice(&v.to_le_bytes());
  }

  #[inline]
  pub fn u64(&mut self, v: u64) {
    self.buf.extend_from_slice(&v.to_le_bytes());
  }

  #[inline]
  pub fn bytes(&mut self, s: &[u8]) {
    self.buf.extend_from_slice(s);
  }

  /// 写入 `u15` 变长整数
  #[inline]
  pub fn u15(&mut self, v: usize) -> Result<(), WireError> {
    if v > U15_MAX as usize {
      return Err(WireError::Overflow);
    }
    if v < 0x80 {
      self.buf.push(v as u8);
    } else {
      self.buf.push(0x80 | ((v >> 8) as u8));
      self.buf.push((v & 0xFF) as u8);
    }
    Ok(())
  }

  #[inline]
  pub fn slice_u15(&mut self, s: &[u8]) -> Result<(), WireError> {
    self.u15(s.len())?;
    self.bytes(s);
    Ok(())
  }
}

/// 面向栈上固定切片 `&mut [u8]` 的零分配写入游标（用于热路径 UDP 发包）
pub struct SliceWr<'a> {
  buf: &'a mut [u8],
  pos: usize,
}

impl<'a> SliceWr<'a> {
  #[inline]
  pub const fn new(buf: &'a mut [u8]) -> Self {
    Self { buf, pos: 0 }
  }

  #[inline]
  pub const fn pos(&self) -> usize {
    self.pos
  }

  #[inline]
  pub fn bytes(&mut self, s: &[u8]) -> Result<(), WireError> {
    let dst = self
      .buf
      .get_mut(self.pos..self.pos + s.len())
      .ok_or(WireError::Overflow)?;
    dst.copy_from_slice(s);
    self.pos += s.len();
    Ok(())
  }

  #[inline]
  pub fn u8(&mut self, v: u8) -> Result<(), WireError> {
    self.bytes(&[v])
  }

  #[inline]
  pub fn u16(&mut self, v: u16) -> Result<(), WireError> {
    self.bytes(&v.to_le_bytes())
  }

  #[inline]
  pub fn u32(&mut self, v: u32) -> Result<(), WireError> {
    self.bytes(&v.to_le_bytes())
  }

  #[inline]
  pub fn u128(&mut self, v: u128) -> Result<(), WireError> {
    self.bytes(&v.to_le_bytes())
  }
}

/// 计算数据切片的 128-bit BLAKE3 摘要（利用 `blake3` SIMD 加速）
#[inline]
pub fn hash128(data: &[u8]) -> [u8; 16] {
  let full = blake3::hash(data);
  let mut out = [0u8; 16];
  out.copy_from_slice(&full.as_bytes()[..16]);
  out
}

/// 单块内容寻址签名
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkSig {
  pub offset: u64,
  pub len: u32,
  pub hash: [u8; 16],
}

/// 对内存切片执行 FastCDC 分块并计算每块 128-bit BLAKE3 摘要
pub fn chunk_slice(data: &[u8], cfg: CdcConfig) -> Vec<ChunkSig> {
  if data.is_empty() {
    return Vec::new();
  }
  let mut out = Vec::with_capacity(data.len() / cfg.avg_size.max(1) + 2);
  let mut off = 0usize;
  while off < data.len() {
    let cut = find_boundary(&data[off..], cfg);
    let slice = &data[off..off + cut];
    out.push(ChunkSig {
      offset: off as u64,
      len: cut as u32,
      hash: hash128(slice),
    });
    off += cut;
  }
  out
}

/// 清单内单文件条目
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
  pub path: String,
  pub size: u64,
  pub chunks: Vec<ChunkSig>,
  pub deleted: bool,
}

/// 全局块号 `[0, total_blocks)` 对应的文件与分块定位项（紧凑 32 字节无填充布局）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockLoc {
  pub offset: u64,
  pub entry_idx: u32,
  pub len: u32,
  pub hash: [u8; 16],
}

/// 内容寻址同步清单 (`Manifest`)
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Manifest {
  pub entries: Vec<ManifestEntry>,
  pub delete_extraneous: bool,
  pub partial: bool,
}

impl Manifest {
  /// 全局总块数
  pub fn total_blocks(&self) -> u32 {
    self.entries.iter().map(|e| e.chunks.len() as u32).sum()
  }

  /// 构建全局块号 `0..total_blocks` 到文件偏移的 $O(1)$ 索引表
  pub fn build_block_map(&self) -> Vec<BlockLoc> {
    let total = self.total_blocks() as usize;
    let mut map = Vec::with_capacity(total);
    for (ei, e) in self.entries.iter().enumerate() {
      let entry_idx = ei as u32;
      for c in &e.chunks {
        map.push(BlockLoc {
          offset: c.offset,
          entry_idx,
          len: c.len,
          hash: c.hash,
        });
      }
    }
    map
  }

  /// 按单个子路径前缀筛选清单（支持单文件、单目录或路径前缀 Selective Sync）
  pub fn filter_subpath(&self, subpath: &str) -> Self {
    let clean = subpath.trim_matches(['/', '\\']);
    if clean.is_empty() {
      return self.clone();
    }
    self.filter_paths(&[clean])
  }

  /// 按多个文件或目录路径多选筛选清单（对标 Resilio Sync `SelectiveStrategyBase` 多选同步）
  ///
  /// - 当 `paths` 为空或包含空路径 `""` 时返回全量清单克隆；
  /// - 当 `paths` 包含 1 个或多个相对路径时，单次遍历同时匹配对应的单文件（`entry.path == sel`）与子目录（`entry.path` 以 `sel/` 开头）。
  pub fn filter_paths<S: AsRef<str>>(&self, paths: &[S]) -> Self {
    if paths.is_empty() {
      return self.clone();
    }
    let mut clean_sels: Vec<&str> = Vec::with_capacity(paths.len());
    for p in paths {
      let c = p.as_ref().trim_matches(['/', '\\']);
      if c.is_empty() {
        return self.clone();
      }
      clean_sels.push(c);
    }
    clean_sels.sort_unstable();
    clean_sels.dedup();

    let entries = self
      .entries
      .iter()
      .filter(|e| matches_any_selected(&e.path, &clean_sels))
      .cloned()
      .collect();
    Self {
      entries,
      delete_extraneous: self.delete_extraneous,
      partial: true,
    }
  }

  /// 紧凑二进制序列化（路径采用前缀压缩 + `u15` 变长整数）
  pub fn encode(&self) -> Result<Vec<u8>, CdcError> {
    let mut out = Vec::with_capacity(16 + self.entries.len() * 32);
    let mut w = Wr::new(&mut out);
    w.bytes(&MANIFEST_MAGIC);
    let mut flags = 0u8;
    if self.delete_extraneous {
      flags |= FLAG_DELETE;
    }
    if self.partial {
      flags |= FLAG_PARTIAL;
    }
    w.u8(flags);
    w.u32(self.entries.len() as u32);

    let mut prev_path: &[u8] = &[];
    for e in &self.entries {
      let pb = e.path.as_bytes();
      let shared = prev_path
        .iter()
        .zip(pb.iter())
        .take_while(|(a, b)| a == b)
        .count()
        .min(0x7FFF);
      w.u15(shared)?;
      w.slice_u15(&pb[shared..])?;
      prev_path = pb;

      if e.deleted {
        w.u8(1);
      } else {
        w.u8(0);
        w.u64(e.size);
        w.u32(e.chunks.len() as u32);
        for c in &e.chunks {
          w.u32(c.len);
          w.bytes(&c.hash);
        }
      }
    }
    Ok(out)
  }

  /// 从紧凑二进制字节流反序列化
  pub fn decode(buf: &[u8]) -> Result<Self, CdcError> {
    let mut r = Rd::new(buf);
    if r.array::<4>()? != MANIFEST_MAGIC {
      return Err(CdcError::Wire(WireError::BadMagic));
    }
    let flags = r.u8()?;
    let cnt = r.u32()? as usize;
    let mut entries = Vec::with_capacity(cnt.min(65536));
    let mut prev_path = Vec::<u8>::new();

    for _ in 0..cnt {
      let shared = r.u15()? as usize;
      let suffix = r.vec_u15()?;
      if shared > prev_path.len() {
        return Err(CdcError::Wire(WireError::Truncated));
      }
      prev_path.truncate(shared);
      prev_path.extend_from_slice(suffix);
      let path = from_utf8(&prev_path)
        .map_err(|_| CdcError::Wire(WireError::InvalidUtf8))?
        .to_owned();

      let del_flag = r.u8()?;
      if del_flag != 0 {
        entries.push(ManifestEntry {
          path,
          size: 0,
          chunks: Vec::new(),
          deleted: true,
        });
      } else {
        let size = r.u64()?;
        let nchunks = r.u32()? as usize;
        let mut chunks = Vec::with_capacity(nchunks.min(65536));
        let mut off = 0u64;
        for _ in 0..nchunks {
          let len = r.u32()?;
          let hash = r.array::<16>()?;
          chunks.push(ChunkSig {
            offset: off,
            len,
            hash,
          });
          off += u64::from(len);
        }
        entries.push(ManifestEntry {
          path,
          size,
          chunks,
          deleted: false,
        });
      }
    }

    Ok(Self {
      entries,
      delete_extraneous: flags & FLAG_DELETE != 0,
      partial: flags & FLAG_PARTIAL != 0,
    })
  }

  /// 计算当前清单的 128-bit BLAKE3 快照根摘要 `root_hash`
  pub fn root_hash(&self) -> Result<[u8; 16], CdcError> {
    let raw = self.encode()?;
    Ok(hash128(&raw))
  }
}

/// 位图置位辅助函数（`usize` 索引）
#[inline]
pub fn set_bit_usize(bits: &mut [u8], bit_idx: usize) {
  if let Some(b) = bits.get_mut(bit_idx >> 3) {
    *b |= 1u8 << (bit_idx & 7);
  }
}

/// 位图读取辅助函数（`usize` 索引）
#[inline]
pub fn get_bit_usize(bits: &[u8], bit_idx: usize) -> bool {
  bits
    .get(bit_idx >> 3)
    .is_some_and(|&b| (b & (1u8 << (bit_idx & 7))) != 0)
}

/// 位图置位辅助函数（`u32` 索引）
#[inline]
pub fn set_bit(bits: &mut [u8], idx: u32) {
  set_bit_usize(bits, idx as usize);
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

/// 遍历位图在区间 `[blk_lo, blk_hi)` 内的所有连续缺失块区间 `(span_lo, span_hi)`（$O(N/64)$ 64 位字长指令扫描）
pub fn for_each_missing_span(
  have_bits: &[u8],
  blk_lo: u32,
  blk_hi: u32,
  mut on_span: impl FnMut(u32, u32),
) {
  let mut cur = blk_lo;
  while cur < blk_hi {
    let rem = (64 - (cur & 7)).min(blk_hi - cur);
    let ones = word_at_bit(have_bits, cur).trailing_ones().min(rem);
    cur += ones;
    if ones == rem {
      continue;
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

/// 判断位图前 `total_blocks` 位是否全部为 1（$O(N/64)$ 64 位字长扫描）
#[inline]
pub fn is_all_have(have_bits: &[u8], total_blocks: u32) -> bool {
  let mut cur = 0u32;
  while cur < total_blocks {
    let rem = (total_blocks - cur).min(64);
    let ones = word_at_bit(have_bits, cur).trailing_ones().min(rem);
    if ones < rem {
      return false;
    }
    cur += rem;
  }
  true
}

/// 判断单条相对路径 `entry_path` 是否命中任意已选文件或目录集合 `sels`
#[inline]
pub fn matches_any_selected<S: AsRef<str>>(entry_path: &str, sels: &[S]) -> bool {
  sels.iter().any(|sel| {
    let sel = sel.as_ref();
    entry_path == sel
      || (entry_path.starts_with(sel) && entry_path.as_bytes().get(sel.len()) == Some(&b'/'))
  })
}
