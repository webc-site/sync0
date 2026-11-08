use std::{cmp::Ordering, fs, path::Path};

use ignore::WalkBuilder;

use super::{
  FsError, PathErr,
  path::{PART_EXT, TMP_EXT, remove, to_slash, to_slash_cow},
};

/// 同步忽略规则文件名（兼容 `.gitignore` 语法）
pub const IGNORE_FILE: &str = ".sync.ignore";
const STAGE_PREFIX: &str = ".s0_stage.";
const PACK_TMP: &str = ".s0_pack.tmp";

/// 扫描出的普通文件条目
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScannedFile {
  pub path: String,
  pub size: u64,
}

pub fn is_staging(path: &Path) -> bool {
  path.file_name().is_some_and(|n| {
    let b = n.as_encoded_bytes();
    b == IGNORE_FILE.as_bytes()
      || b == PACK_TMP.as_bytes()
      || b.starts_with(STAGE_PREFIX.as_bytes())
      || b.ends_with(TMP_EXT.as_bytes())
      || b.ends_with(PART_EXT.as_bytes())
  })
}

fn walk_builder(root: &Path) -> WalkBuilder {
  let mut w = WalkBuilder::new(root);
  w.standard_filters(false)
    .follow_links(false)
    .add_custom_ignore_filename(IGNORE_FILE)
    .filter_entry(|e| !is_staging(e.path()));
  w
}

/// 递归扫描目录（自动应用各层级 `.sync.ignore` 与暂存文件过滤），按相对路径升序返回
pub fn scan_files(root: &Path) -> Result<Vec<ScannedFile>, FsError> {
  scan_files_in(root, root)
}

/// 递归扫描 `root` 下的指定子目录 `target`，返回相对于 `root` 的有序文件列表
pub fn scan_files_in(root: &Path, target: &Path) -> Result<Vec<ScannedFile>, FsError> {
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
    let size = entry.metadata()?.len();
    out.push(ScannedFile { path, size });
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
