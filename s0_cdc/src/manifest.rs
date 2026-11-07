use core::str::from_utf8;
use std::io::Read;

use s0_const::{FLAG_DELETE, FLAG_PARTIAL, MANIFEST_MAGIC};
use s0_wire::{Rd, WireError, Wr};

use crate::{CdcConfig, CdcError, find_boundary};

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

/// 流式读取文件执行 FastCDC 分块（内存占用恒定 $O(\text{max\_size})$）
pub fn chunk_reader(
  reader: &mut impl Read,
  cfg: CdcConfig,
) -> Result<(Vec<ChunkSig>, u64), CdcError> {
  let cap = cfg.max_size * 2;
  let mut buf = vec![0u8; cap];
  let mut filled = 0usize;
  let mut file_off = 0u64;
  let mut out = Vec::new();

  loop {
    let n = reader.read(&mut buf[filled..])?;
    if n == 0 {
      break;
    }
    filled += n;
    let mut pos = 0usize;
    while filled - pos >= cfg.max_size {
      let cut = find_boundary(&buf[pos..filled], cfg);
      let slice = &buf[pos..pos + cut];
      out.push(ChunkSig {
        offset: file_off,
        len: cut as u32,
        hash: hash128(slice),
      });
      file_off += cut as u64;
      pos += cut;
    }
    if pos > 0 {
      buf.copy_within(pos..filled, 0);
      filled -= pos;
    }
  }

  let mut pos = 0usize;
  while pos < filled {
    let cut = find_boundary(&buf[pos..filled], cfg);
    let slice = &buf[pos..pos + cut];
    out.push(ChunkSig {
      offset: file_off,
      len: cut as u32,
      hash: hash128(slice),
    });
    file_off += cut as u64;
    pos += cut;
  }

  Ok((out, file_off))
}

/// 清单内单文件条目
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
  pub path: String,
  pub size: u64,
  pub chunks: Vec<ChunkSig>,
  pub deleted: bool,
}

/// 全局块号 `[0, total_blocks)` 对应的文件与分块定位项
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockLoc {
  pub entry_idx: u32,
  pub chunk_idx: u32,
  pub offset: u64,
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
      for (ci, c) in e.chunks.iter().enumerate() {
        map.push(BlockLoc {
          entry_idx: ei as u32,
          chunk_idx: ci as u32,
          offset: c.offset,
          len: c.len,
          hash: c.hash,
        });
      }
    }
    map
  }

  /// 按子路径前缀筛选清单（支持单文件、单目录或路径前缀 Selective Sync）
  pub fn filter_subpath(&self, subpath: &str) -> Self {
    let clean = subpath.trim_matches('/');
    if clean.is_empty() {
      return self.clone();
    }
    let entries = self
      .entries
      .iter()
      .filter(|e| {
        e.path == clean
          || (e.path.starts_with(clean) && e.path.as_bytes().get(clean.len()) == Some(&b'/'))
      })
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

/// 位图置位辅助函数
#[inline]
pub fn set_bit(bits: &mut [u8], idx: u32) {
  let byte_idx = (idx >> 3) as usize;
  if let Some(b) = bits.get_mut(byte_idx) {
    *b |= 1u8 << (idx & 7);
  }
}

/// 位图读取辅助函数
#[inline]
pub fn get_bit(bits: &[u8], idx: u32) -> bool {
  let byte_idx = (idx >> 3) as usize;
  bits
    .get(byte_idx)
    .is_some_and(|&b| (b & (1u8 << (idx & 7))) != 0)
}

/// 遍历位图在区间 `[blk_lo, blk_hi)` 内的所有连续缺失块区间 `(span_lo, span_hi)`
pub fn for_each_missing_span(
  have_bits: &[u8],
  blk_lo: u32,
  blk_hi: u32,
  mut on_span: impl FnMut(u32, u32),
) {
  let mut cur = blk_lo;
  while cur < blk_hi {
    if (cur & 7) == 0 && cur + 8 <= blk_hi && have_bits.get((cur >> 3) as usize) == Some(&0xFF) {
      cur += 8;
      continue;
    }
    if get_bit(have_bits, cur) {
      cur += 1;
      continue;
    }
    let start = cur;
    cur += 1;
    while cur < blk_hi {
      if (cur & 7) == 0
        && cur + 8 <= blk_hi
        && have_bits.get((cur >> 3) as usize).copied().unwrap_or(0) == 0
      {
        cur += 8;
        continue;
      }
      if get_bit(have_bits, cur) {
        break;
      }
      cur += 1;
    }
    on_span(start, cur);
  }
}
