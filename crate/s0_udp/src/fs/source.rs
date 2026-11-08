use std::{
  fs::File,
  path::{Path, PathBuf},
  sync::Arc,
};

use s0_cdc::{BlockLoc, FileAt, Manifest};

use super::{FsError, SharedSink, fd_cache::FdCache};

/// 从源目录按全局块号 `block_idx` 无锁读取对应分块内容（$O(1)$ 有界句柄缓存，防止海量文件耗尽 OS FD）
pub struct SourceReader {
  root: PathBuf,
  is_single_file: bool,
  manifest: Manifest,
  block_map: Vec<BlockLoc>,
  fds: FdCache,
}

impl SourceReader {
  pub fn open(root: &Path, manifest: Manifest) -> Result<Self, FsError> {
    let block_map = manifest.build_block_map();
    Ok(Self {
      root: root.to_path_buf(),
      is_single_file: root.is_file(),
      manifest,
      block_map,
      fds: FdCache::new(),
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

  #[inline]
  pub fn block_loc(&self, block_idx: u32) -> Option<BlockLoc> {
    self.block_map.get(block_idx as usize).copied()
  }

  /// 读取全局 `block_idx` 的原始字节到 `buf`
  pub fn read_block(&self, block_idx: u32, buf: &mut Vec<u8>) -> Result<(), FsError> {
    let loc = self
      .block_map
      .get(block_idx as usize)
      .copied()
      .ok_or(FsError::BlockCorrupted(block_idx))?;
    buf.resize(loc.len as usize, 0);
    self.read_loc_into(block_idx, loc, buf)
  }

  /// 读取指定 `loc` 到定长切片 `dst`（供单块读取与 SuperBlock 跨小文件流式拼装复用）
  pub fn read_loc_into(
    &self,
    block_idx: u32,
    loc: BlockLoc,
    dst: &mut [u8],
  ) -> Result<(), FsError> {
    let ei = loc.entry_idx as usize;
    let e = self
      .manifest
      .entries
      .get(ei)
      .ok_or(FsError::BlockCorrupted(block_idx))?;
    if e.deleted || e.size == 0 {
      return Err(FsError::BlockCorrupted(block_idx));
    }
    if e.chunks.len() == 1 && !self.is_single_file {
      let f = File::open(self.root.join(&e.path))?;
      f.read_exact_at_pos(dst, loc.offset)?;
      return Ok(());
    }
    let f = self.fds.get_or_open(loc.entry_idx, || {
      let p = if self.is_single_file {
        self.root.clone()
      } else {
        self.root.join(&e.path)
      };
      File::open(p)
    })?;
    f.read_exact_at_pos(dst, loc.offset)?;
    Ok(())
  }
}

/// 统一块数据源：支持从已落盘文件 (`Committed`) 或正在下载的暂存池 (`InProgress`，实现边下边传/下载即做种) 无锁读取已校验块
#[derive(Clone, Copy)]
pub enum BlockSource<'a> {
  Committed(&'a SourceReader),
  InProgress(&'a SharedSink),
}

impl<'a> From<&'a SourceReader> for BlockSource<'a> {
  #[inline]
  fn from(r: &'a SourceReader) -> Self {
    Self::Committed(r)
  }
}

impl<'a> From<&'a Arc<SourceReader>> for BlockSource<'a> {
  #[inline]
  fn from(r: &'a Arc<SourceReader>) -> Self {
    Self::Committed(r.as_ref())
  }
}

impl<'a> From<&'a SharedSink> for BlockSource<'a> {
  #[inline]
  fn from(s: &'a SharedSink) -> Self {
    Self::InProgress(s)
  }
}

impl<'a> From<&'a Arc<SharedSink>> for BlockSource<'a> {
  #[inline]
  fn from(s: &'a Arc<SharedSink>) -> Self {
    Self::InProgress(s.as_ref())
  }
}

impl BlockSource<'_> {
  #[inline]
  pub fn manifest(&self) -> &Manifest {
    match self {
      Self::Committed(r) => r.manifest(),
      Self::InProgress(s) => s.manifest(),
    }
  }

  #[inline]
  pub fn total_blocks(&self) -> u32 {
    match self {
      Self::Committed(r) => r.total_blocks(),
      Self::InProgress(s) => s.total_blocks(),
    }
  }

  #[inline]
  pub fn has_block(&self, block_idx: u32) -> bool {
    match self {
      Self::Committed(r) => block_idx < r.total_blocks(),
      Self::InProgress(s) => s.has_block(block_idx),
    }
  }

  #[inline]
  pub fn block_loc(&self, block_idx: u32) -> Option<BlockLoc> {
    match self {
      Self::Committed(r) => r.block_loc(block_idx),
      Self::InProgress(s) => s.block_loc(block_idx),
    }
  }

  #[inline]
  pub fn read_block(&self, block_idx: u32, buf: &mut Vec<u8>) -> Result<(), FsError> {
    match self {
      Self::Committed(r) => r.read_block(block_idx, buf),
      Self::InProgress(s) => s.read_verified_block(block_idx, buf),
    }
  }

  /// 读取指定块到切片 `dst`
  #[inline]
  pub fn read_loc_into(
    &self,
    block_idx: u32,
    loc: BlockLoc,
    dst: &mut [u8],
  ) -> Result<(), FsError> {
    match self {
      Self::Committed(r) => r.read_loc_into(block_idx, loc, dst),
      Self::InProgress(s) => s.read_verified_loc_into(block_idx, loc, dst),
    }
  }
}
