use std::{
  ffi::OsString,
  fs::{self, File},
  path::{Path, PathBuf},
  sync::atomic::{AtomicU64, Ordering},
};

use s0_cdc::{BasisIndex, BlockLoc, CdcConfig, FileAt, Manifest, hash128, match_basis_file};

use super::{
  FsError, copy_file_spans, create_trunc,
  fd_cache::FdCache,
  open_rw,
  pack::{SmallPack, UNPACKED_OFFSET},
  prune_dir, remove, sanitize_rel_path,
};

/// 支持多个对等节点并发无锁写入、断点续传与边下边传点播读取的接收池 (`SharedSink`)
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
  /// 初始化接收池：对本地已有同名文件与未完成 `.s0_stage.<filename>` 暂存文件执行 FastCDC 基准匹配，支持零状态文件断点续传
  pub fn prepare(root: &Path, manifest: Manifest) -> Result<Self, FsError> {
    let root_existed = root.is_dir();
    fs::create_dir_all(root)?;
    let block_map = manifest.build_block_map();
    let total_blocks = block_map.len() as u32;
    let word_cnt = (total_blocks as usize).div_ceil(64);
    let mut initial_have_bits = vec![0u8; (total_blocks as usize).div_ceil(8)];

    let n_entries = manifest.entries.len();
    let mut needs_staging = Vec::with_capacity(n_entries);
    let mut pack_offsets = vec![UNPACKED_OFFSET; n_entries];
    let mut pack_total = 0u64;
    let (staging_fds, basis_fds) = (FdCache::new(), FdCache::new());
    let (mut bit_cursor, mut delta_cursor) = (0usize, 0u64);
    let (mut basis_idx, mut io_buf, mut last_parent) = (BasisIndex::new(), Vec::new(), None);

    for (ei, e) in manifest.entries.iter().enumerate() {
      let clean_rel = sanitize_rel_path(&e.path)?;
      if e.deleted || e.size == 0 {
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
      let base_bit = bit_cursor;
      let final_path = root.join(&clean_rel);
      let tmp_path = Self::tmp_path_for(&final_path);
      let mut basis_hit = None;

      if root_existed
        && let Ok(basis) = File::open(&final_path)
        && let Ok(meta) = basis.metadata()
      {
        let (basis_len, mut bc) = (meta.len(), base_bit);
        let plan = match_basis_file(
          &e.chunks,
          (&basis, basis_len),
          CdcConfig::DEFAULT,
          (&mut basis_idx, &mut io_buf),
          &mut initial_have_bits,
          &mut bc,
          &mut delta_cursor,
        )?;
        if plan.delta_bytes == 0
          && basis_len == e.size
          && plan
            .copy_spans
            .first()
            .is_none_or(|s| plan.copy_spans.len() == 1 && s.src_off == 0 && s.dst_off == 0)
        {
          let _ = remove(&tmp_path);
          bit_cursor = base_bit + e.chunks.len();
          basis_fds.insert(ei as u32, basis);
          needs_staging.push(false);
          continue;
        }
        if plan.reused_bytes > 0 {
          basis_hit = Some((basis, plan));
        }
      }

      let mut stage_reused = 0u64;
      if root_existed
        && let Ok(stage) = File::open(&tmp_path)
        && let Ok(meta) = stage.metadata()
        && meta.len() > 0
      {
        let mut bc = base_bit;
        stage_reused = match_basis_file(
          &e.chunks,
          (&stage, meta.len()),
          CdcConfig::DEFAULT,
          (&mut basis_idx, &mut io_buf),
          &mut initial_have_bits,
          &mut bc,
          &mut delta_cursor,
        )?
        .reused_bytes;
      }
      bit_cursor = base_bit + e.chunks.len();

      if e.chunks.len() == 1 && basis_hit.is_none() && stage_reused == 0 {
        pack_offsets[ei] = pack_total;
        pack_total += e.size;
      } else if stage_reused > 0 || basis_hit.is_some() {
        let tmp_file = open_rw(&tmp_path, stage_reused == 0, e.size)?;
        if let Some((basis, plan)) = basis_hit {
          copy_file_spans(&basis, &tmp_file, &plan.copy_spans, &mut io_buf)?;
        }
        staging_fds.insert(ei as u32, tmp_file);
      } else {
        delta_cursor += e.size;
        let _ = remove(&tmp_path);
      }
      needs_staging.push(true);
    }

    let small_pack = SmallPack::create(root, pack_total)?;
    let have_words = (0..word_cnt)
      .map(|w_idx| {
        let (mut bytes, sub) = ([0u8; 8], &initial_have_bits[w_idx * 8..]);
        let n = sub.len().min(8);
        bytes[..n].copy_from_slice(&sub[..n]);
        AtomicU64::new(u64::from_le_bytes(bytes))
      })
      .collect();

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

  /// 生成目标文件的同目录暂存文件路径 `.s0_stage.<filename>`
  pub fn tmp_path_for(final_path: &Path) -> PathBuf {
    let mut os = OsString::from(".s0_stage.");
    if let Some(name) = final_path.file_name() {
      os.push(name);
    }
    match final_path.parent() {
      Some(p) if !p.as_os_str().is_empty() => p.join(os),
      _ => PathBuf::from(os),
    }
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
  pub fn block_size(&self, block_idx: u32) -> u32 {
    self.block_loc(block_idx).map_or(0, |loc| loc.len)
  }

  /// 计算全部块的传输字节权重向量（本地已命中块权重为 `1`，缺失块按真实字节长度 `len.max(1024)` 加权）
  pub fn block_weights(&self) -> Vec<u64> {
    (0..self.total_blocks())
      .map(|b| {
        if self.has_block(b) {
          1
        } else {
          u64::from(self.block_size(b)).max(1024)
        }
      })
      .collect()
  }

  #[inline]
  pub fn initial_have_bits(&self) -> &[u8] {
    &self.initial_have_bits
  }

  /// 导出当前最新的位图快照（64 位字长批量拷贝）
  pub fn snapshot_have_bits(&self) -> Vec<u8> {
    let mut out = vec![0u8; (self.total_blocks() as usize).div_ceil(8)];
    for (wi, atom) in self.have_words.iter().enumerate() {
      let bytes = atom.load(Ordering::Acquire).to_le_bytes();
      if let Some(dst) = out.get_mut(wi * 8..) {
        let n = dst.len().min(8);
        dst[..n].copy_from_slice(&bytes[..n]);
      }
    }
    out
  }

  #[inline]
  fn word_has_mask(&self, wi: usize, mask: u64) -> bool {
    self
      .have_words
      .get(wi)
      .is_some_and(|w| (w.load(Ordering::Acquire) & mask) == mask)
  }

  /// 判断全局块 `block_idx` 是否已校验落盘
  #[inline]
  pub fn has_block(&self, block_idx: u32) -> bool {
    self.word_has_mask((block_idx >> 6) as usize, 1u64 << (block_idx & 63))
  }

  /// 计算当前已校验落盘的总块数
  pub fn verified_block_count(&self) -> u32 {
    let total = self.total_blocks();
    let (full_words, rem_bits) = ((total >> 6) as usize, total & 63);
    let mut done: u32 = self.have_words[..full_words]
      .iter()
      .map(|w| w.load(Ordering::Acquire).count_ones())
      .sum();
    if rem_bits > 0
      && let Some(last) = self.have_words.get(full_words)
    {
      done += (last.load(Ordering::Acquire) & ((1u64 << rem_bits) - 1)).count_ones();
    }
    done
  }

  /// 判断区间 `[blk_lo, blk_hi)` 内所有块是否全部到位（$O((\text{hi}-\text{lo})/64)$ 64 位字长掩码判定）
  pub fn is_range_complete(&self, blk_lo: u32, blk_hi: u32) -> bool {
    let hi = blk_hi.min(self.total_blocks());
    if blk_lo >= hi {
      return true;
    }
    let (first_w, last_w) = ((blk_lo >> 6) as usize, ((hi - 1) >> 6) as usize);
    let (lo_mask, hi_mask) = (!0u64 << (blk_lo & 63), !0u64 >> (63 - ((hi - 1) & 63)));
    if first_w == last_w {
      return self.word_has_mask(first_w, lo_mask & hi_mask);
    }
    self.word_has_mask(first_w, lo_mask)
      && self.word_has_mask(last_w, hi_mask)
      && self
        .have_words
        .get(first_w + 1..last_w)
        .unwrap_or(&[])
        .iter()
        .all(|w| w.load(Ordering::Acquire) == u64::MAX)
  }

  /// 校验收到块的 128-bit `gxhash128` 摘要并无锁写入聚合流或 `.s0_stage.<filename>`
  pub fn write_verified_block(&self, block_idx: u32, data: &[u8]) -> Result<bool, FsError> {
    if self.has_block(block_idx) {
      return Ok(false);
    }
    let loc = self
      .block_loc(block_idx)
      .ok_or(FsError::BlockCorrupted(block_idx))?;
    let ei = loc.entry_idx as usize;
    if data.len() != loc.len as usize
      || hash128(data) != loc.hash
      || !self.needs_staging.get(ei).copied().unwrap_or(false)
    {
      return Err(FsError::BlockCorrupted(block_idx));
    }
    let e = &self.manifest.entries[ei];
    let pack_off = self.pack_offsets[ei];
    if pack_off != UNPACKED_OFFSET {
      self.small_pack.write_at(pack_off, data)?;
    } else {
      let tmp = Self::tmp_path_for(&self.root.join(&e.path));
      let f = self
        .staging_fds
        .get_or_open(loc.entry_idx, || open_rw(&tmp, false, e.size))?;
      f.write_all_at_pos(data, loc.offset)?;
    }
    let bit = 1u64 << (block_idx & 63);
    let prev = self.have_words[(block_idx >> 6) as usize].fetch_or(bit, Ordering::Release);
    Ok((prev & bit) == 0)
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
    let ei = loc.entry_idx as usize;
    let e = &self.manifest.entries[ei];
    if self.needs_staging[ei] {
      let pack_off = self.pack_offsets[ei];
      if pack_off != UNPACKED_OFFSET {
        self.small_pack.read_at(pack_off, dst)?;
      } else {
        let f = self.staging_fds.get_or_open(loc.entry_idx, || {
          let final_p = self.root.join(&e.path);
          File::open(Self::tmp_path_for(&final_p)).or_else(|_| File::open(final_p))
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

  /// 点播流式读取：从条目 `entry_idx` 的字节偏移 `file_offset` 读取已校验块数据到 `dst`
  pub fn read_at_offset(
    &self,
    entry_idx: u32,
    file_offset: u64,
    dst: &mut [u8],
  ) -> Result<usize, FsError> {
    let e = self
      .manifest
      .entries
      .get(entry_idx as usize)
      .ok_or(FsError::BlockCorrupted(entry_idx))?;
    if e.deleted || file_offset >= e.size || dst.is_empty() {
      return Ok(0);
    }
    let max_avail = (e.size - file_offset).min(dst.len() as u64) as usize;
    let entry_blk_base = self.block_map.partition_point(|b| b.entry_idx < entry_idx) as u32;
    let mut c_idx = e
      .chunks
      .partition_point(|c| c.offset.saturating_add(u64::from(c.len)) <= file_offset);
    let (mut written, mut blk_buf) = (0usize, Vec::new());
    while written < max_avail && c_idx < e.chunks.len() {
      let (c, blk_idx) = (&e.chunks[c_idx], entry_blk_base + (c_idx as u32));
      let (clen, in_off) = (
        c.len as usize,
        (file_offset + (written as u64) - c.offset) as usize,
      );
      let to_copy = (max_avail - written).min(clen.saturating_sub(in_off));
      let dst_slice = &mut dst[written..written + to_copy];
      if in_off == 0 && to_copy == clen {
        self.read_verified_loc_into(blk_idx, self.block_map[blk_idx as usize], dst_slice)?;
      } else {
        self.read_verified_block(blk_idx, &mut blk_buf)?;
        dst_slice.copy_from_slice(&blk_buf[in_off..in_off + to_copy]);
      }
      written += to_copy;
      c_idx += 1;
    }
    Ok(written)
  }

  /// 提交块区间 `[blk_lo, blk_hi)`：全量区间执行原子重命名提交，子区间校验完整性后原地写入目标文件对应偏移
  pub fn commit_range(&self, blk_lo: u32, blk_hi: u32) -> Result<(), FsError> {
    let total = self.total_blocks();
    let hi = blk_hi.min(total);
    let lo = blk_lo.min(hi);
    if lo == 0 && hi == total {
      return self.commit();
    }
    if !self.is_range_complete(lo, hi) {
      let done = (lo..hi).filter(|&b| self.has_block(b)).count() as u32;
      return Err(FsError::IncompleteCommit(done, hi - lo));
    }
    let (mut buf, mut cur_ei, mut cur_file) = (Vec::new(), u32::MAX, None);
    for b in lo..hi {
      let loc = self.block_map[b as usize];
      self.read_verified_block(b, &mut buf)?;
      if loc.entry_idx != cur_ei {
        cur_ei = loc.entry_idx;
        let e = &self.manifest.entries[cur_ei as usize];
        let final_path = self.root.join(sanitize_rel_path(&e.path)?);
        if let Some(parent) = final_path.parent() {
          fs::create_dir_all(parent)?;
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
        let _ = remove(&Self::tmp_path_for(&self.root.join(clean_rel)));
      }
    }
    Ok(())
  }

  /// 校验全部块到位后，从 `SmallPack` 并发落盘小文件并将 `.s0_stage.<filename>` 原子重命名为最终文件
  pub fn commit(&self) -> Result<(), FsError> {
    let (total, done) = (self.total_blocks(), self.verified_block_count());
    if done != total {
      return Err(FsError::IncompleteCommit(done, total));
    }
    self.staging_fds.clear();
    self.basis_fds.clear();
    let need_prune = self.manifest.delete_extraneous && !self.manifest.partial;
    let (mut keep_paths, mut pack_jobs) = (Vec::new(), Vec::new());
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
        create_trunc(&final_path)?;
      } else if self.needs_staging[ei] {
        let pack_off = self.pack_offsets[ei];
        if pack_off != UNPACKED_OFFSET {
          pack_jobs.push((final_path, pack_off, e.size as u32));
        } else {
          fs::rename(Self::tmp_path_for(&final_path), &final_path)?;
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
