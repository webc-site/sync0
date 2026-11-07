mod filter;
mod path;

use std::{
  fs::{self, File, OpenOptions},
  io,
  path::{Path, PathBuf},
  sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
  },
};

pub use filter::{IGNORE_FILE, Ignore, ScannedFile, is_staging, prune_dir, scan_files};
use gxhash::HashSet;
pub use path::{MAX_PATH, Staging, rel_path, remove, safe_join};
pub use s0_cdc::FileAt;
use s0_cdc::{
  BlockLoc, CdcConfig, CdcError, Manifest, ManifestEntry, chunk_reader, hash128, set_bit,
};

#[derive(Debug, thiserror::Error)]
pub enum FsError {
  #[error("IO 错误: {0}")]
  Io(#[from] io::Error),
  #[error("CDC 清单错误: {0}")]
  Cdc(#[from] CdcError),
  #[error("目录遍历/忽略规则错误: {0}")]
  Ignore(#[from] ignore::Error),
  #[error("块 {0} BLAKE3 摘要校验不匹配或长度非法")]
  BlockCorrupted(u32),
  #[error("尚有未完成落盘的块 ({0}/{1})，拒绝原子提交")]
  IncompleteCommit(u32, u32),
  #[error("非法相对路径: {0}")]
  BadPath(String),
}

/// 校验相对路径安全性（防止 `..` 路径穿越）
pub fn sanitize_rel_path(rel: &str) -> Result<String, FsError> {
  let trimmed = rel.trim_matches(['/', '\\']);
  if trimmed.is_empty() {
    return Ok(String::new());
  }
  for seg in trimmed.split(['/', '\\']) {
    if seg.is_empty() || seg == "." || seg == ".." {
      return Err(FsError::BadPath(rel.to_owned()));
    }
  }
  if trimmed.contains('\\') {
    Ok(trimmed.replace('\\', "/"))
  } else {
    Ok(trimmed.to_owned())
  }
}

/// 递归扫描本地目录或单文件并构建 `Manifest`（支持指定子路径 `subpath`）
pub fn build_manifest_from_path(
  root: &Path,
  subpath: &str,
  delete_extraneous: bool,
) -> Result<Manifest, FsError> {
  let clean_sub = sanitize_rel_path(subpath)?;
  if root.is_file() {
    let mut f = File::open(root)?;
    let (chunks, size) = chunk_reader(&mut f, CdcConfig::DEFAULT)?;
    let file_name = root
      .file_name()
      .and_then(|s| s.to_str())
      .unwrap_or("file")
      .to_owned();
    return Ok(Manifest {
      entries: vec![ManifestEntry {
        path: file_name,
        size,
        chunks,
        deleted: false,
      }],
      delete_extraneous: false,
      partial: true,
    });
  }

  let target = if clean_sub.is_empty() {
    root.to_path_buf()
  } else {
    root.join(&clean_sub)
  };
  if !target.exists() {
    return Err(FsError::Io(io::Error::new(
      io::ErrorKind::NotFound,
      format!("路径不存在: {}", target.display()),
    )));
  }

  if target.is_file() {
    let mut f = File::open(&target)?;
    let (chunks, size) = chunk_reader(&mut f, CdcConfig::DEFAULT)?;
    return Ok(Manifest {
      entries: vec![ManifestEntry {
        path: clean_sub,
        size,
        chunks,
        deleted: false,
      }],
      delete_extraneous: false,
      partial: true,
    });
  }

  let mut rel_files = Vec::new();
  collect_files_rec(root, &target, &mut rel_files)?;
  rel_files.sort_unstable();

  let mut entries = Vec::with_capacity(rel_files.len());
  for rel in rel_files {
    let full = root.join(&rel);
    let mut f = File::open(&full)?;
    let (chunks, size) = chunk_reader(&mut f, CdcConfig::DEFAULT)?;
    entries.push(ManifestEntry {
      path: rel,
      size,
      chunks,
      deleted: false,
    });
  }

  Ok(Manifest {
    entries,
    delete_extraneous: delete_extraneous && clean_sub.is_empty(),
    partial: !clean_sub.is_empty(),
  })
}

fn collect_files_rec(root: &Path, dir: &Path, out: &mut Vec<String>) -> io::Result<()> {
  for ent in fs::read_dir(dir)? {
    let ent = ent?;
    let p = ent.path();
    let name = ent.file_name();
    let name_str = name.to_string_lossy();
    if name_str.ends_with(".s0tmp") || name_str.starts_with(".git") {
      continue;
    }
    let ft = ent.file_type()?;
    if ft.is_dir() {
      collect_files_rec(root, &p, out)?;
    } else if ft.is_file()
      && let Ok(rel) = p.strip_prefix(root)
    {
      let s = rel.to_string_lossy().replace('\\', "/");
      out.push(s);
    }
  }
  Ok(())
}

/// 从源目录按全局块号 `block_idx` 无锁读取对应分块内容
pub struct SourceReader {
  root: PathBuf,
  manifest: Manifest,
  block_map: Vec<BlockLoc>,
  files: Vec<Option<Arc<File>>>,
}

impl SourceReader {
  pub fn open(root: &Path, manifest: Manifest) -> Result<Self, FsError> {
    let block_map = manifest.build_block_map();
    let mut files = Vec::with_capacity(manifest.entries.len());
    for e in &manifest.entries {
      if e.deleted || e.size == 0 {
        files.push(None);
      } else {
        let p = if root.is_file() {
          root.to_path_buf()
        } else {
          root.join(&e.path)
        };
        files.push(Some(Arc::new(File::open(p)?)));
      }
    }
    Ok(Self {
      root: root.to_path_buf(),
      manifest,
      block_map,
      files,
    })
  }

  #[inline]
  pub fn manifest(&self) -> &Manifest {
    &self.manifest
  }

  #[inline]
  pub fn root(&self) -> &Path {
    &self.root
  }

  #[inline]
  pub fn total_blocks(&self) -> u32 {
    self.block_map.len() as u32
  }

  /// 读取全局 `block_idx` 的原始字节到 `buf`
  pub fn read_block(&self, block_idx: u32, buf: &mut Vec<u8>) -> Result<(), FsError> {
    let loc = self
      .block_map
      .get(block_idx as usize)
      .copied()
      .ok_or(FsError::BlockCorrupted(block_idx))?;
    let f = self
      .files
      .get(loc.entry_idx as usize)
      .and_then(|o| o.as_ref())
      .ok_or(FsError::BlockCorrupted(block_idx))?;
    buf.resize(loc.len as usize, 0);
    f.read_exact_at_pos(buf, loc.offset)?;
    Ok(())
  }
}

/// 支持多个对等节点并发无锁写入同一套 `.s0tmp` 暂存文件的接收池 (`SharedSink`)
pub struct SharedSink {
  root: PathBuf,
  manifest: Manifest,
  block_map: Vec<BlockLoc>,
  staging_files: Vec<Option<Arc<File>>>,
  have_words: Vec<AtomicU64>,
  initial_have_bits: Vec<u8>,
}

impl SharedSink {
  /// 初始化接收池：对本地已有同名文件执行流式 BLAKE3 基准匹配，复用相同块并预分配 `.s0tmp`
  pub fn prepare(root: &Path, manifest: Manifest) -> Result<Self, FsError> {
    fs::create_dir_all(root)?;
    let block_map = manifest.build_block_map();
    let total_blocks = block_map.len() as u32;
    let bit_bytes = (total_blocks as usize).div_ceil(8);
    let word_cnt = (total_blocks as usize).div_ceil(64);
    let mut initial_have_bits = vec![0u8; bit_bytes];

    let mut staging_files = Vec::with_capacity(manifest.entries.len());
    let mut base_blk = 0u32;
    let mut io_buf = vec![0u8; 64 * 1024];

    for e in &manifest.entries {
      let clean_rel = sanitize_rel_path(&e.path)?;
      if e.deleted {
        staging_files.push(None);
        continue;
      }
      let final_path = root.join(&clean_rel);
      if let Some(parent) = final_path.parent() {
        fs::create_dir_all(parent)?;
      }
      if e.size == 0 {
        staging_files.push(None);
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

      // 若本地已存在基准文件，流式比对块摘要并直接拷贝命中块至 `.s0tmp`
      if let Ok(basis) = File::open(&final_path)
        && let Ok(meta) = basis.metadata()
      {
        let basis_len = meta.len();
        for (ci, chunk) in e.chunks.iter().enumerate() {
          let end = chunk.offset.saturating_add(u64::from(chunk.len));
          if end <= basis_len {
            let n = chunk.len as usize;
            if io_buf.len() < n {
              io_buf.resize(n, 0);
            }
            if basis
              .read_exact_at_pos(&mut io_buf[..n], chunk.offset)
              .is_ok()
              && hash128(&io_buf[..n]) == chunk.hash
            {
              tmp_file.write_all_at_pos(&io_buf[..n], chunk.offset)?;
              set_bit(&mut initial_have_bits, base_blk + ci as u32);
            }
          }
        }
      }

      base_blk += e.chunks.len() as u32;
      staging_files.push(Some(Arc::new(tmp_file)));
    }

    let mut have_words = Vec::with_capacity(word_cnt);
    for w_idx in 0..word_cnt {
      let mut word = 0u64;
      for b_idx in 0..8 {
        if let Some(&byte) = initial_have_bits.get(w_idx * 8 + b_idx) {
          word |= u64::from(byte) << (b_idx * 8);
        }
      }
      have_words.push(AtomicU64::new(word));
    }

    Ok(Self {
      root: root.to_path_buf(),
      manifest,
      block_map,
      staging_files,
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
  pub fn total_blocks(&self) -> u32 {
    self.block_map.len() as u32
  }

  #[inline]
  pub fn initial_have_bits(&self) -> &[u8] {
    &self.initial_have_bits
  }

  /// 导出区间 `[blk_lo, blk_hi)` 当前最新的位图快照
  pub fn snapshot_have_bits(&self) -> Vec<u8> {
    let total = self.total_blocks() as usize;
    let mut out = vec![0u8; total.div_ceil(8)];
    for (wi, atom) in self.have_words.iter().enumerate() {
      let w = atom.load(Ordering::Acquire);
      let bytes = w.to_le_bytes();
      for (bi, &b) in bytes.iter().enumerate() {
        if let Some(slot) = out.get_mut(wi * 8 + bi) {
          *slot = b;
        }
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

  /// 判断区间 `[blk_lo, blk_hi)` 内所有块是否全部到位
  pub fn is_range_complete(&self, blk_lo: u32, blk_hi: u32) -> bool {
    let hi = blk_hi.min(self.total_blocks());
    let mut cur = blk_lo;
    while cur < hi {
      if (cur & 63) == 0 && cur + 64 <= hi {
        let wi = (cur >> 6) as usize;
        if self
          .have_words
          .get(wi)
          .is_none_or(|w| w.load(Ordering::Acquire) != u64::MAX)
        {
          return false;
        }
        cur += 64;
      } else {
        if !self.has_block(cur) {
          return false;
        }
        cur += 1;
      }
    }
    true
  }

  /// 校验收到块的 128-bit BLAKE3 摘要并无锁原位写入 `.s0tmp`（多线程并发安全）
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
    let f = self
      .staging_files
      .get(loc.entry_idx as usize)
      .and_then(|o| o.as_ref())
      .ok_or(FsError::BlockCorrupted(block_idx))?;
    f.write_all_at_pos(data, loc.offset)?;

    let wi = (block_idx >> 6) as usize;
    let bit = 1u64 << (block_idx & 63);
    let prev = self.have_words[wi].fetch_or(bit, Ordering::Release);
    Ok((prev & bit) == 0)
  }

  /// 校验全部块到位后，将所有 `.s0tmp` 原子重命名为最终文件并执行删除清理
  pub fn commit(&self) -> Result<(), FsError> {
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
    if done != total {
      return Err(FsError::IncompleteCommit(done, total));
    }

    let mut valid_paths = HashSet::default();
    for e in &self.manifest.entries {
      let clean_rel = sanitize_rel_path(&e.path)?;
      let final_path = self.root.join(&clean_rel);
      if e.deleted {
        let _ = fs::remove_file(&final_path);
        continue;
      }
      valid_paths.insert(clean_rel);
      if e.size == 0 {
        if let Some(parent) = final_path.parent() {
          fs::create_dir_all(parent)?;
        }
        File::create(&final_path)?;
      } else {
        let tmp_path = Self::tmp_path_for(&final_path);
        fs::rename(&tmp_path, &final_path)?;
      }
    }

    if self.manifest.delete_extraneous && !self.manifest.partial {
      let mut existing = Vec::new();
      collect_files_rec(&self.root, &self.root, &mut existing)?;
      for rel in existing {
        if !valid_paths.contains(&rel) {
          let _ = fs::remove_file(self.root.join(&rel));
        }
      }
    }
    Ok(())
  }
}

/// 辅助函数：检查本地文件的块位图是否全部命中
pub fn is_all_have(have_bits: &[u8], total_blocks: u32) -> bool {
  let full_bytes = (total_blocks >> 3) as usize;
  let rem_bits = total_blocks & 7;
  if have_bits.len() < full_bytes + usize::from(rem_bits > 0) {
    return false;
  }
  if !have_bits[..full_bytes].iter().all(|&b| b == 0xFF) {
    return false;
  }
  if rem_bits > 0 {
    let mask = (1u8 << rem_bits) - 1;
    if (have_bits[full_bytes] & mask) != mask {
      return false;
    }
  }
  true
}
