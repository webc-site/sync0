mod cache;
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
  thread::{self, available_parallelism},
};

pub use cache::{SmallCache, scan_and_cache_files, take_or_clone_small_cache};
pub use filter::{IGNORE_FILE, ScannedFile, is_staging, prune_dir, scan_files, scan_files_in};
use ignore::Error as IgnoreError;
pub use path::{
  MAX_PATH, PathErr, Staging, create_trunc, dir_has_entries, open_nonempty_file, open_rw, rel_path,
  remove, safe_join,
};
pub use s0_cdc::is_all_have;
use s0_cdc::{CdcConfig, CdcError, CopySpan, Manifest, ManifestEntry, chunk_file, copy_file_span};
pub use sink::SharedSink;
pub use source::{BlockSource, SourceReader};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FsError {
  #[error("IO 错误: {0}")]
  Io(#[from] io::Error),
  #[error("CDC 清单错误: {0}")]
  Cdc(#[from] CdcError),
  #[error("目录遍历/忽略规则错误: {0}")]
  Ignore(#[from] IgnoreError),
  #[error("块 {0} gxhash128 摘要校验不匹配或长度非法")]
  BlockCorrupted(u32),
  #[error("尚有未完成落盘的块 ({0}/{1})，拒绝原子提交")]
  IncompleteCommit(u32, u32),
  #[error("非法相对路径 ({0:?})")]
  BadPath(PathErr),
}

/// 计算并行分片线程数 `[1, 8]` 与每片长度
#[inline]
pub fn par_chunk_plan(len: usize) -> (usize, usize) {
  let n_threads = available_parallelism()
    .map_or(4, |n| n.get())
    .clamp(1, 8)
    .min(len.div_ceil(8).max(1));
  (n_threads, len.div_ceil(n_threads))
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
      match h.join() {
        Ok(Ok(())) => {}
        Ok(Err(e)) if first_err.is_none() => first_err = Some(e),
        Err(_) if first_err.is_none() => {
          first_err = Some(io::Error::other("parallel chunk worker panic"));
        }
        _ => {}
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
    let mut buf = PathBuf::with_capacity(root.as_os_str().len() + 64);
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

/// 批量按偏移区间拷贝本地基准文件块到目标暂存文件
pub fn copy_file_spans(
  src: &File,
  dst: &File,
  spans: &[CopySpan],
  buf: &mut Vec<u8>,
) -> Result<(), CdcError> {
  if spans.is_empty() {
    return Ok(());
  }
  buf.resize(buf.len().max(64 * 1024), 0);
  for sp in spans {
    copy_file_span(src, sp.src_off, dst, sp.dst_off, sp.len, buf)?;
  }
  Ok(())
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

/// 递归扫描本地目录或单文件并构建 `Manifest`（支持指定子路径 `subpath`，复用单块 1MiB I/O 缓冲并遵循 `.sync.ignore`）
pub fn build_manifest_from_path(
  root: &Path,
  subpath: &str,
  delete_extraneous: bool,
) -> Result<Manifest, FsError> {
  let clean_sub = sanitize_rel_path(subpath)?;
  if clean_sub.is_empty() {
    build_manifest_from_paths::<&str>(root, &[], delete_extraneous)
  } else {
    build_manifest_from_paths(root, &[clean_sub.as_str()], delete_extraneous)
  }
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
    let size = f.metadata()?.len();
    let chunks = chunk_file(&f, size, CdcConfig::DEFAULT)?;
    let rel_name = if let Some(first) = clean_sels.first() {
      first.clone()
    } else {
      root
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_owned()
    };
    return Ok(Manifest {
      entries: vec![ManifestEntry {
        path: rel_name,
        size,
        chunks,
        deleted: false,
      }],
      delete_extraneous: false,
      partial: true,
    });
  }

  if clean_sels.is_empty() {
    if !root.exists() {
      return Err(FsError::Io(io::Error::from(io::ErrorKind::NotFound)));
    }
    let scanned = scan_files_in(root, root)?;
    let entries = chunk_scanned_entries(root, scanned)?;
    return Ok(Manifest {
      entries,
      delete_extraneous,
      partial: false,
    });
  }

  let mut scanned = Vec::new();
  for sel in &clean_sels {
    let target = root.join(sel);
    if !target.exists() {
      return Err(FsError::Io(io::Error::from(io::ErrorKind::NotFound)));
    }
    if target.is_file() {
      let size = target.metadata()?.len();
      scanned.push(ScannedFile {
        path: sel.clone(),
        size,
      });
    } else {
      let mut sub_files = scan_files_in(root, &target)?;
      scanned.append(&mut sub_files);
    }
  }
  scanned.sort_unstable_by(|a, b| a.path.cmp(&b.path));
  scanned.dedup_by(|a, b| a.path == b.path);

  let entries = chunk_scanned_entries(root, scanned)?;
  Ok(Manifest {
    entries,
    delete_extraneous: false,
    partial: true,
  })
}

fn chunk_scanned_entries(
  root: &Path,
  scanned: Vec<ScannedFile>,
) -> Result<Vec<ManifestEntry>, FsError> {
  let (all_chunks, _cache) = scan_and_cache_files(root, &scanned)?;
  Ok(
    scanned
      .into_iter()
      .zip(all_chunks)
      .map(|(sf, chunks)| ManifestEntry {
        path: sf.path,
        size: sf.size,
        chunks,
        deleted: false,
      })
      .collect(),
  )
}
