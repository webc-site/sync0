use std::{
  fs::{File, OpenOptions, create_dir_all, rename},
  mem::take,
  path::{Path, PathBuf},
};

use s0_cdc::{
  BasisIndex, CdcConfig, CopySpan, DataSpan, FileAt, FileDelta, FileSpan, copy_file_span,
  copy_file_spans, locate_span, match_basis_file, match_chunks, set_bit_usize,
};
use s0_const::MAX_DATAGRAM;
use s0_fs::{
  PAR_SCAN_MIN_FILES, RootBuf, SinglePrecheck, SmallBatchFlusher, SmallWriteJob, build_dir_basis,
  check_single_chunk, create_trunc, dir_has_entries, ensure_parent_dirs, open_nonempty_file,
  open_rw, precheck_single_chunks, prune_dir, stage_path,
};

use crate::{
  UdpError,
  bench::{manifest::Entry, manifest_net::try_build_final_dir_ack},
};

/// 本地基准块拷贝缓冲（256 KiB）
const COPY_BUF_LEN: usize = 256 * 1024;
/// 接收端小文件内存聚合单文件上限（256 KiB，覆盖 99.7% 源码小文件与跨 128 KiB 块边界小文件）
const MAX_SMALL_WRITE_FILE: u64 = 256 * 1024;
/// 接收端小文件流式单批聚合缓冲阈值（4 MiB）
const SMALL_BATCH_BUF: usize = 4 << 20;
/// 接收端小文件流式单批初始预留容量（2 MiB）
const SMALL_BATCH_INIT_CAP: usize = 2 << 20;
/// 接收端小文件流式单批聚合文件数阈值
const SMALL_BATCH_FILES: usize = 512;
/// 接收端小文件流式单批初始预留文件条目数
const SMALL_WRITES_INIT_CAP: usize = 256;

struct TargetFile {
  write_path: PathBuf,
  size: u64,
  rem: u64,
  created: bool,
  partial: Vec<u8>,
}

struct PendingCopy {
  basis_file: Option<File>,
  tmp_path: PathBuf,
  rename_to: Option<PathBuf>,
  size: u64,
  spans: Vec<CopySpan>,
}

/// 统一写出端：支持单文件与目录，支持基于 FastCDC 基准池的增量重建、原子替换与远端多余文件清理
pub(crate) struct Sink {
  out_root: PathBuf,
  files: Vec<TargetFile>,
  spans: Vec<FileSpan>,
  span_hint: usize,
  pending_copies: Vec<PendingCopy>,
  pool_paths: Vec<PathBuf>,
  staged: Vec<(usize, PathBuf)>,
  prune: Option<Vec<String>>,
  open: Option<(usize, File)>,
  small_buf: Vec<u8>,
  small_writes: Vec<SmallWriteJob>,
  flusher: SmallBatchFlusher,
}

impl Sink {
  fn push_full(&mut self, write_path: PathBuf, size: u64, delta_total: &mut u64) {
    let d_off = *delta_total;
    *delta_total += size;
    let span = DataSpan {
      file_off: 0,
      delta_off: d_off,
      len: size,
    };
    let _ = self.push_data_spans(write_path, size, size, false, vec![span]);
  }

  fn push_data_spans(
    &mut self,
    write_path: PathBuf,
    size: u64,
    rem: u64,
    created: bool,
    data_spans: Vec<DataSpan>,
  ) -> usize {
    let file_idx = self.files.len();
    self.files.push(TargetFile {
      write_path,
      size,
      rem,
      created,
      partial: Vec::new(),
    });
    for d in data_spans {
      self.spans.push(FileSpan::from_data(file_idx, d));
    }
    file_idx
  }

  fn apply_plan(
    &mut self,
    plan: FileDelta,
    basis_file: Option<File>,
    target_path: PathBuf,
    size: u64,
  ) -> Result<(), UdpError> {
    if !plan.copy_spans.is_empty() {
      if basis_file.is_some() && plan.is_in_place() {
        drop(basis_file);
        let _ = open_rw(&target_path, false, size)?;
        if plan.delta_bytes > 0 {
          let _ = self.push_data_spans(target_path, size, plan.delta_bytes, true, plan.data_spans);
        }
        return Ok(());
      }
      let tmp_path = stage_path(&target_path);
      let rename_to = if plan.delta_bytes == 0 {
        Some(target_path)
      } else {
        let file_idx = self.push_data_spans(
          tmp_path.clone(),
          size,
          plan.delta_bytes,
          true,
          plan.data_spans,
        );
        self.staged.push((file_idx, target_path));
        None
      };
      self.pending_copies.push(PendingCopy {
        basis_file,
        tmp_path,
        rename_to,
        size,
        spans: plan.copy_spans,
      });
    } else {
      drop(basis_file);
      let _ = self.push_data_spans(target_path, size, plan.delta_bytes, false, plan.data_spans);
    }
    Ok(())
  }

  pub(crate) fn prepare(
    out: &Path,
    entries: Vec<Entry>,
    base_ack: &[u8],
    delete: bool,
  ) -> Result<(Self, u64, Vec<u8>), UdpError> {
    let single_file = entries.len() == 1 && entries[0].path.is_empty();
    let out_existed = if single_file {
      out.exists()
    } else {
      dir_has_entries(out)
    };
    let (sk, dt, have_bits) =
      Self::prepare_with_limit(out, &entries, (single_file, out_existed), usize::MAX)?;
    let (mut sk, delta_total, final_ack) = if let Some(ack) =
      try_build_final_dir_ack(base_ack, &have_bits, dt == 0)?
    {
      (sk, dt, ack)
    } else {
      let max_bits = MAX_DATAGRAM.saturating_sub(base_ack.len() + 4) * 8;
      let (sk, dt, have_bits) =
        Self::prepare_with_limit(out, &entries, (single_file, out_existed), max_bits)?;
      let ack = try_build_final_dir_ack(base_ack, &have_bits, dt == 0)?.ok_or(UdpError::Corrupt)?;
      (sk, dt, ack)
    };
    if delete && !single_file && out_existed {
      sk.prune = Some(entries.into_iter().map(|e| e.path).collect());
    }
    Ok((sk, delta_total, final_ack))
  }

  fn prepare_with_limit(
    out: &Path,
    entries: &[Entry],
    (single_file, out_existed): (bool, bool),
    max_bits: usize,
  ) -> Result<(Self, u64, Vec<u8>), UdpError> {
    let mut root_buf = RootBuf::new(out);
    if single_file {
      if let Some(parent) = out.parent()
        && !parent.as_os_str().is_empty()
      {
        create_dir_all(parent)?;
      }
    } else {
      ensure_parent_dirs(out, entries.iter().map(|e| e.path.as_str()))?;
    }

    let total_chunks: usize = entries.iter().map(|e| e.chunks.len()).sum();
    let mut have_bits = vec![0u8; total_chunks.div_ceil(8)];
    let (mut bit_cursor, mut delta_total) = (0usize, 0u64);
    let mut sk = Self {
      out_root: out.to_path_buf(),
      files: Vec::with_capacity(entries.len()),
      spans: Vec::with_capacity(entries.len()),
      span_hint: 0,
      pending_copies: Vec::new(),
      pool_paths: Vec::new(),
      staged: Vec::new(),
      prune: None,
      open: None,
      small_buf: Vec::new(),
      small_writes: Vec::new(),
      flusher: SmallBatchFlusher::default(),
    };
    let (mut io_buf, mut file_basis, mut dir_basis) = (Vec::new(), BasisIndex::new(), None);
    let min_sz = CdcConfig::DEFAULT.min_size as u64;
    let prechecked = if out_existed && !single_file && entries.len() >= PAR_SCAN_MIN_FILES {
      precheck_single_chunks(out, entries, min_sz, |e| (&e.path, e.size, &e.chunks))?
    } else {
      Vec::new()
    };

    for (idx, e) in entries.iter().enumerate() {
      let target = if single_file {
        out
      } else {
        root_buf.join(&e.path)
      };
      if e.size == 0 {
        if !(out_existed && target.metadata().is_ok_and(|m| m.is_file() && m.len() == 0)) {
          create_trunc(target)?;
        }
        continue;
      }
      if !out_existed {
        bit_cursor += e.chunks.len();
        sk.push_full(target.to_path_buf(), e.size, &mut delta_total);
        continue;
      }
      if bit_cursor.saturating_add(e.chunks.len()) <= max_bits {
        let mut basis_opt = None;
        let pre = match prechecked.get(idx).copied() {
          Some(p) => p,
          None => {
            basis_opt = open_nonempty_file(target);
            if e.chunks.len() == 1
              && let Some((ref bf, bs)) = basis_opt
            {
              check_single_chunk(bf, bs, e.size, e.chunks[0].hash, min_sz, &mut io_buf)?
            } else {
              SinglePrecheck::Fallback
            }
          }
        };
        match pre {
          SinglePrecheck::SameHash => {
            set_bit_usize(&mut have_bits, bit_cursor);
            bit_cursor += 1;
            continue;
          }
          SinglePrecheck::NeedFull => {
            bit_cursor += 1;
            sk.push_full(target.to_path_buf(), e.size, &mut delta_total);
            continue;
          }
          SinglePrecheck::Fallback => {}
        }
        if basis_opt.is_none() {
          basis_opt = open_nonempty_file(target);
        }
        if let Some((basis_file, basis_size)) = basis_opt.take() {
          let plan = match_basis_file(
            &e.chunks,
            (&basis_file, basis_size),
            CdcConfig::DEFAULT,
            (&mut file_basis, &mut io_buf),
            &mut have_bits,
            &mut bit_cursor,
            &mut delta_total,
          )?;
          if basis_size == e.size && plan.is_identical_to(e.size) {
            continue;
          }
          sk.apply_plan(plan, Some(basis_file), target.to_path_buf(), e.size)?;
          continue;
        }
        if !single_file {
          let (pool, _) = match &mut dir_basis {
            Some(p) => p,
            None => dir_basis.insert(build_dir_basis(out, entries, &mut io_buf, |e| &e.path)),
          };
          if !pool.is_empty() {
            let plan = match_chunks(
              &e.chunks,
              pool,
              &mut have_bits,
              &mut bit_cursor,
              &mut delta_total,
            );
            sk.apply_plan(plan, None, target.to_path_buf(), e.size)?;
            continue;
          }
        }
      }
      bit_cursor += e.chunks.len();
      sk.push_full(target.to_path_buf(), e.size, &mut delta_total);
    }
    if let Some((_, p)) = dir_basis {
      sk.pool_paths = p;
    }
    Ok((sk, delta_total, have_bits))
  }

  pub(crate) fn stage_copies(&mut self) -> Result<(), UdpError> {
    if self.pending_copies.is_empty() {
      return Ok(());
    }
    let mut copy_buf = vec![0u8; COPY_BUF_LEN];
    let mut cached_pool: Option<(u32, File)> = None;
    for pc in self.pending_copies.drain(..) {
      let tmp_file = open_rw(&pc.tmp_path, true, pc.size)?;
      if let Some(basis_file) = &pc.basis_file {
        copy_file_spans(basis_file, &tmp_file, &pc.spans, &mut copy_buf)?;
      } else {
        for c in &pc.spans {
          let src_f = match &cached_pool {
            Some((idx, f)) if *idx == c.src_file => f,
            _ => {
              let f = File::open(&self.pool_paths[c.src_file as usize])?;
              &cached_pool.insert((c.src_file, f)).1
            }
          };
          copy_file_span(src_f, c.src_off, &tmp_file, c.dst_off, c.len, &mut copy_buf)?;
        }
      }
      drop(tmp_file);
      drop(pc.basis_file);
      if let Some(target_path) = pc.rename_to {
        rename(&pc.tmp_path, &target_path)?;
      }
    }
    Ok(())
  }

  pub(crate) fn write(&mut self, data: &[u8], off: u64) -> Result<(), UdpError> {
    let end = off + data.len() as u64;
    let start = locate_span(&self.spans, &mut self.span_hint, off);
    for (i, sp) in self.spans[start..].iter().enumerate() {
      if sp.delta_off >= end {
        break;
      }
      if sp.delta_off + sp.len <= end {
        self.span_hint = start + i + 1;
      } else {
        self.span_hint = start + i;
      }
      let s = off.max(sp.delta_off);
      let t = end.min(sp.delta_off + sp.len);
      if t > s {
        let len = t - s;
        let slice = &data[(s - off) as usize..(t - off) as usize];
        let file_off = sp.file_off + (s - sp.delta_off);
        let idx = sp.idx;
        let tf = &mut self.files[idx];
        tf.rem = tf.rem.saturating_sub(len);
        if !tf.created && tf.size <= MAX_SMALL_WRITE_FILE {
          let ready = if file_off == 0 && len == tf.size {
            Some((take(&mut tf.write_path), slice, None))
          } else {
            if tf.partial.is_empty() {
              tf.partial.resize(tf.size as usize, 0);
            }
            let f_off = file_off as usize;
            tf.partial[f_off..f_off + (len as usize)].copy_from_slice(slice);
            (tf.rem == 0).then(|| {
              (
                take(&mut tf.write_path),
                &[][..],
                Some(take(&mut tf.partial)),
              )
            })
          };
          if let Some((path, inline, owned)) = ready {
            tf.created = true;
            let bytes = owned.as_deref().unwrap_or(inline);
            if self.small_buf.is_empty() {
              self
                .small_buf
                .reserve(SMALL_BATCH_BUF.min(SMALL_BATCH_INIT_CAP));
              self.small_writes.reserve(SMALL_WRITES_INIT_CAP);
            }
            let buf_off = self.small_buf.len() as u32;
            self.small_buf.extend_from_slice(bytes);
            self.small_writes.push((path, buf_off, bytes.len() as u32));
            if self.small_buf.len() >= SMALL_BATCH_BUF
              || self.small_writes.len() >= SMALL_BATCH_FILES
            {
              let writes = take(&mut self.small_writes);
              let buf = take(&mut self.small_buf);
              self.flusher.submit(writes, buf)?;
            }
          }
        } else if !tf.created && file_off == 0 && len == tf.size {
          let f = create_trunc(&tf.write_path)?;
          f.write_all_at_pos(slice, 0)?;
          tf.created = true;
        } else if let Some((cached_idx, f)) = &self.open
          && *cached_idx == idx
        {
          f.write_all_at_pos(slice, file_off)?;
          if tf.rem == 0 {
            self.open = None;
          }
        } else {
          let f = if tf.created {
            OpenOptions::new().write(true).open(&tf.write_path)?
          } else {
            let f = create_trunc(&tf.write_path)?;
            f.set_len(tf.size)?;
            tf.created = true;
            f
          };
          f.write_all_at_pos(slice, file_off)?;
          if tf.rem > 0 {
            self.open = Some((idx, f));
          }
        }
      }
    }
    Ok(())
  }

  pub(crate) fn finalize(&mut self) -> Result<(), UdpError> {
    self.open = None;
    let writes = take(&mut self.small_writes);
    let buf = take(&mut self.small_buf);
    self.flusher.finish(writes, buf)?;
    for (file_idx, target_path) in self.staged.drain(..) {
      rename(&self.files[file_idx].write_path, &target_path)?;
    }
    if let Some(keep) = self.prune.take() {
      prune_dir(&self.out_root, &keep)?;
    }
    Ok(())
  }
}
