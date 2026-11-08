use std::{
  borrow::Cow,
  ffi::{OsStr, OsString},
  fmt::Write,
  fs::{self, File, OpenOptions},
  io::{self, ErrorKind},
  path::{Path, PathBuf},
  str,
};

use gxhash::gxhash64;

use super::FsError;

/// 协议路径最大字节数
pub const MAX_PATH: usize = 32_767;
/// 单个文件名最大字节数
const NAME_MAX: usize = 255;
pub(crate) const TMP_EXT: &str = ".s0tmp";
pub(crate) const PART_EXT: &str = ".s0part";

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
  Symlink = 4,
  ParentNotDir = 5,
  MissingFileName = 6,
}

fn rel_path_str(b: &[u8]) -> Result<&str, FsError> {
  if b.is_empty() || b.len() > MAX_PATH {
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

/// 校验协议相对路径：UTF-8、非空、非绝对、无空段 / `.` / `..` / `\` / `:` / NUL
pub fn rel_path(b: &[u8]) -> Result<&Path, FsError> {
  rel_path_str(b).map(Path::new)
}

/// 在根目录 `root` 下安全解析相对路径 `rel`
pub fn safe_join(root: &Path, rel: &[u8]) -> Result<PathBuf, FsError> {
  let s = rel_path_str(rel)?;
  let mut cur = PathBuf::with_capacity(root.as_os_str().len() + 1 + s.len());
  cur.push(root);
  let mut it = s.split('/').peekable();
  let mut check_fs = true;
  while let Some(seg) = it.next() {
    cur.push(seg);
    if check_fs {
      match fs::symlink_metadata(&cur) {
        Ok(m) => {
          if m.file_type().is_symlink() {
            return Err(FsError::BadPath(PathErr::Symlink));
          }
          if it.peek().is_some() && !m.is_dir() {
            return Err(FsError::BadPath(PathErr::ParentNotDir));
          }
        }
        Err(e) if e.kind() == ErrorKind::NotFound => {
          check_fs = false;
        }
        Err(e) => return Err(e.into()),
      }
    }
  }
  Ok(cur)
}

/// 目标文件的暂存路径：与目标同目录，保证 rename 原子
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Staging {
  pub tmp: PathBuf,
  pub part: PathBuf,
}

impl Staging {
  pub fn of(path: &Path) -> Result<Self, FsError> {
    let name = path
      .file_name()
      .ok_or(FsError::BadPath(PathErr::MissingFileName))?;
    let dir = path.parent().unwrap_or(Path::new(""));
    Ok(Self {
      tmp: staged(dir, name, TMP_EXT),
      part: staged(dir, name, PART_EXT),
    })
  }

  /// 删除残留暂存文件（不存在视为成功）
  pub fn clean(&self) -> Result<(), FsError> {
    remove(&self.tmp)?;
    remove(&self.part)
  }
}

fn staged(dir: &Path, name: &OsStr, ext: &str) -> PathBuf {
  let mut s = OsString::with_capacity(1 + name.len().min(NAME_MAX) + ext.len());
  s.push(".");
  if 1 + name.len() + PART_EXT.len() > NAME_MAX {
    let _ = write!(s, "s0-{:016x}", gxhash64(name.as_encoded_bytes(), 0));
  } else {
    s.push(name);
  }
  s.push(ext);
  if dir.as_os_str().is_empty() {
    PathBuf::from(s)
  } else {
    dir.join(s)
  }
}

/// 删除文件，不存在视为成功
pub fn remove(p: &Path) -> Result<(), FsError> {
  match fs::remove_file(p) {
    Err(e) if e.kind() != ErrorKind::NotFound => Err(e.into()),
    _ => Ok(()),
  }
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
  if truncate || f.metadata().map_or(true, |m| m.len() < min_len) {
    f.set_len(min_len)?;
  }
  Ok(f)
}
