use std::{
  fs::{self, File, OpenOptions},
  path::{Path, PathBuf},
};

use s0_cdc::{
  BasisIndex, Config, CopySpan, DataSpan, FileAt, FileDelta, copy_file_span, hash128,
  match_basis_file, match_chunks, set_bit_usize,
};

use super::{
  BLK, DGRAM, RootBuf, ZSTD_LEVEL,
  manifest::{Entry, try_build_final_dir_ack},
  now_us,
};
use crate::{
  Receiver, UdpError,
  s0_codec::Codec,
  s0_fs::{Staging, prune_dir, remove, scan_files},
};

/// 本地基准块拷贝缓冲（256 KiB）
const COPY_BUF_LEN: usize = 256 * 1024;
/// 目录内跨文件去重扫描的基准总字节上限（128 MiB）
const MAX_DIR_BASIS_BYTES: u64 = 128 << 20;

fn create_trunc(path: &Path) -> Result<File, UdpError> {
  Ok(
    OpenOptions::new()
      .create(true)
      .truncate(true)
      .write(true)
      .open(path)?,
  )
}

struct TargetFile {
  write_path: PathBuf,
  size: u64,
  rem: u64,
  created: bool,
}

#[derive(Clone, Copy, Debug)]
struct RxSpan {
  file_idx: usize,
  dst_off: u64,
  delta_off: u64,
  len: u64,
}

struct PendingCopy {
  basis_file: Option<File>,
  tmp_path: PathBuf,
  rename_to: Option<PathBuf>,
  size: u64,
  spans: Vec<CopySpan>,
}

/// 统一写出端：支持单文件与目录，支持基于 FastCDC 基准池的增量重建、原子替换与远端多余文件清理
pub(super) struct Sink {
  out_root: PathBuf,
  files: Vec<TargetFile>,
  spans: Vec<RxSpan>,
  span_hint: usize,
  pending_copies: Vec<PendingCopy>,
  pool_paths: Vec<PathBuf>,
  staged: Vec<(usize, PathBuf)>,
  tombstones: Vec<PathBuf>,
  prune: Option<Vec<String>>,
  open: Option<(usize, File)>,
}

impl Sink {
  fn push_full(&mut self, write_path: PathBuf, size: u64, delta_total: &mut u64) {
    let d_off = *delta_total;
    *delta_total += size;
    let file_idx = self.files.len();
    self.files.push(TargetFile {
      write_path,
      size,
      rem: size,
      created: false,
    });
    self.spans.push(RxSpan {
      file_idx,
      dst_off: 0,
      delta_off: d_off,
      len: size,
    });
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
    });
    for d in data_spans {
      self.spans.push(RxSpan {
        file_idx,
        dst_off: d.file_off,
        delta_off: d.delta_off,
        len: d.len,
      });
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
      let tmp_path = Staging::of(&target_path)?.tmp;
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

  pub(super) fn prepare(
    out: &Path,
    entries: Vec<Entry>,
    base_ack: &[u8],
    delete: bool,
  ) -> Result<(Self, u64, Vec<u8>), UdpError> {
    let single_file = entries.len() == 1 && entries[0].path.is_empty();
    let out_existed = if single_file {
      out.exists()
    } else {
      out.is_dir()
    };
    let (sk, dt, have_bits) = Self::prepare_with_limit(out, &entries, usize::MAX)?;
    let (mut sk, delta_total, final_ack) = if let Some(ack) =
      try_build_final_dir_ack(base_ack, &have_bits, dt == 0)?
    {
      (sk, dt, ack)
    } else {
      let max_bits = DGRAM.saturating_sub(base_ack.len() + 4) * 8;
      let (sk, dt, have_bits) = Self::prepare_with_limit(out, &entries, max_bits)?;
      let ack = try_build_final_dir_ack(base_ack, &have_bits, dt == 0)?.ok_or(UdpError::Corrupt)?;
      (sk, dt, ack)
    };
    if delete && !single_file && out_existed {
      let keep: Vec<String> = entries
        .into_iter()
        .filter(|e| !e.del)
        .map(|e| e.path)
        .collect();
      sk.prune = Some(keep);
    }
    Ok((sk, delta_total, final_ack))
  }

  fn prepare_with_limit(
    out: &Path,
    entries: &[Entry],
    max_bits: usize,
  ) -> Result<(Self, u64, Vec<u8>), UdpError> {
    let single_file = entries.len() == 1 && entries[0].path.is_empty();
    let out_existed = if single_file {
      out.exists()
    } else {
      out.is_dir()
    };
    let mut root_buf = RootBuf::new(out);
    if single_file {
      if let Some(parent) = out.parent()
        && !parent.as_os_str().is_empty()
      {
        fs::create_dir_all(parent)?;
      }
    } else {
      fs::create_dir_all(out)?;
      let mut last_dir: Option<&Path> = None;
      for e in entries {
        if !e.del
          && let Some(dir) = Path::new(&e.path).parent()
          && !dir.as_os_str().is_empty()
          && last_dir != Some(dir)
        {
          fs::create_dir_all(root_buf.join(dir))?;
          last_dir = Some(dir);
        }
      }
    }

    let total_chunks: usize = entries.iter().map(|e| e.chunks.len()).sum();
    let mut have_bits = vec![0u8; total_chunks.div_ceil(8)];
    let mut bit_cursor = 0usize;
    let mut delta_total = 0u64;

    let mut sk = Self {
      out_root: out.to_path_buf(),
      files: Vec::with_capacity(entries.len()),
      spans: Vec::with_capacity(entries.len()),
      span_hint: 0,
      pending_copies: Vec::new(),
      pool_paths: Vec::new(),
      staged: Vec::new(),
      tombstones: Vec::new(),
      prune: None,
      open: None,
    };
    let mut io_buf = Vec::new();
    let mut file_basis = BasisIndex::new();
    let mut dir_basis: Option<(BasisIndex, Vec<PathBuf>)> = None;
    let min_sz = Config::DEFAULT.min_size as u64;

    for e in entries {
      let target = if single_file {
        out
      } else {
        root_buf.join(&e.path)
      };

      if e.del {
        sk.tombstones.push(target.to_path_buf());
        continue;
      }

      if e.size == 0 {
        let same_empty =
          out_existed && target.metadata().is_ok_and(|m| m.is_file() && m.len() == 0);
        if !same_empty {
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
        let basis_opt = File::open(target).ok().and_then(|f| {
          let m = f.metadata().ok()?;
          (m.is_file() && m.len() > 0).then_some((f, m.len()))
        });

        if let Some((basis_file, basis_size)) = basis_opt {
          if e.chunks.len() == 1 {
            if basis_size == e.size {
              let n = e.size as usize;
              if io_buf.len() < n {
                io_buf.resize(n, 0);
              }
              basis_file.read_exact_at_pos(&mut io_buf[..n], 0)?;
              if hash128(&io_buf[..n]) == e.chunks[0].hash {
                set_bit_usize(&mut have_bits, bit_cursor);
                bit_cursor += 1;
                continue;
              }
              drop(basis_file);
              bit_cursor += 1;
              sk.push_full(target.to_path_buf(), e.size, &mut delta_total);
              continue;
            }
            if basis_size < e.size || basis_size <= min_sz {
              drop(basis_file);
              bit_cursor += 1;
              sk.push_full(target.to_path_buf(), e.size, &mut delta_total);
              continue;
            }
          }

          let plan = match_basis_file(
            &e.chunks,
            (&basis_file, basis_size),
            Config::DEFAULT,
            (&mut file_basis, &mut io_buf),
            &mut have_bits,
            &mut bit_cursor,
            &mut delta_total,
          )?;

          let in_place = plan.delta_bytes == 0
            && basis_size == e.size
            && plan.copy_spans.len() == 1
            && plan.copy_spans[0].src_off == 0
            && plan.copy_spans[0].dst_off == 0
            && plan.copy_spans[0].len == e.size;

          if in_place {
            continue;
          }

          sk.apply_plan(plan, Some(basis_file), target.to_path_buf(), e.size)?;
          continue;
        }

        if !single_file {
          let (pool, _) = match &mut dir_basis {
            Some(p) => p,
            None => dir_basis.insert(build_dir_basis(out, entries, &mut io_buf)?),
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

  pub(super) fn stage_copies(&mut self) -> Result<(), UdpError> {
    if self.pending_copies.is_empty() {
      return Ok(());
    }
    let mut copy_buf = vec![0u8; COPY_BUF_LEN];
    let mut cached_pool: Option<(u32, File)> = None;
    for pc in self.pending_copies.drain(..) {
      let tmp_file = create_trunc(&pc.tmp_path)?;
      tmp_file.set_len(pc.size)?;
      if let Some(basis_file) = &pc.basis_file {
        for c in &pc.spans {
          copy_file_span(
            basis_file,
            c.src_off,
            &tmp_file,
            c.dst_off,
            c.len,
            &mut copy_buf,
          )?;
        }
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
        fs::rename(&pc.tmp_path, &target_path)?;
      }
    }
    Ok(())
  }

  pub(super) fn write(&mut self, data: &[u8], off: u64) -> Result<(), UdpError> {
    let end = off + data.len() as u64;
    let start = if let Some(sp) = self.spans.get(self.span_hint)
      && sp.delta_off <= off
      && (self.span_hint == 0
        || self.spans[self.span_hint - 1].delta_off + self.spans[self.span_hint - 1].len <= off)
    {
      let mut idx = self.span_hint;
      while idx < self.spans.len() && self.spans[idx].delta_off + self.spans[idx].len <= off {
        idx += 1;
      }
      idx
    } else {
      self
        .spans
        .partition_point(|sp| sp.delta_off + sp.len <= off)
    };
    self.span_hint = start;
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
        let file_off = sp.dst_off + (s - sp.delta_off);
        let idx = sp.file_idx;
        let tf = &mut self.files[idx];
        tf.rem = tf.rem.saturating_sub(len);
        if !tf.created && file_off == 0 && len == tf.size {
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
            if file_off > 0 {
              f.set_len(tf.size)?;
            }
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

  pub(super) fn finalize(&mut self) -> Result<(), UdpError> {
    self.open = None;
    for (file_idx, target_path) in self.staged.drain(..) {
      fs::rename(&self.files[file_idx].write_path, &target_path)?;
    }
    for p in self.tombstones.drain(..) {
      if let Ok(m) = fs::symlink_metadata(&p) {
        if m.is_dir() {
          prune_dir(&p, &[] as &[&str])?;
          let _ = fs::remove_dir(&p);
        } else {
          remove(&p)?;
        }
      }
      let mut cur = p.parent();
      while let Some(dir) = cur {
        if dir == self.out_root || !dir.starts_with(&self.out_root) {
          break;
        }
        if fs::remove_dir(dir).is_err() {
          break;
        }
        cur = dir.parent();
      }
    }
    if let Some(keep) = self.prune.take() {
      prune_dir(&self.out_root, &keep)?;
    }
    Ok(())
  }
}

fn build_dir_basis(
  out: &Path,
  entries: &[Entry],
  io_buf: &mut Vec<u8>,
) -> Result<(BasisIndex, Vec<PathBuf>), UdpError> {
  let Ok(existing) = scan_files(out) else {
    return Ok((BasisIndex::new(), Vec::new()));
  };
  let mut root = RootBuf::new(out);
  let mut index = BasisIndex::new();
  let mut paths = Vec::new();
  let mut total_indexed = 0u64;
  let mut entry_cur = 0usize;
  for sf in existing {
    if sf.size == 0 {
      continue;
    }
    while entry_cur < entries.len() && entries[entry_cur].path.as_str() < sf.path.as_str() {
      entry_cur += 1;
    }
    if entry_cur < entries.len() && entries[entry_cur].path == sf.path && !entries[entry_cur].del {
      continue;
    }
    if total_indexed + sf.size > MAX_DIR_BASIS_BYTES {
      break;
    }
    let p = root.join(&sf.path);
    if let Ok(f) = File::open(p) {
      let file_idx = paths.len() as u32;
      if index
        .insert_file_with_buf(file_idx, &f, sf.size, Config::DEFAULT, io_buf)
        .is_ok()
      {
        paths.push(p.to_path_buf());
        total_indexed += sf.size;
      }
    }
  }
  Ok((index, paths))
}

/// 块写出器：解压器按需创建，解压缓冲全程复用，并实测 EWMA 解压速率供 `ACK.dec` 上报
pub(super) struct Dec {
  codec: Option<Codec>,
  out: Vec<u8>,
  dec_mbps: u16,
}

impl Default for Dec {
  fn default() -> Self {
    Self {
      codec: None,
      out: Vec::new(),
      dec_mbps: 2_000,
    }
  }
}

impl Dec {
  pub(super) const fn dec_mbps(&self) -> u16 {
    self.dec_mbps
  }

  pub(super) fn write(
    &mut self,
    recv: &mut Receiver,
    sink: &mut Sink,
    blk: u32,
    zstd: bool,
    total: u64,
  ) -> Result<(), UdpError> {
    let Some(data) = recv.data(blk) else {
      return Ok(());
    };
    let off = u64::from(blk) * BLK as u64;
    if zstd {
      let raw_len = total.saturating_sub(off).min(BLK as u64) as usize;
      let c = match &mut self.codec {
        Some(c) => c,
        None => self.codec.insert(Codec::new(ZSTD_LEVEL)?),
      };
      self.out.resize(raw_len, 0);
      let t0 = now_us();
      if c.decompress_block(data, &mut self.out)? != raw_len {
        return Err(UdpError::Corrupt);
      }
      let dt_us = now_us().saturating_sub(t0).max(1);
      let sample_mbps = ((raw_len as u64) / dt_us).clamp(1, u16::MAX as u64) as u16;
      self.dec_mbps = ((u32::from(self.dec_mbps) * 3 + u32::from(sample_mbps)) / 4) as u16;
      sink.write(&self.out, off)?;
    } else {
      sink.write(data, off)?;
    }
    recv.release(blk)
  }
}
