use core::{cmp::Ordering, mem::take};

use s0_const::{DEFAULT_FILE_MODE, FsOp, norm_mode};

use crate::{ChunkSig, Manifest, ManifestEntry, Rd, WireError, Wr, bitmap::matches_sorted_sels};

/// 单条相对路径增量变更事件（按路径升序排列，支持 Front-Coding 前缀差分编码、权限 `mode`、毫秒级修改时间 `mtime` 与内联分块签名）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathChange {
  pub op: FsOp,
  pub path: String,
  pub size: u64,
  pub mode: u16,
  pub mtime: u64,
  pub chunks: Vec<ChunkSig>,
}

impl PathChange {
  #[inline]
  pub const fn upsert(path: String, size: u64, chunks: Vec<ChunkSig>) -> Self {
    Self::upsert_meta(path, size, DEFAULT_FILE_MODE, 0, chunks)
  }

  #[inline]
  pub const fn upsert_meta(
    path: String,
    size: u64,
    mode: u16,
    mtime: u64,
    chunks: Vec<ChunkSig>,
  ) -> Self {
    Self {
      op: FsOp::Upsert,
      path,
      size,
      mode: norm_mode(mode),
      mtime,
      chunks,
    }
  }

  #[inline]
  pub const fn meta(path: String, mode: u16, mtime: u64) -> Self {
    Self {
      op: FsOp::Meta,
      path,
      size: 0,
      mode: norm_mode(mode),
      mtime,
      chunks: Vec::new(),
    }
  }

  #[inline]
  pub const fn remove(path: String) -> Self {
    Self {
      op: FsOp::Remove,
      path,
      size: 0,
      mode: 0,
      mtime: 0,
      chunks: Vec::new(),
    }
  }

  #[inline]
  pub fn to_entry(&self) -> ManifestEntry {
    ManifestEntry {
      path: self.path.clone(),
      size: self.size,
      mode: norm_mode(self.mode),
      mtime: self.mtime,
      chunks: self.chunks.clone(),
      deleted: false,
    }
  }

  /// 将 `PathChange` 切片紧凑编码为 Front-Coding 路径 + `FsOp` 流（供 `SubPush` 与 `.s0_subs.bin` AOF 增量日志共用）
  pub fn encode_slice(changes: &[Self], w: &mut Wr<'_>) -> Result<(), WireError> {
    w.uvar(changes.len() as u64);
    let mut prev_path: &[u8] = &[];
    for (i, c) in changes.iter().enumerate() {
      w.u8(c.op as u8);
      w.front_str(i == 0, &mut prev_path, c.path.as_bytes())?;
      if c.op != FsOp::Remove {
        w.u15(usize::from(norm_mode(c.mode)))?;
        w.uvar(c.mtime);
        if c.op == FsOp::Upsert {
          w.uvar(c.chunks.len() as u64);
          ChunkSig::encode_slice(&c.chunks, w)?;
        }
      }
    }
    Ok(())
  }

  /// 从游标解码 `PathChange` 列表（支持 `u64::MAX` 级数量，仅受剩余缓冲区字节上界约束）
  pub fn decode_vec(r: &mut Rd<'_>) -> Result<Vec<Self>, WireError> {
    let change_cnt = r.uvar_usize()?;
    if change_cnt > r.rem() / 2 {
      return Err(WireError::Truncated);
    }
    let mut changes = Vec::with_capacity(change_cnt.min(65536));
    let mut prev_path = Vec::<u8>::new();
    for i in 0..change_cnt {
      let op = FsOp::from_repr(r.u8()?).ok_or(WireError::BadOp)?;
      let path = r.front_str(i == 0, &mut prev_path)?;
      let change = if op == FsOp::Remove {
        Self::remove(path)
      } else {
        let mode = norm_mode(r.u15()?);
        let mtime = r.uvar()?;
        if op == FsOp::Upsert {
          let nchunks = r.uvar_usize()?;
          let (chunks, size) = ChunkSig::decode_vec(r, nchunks)?;
          Self::upsert_meta(path, size, mode, mtime, chunks)
        } else {
          Self::meta(path, mode, mtime)
        }
      };
      changes.push(change);
    }
    Ok(changes)
  }
}

impl Manifest {
  /// 单趟 $O(N + M)$ 双指针归并计算新清单 `curr` 相对于当前清单 `self` 的增改 (`FsOp::Upsert`)、仅元数据修改 (`FsOp::Meta`) 与删除 (`FsOp::Remove`) 集合
  pub fn diff(&self, curr: &Self) -> Vec<PathChange> {
    let (a, b) = (self.entries.as_slice(), curr.entries.as_slice());
    let (mut i, mut j) = (0usize, 0usize);
    let mut out = Vec::new();
    while i < a.len() && j < b.len() {
      let (ea, eb) = (&a[i], &b[j]);
      match ea.path.cmp(&eb.path) {
        Ordering::Less => {
          if !ea.deleted {
            out.push(PathChange::remove(ea.path.clone()));
          }
          i += 1;
        }
        Ordering::Greater => {
          out.push(eb.to_change());
          j += 1;
        }
        Ordering::Equal => {
          if ea.deleted != eb.deleted {
            out.push(eb.to_change());
          } else if !eb.deleted {
            if ea.size != eb.size || ea.chunks != eb.chunks {
              out.push(eb.to_change());
            } else if norm_mode(ea.mode) != norm_mode(eb.mode) || ea.mtime != eb.mtime {
              out.push(PathChange::meta(eb.path.clone(), eb.mode, eb.mtime));
            }
          }
          i += 1;
          j += 1;
        }
      }
    }
    out.extend(
      a[i..]
        .iter()
        .filter(|ea| !ea.deleted)
        .map(|ea| PathChange::remove(ea.path.clone())),
    );
    out.extend(b[j..].iter().map(ManifestEntry::to_change));
    out
  }

  /// 从增量变更列表中提取全部 `FsOp::Upsert` 条目构造子集 `Manifest`（免除二次 `Op::Stat` 往返）
  pub fn from_upsert_changes(changes: &[PathChange]) -> Self {
    let mut entries: Vec<ManifestEntry> = changes
      .iter()
      .filter(|c| c.op == FsOp::Upsert)
      .map(PathChange::to_entry)
      .collect();
    entries.sort_unstable_by(|a, b| a.path.cmp(&b.path));
    entries.dedup_by(|a, b| a.path == b.path);
    Self {
      entries,
      delete_extraneous: false,
      partial: true,
    }
  }

  /// 将增量变更列表原位合并至当前 `Manifest`（$O(N + M \log M)$ 批量归并，避免接收端全盘重扫与重算未变更文件块签名）
  pub fn apply_changes(&mut self, changes: &[PathChange]) {
    if changes.is_empty() {
      return;
    }

    let mut rem_sels: Vec<&str> = Vec::new();
    let mut mut_ops: Vec<&PathChange> = Vec::with_capacity(changes.len());
    for c in changes {
      if c.op == FsOp::Remove {
        rem_sels.push(&c.path);
      } else {
        mut_ops.push(c);
      }
    }
    rem_sels.sort_unstable();
    rem_sels.dedup();
    if !rem_sels.is_empty() {
      mut_ops.retain(|c| !matches_sorted_sels(&c.path, &rem_sels));
    }
    mut_ops.sort_unstable_by(|a, b| a.path.cmp(&b.path));
    mut_ops.dedup_by(|a, b| a.path == b.path);

    let old_entries = take(&mut self.entries);
    let mut merged = Vec::with_capacity(old_entries.len() + mut_ops.len());
    let mut old_iter = old_entries
      .into_iter()
      .filter(|e| rem_sels.is_empty() || !matches_sorted_sels(&e.path, &rem_sels))
      .peekable();
    let mut op_idx = 0usize;

    while let Some(old_peek) = old_iter.peek() {
      let Some(&c) = mut_ops.get(op_idx) else {
        merged.extend(old_iter);
        break;
      };
      match old_peek.path.as_str().cmp(c.path.as_str()) {
        Ordering::Less => merged.push(old_iter.next().unwrap()),
        Ordering::Greater => {
          if c.op == FsOp::Upsert {
            merged.push(c.to_entry());
          }
          op_idx += 1;
        }
        Ordering::Equal => {
          let mut old_e = old_iter.next().unwrap();
          if c.op == FsOp::Upsert {
            merged.push(c.to_entry());
          } else if c.op == FsOp::Meta {
            old_e.mode = norm_mode(c.mode);
            old_e.mtime = c.mtime;
            merged.push(old_e);
          }
          op_idx += 1;
        }
      }
    }

    merged.extend(
      mut_ops[op_idx..]
        .iter()
        .filter(|c| c.op == FsOp::Upsert)
        .map(|c| c.to_entry()),
    );
    self.entries = merged;
  }
}
