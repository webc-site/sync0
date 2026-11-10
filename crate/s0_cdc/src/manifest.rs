pub use s0_const::norm_mode;
use s0_const::{DEFAULT_FILE_MODE, FLAG_DELETE, FLAG_PARTIAL, HASH_LEN, MANIFEST_MAGIC};

use crate::{ChunkSig, PathChange, Rd, WireError, Wr, bitmap::matches_sorted_sels, hash128};

/// 清单内单文件条目
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
  pub path: String,
  pub size: u64,
  pub mode: u16,
  pub mtime: u64,
  pub chunks: Vec<ChunkSig>,
  pub deleted: bool,
}

impl ManifestEntry {
  #[inline]
  pub fn to_change(&self) -> PathChange {
    if self.deleted {
      PathChange::remove(self.path.clone())
    } else {
      PathChange::upsert_meta(
        self.path.clone(),
        self.size,
        self.mode,
        self.mtime,
        self.chunks.clone(),
      )
    }
  }
}

/// 全局块号 `[0, total_blocks)` 对应的文件与分块定位项（紧凑 32 字节无填充布局）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockLoc {
  pub offset: u64,
  pub entry_idx: u32,
  pub len: u32,
  pub hash: [u8; HASH_LEN],
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

  /// 将相对于所选文件（或清单逻辑字节流）的字节区间 `[byte_lo, byte_hi)` 在 $O(\log N)$ 时间内映射为全局块号半开区间 `[blk_lo, blk_hi)`
  pub fn locate_byte_range(&self, byte_lo: u64, byte_hi: u64) -> (u32, u32) {
    if byte_lo >= byte_hi || self.entries.is_empty() {
      return (0, 0);
    }
    let (mut logical_base, mut blk_base, mut found_lo, mut found_hi) = (0u64, 0u32, u32::MAX, 0u32);
    for e in &self.entries {
      if e.deleted || e.size == 0 || e.chunks.is_empty() {
        continue;
      }
      let entry_hi = logical_base.saturating_add(e.size);
      if entry_hi > byte_lo && logical_base < byte_hi {
        let (c_lo, c_hi) = chunk_range_in_slice(
          &e.chunks,
          byte_lo.saturating_sub(logical_base),
          byte_hi.saturating_sub(logical_base),
        );
        if c_lo < c_hi {
          found_lo = found_lo.min(blk_base + c_lo);
          found_hi = found_hi.max(blk_base + c_hi);
        }
      }
      logical_base = entry_hi;
      blk_base += e.chunks.len() as u32;
      if logical_base >= byte_hi {
        break;
      }
    }
    if found_lo < found_hi {
      (found_lo, found_hi)
    } else {
      (0, 0)
    }
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
      .filter(|e| matches_sorted_sels(&e.path, &clean_sels))
      .cloned()
      .collect();
    Self {
      entries,
      delete_extraneous: self.delete_extraneous,
      partial: true,
    }
  }

  /// 紧凑二进制序列化（路径采用前缀压缩 + `uvar` 变长整数，默认 `0o644` 权限零字节开销）
  pub fn encode(&self) -> Result<Vec<u8>, WireError> {
    let mut out = Vec::with_capacity(8 + self.entries.len() * 24);
    let mut w = Wr::new(&mut out);
    w.u32(MANIFEST_MAGIC);
    let mut flags = 0u8;
    if self.delete_extraneous {
      flags |= FLAG_DELETE;
    }
    if self.partial {
      flags |= FLAG_PARTIAL;
    }
    w.u8(flags);
    w.uvar(self.entries.len() as u64);

    let mut prev_path: &[u8] = &[];
    for (i, e) in self.entries.iter().enumerate() {
      w.front_str(i == 0, &mut prev_path, e.path.as_bytes())?;

      if e.deleted {
        w.uvar(0);
      } else {
        let mode = norm_mode(e.mode);
        let has_meta = u64::from(mode != DEFAULT_FILE_MODE || e.mtime != 0);
        w.uvar((((e.chunks.len() as u64) << 1) | has_meta) + 1);
        if has_meta != 0 {
          w.u15(usize::from(mode))?;
          w.uvar(e.mtime);
        }
        ChunkSig::encode_slice(&e.chunks, &mut w)?;
      }
    }
    Ok(out)
  }

  /// 从紧凑二进制字节流反序列化
  pub fn decode(buf: &[u8]) -> Result<Self, WireError> {
    let mut r = Rd::new(buf);
    if r.u32()? != MANIFEST_MAGIC {
      return Err(WireError::BadMagic);
    }
    let flags = r.u8()?;
    let cnt = r.uvar_usize()?;
    if cnt > r.rem() / 2 {
      return Err(WireError::Truncated);
    }
    let mut entries = Vec::with_capacity(cnt.min(65536));
    let mut prev_path = Vec::<u8>::new();

    for i in 0..cnt {
      let path = r.front_str(i == 0, &mut prev_path)?;

      let tag = r.uvar()?;
      if tag == 0 {
        entries.push(ManifestEntry {
          path,
          size: 0,
          mode: DEFAULT_FILE_MODE,
          mtime: 0,
          chunks: Vec::new(),
          deleted: true,
        });
      } else {
        let raw = tag - 1;
        let has_meta = (raw & 1) != 0;
        let nchunks = usize::try_from(raw >> 1).map_err(|_| WireError::Overflow)?;
        let (mode, mtime) = if has_meta {
          (norm_mode(r.u15()?), r.uvar()?)
        } else {
          (DEFAULT_FILE_MODE, 0)
        };
        let (chunks, size) = ChunkSig::decode_vec(&mut r, nchunks)?;
        entries.push(ManifestEntry {
          path,
          size,
          mode,
          mtime,
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

  /// 计算当前清单的 128-bit `gxhash128` 快照根摘要 `root_hash`
  pub fn root_hash(&self) -> Result<[u8; HASH_LEN], WireError> {
    let raw = self.encode()?;
    Ok(hash128(&raw))
  }
}

#[inline]
fn chunk_range_in_slice(chunks: &[ChunkSig], rel_lo: u64, rel_hi: u64) -> (u32, u32) {
  let lo = chunks.partition_point(|c| c.offset.saturating_add(u64::from(c.len)) <= rel_lo) as u32;
  let hi = chunks.partition_point(|c| c.offset < rel_hi) as u32;
  if lo < hi { (lo, hi) } else { (0, 0) }
}
