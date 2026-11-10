mod cache;
mod compio_io;
mod fd_cache;
mod filter;
mod pack;
mod path;
mod sink;
mod source;

use std::{
  fs::File,
  io,
  path::{Path, PathBuf},
  thread,
  time::{SystemTime, UNIX_EPOCH},
};

pub use cache::{
  PAR_SCAN_MIN_FILES, SinglePrecheck, SmallCache, build_dir_basis, check_single_chunk,
  precheck_single_chunks, scan_and_cache_files,
};
pub use compio_io::{SmallBatchFlusher, SmallWriteJob, flush_memory_slices};
use filter::scan_files_in;
pub use filter::{
  SUBS_FILE, ScannedFile, apply_file_metas, is_staging_path, meta_mode, meta_mtime_ms, prune_dir,
  scan_files, set_file_meta,
};
use ignore::Error as IgnoreError;
pub use pack::{AtomicBitset, DisjointArena};
pub use path::{
  PathErr, create_trunc, dir_has_entries, ensure_parent_dirs, open_nonempty_file, open_rw,
  rel_path, remove, remove_tree_and_empty_parents, stage_path,
};
use s0_cdc::{
  CdcConfig, MAX_PAR_THREADS, Manifest, ManifestEntry, chunk_file, join_io, max_par_threads,
};
pub use sink::SharedSink;
pub use source::{BlockSource, SourceReader};
use thiserror::Error;

/// `RootBuf` 预留相对路径拼接容量
const ROOT_BUF_EXTRA_CAP: usize = 64;
/// 增量扫描免重算小文件尺寸阈值（64 KiB）
const INC_STABLE_SIZE_BYTES: u64 = 64 * 1024;
/// 增量扫描小文件 `mtime` 稳定防抖窗口（50 ms = 50,000,000 ns）
const INC_MTIME_GUARD_NS: u128 = 50_000_000;

#[derive(Debug, Error)]
pub enum FsError {
  #[error(transparent)]
  Io(#[from] io::Error),
  #[error(transparent)]
  Ignore(#[from] IgnoreError),
  #[error("块 {0} gxhash128 摘要校验不匹配或长度非法")]
  BlockCorrupted(u32),
  #[error("尚有未完成落盘的块 ({0}/{1})，拒绝原子提交")]
  IncompleteCommit(u32, u32),
  #[error("非法相对路径 ({0:?})")]
  BadPath(PathErr),
}

/// 计算并行分片线程数 `[1, MAX_PAR_THREADS]` 与每片长度
#[inline]
pub fn par_chunk_plan(len: usize) -> (usize, usize) {
  let n_threads = max_par_threads().min(len.div_ceil(MAX_PAR_THREADS).max(1));
  (n_threads, len.div_ceil(n_threads).max(1))
}

/// 将切片按 `par_chunk_plan` 切分并在 `thread::scope` 中并发执行
pub fn run_par_chunks<T, F>(items: &[T], worker: F) -> io::Result<()>
where
  T: Sync,
  F: Fn(&[T]) -> io::Result<()> + Sync,
{
  if items.is_empty() {
    return Ok(());
  }
  let (workers, chunk_sz) = par_chunk_plan(items.len());
  if workers <= 1 {
    return worker(items);
  }
  let mut first_err: Option<io::Error> = None;
  thread::scope(|s| {
    let handles: Vec<_> = items
      .chunks(chunk_sz)
      .map(|slice| s.spawn(|| worker(slice)))
      .collect();
    for h in handles {
      if let Err(e) = join_io(h.join())
        && first_err.is_none()
      {
        first_err = Some(e);
      }
    }
  });
  first_err.map_or(Ok(()), Err)
}

/// 复用单块堆内存的根目录路径拼接器，消除海量小文件热路径 `Path::join` 重复堆分配
pub struct RootBuf {
  buf: PathBuf,
  base_len: usize,
}

impl RootBuf {
  pub fn new(root: &Path) -> Self {
    let mut buf = PathBuf::with_capacity(root.as_os_str().len() + ROOT_BUF_EXTRA_CAP);
    buf.push(root);
    buf.push("x");
    buf.pop();
    let base_len = buf.as_os_str().len();
    Self { buf, base_len }
  }

  pub fn join(&mut self, rel: impl AsRef<Path>) -> &Path {
    while self.buf.as_os_str().len() > self.base_len {
      if !self.buf.pop() {
        break;
      }
    }
    self.buf.push(rel);
    &self.buf
  }
}

/// 校验相对路径安全性（防止 `..` 路径穿越，复用 `rel_path` 规则）
pub fn sanitize_rel_path(rel: &str) -> Result<String, FsError> {
  let trimmed = rel.trim_matches(['/', '\\']);
  if trimmed.is_empty() {
    return Ok(String::new());
  }
  let normalized = if trimmed.contains('\\') {
    trimmed.replace('\\', "/")
  } else {
    trimmed.to_owned()
  };
  rel_path(normalized.as_bytes())?;
  Ok(normalized)
}

/// 按多选路径列表（支持全量 `&[]` 或同时指定多个单文件与子目录）构建 `Manifest`
pub fn build_manifest_from_paths<S: AsRef<str>>(
  root: &Path,
  paths: &[S],
  delete_extraneous: bool,
) -> Result<Manifest, FsError> {
  let mut clean_sels = Vec::with_capacity(paths.len());
  for p in paths {
    let c = sanitize_rel_path(p.as_ref())?;
    if !c.is_empty() {
      clean_sels.push(c);
    }
  }
  clean_sels.sort_unstable();
  clean_sels.dedup();

  if root.is_file() {
    let f = File::open(root)?;
    let meta = f.metadata()?;
    let size = meta.len();
    let mode = meta_mode(&meta);
    let mtime = meta_mtime_ms(&meta);
    let chunks = chunk_file(&f, size, CdcConfig::DEFAULT)?;
    let rel_name = clean_sels.into_iter().next().unwrap_or_else(|| {
      root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_owned()
    });
    return Ok(Manifest {
      entries: vec![ManifestEntry {
        path: rel_name,
        size,
        mode,
        mtime,
        chunks,
        deleted: false,
      }],
      delete_extraneous: false,
      partial: true,
    });
  }

  if !root.exists() {
    return Err(FsError::Io(io::Error::from(io::ErrorKind::NotFound)));
  }
  if clean_sels.is_empty() {
    let scanned = scan_files_in(root, root)?;
    let entries = chunk_scanned_entries(root, &scanned)?;
    return Ok(Manifest {
      entries,
      delete_extraneous,
      partial: false,
    });
  }

  let mut scanned = Vec::new();
  for sel in clean_sels {
    let target = root.join(&sel);
    let Ok(meta) = target.metadata() else {
      continue;
    };
    if meta.is_file() {
      scanned.push(ScannedFile::from_meta(sel, &meta));
    } else {
      let mut sub_files = scan_files_in(root, &target)?;
      scanned.append(&mut sub_files);
    }
  }
  scanned.sort_unstable_by(|a, b| a.path.cmp(&b.path));
  scanned.dedup_by(|a, b| a.path == b.path);

  let entries = chunk_scanned_entries(root, &scanned)?;
  Ok(Manifest {
    entries,
    delete_extraneous: false,
    partial: true,
  })
}

/// 基于上一轮基准 `(prev_manifest, prev_scanned)` 的 $O(N)$ 增量目录扫描：
/// 对 `(size, mtime_ns)` 未发生变化的已有文件直接复用 `ManifestEntry` 分块签名（若仅 `mode` 改变则零磁盘读取更新 `mode`），仅对新增或内容修改的文件执行 FastCDC 切块
pub fn build_manifest_incremental(
  root: &Path,
  prev: Option<(&Manifest, &[ScannedFile])>,
) -> Result<(Manifest, Vec<ScannedFile>), FsError> {
  if !root.exists() {
    return Err(FsError::Io(io::Error::from(io::ErrorKind::NotFound)));
  }
  let scanned = scan_files_in(root, root)?;
  let Some((prev_mf, prev_scanned)) = prev.filter(|(m, s)| m.entries.len() == s.len()) else {
    let entries = chunk_scanned_entries(root, &scanned)?;
    return Ok((
      Manifest {
        entries,
        delete_extraneous: false,
        partial: false,
      },
      scanned,
    ));
  };

  let now_ns = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |d| d.as_nanos());
  let mut entries: Vec<ManifestEntry> = Vec::with_capacity(scanned.len());
  let mut dirty_indices = Vec::new();
  let mut dirty_files = Vec::new();
  let mut j = 0usize;

  for (i, sf) in scanned.iter().enumerate() {
    while j < prev_scanned.len() && prev_scanned[j].path < sf.path {
      j += 1;
    }
    if let Some(prev_sf) = prev_scanned.get(j)
      && prev_sf.path == sf.path
      && prev_sf.size == sf.size
      && prev_sf.mtime_ns == sf.mtime_ns
      && sf.mtime_ns > 0
      && (prev_sf.mode != sf.mode
        || sf.size > INC_STABLE_SIZE_BYTES
        || !sf.mtime_ns.is_multiple_of(1_000_000_000)
        || now_ns.saturating_sub(sf.mtime_ns) > INC_MTIME_GUARD_NS)
    {
      let mut entry = prev_mf.entries[j].clone();
      entry.mode = sf.mode;
      entry.mtime = sf.mtime_ms();
      entries.push(entry);
    } else {
      dirty_indices.push(i);
      dirty_files.push(sf.clone());
      entries.push(sf.to_entry(Vec::new()));
    }
  }

  if !dirty_files.is_empty() {
    let (dirty_chunks, _) = scan_and_cache_files(root, &dirty_files)?;
    for (idx, chunks) in dirty_indices.into_iter().zip(dirty_chunks) {
      entries[idx].chunks = chunks;
    }
  }

  Ok((
    Manifest {
      entries,
      delete_extraneous: false,
      partial: false,
    },
    scanned,
  ))
}

fn chunk_scanned_entries(
  root: &Path,
  scanned: &[ScannedFile],
) -> Result<Vec<ManifestEntry>, FsError> {
  let (all_chunks, _cache) = scan_and_cache_files(root, scanned)?;
  Ok(
    scanned
      .iter()
      .zip(all_chunks)
      .map(|(sf, chunks)| sf.to_entry(chunks))
      .collect(),
  )
}
