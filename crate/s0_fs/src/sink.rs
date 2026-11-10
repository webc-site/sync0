use std::{
  fs::{File, create_dir_all, rename},
  path::{Path, PathBuf},
};

use s0_cdc::{
  BasisIndex, BlockLoc, CdcConfig, FileAt, Manifest, copy_file_spans, hash128, match_basis_file,
  norm_mode, set_bit_usize,
};
use s0_const::{DEFAULT_FILE_MODE, MIN_MISSING_BLOCK_WEIGHT};

use crate::{
  FsError, apply_file_metas,
  cache::{PAR_SCAN_MIN_FILES, invalidate_small_cache, precheck_manifest_single_chunks},
  create_trunc, dir_has_entries, ensure_parent_dirs,
  fd_cache::FdCache,
  open_nonempty_file, open_rw,
  pack::{AtomicBitset, SmallPack, UNPACKED_OFFSET},
  prune_dir, remove, sanitize_rel_path, stage_path,
};

/// 支持多个对等节点并发无锁写入、断点续传与边下边传点播读取的接收池 (`SharedSink`)
pub struct SharedSink {
  root: PathBuf,
  manifest: Manifest,
  pub(crate) block_map: Vec<BlockLoc>,
  weight_prefix: Box<[u64]>,
  needs_staging: Vec<bool>,
  in_place: Vec<bool>,
  pack_offsets: Vec<u64>,
  small_pack: SmallPack,
  staging_fds: FdCache,
  basis_fds: FdCache,
  have_bits: AtomicBitset,
  initial_have_bits: Vec<u8>,
}

impl SharedSink {
  /// 初始化接收池：对本地已有同名文件与未完成 `.s0_stage.<filename>` 暂存文件执行 FastCDC 基准匹配，支持零状态文件断点续传
  pub fn prepare(root: &Path, manifest: Manifest) -> Result<Self, FsError> {
    let root_existed = dir_has_entries(root);
    let block_map = manifest.build_block_map();
    let total_blocks = block_map.len();
    let mut weight_prefix = Vec::with_capacity(total_blocks + 1);
    weight_prefix.push(0u64);
    for loc in &block_map {
      let next = weight_prefix.last().copied().unwrap_or(0)
        + u64::from(loc.len).max(MIN_MISSING_BLOCK_WEIGHT);
      weight_prefix.push(next);
    }
    let mut initial_have_bits = vec![0u8; total_blocks.div_ceil(8)];

    let (n_entries, mut pack_total) = (manifest.entries.len(), 0u64);
    let clean_rels = manifest
      .entries
      .iter()
      .map(|e| sanitize_rel_path(&e.path))
      .collect::<Result<Vec<_>, _>>()?;
    let active_rels = manifest
      .entries
      .iter()
      .zip(&clean_rels)
      .filter(|(e, _)| !e.deleted)
      .map(|(_, r)| r.as_str());
    ensure_parent_dirs(root, active_rels)?;

    let mut needs_staging = Vec::with_capacity(n_entries);
    let (mut in_place, mut pack_offsets) =
      (vec![false; n_entries], vec![UNPACKED_OFFSET; n_entries]);
    let (staging_fds, basis_fds) = (FdCache::new(), FdCache::new());
    let mut bit_cursor = 0usize;
    let (mut basis_idx, mut io_buf) = (BasisIndex::new(), Vec::new());
    let prechecked = if root_existed && n_entries >= PAR_SCAN_MIN_FILES {
      precheck_manifest_single_chunks(root, &manifest.entries)
    } else {
      Vec::new()
    };

    for (ei, e) in manifest.entries.iter().enumerate() {
      let clean_rel = &clean_rels[ei];
      if e.deleted || e.size == 0 {
        needs_staging.push(false);
        continue;
      }
      let base_bit = bit_cursor;
      bit_cursor = base_bit + e.chunks.len();
      if prechecked.get(ei).copied().unwrap_or(false) {
        set_bit_usize(&mut initial_have_bits, base_bit);
        needs_staging.push(false);
        continue;
      }
      if (!root_existed || !prechecked.is_empty()) && e.chunks.len() == 1 {
        pack_offsets[ei] = pack_total;
        pack_total += e.size;
        needs_staging.push(true);
        continue;
      }
      if !root_existed {
        needs_staging.push(true);
        continue;
      }
      let final_path = root.join(clean_rel);
      let (tmp_path, mut basis_hit) = (stage_path(&final_path), None);
      let mut match_candidate = |p: &Path| -> Result<Option<(File, u64, _)>, FsError> {
        let Some((f, len)) = open_nonempty_file(p) else {
          return Ok(None);
        };
        let (mut bc, mut dc) = (base_bit, 0u64);
        let plan = match_basis_file(
          &e.chunks,
          (&f, len),
          CdcConfig::DEFAULT,
          (&mut basis_idx, &mut io_buf),
          &mut initial_have_bits,
          &mut bc,
          &mut dc,
        )?;
        Ok(Some((f, len, plan)))
      };

      if let Some((basis, basis_len, plan)) = match_candidate(&final_path)? {
        if basis_len == e.size && plan.is_identical_to(e.size) {
          let _ = remove(&tmp_path);
          basis_fds.insert(ei as u32, basis);
          needs_staging.push(false);
          continue;
        }
        if plan.reused_bytes > 0 {
          basis_hit = Some((basis, plan));
        }
      }

      let stage_reused = match_candidate(&tmp_path)?.map_or(0, |(_, _, p)| p.reused_bytes);

      if e.chunks.len() == 1 && basis_hit.is_none() && stage_reused == 0 {
        pack_offsets[ei] = pack_total;
        pack_total += e.size;
      } else if stage_reused == 0 && basis_hit.as_ref().is_some_and(|(_, p)| p.is_in_place()) {
        drop(basis_hit);
        let _ = remove(&tmp_path);
        staging_fds.insert(ei as u32, open_rw(&final_path, false, e.size)?);
        in_place[ei] = true;
      } else if stage_reused > 0 || basis_hit.is_some() {
        let tmp_file = open_rw(&tmp_path, stage_reused == 0, e.size)?;
        if let Some((basis, plan)) = basis_hit {
          copy_file_spans(&basis, &tmp_file, &plan.copy_spans, &mut io_buf)?;
        }
        staging_fds.insert(ei as u32, tmp_file);
      } else {
        let _ = remove(&tmp_path);
      }
      needs_staging.push(true);
    }

    let small_pack = SmallPack::create(root, pack_total)?;
    let have_bits = AtomicBitset::from_bits(&initial_have_bits, total_blocks as u32);

    Ok(Self {
      root: root.to_path_buf(),
      manifest,
      block_map,
      weight_prefix: weight_prefix.into_boxed_slice(),
      needs_staging,
      in_place,
      pack_offsets,
      small_pack,
      staging_fds,
      basis_fds,
      have_bits,
      initial_have_bits,
    })
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
  #[inline]
  pub fn initial_have_bits(&self) -> &[u8] {
    &self.initial_have_bits
  }
  #[inline]
  pub fn snapshot_have_bits(&self) -> Vec<u8> {
    self.have_bits.snapshot()
  }
  #[inline]
  pub fn has_block(&self, block_idx: u32) -> bool {
    self.have_bits.has(block_idx)
  }
  #[inline]
  pub fn next_missing(&self, from: u32, hi: u32) -> u32 {
    self.have_bits.next_missing(from, hi)
  }
  #[inline]
  pub fn is_range_complete(&self, blk_lo: u32, blk_hi: u32) -> bool {
    self.have_bits.is_range_complete(blk_lo, blk_hi)
  }
  #[inline]
  pub fn for_each_missing_span(&self, blk_lo: u32, blk_hi: u32, on_span: impl FnMut(u32, u32)) {
    self
      .have_bits
      .for_each_missing_span(blk_lo, blk_hi, on_span);
  }
  #[inline]
  pub fn longest_have_span(&self) -> (u32, u32) {
    self.have_bits.longest_have_span()
  }

  /// 连续缺失块区间 `[blk_lo, blk_hi)` 的字节权重和（$O(1)$ 前缀和差分）
  #[inline]
  pub fn span_weight(&self, blk_lo: u32, blk_hi: u32) -> u64 {
    let hi = (blk_hi as usize).min(self.block_map.len());
    let lo = (blk_lo as usize).min(hi);
    self.weight_prefix[hi] - self.weight_prefix[lo]
  }

  /// 计算全部块的传输字节权重向量（本地已命中块权重为 `1`，缺失块按真实字节长度 `len.max(MIN_MISSING_BLOCK_WEIGHT)` 加权）
  pub fn block_weights(&self) -> Vec<u64> {
    (0..self.total_blocks())
      .map(|b| {
        if self.has_block(b) {
          1
        } else {
          self.span_weight(b, b + 1)
        }
      })
      .collect()
  }

  /// 裁剪 `[blk_lo, blk_hi)` 首尾已落盘的块，返回首个缺失块到末个缺失块+1 的紧凑区间；若区间内全部就绪则返回 `None`
  #[inline]
  pub fn trim_missing_range(&self, blk_lo: u32, blk_hi: u32) -> Option<(u32, u32)> {
    self.have_bits.trim_missing_range(blk_lo, blk_hi)
  }

  #[inline]
  fn target_path(&self, ei: usize) -> PathBuf {
    let p = self.root.join(&self.manifest.entries[ei].path);
    if self.in_place[ei] { p } else { stage_path(&p) }
  }

  /// 校验 `gxhash128` 分块摘要并无锁写入到聚合流、原位目标文件或 `.s0_stage.<filename>`（阻断恶意节点伪造块落盘与投毒扩散）
  pub fn write_verified_block(&self, block_idx: u32, data: &[u8]) -> Result<bool, FsError> {
    if self.has_block(block_idx) {
      return Ok(false);
    }
    let loc = self
      .block_loc(block_idx)
      .ok_or(FsError::BlockCorrupted(block_idx))?;
    let ei = loc.entry_idx as usize;
    if data.len() != loc.len as usize
      || !self.needs_staging.get(ei).copied().unwrap_or(false)
      || hash128(data) != loc.hash
    {
      return Err(FsError::BlockCorrupted(block_idx));
    }
    let pack_off = self.pack_offsets[ei];
    if pack_off != UNPACKED_OFFSET {
      self.small_pack.write_at(pack_off, data)?;
    } else {
      self
        .staging_fds
        .get_or_open(loc.entry_idx, || {
          open_rw(&self.target_path(ei), false, self.manifest.entries[ei].size)
        })?
        .write_all_at_pos(data, loc.offset)?;
    }
    Ok(self.have_bits.mark(block_idx))
  }

  /// 批量校验每个子块的 `gxhash128` 摘要并写入 `SuperBlock` 连续小文件切片（偏移连续时合并为单次 `pwrite`）
  pub fn write_superblock(
    &self,
    start_blk: u32,
    count: u32,
    raw_slice: &[u8],
  ) -> Result<(usize, u32), FsError> {
    let end_blk = start_blk.saturating_add(count);
    let first_loc = self
      .block_loc(start_blk)
      .ok_or(FsError::BlockCorrupted(start_blk))?;
    let base_off = self
      .pack_offsets
      .get(first_loc.entry_idx as usize)
      .copied()
      .unwrap_or(UNPACKED_OFFSET);
    if base_off != UNPACKED_OFFSET {
      let (mut expected_off, mut cur) = (base_off, 0usize);
      let ok_contig = (start_blk..end_blk).all(|b| {
        self.block_loc(b).is_some_and(|loc| {
          let (ei, len) = (loc.entry_idx as usize, loc.len as usize);
          let ok = self.pack_offsets.get(ei).copied() == Some(expected_off)
            && self.needs_staging.get(ei).copied().unwrap_or(false)
            && raw_slice
              .get(cur..cur + len)
              .is_some_and(|s| hash128(s) == loc.hash);
          expected_off += u64::from(loc.len);
          cur += len;
          ok
        })
      });
      if ok_contig && cur == raw_slice.len() {
        self.small_pack.write_at(base_off, raw_slice)?;
        return Ok(
          (start_blk..end_blk)
            .filter(|&b| self.have_bits.mark(b))
            .fold((0usize, 0u32), |(bytes, blks), b| {
              (bytes + self.block_map[b as usize].len as usize, blks + 1)
            }),
        );
      }
    }
    let (mut cur, mut w_bytes, mut w_blks) = (0usize, 0usize, 0u32);
    for b in start_blk..end_blk {
      let len = self.block_loc(b).ok_or(FsError::BlockCorrupted(b))?.len as usize;
      let sub = raw_slice
        .get(cur..cur + len)
        .ok_or(FsError::BlockCorrupted(b))?;
      if self.write_verified_block(b, sub)? {
        w_bytes += len;
        w_blks += 1;
      }
      cur += len;
    }
    (cur == raw_slice.len())
      .then_some((w_bytes, w_blks))
      .ok_or(FsError::BlockCorrupted(start_blk))
  }

  /// 边下边传（Leecher Seeding）：无锁读取已校验块供其他节点拉取
  pub fn read_verified_block(&self, block_idx: u32, buf: &mut Vec<u8>) -> Result<(), FsError> {
    let loc = self
      .block_loc(block_idx)
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
    let (ei, path) = (
      loc.entry_idx as usize,
      &self.manifest.entries[loc.entry_idx as usize].path,
    );
    let f = if self.needs_staging[ei] {
      let pack_off = self.pack_offsets[ei];
      if pack_off != UNPACKED_OFFSET {
        return Ok(self.small_pack.read_at(pack_off, dst)?);
      }
      self.staging_fds.get_or_open(loc.entry_idx, || {
        File::open(self.target_path(ei)).or_else(|_| File::open(self.root.join(path)))
      })?
    } else {
      self
        .basis_fds
        .get_or_open(loc.entry_idx, || File::open(self.root.join(path)))?
    };
    Ok(f.read_exact_at_pos(dst, loc.offset)?)
  }

  fn check_complete(&self, lo: u32, hi: u32) -> Result<(), FsError> {
    if !self.is_range_complete(lo, hi) {
      let have = (lo..hi).filter(|&b| self.has_block(b)).count() as u32;
      return Err(FsError::IncompleteCommit(have, hi - lo));
    }
    Ok(())
  }

  /// 提交块区间 `[blk_lo, blk_hi)`：全量区间执行原子重命名提交，子区间校验完整性后原地写入目标文件对应偏移
  pub fn commit_range(&self, blk_lo: u32, blk_hi: u32) -> Result<(), FsError> {
    let total = self.total_blocks();
    let (hi, lo) = (blk_hi.min(total), blk_lo.min(blk_hi.min(total)));
    if lo == 0 && hi == total {
      return self.commit();
    }
    self.check_complete(lo, hi)?;
    let (mut buf, mut cur_ei, mut cur_file) = (Vec::new(), u32::MAX, None);
    for b in lo..hi {
      let loc = self.block_map[b as usize];
      self.read_verified_block(b, &mut buf)?;
      if loc.entry_idx != cur_ei {
        cur_ei = loc.entry_idx;
        let e = &self.manifest.entries[cur_ei as usize];
        let final_path = self.root.join(sanitize_rel_path(&e.path)?);
        if let Some(parent) = final_path.parent() {
          create_dir_all(parent)?;
        }
        cur_file = Some(open_rw(&final_path, false, e.size)?);
      }
      if let Some(ref f) = cur_file {
        f.write_all_at_pos(&buf, loc.offset)?;
      }
    }
    drop(cur_file);
    self.staging_fds.clear();
    for e in &self.manifest.entries {
      if let Ok(clean_rel) = sanitize_rel_path(&e.path) {
        let _ = remove(&stage_path(&self.root.join(clean_rel)));
      }
    }
    invalidate_small_cache(&self.root);
    Ok(())
  }

  /// 校验全部块到位后，从 `SmallPack` 并发落盘小文件并将 `.s0_stage.<filename>` 原子重命名为最终文件
  pub fn commit(&self) -> Result<(), FsError> {
    self.check_complete(0, self.total_blocks())?;
    self.staging_fds.clear();
    self.basis_fds.clear();
    let need_prune = self.manifest.delete_extraneous && !self.manifest.partial;
    let (mut keep_paths, mut pack_jobs, mut meta_jobs) = (Vec::new(), Vec::new(), Vec::new());
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
      let mode = norm_mode(e.mode);
      let need_meta = mode != DEFAULT_FILE_MODE || e.mtime > 0 || e.size == 0 || self.in_place[ei];
      if e.size == 0 {
        create_trunc(&final_path)?;
      } else if self.needs_staging[ei] && self.pack_offsets[ei] != UNPACKED_OFFSET {
        pack_jobs.push((final_path.clone(), self.pack_offsets[ei], e.size as u32));
      } else if self.needs_staging[ei] && !self.in_place[ei] {
        rename(stage_path(&final_path), &final_path)?;
      }
      if need_meta {
        meta_jobs.push((final_path, mode, e.mtime));
      }
    }
    self.small_pack.flush_jobs(&pack_jobs)?;
    apply_file_metas(&meta_jobs);
    if need_prune {
      prune_dir(&self.root, &keep_paths)?;
    }
    invalidate_small_cache(&self.root);
    Ok(())
  }
}
