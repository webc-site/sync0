use std::{
  cmp::Ordering,
  fs,
  path::{Path, PathBuf},
};

use ignore::{
  WalkBuilder,
  gitignore::{Gitignore, GitignoreBuilder},
};

use crate::{
  FsError,
  path::{PART_EXT, TMP_EXT, remove, to_slash, to_slash_cow},
};

/// 同步忽略规则文件名（兼容 `.gitignore` 语法）
pub const IGNORE_FILE: &str = ".sync.ignore";
const TMP_GLOB: &str = "*.s0tmp";
const PART_GLOB: &str = "*.s0part";

/// 扫描出的普通文件条目
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScannedFile {
  pub path: String,
  pub size: u64,
}

/// `.sync.ignore` 过滤器
#[derive(Clone, Debug)]
pub struct Ignore {
  root: PathBuf,
  gi: Gitignore,
}

impl Ignore {
  /// 从 `root` 目录加载 `.sync.ignore`（不存在则仅过滤 `.s0tmp`/`.s0part`）
  pub fn load(root: &Path) -> Result<Self, FsError> {
    let mut b = default_builder(root)?;
    let cfg = root.join(IGNORE_FILE);
    if cfg.is_file()
      && let Some(err) = b.add(cfg)
    {
      return Err(err.into());
    }
    Ok(Self {
      root: root.to_path_buf(),
      gi: b.build()?,
    })
  }

  /// 从文本规则构造过滤器
  pub fn from_rules(root: &Path, rules: &str) -> Result<Self, FsError> {
    let mut b = default_builder(root)?;
    for line in rules.lines() {
      b.add_line(None, line)?;
    }
    Ok(Self {
      root: root.to_path_buf(),
      gi: b.build()?,
    })
  }

  /// 判断路径（绝对或相对 `root`）是否被忽略
  pub fn is_ignored(&self, path: &Path, is_dir: bool) -> bool {
    if is_staging(path) {
      return true;
    }
    let rel = path.strip_prefix(&self.root).unwrap_or(path);
    if rel.as_os_str().is_empty() {
      return false;
    }
    self.gi.matched_path_or_any_parents(rel, is_dir).is_ignore()
  }
}

fn default_builder(root: &Path) -> Result<GitignoreBuilder, FsError> {
  let mut b = GitignoreBuilder::new(root);
  b.add_line(None, IGNORE_FILE)?;
  b.add_line(None, TMP_GLOB)?;
  b.add_line(None, PART_GLOB)?;
  Ok(b)
}

pub fn is_staging(path: &Path) -> bool {
  path.file_name().is_some_and(|n| {
    let b = n.as_encoded_bytes();
    b == IGNORE_FILE.as_bytes()
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
  let mut out = Vec::new();
  for entry in walk_builder(root).build() {
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
      .map_err(|_| FsError::BadPath("路径前缀".to_owned()))?;
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
      .map_err(|_| FsError::BadPath("路径前缀".to_owned()))?;
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
