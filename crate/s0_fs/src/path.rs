use std::{
  borrow::Cow,
  ffi::OsString,
  fmt::Write,
  fs::{self, File, OpenOptions},
  io::{self, ErrorKind},
  path::{Path, PathBuf},
  str,
};

use gxhash::gxhash64;
use s0_const::U15_MAX;

use crate::{FsError, RootBuf, filter::STAGE_PREFIX, run_par_chunks};

/// 单个文件名最大字节数
const NAME_MAX: usize = 255;
/// 启用多线程并发建目录的最小子目录数阈值
const PAR_MKDIR_MIN_DIRS: usize = 64;

/// 预创建相对路径列表对应的所有父目录（目录数 `>= 64` 时自动启用多线程并发创建）
pub fn ensure_parent_dirs<'a>(
  root: &Path,
  rel_paths: impl IntoIterator<Item = &'a str>,
) -> io::Result<()> {
  fs::create_dir_all(root)?;
  let mut dirs: Vec<&Path> = Vec::new();
  for p in rel_paths {
    if let Some(dir) = Path::new(p).parent()
      && !dir.as_os_str().is_empty()
      && dirs.last().copied() != Some(dir)
    {
      dirs.push(dir);
    }
  }
  if dirs.len() >= PAR_MKDIR_MIN_DIRS {
    run_par_chunks(&dirs, |slice| {
      let mut rb = RootBuf::new(root);
      for &dir in slice {
        fs::create_dir_all(rb.join(dir))?;
      }
      Ok(())
    })
  } else {
    let mut rb = RootBuf::new(root);
    for dir in dirs {
      fs::create_dir_all(rb.join(dir))?;
    }
    Ok(())
  }
}

/// 将相对路径转为 `/` 分隔的借用/拥有 UTF-8 字符串（Unix 下零堆分配借用）
pub(crate) fn to_slash_cow(rel: &Path) -> Cow<'_, str> {
  let s = rel.to_string_lossy();
  if s.contains('\\') {
    Cow::Owned(s.replace('\\', "/"))
  } else {
    s
  }
}

/// 将相对路径转为 `/` 分隔的 UTF-8 字符串（Unix 下零额外分配）
pub(crate) fn to_slash(rel: &Path) -> String {
  to_slash_cow(rel).into_owned()
}

/// 路径校验错误码（数值枚举，避免字符串堆分配）
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathErr {
  EmptyOrTooLong = 1,
  InvalidUtf8 = 2,
  InvalidSegment = 3,
}

/// 校验协议相对路径：UTF-8、非空、非绝对、无空段 / `.` / `..` / `\` / `:` / NUL
pub fn rel_path(b: &[u8]) -> Result<&str, FsError> {
  if b.is_empty() || b.len() > U15_MAX {
    return Err(FsError::BadPath(PathErr::EmptyOrTooLong));
  }
  let s = str::from_utf8(b).map_err(|_| FsError::BadPath(PathErr::InvalidUtf8))?;
  if s
    .split('/')
    .any(|c| matches!(c, "" | "." | "..") || c.contains(['\0', '\\', ':']))
  {
    return Err(FsError::BadPath(PathErr::InvalidSegment));
  }
  Ok(s)
}

/// 生成目标文件的同目录原子重命名暂存路径 `.s0_stage.<filename>`（超长文件名自动退化为 `gxhash64` 短名）
pub fn stage_path(final_path: &Path) -> PathBuf {
  let mut s = OsString::from(STAGE_PREFIX);
  if let Some(name) = final_path.file_name() {
    if STAGE_PREFIX.len() + name.len() > NAME_MAX {
      let _ = write!(s, "{:016x}", gxhash64(name.as_encoded_bytes(), 0));
    } else {
      s.push(name);
    }
  }
  match final_path.parent() {
    Some(p) if !p.as_os_str().is_empty() => p.join(s),
    _ => PathBuf::from(s),
  }
}

/// 删除文件，不存在视为成功
pub fn remove(p: &Path) -> Result<(), FsError> {
  match fs::remove_file(p) {
    Err(e) if e.kind() != ErrorKind::NotFound => Err(e.into()),
    _ => Ok(()),
  }
}

/// 删除相对于 `root` 的目标文件或目录，并自底向上清理直到 `root` 的空父目录
pub fn remove_tree_and_empty_parents(root: &Path, clean_rel: &str) -> Result<(), FsError> {
  let target = root.join(clean_rel);
  if target.is_dir() {
    match fs::remove_dir_all(&target) {
      Err(e) if e.kind() != ErrorKind::NotFound => return Err(e.into()),
      _ => {}
    }
  } else {
    remove(&target)?;
  }
  let mut cur = target.parent();
  while let Some(dir) = cur {
    if dir == root || !dir.starts_with(root) || fs::remove_dir(dir).is_err() {
      break;
    }
    cur = dir.parent();
  }
  Ok(())
}

/// 以 `create + truncate + write` 打开文件用于截断写入
pub fn create_trunc(path: &Path) -> io::Result<File> {
  OpenOptions::new()
    .create(true)
    .truncate(true)
    .write(true)
    .open(path)
}

/// 以 `create + read + write` 打开文件并确保文件长度不小于 `min_len`
pub fn open_rw(path: &Path, truncate: bool, min_len: u64) -> io::Result<File> {
  let f = OpenOptions::new()
    .create(true)
    .truncate(truncate)
    .read(true)
    .write(true)
    .open(path)?;
  if truncate || !f.metadata().is_ok_and(|m| m.len() >= min_len) {
    f.set_len(min_len)?;
  }
  Ok(f)
}

/// 打开已存在的非空普通文件并返回 `(File, len)`
#[inline]
pub fn open_nonempty_file(path: &Path) -> Option<(File, u64)> {
  let f = File::open(path).ok()?;
  let m = f.metadata().ok()?;
  (m.is_file() && m.len() > 0).then_some((f, m.len()))
}

/// 判断目录是否存在且至少包含一个条目（空目录视同全新目标，避免全量同步时触发数十万次无效 `open`）
#[inline]
pub fn dir_has_entries(dir: &Path) -> bool {
  fs::read_dir(dir).is_ok_and(|mut it| it.next().is_some())
}
