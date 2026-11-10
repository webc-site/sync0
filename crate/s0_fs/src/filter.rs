#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{
  cmp::Ordering,
  fs::{self, OpenOptions, Permissions},
  io,
  path::{Path, PathBuf},
  time::{Duration, UNIX_EPOCH},
};

use ignore::WalkBuilder;
use s0_cdc::{ChunkSig, ManifestEntry, norm_mode};
#[cfg(not(unix))]
use s0_const::DEFAULT_FILE_MODE;

use crate::{
  FsError, PathErr,
  path::{remove, to_slash, to_slash_cow},
  run_par_chunks,
};

/// 同步忽略规则文件名（兼容 `.gitignore` 语法）
const IGNORE_FILE: &str = ".sync.ignore";
pub(crate) const STAGE_PREFIX: &str = ".s0_stage.";
pub(crate) const PACK_TMP: &str = ".s0_pack.tmp";
/// 订阅者与基准快照持久化文件名（扫描与镜像清理时自动跳过）
pub const SUBS_FILE: &str = ".s0_subs.bin";

/// 提取文件元数据修改时间纳秒时间戳
#[inline]
pub(crate) fn meta_mtime_ns(meta: &fs::Metadata) -> u128 {
  meta
    .modified()
    .ok()
    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
    .map_or(0, |d| d.as_nanos())
}

/// 提取文件元数据毫秒级最后修改时间 (`mtime_ms`)
#[inline]
pub fn meta_mtime_ms(meta: &fs::Metadata) -> u64 {
  (meta_mtime_ns(meta) / 1_000_000) as u64
}

/// 提取文件 POSIX 权限位（保留低 9 位 `0o777`）
#[inline]
pub fn meta_mode(meta: &fs::Metadata) -> u16 {
  #[cfg(unix)]
  {
    norm_mode(meta.permissions().mode() as u16)
  }
  #[cfg(not(unix))]
  {
    if meta.permissions().readonly() {
      0o444
    } else {
      DEFAULT_FILE_MODE
    }
  }
}

/// 设置目标文件 POSIX 权限位
#[inline]
fn set_file_mode(path: &Path, mode: u16) -> io::Result<()> {
  let m = norm_mode(mode);
  #[cfg(unix)]
  {
    fs::set_permissions(path, Permissions::from_mode(u32::from(m)))
  }
  #[cfg(not(unix))]
  {
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_readonly(m & 0o200 == 0);
    fs::set_permissions(path, perms)
  }
}

/// 同步设置目标文件的毫秒级最后修改时间 (`mtime`) 与 POSIX 权限位 (`mode`)
pub fn set_file_meta(path: &Path, mode: u16, mtime: u64) -> io::Result<()> {
  let m = norm_mode(mode);
  if mtime > 0 {
    let ts = UNIX_EPOCH + Duration::from_millis(mtime);
    if let Ok(f) = OpenOptions::new().write(true).open(path) {
      let _ = f.set_modified(ts);
    } else if m & 0o200 == 0 {
      let _ = set_file_mode(path, m | 0o200);
      if let Ok(f) = OpenOptions::new().write(true).open(path) {
        let _ = f.set_modified(ts);
      }
    }
  }
  set_file_mode(path, m)
}

/// 并发批量设置多个文件的毫秒级最后修改时间 (`mtime`) 与 POSIX 权限位 (`mode`)
pub fn apply_file_metas(jobs: &[(PathBuf, u16, u64)]) {
  let _ = run_par_chunks(jobs, |slice| {
    for (p, m, mt) in slice {
      let _ = set_file_meta(p, *m, *mt);
    }
    Ok(())
  });
}

/// 扫描出的普通文件条目（按相对路径升序排列，携带字节大小、纳秒级 `mtime_ns` 与权限 `mode` 用于 $O(N)$ 增量变更检测）
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScannedFile {
  pub path: String,
  pub size: u64,
  pub mtime_ns: u128,
  pub mode: u16,
}

impl ScannedFile {
  #[inline]
  pub(crate) fn from_meta(path: String, meta: &fs::Metadata) -> Self {
    Self {
      path,
      size: meta.len(),
      mtime_ns: meta_mtime_ns(meta),
      mode: meta_mode(meta),
    }
  }

  /// 毫秒级最后修改时间
  #[inline]
  pub const fn mtime_ms(&self) -> u64 {
    (self.mtime_ns / 1_000_000) as u64
  }

  #[inline]
  pub fn to_entry(&self, chunks: Vec<ChunkSig>) -> ManifestEntry {
    ManifestEntry {
      path: self.path.clone(),
      size: self.size,
      mode: self.mode,
      mtime: self.mtime_ms(),
      chunks,
      deleted: false,
    }
  }
}

/// 判断路径是否为内部暂存文件、包文件或订阅状态文件
#[inline]
pub fn is_staging_path(path: &Path) -> bool {
  path.file_name().is_some_and(|n| {
    let b = n.as_encoded_bytes();
    b == IGNORE_FILE.as_bytes()
      || b == PACK_TMP.as_bytes()
      || b == SUBS_FILE.as_bytes()
      || b.starts_with(STAGE_PREFIX.as_bytes())
  })
}

fn walk_builder(root: &Path) -> WalkBuilder {
  let mut w = WalkBuilder::new(root);
  w.standard_filters(false)
    .follow_links(false)
    .add_custom_ignore_filename(IGNORE_FILE)
    .filter_entry(|e| !is_staging_path(e.path()));
  w
}

/// 递归扫描目录（自动应用各层级 `.sync.ignore` 与暂存文件过滤），按相对路径升序返回
pub fn scan_files(root: &Path) -> Result<Vec<ScannedFile>, FsError> {
  scan_files_in(root, root)
}

/// 递归扫描 `root` 下的指定子目录 `target`，返回相对于 `root` 的有序文件列表
pub(crate) fn scan_files_in(root: &Path, target: &Path) -> Result<Vec<ScannedFile>, FsError> {
  let mut out = Vec::new();
  for entry in walk_builder(target).build() {
    let entry = entry?;
    let Some(ft) = entry.file_type() else {
      continue;
    };
    if !ft.is_file() {
      continue;
    }
    let p = entry.path();
    let rel = p
      .strip_prefix(root)
      .map_err(|_| FsError::BadPath(PathErr::InvalidSegment))?;
    let path = to_slash(rel);
    let meta = entry.metadata()?;
    out.push(ScannedFile::from_meta(path, &meta));
  }
  out.sort_unstable_by(|a, b| a.path.cmp(&b.path));
  Ok(out)
}

fn cmp_dir_prefix(k: &str, dir: &str) -> Ordering {
  let kb = k.as_bytes();
  let db = dir.as_bytes();
  if kb.len() <= db.len() {
    kb.cmp(db).then(Ordering::Less)
  } else {
    kb[..db.len()].cmp(db).then_with(|| kb[db.len()].cmp(&b'/'))
  }
}

/// 清理目标目录 `root` 中不在 `keep_sorted`（按 `/` 相对路径升序排列）内的多余文件与空子目录
pub fn prune_dir<S: AsRef<str>>(root: &Path, keep_sorted: &[S]) -> Result<u32, FsError> {
  if !root.is_dir() {
    return Ok(0);
  }
  let mut removed = 0u32;
  let mut dirs = Vec::new();
  for entry in walk_builder(root).build() {
    let entry = entry?;
    if entry.depth() == 0 {
      continue;
    }
    let Some(ft) = entry.file_type() else {
      continue;
    };
    let p = entry.path();
    let rel = p
      .strip_prefix(root)
      .map_err(|_| FsError::BadPath(PathErr::InvalidSegment))?;
    let slash = to_slash_cow(rel);
    if ft.is_dir() {
      let idx = keep_sorted.partition_point(|k| cmp_dir_prefix(k.as_ref(), &slash).is_lt());
      let has_kept = keep_sorted
        .get(idx)
        .is_some_and(|k| cmp_dir_prefix(k.as_ref(), &slash).is_eq());
      if !has_kept {
        dirs.push(entry.into_path());
      }
    } else if keep_sorted
      .binary_search_by(|k| k.as_ref().cmp(slash.as_ref()))
      .is_err()
    {
      remove(p)?;
      removed = removed.saturating_add(1);
    }
  }
  for d in dirs.into_iter().rev() {
    let _ = fs::remove_dir(d);
  }
  Ok(removed)
}
