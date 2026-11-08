use std::{array::from_fn, fs::File, io, sync::Arc};

use parking_lot::Mutex;

const SLOTS: usize = 64;
type FdSlot = Mutex<Option<(u32, Arc<File>)>>;

/// 固定容量 $O(1)$ 分片文件句柄缓存（最多同时保持 64 个打开的 OS 文件描述符，彻底杜绝百万级文件耗尽 FD）
pub(super) struct FdCache {
  slots: [FdSlot; SLOTS],
}

impl FdCache {
  pub(super) fn new() -> Self {
    Self {
      slots: from_fn(|_| Mutex::new(None)),
    }
  }

  /// 获取或按需打开 `entry_idx` 对应的文件句柄
  #[inline]
  pub(super) fn get_or_open(
    &self,
    entry_idx: u32,
    open_fn: impl FnOnce() -> io::Result<File>,
  ) -> io::Result<Arc<File>> {
    let slot = &self.slots[(entry_idx as usize) & (SLOTS - 1)];
    {
      let guard = slot.lock();
      if let Some((cached_id, ref f)) = *guard
        && cached_id == entry_idx
      {
        return Ok(Arc::clone(f));
      }
    }
    let opened = Arc::new(open_fn()?);
    *slot.lock() = Some((entry_idx, Arc::clone(&opened)));
    Ok(opened)
  }

  /// 显式插入已打开的文件句柄
  #[inline]
  pub(super) fn insert(&self, entry_idx: u32, file: File) {
    let slot = &self.slots[(entry_idx as usize) & (SLOTS - 1)];
    *slot.lock() = Some((entry_idx, Arc::new(file)));
  }

  /// 清空并释放全部缓存句柄（Windows 重命名/删除前释放文件锁）
  pub(super) fn clear(&self) {
    for slot in &self.slots {
      *slot.lock() = None;
    }
  }
}
