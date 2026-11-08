use std::{
  fs::{self, File, OpenOptions},
  path::{Path, PathBuf},
  sync::atomic::{AtomicU64, Ordering},
};

use s0_cdc::{
  BasisIndex, BlockLoc, CdcConfig, FileAt, Manifest, copy_file_span, hash128, match_basis_file,
};

use super::{
  FsError,
  fd_cache::FdCache,
  pack::{SmallPack, UNPACKED_OFFSET},
  prune_dir, remove, sanitize_rel_path,
};

/// 支持多个对等节点并发无锁写入与边下边传读取的接收池 (`SharedSink`)
///
/// 空间复杂度优化至理论极限：
/// - 小文件采用 `SmallPack`（`<= 8 MiB` 单块连续内存 / `> 8 MiB` 单文件流 `.s0_pack.tmp`），内存 $O(1)$；
/// - 大文件与基准文件采用 64 槽位 `FdCache`，打开句柄数严格 $O(1) \le 65$。
pub struct SharedSink {
  root: PathBuf,
  manifest: Manifest,
  block_map: Vec<BlockLoc>,
  needs_staging: Vec<bool>,
  pack_offsets: Vec<u64>,
  small_pack: SmallPack,
  staging_fds: FdCache,
  basis_fds: FdCache,
  have_words: Vec<AtomicU64>,
  initial_have_bits: Vec<u8>,
}

impl SharedSink {
  /// 初始化接收池：对本地已有同名文件执行 FastCDC 错位抗性基准匹配，100% 命中文件零拷贝跳过，小文件流式聚合
  pub fn prepare(root: &Path, manifest: Manifest) -> Result<Self, FsError> {
    let root_existed = root.is_dir();
    fs::create_dir_all(root)?;
    let block_map = manifest.build_block_map();
    let total_blocks = block_map.len() as u32;
    let bit_bytes = (total_blocks as usize).div_ceil(8);
    let word_cnt = (total_blocks as usize).div_ceil(64);
    let mut initial_have_bits = vec![0u8; bit_bytes];

    let n_entries = manifest.entries.len();
    let mut needs_staging = Vec::with_capacity(n_entries);
    let mut pack_offsets = vec![UNPACKED_OFFSET; n_entries];
    let mut pack_total = 0u64;
    let staging_fds = FdCache::new();
    let basis_fds = FdCache::new();

    let mut bit_cursor = 0usize;
    let mut delta_cursor = 0u64;
    let mut basis_idx = BasisIndex::new();
    let mut io_buf = Vec::new();
    let mut last_parent: Option<PathBuf> = None;

    for (ei, e) in manifest.entries.iter().enumerate() {
      let clean_rel = sanitize_rel_path(&e.path)?;
      if e.deleted {
        needs_staging.push(false);
        continue;
      }
      if let Some(parent) = Path::new(&clean_rel).parent()
        && !parent.as_os_str().is_empty()
        && last_parent.as_deref() != Some(parent)
      {
        fs::create_dir_all(root.join(parent))?;
        last_parent = Some(parent.to_path_buf());
      }
      if e.size == 0 {
        needs_staging.push(false);
        continue;
      }
      if root_existed
        && let final_path = root.join(&clean_rel)
        && let Ok(basis) = File::open(&final_path)
        && let Ok(meta) = basis.metadata()
      {
        let basis_len = meta.len();
        let plan = match_basis_file(
          &e.chunks,
          (&basis, basis_len),
          CdcConfig::DEFAULT,
          (&mut basis_idx, &mut io_buf),
          &mut initial_have_bits,
          &mut bit_cursor,
          &mut delta_cursor,
        )?;
        if plan.delta_bytes == 0
          && basis_len == e.size
          && plan.copy_spans.len() <= 1
          && plan
            .copy_spans
            .first()
            .is_none_or(|s| s.src_off == 0 && s.dst_off == 0)
        {
          basis_fds.insert(ei as u32, basis);
          needs_staging.push(false);
          continue;
        }

        if e.chunks.len() == 1 {
          pack_offsets[ei] = pack_total;
          pack_total += e.size;
          needs_staging.push(true);
          continue;
        }

        let tmp_path = Self::tmp_path_for(&final_path);
        let tmp_file = OpenOptions::new()
          .create(true)
          .truncate(true)
          .read(true)
          .write(true)
          .open(&tmp_path)?;
        tmp_file.set_len(e.size)?;
        if !plan.copy_spans.is_empty() {
          if io_buf.len() < 64 * 1024 {
            io_buf.resize(64 * 1024, 0);
          }
          for sp in &plan.copy_spans {
            copy_file_span(
              &basis,
              sp.src_off,
              &tmp_file,
              sp.dst_off,
              sp.len,
              &mut io_buf,
            )?;
          }
        }
        staging_fds.insert(ei as u32, tmp_file);
        needs_staging.push(true);
      } else {
        bit_cursor += e.chunks.len();
        delta_cursor += e.size;
        needs_staging.push(true);
        if e.chunks.len() == 1 {
          pack_offsets[ei] = pack_total;
          pack_total += e.size;
        }
      }
    }

    let small_pack = SmallPack::create(root, pack_total)?;

    let mut have_words = Vec::with_capacity(word_cnt);
    for w_idx in 0..word_cnt {
      let base = w_idx * 8;
      let mut bytes = [0u8; 8];
      if let Some(sub) = initial_have_bits.get(base..) {
        let n = sub.len().min(8);
        bytes[..n].copy_from_slice(&sub[..n]);
      }
      have_words.push(AtomicU64::new(u64::from_le_bytes(bytes)));
    }

    Ok(Self {
      root: root.to_path_buf(),
      manifest,
      block_map,
      needs_staging,
      pack_offsets,
      small_pack,
      staging_fds,
      basis_fds,
      have_words,
      initial_have_bits,
    })
  }

  fn tmp_path_for(final_path: &Path) -> PathBuf {
    let mut os = final_path.as_os_str().to_owned();
    os.push(".s0tmp");
    PathBuf::from(os)
  }

  #[inline]
  pub fn manifest(&self) -> &Manifest {
    &self.manifest
  }

  #[inline]
  pub fn total_blocks(&self) -> u32 {
    self.block_map.len() as u32
  }

  #[inline]
  pub fn block_loc(&self, block_idx: u32) -> Option<BlockLoc> {
    self.block_map.get(block_idx as usize).copied()
  }

  /// 获取全局块 `block_idx` 的原始字节长度
  #[inline]
  pub fn block_size(&self, block_idx: u32) -> u32 {
    self
      .block_map
      .get(block_idx as usize)
      .map_or(0, |loc| loc.len)
  }

  /// 计算全部块的传输字节权重向量（本地已命中块权重为 `1`，缺失块按真实字节长度 `len.max(1024)` 加权）
  pub fn block_weights(&self) -> Vec<u64> {
    let total = self.total_blocks();
    let mut weights = Vec::with_capacity(total as usize);
    for b in 0..total {
      let w = if self.has_block(b) {
        1
      } else {
        u64::from(self.block_size(b)).max(1024)
      };
      weights.push(w);
    }
    weights
  }

  #[inline]
  pub fn initial_have_bits(&self) -> &[u8] {
    &self.initial_have_bits
  }

  /// 导出当前最新的位图快照（64 位字长批量拷贝）
  pub fn snapshot_have_bits(&self) -> Vec<u8> {
    let total = self.total_blocks() as usize;
    let mut out = vec![0u8; total.div_ceil(8)];
    for (wi, atom) in self.have_words.iter().enumerate() {
      let bytes = atom.load(Ordering::Acquire).to_le_bytes();
      let base = wi * 8;
      if let Some(dst) = out.get_mut(base..) {
        let n = dst.len().min(8);
        dst[..n].copy_from_slice(&bytes[..n]);
      }
    }
    out
  }

  /// 判断全局块 `block_idx` 是否已校验落盘
  #[inline]
  pub fn has_block(&self, block_idx: u32) -> bool {
    let wi = (block_idx >> 6) as usize;
    let bit = 1u64 << (block_idx & 63);
    self
      .have_words
      .get(wi)
      .is_some_and(|w| (w.load(Ordering::Acquire) & bit) != 0)
  }

  /// 计算当前已校验落盘的总块数（用于实时边下边传汇报）
  pub fn verified_block_count(&self) -> u32 {
    let total = self.total_blocks();
    let full_words = (total >> 6) as usize;
    let rem_bits = total & 63;
    let mut done = 0u32;
    for w in &self.have_words[..full_words] {
      done += w.load(Ordering::Acquire).count_ones();
    }
    if rem_bits > 0
      && let Some(last) = self.have_words.get(full_words)
    {
      let mask = (1u64 << rem_bits) - 1;
      done += (last.load(Ordering::Acquire) & mask).count_ones();
    }
    done
  }

  /// 判断区间 `[blk_lo, blk_hi)` 内所有块是否全部到位（$O((\text{hi}-\text{lo})/64)$ 64 位字长掩码判定）
  pub fn is_range_complete(&self, blk_lo: u32, blk_hi: u32) -> bool {
    let hi = blk_hi.min(self.total_blocks());
    if blk_lo >= hi {
      return true;
    }
    let first_w = (blk_lo >> 6) as usize;
    let last_w = ((hi - 1) >> 6) as usize;
    let lo_mask = !0u64 << (blk_lo & 63);
    let hi_mask = !0u64 >> (63 - ((hi - 1) & 63));
    if first_w == last_w {
      let mask = lo_mask & hi_mask;
      return self
        .have_words
        .get(first_w)
        .is_some_and(|w| (w.load(Ordering::Acquire) & mask) == mask);
    }
    if self
      .have_words
      .get(first_w)
      .is_none_or(|w| (w.load(Ordering::Acquire) & lo_mask) != lo_mask)
    {
      return false;
    }
    for w in self.have_words.get(first_w + 1..last_w).unwrap_or(&[]) {
      if w.load(Ordering::Acquire) != u64::MAX {
        return false;
      }
    }
    self
      .have_words
      .get(last_w)
      .is_some_and(|w| (w.load(Ordering::Acquire) & hi_mask) == hi_mask)
  }

  /// 校验收到块的 128-bit BLAKE3 摘要并无锁写入聚合流或 `.s0tmp`（多线程并发安全）
  pub fn write_verified_block(&self, block_idx: u32, data: &[u8]) -> Result<bool, FsError> {
    if self.has_block(block_idx) {
      return Ok(false);
    }
    let loc = self
      .block_map
      .get(block_idx as usize)
      .copied()
      .ok_or(FsError::BlockCorrupted(block_idx))?;
    if data.len() != loc.len as usize || hash128(data) != loc.hash {
      return Err(FsError::BlockCorrupted(block_idx));
    }
    let ei = loc.entry_idx as usize;
    if !self.needs_staging.get(ei).copied().unwrap_or(false) {
      return Err(FsError::BlockCorrupted(block_idx));
    }
    let e = self
      .manifest
      .entries
      .get(ei)
      .ok_or(FsError::BlockCorrupted(block_idx))?;
    let pack_off = self
      .pack_offsets
      .get(ei)
      .copied()
      .unwrap_or(UNPACKED_OFFSET);
    if pack_off != UNPACKED_OFFSET {
      self.small_pack.write_at(pack_off, data)?;
    } else {
      let f = self.staging_fds.get_or_open(loc.entry_idx, || {
        let tmp_path = Self::tmp_path_for(&self.root.join(&e.path));
        let opened = OpenOptions::new()
          .create(true)
          .truncate(false)
          .read(true)
          .write(true)
          .open(tmp_path)?;
        let _ = opened.set_len(e.size);
        Ok(opened)
      })?;
      f.write_all_at_pos(data, loc.offset)?;
    }

    let wi = (block_idx >> 6) as usize;
    let bit = 1u64 << (block_idx & 63);
    let prev = self.have_words[wi].fetch_or(bit, Ordering::Release);
    Ok((prev & bit) == 0)
  }

  /// 边下边传（Leecher Seeding）：无锁读取当前接收池中已通过 BLAKE3 校验的块供其他节点拉取
  pub fn read_verified_block(&self, block_idx: u32, buf: &mut Vec<u8>) -> Result<(), FsError> {
    let loc = self
      .block_map
      .get(block_idx as usize)
      .copied()
      .ok_or(FsError::BlockCorrupted(block_idx))?;
    buf.resize(loc.len as usize, 0);
    self.read_verified_loc_into(block_idx, loc, buf)
  }

  /// 读取已校验块到定长切片 `dst`
  pub fn read_verified_loc_into(
    &self,
    block_idx: u32,
    loc: BlockLoc,
    dst: &mut [u8],
  ) -> Result<(), FsError> {
    if !self.has_block(block_idx) {
      return Err(FsError::BlockCorrupted(block_idx));
    }
    let ei = loc.entry_idx as usize;
    let e = self
      .manifest
      .entries
      .get(ei)
      .ok_or(FsError::BlockCorrupted(block_idx))?;
    if self.needs_staging.get(ei).copied().unwrap_or(false) {
      let pack_off = self
        .pack_offsets
        .get(ei)
        .copied()
        .unwrap_or(UNPACKED_OFFSET);
      if pack_off != UNPACKED_OFFSET {
        self.small_pack.read_at(pack_off, dst)?;
      } else {
        let f = self.staging_fds.get_or_open(loc.entry_idx, || {
          File::open(Self::tmp_path_for(&self.root.join(&e.path)))
        })?;
        f.read_exact_at_pos(dst, loc.offset)?;
      }
    } else {
      let f = self
        .basis_fds
        .get_or_open(loc.entry_idx, || File::open(self.root.join(&e.path)))?;
      f.read_exact_at_pos(dst, loc.offset)?;
    }
    Ok(())
  }

  /// 校验全部块到位后，从 `SmallPack`（内存池或 `.s0_pack.tmp` 文件流）并发落盘小文件并将 `.s0tmp` 原子重命名为最终文件
  pub fn commit(&self) -> Result<(), FsError> {
    let total = self.total_blocks();
    let done = self.verified_block_count();
    if done != total {
      return Err(FsError::IncompleteCommit(done, total));
    }

    self.staging_fds.clear();
    self.basis_fds.clear();

    let need_prune = self.manifest.delete_extraneous && !self.manifest.partial;
    let mut keep_paths = if need_prune {
      Vec::with_capacity(self.manifest.entries.len())
    } else {
      Vec::new()
    };
    let mut pack_jobs: Vec<(PathBuf, u64, u32)> = Vec::new();

    for (ei, e) in self.manifest.entries.iter().enumerate() {
      let clean_rel = sanitize_rel_path(&e.path)?;
      let final_path = self.root.join(&clean_rel);
      if e.deleted {
        remove(&final_path)?;
        continue;
      }
      if need_prune {
        keep_paths.push(clean_rel);
      }
      if e.size == 0 {
        if let Some(parent) = final_path.parent() {
          fs::create_dir_all(parent)?;
        }
        File::create(&final_path)?;
      } else if self.needs_staging.get(ei).copied().unwrap_or(false) {
        let pack_off = self
          .pack_offsets
          .get(ei)
          .copied()
          .unwrap_or(UNPACKED_OFFSET);
        if pack_off != UNPACKED_OFFSET {
          pack_jobs.push((final_path, pack_off, e.size as u32));
        } else {
          let tmp_path = Self::tmp_path_for(&final_path);
          fs::rename(&tmp_path, &final_path)?;
        }
      }
    }

    self.small_pack.flush_jobs(&pack_jobs)?;

    if need_prune {
      prune_dir(&self.root, &keep_paths)?;
    }

    Ok(())
  }
}

impl Drop for SharedSink {
  fn drop(&mut self) {
    self.small_pack.cleanup();
  }
}
