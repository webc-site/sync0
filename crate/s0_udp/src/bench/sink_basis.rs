use std::{
  fs::File,
  path::{Path, PathBuf},
  thread,
};

use s0_cdc::{BasisIndex, CdcConfig, FileAt, hash128};

use super::{
  MAX_BLOCK, RootBuf, ZSTD_DEFAULT_LEVEL, manifest::Entry, now_us, par_chunk_plan, sink::Sink,
};
use crate::{
  Receiver, UdpError,
  codec::Codec,
  compio_io::flush_memory_slices_compio,
  fs::{create_trunc, open_nonempty_file, run_par_chunks, scan_files},
};

/// 目录内跨文件去重扫描的基准总字节上限（128 MiB）
const MAX_DIR_BASIS_BYTES: u64 = 128 << 20;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SinglePrecheck {
  Fallback,
  SameHash,
  NeedFull,
}

pub(super) struct TargetFile {
  pub(super) write_path: PathBuf,
  pub(super) size: u64,
  pub(super) rem: u64,
  pub(super) created: bool,
}

pub(super) fn check_single_chunk(
  basis_file: &File,
  basis_size: u64,
  e: &Entry,
  min_sz: u64,
  io_buf: &mut Vec<u8>,
) -> Result<SinglePrecheck, UdpError> {
  if basis_size == e.size {
    let n = e.size as usize;
    if io_buf.len() < n {
      io_buf.resize(n, 0);
    }
    basis_file.read_exact_at_pos(&mut io_buf[..n], 0)?;
    Ok(if hash128(&io_buf[..n]) == e.chunks[0].hash {
      SinglePrecheck::SameHash
    } else {
      SinglePrecheck::NeedFull
    })
  } else if basis_size < e.size || basis_size <= min_sz {
    Ok(SinglePrecheck::NeedFull)
  } else {
    Ok(SinglePrecheck::Fallback)
  }
}

pub(super) fn precheck_single_chunks(
  out: &Path,
  entries: &[Entry],
  min_sz: u64,
) -> Result<Vec<SinglePrecheck>, UdpError> {
  let (n_threads, chunk_len) = par_chunk_plan(entries.len());
  let res: Vec<Result<Vec<SinglePrecheck>, UdpError>> = thread::scope(|s| {
    let mut handles = Vec::with_capacity(n_threads);
    for slice in entries.chunks(chunk_len) {
      handles.push(s.spawn(move || {
        let mut root_buf = RootBuf::new(out);
        let mut io_buf = Vec::new();
        let mut v = Vec::with_capacity(slice.len());
        for e in slice {
          if e.del || e.size == 0 || e.chunks.len() != 1 {
            v.push(SinglePrecheck::Fallback);
            continue;
          }
          let target = root_buf.join(&e.path);
          let Some((basis_file, basis_size)) = open_nonempty_file(target) else {
            v.push(SinglePrecheck::Fallback);
            continue;
          };
          v.push(check_single_chunk(
            &basis_file,
            basis_size,
            e,
            min_sz,
            &mut io_buf,
          )?);
        }
        Ok(v)
      }));
    }
    handles
      .into_iter()
      .map(|h| h.join().unwrap_or(Err(UdpError::Corrupt)))
      .collect()
  });
  let mut out_vec = Vec::with_capacity(entries.len());
  for r in res {
    out_vec.extend(r?);
  }
  Ok(out_vec)
}

pub(super) fn flush_small_writes(
  files: &[TargetFile],
  writes: &[(usize, u32, u32)],
  buf: &[u8],
) -> Result<(), UdpError> {
  Ok(run_par_chunks(writes, |slice| {
    if flush_memory_slices_compio(slice.len(), |i| {
      let (idx, off, len) = slice[i];
      (
        files[idx].write_path.as_path(),
        &buf[off as usize..(off + len) as usize],
      )
    })
    .is_ok()
    {
      return Ok(());
    }
    for &(idx, off, len) in slice {
      let data = &buf[off as usize..(off + len) as usize];
      create_trunc(&files[idx].write_path)?.write_all_at_pos(data, 0)?;
    }
    Ok(())
  })?)
}

pub(super) fn build_dir_basis(
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
        .insert_file_with_buf(file_idx, &f, sf.size, CdcConfig::DEFAULT, io_buf)
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
    let off = u64::from(blk) * MAX_BLOCK as u64;
    if zstd {
      let raw_len = total.saturating_sub(off).min(MAX_BLOCK as u64) as usize;
      let c = match &mut self.codec {
        Some(c) => c,
        None => self.codec.insert(Codec::new(ZSTD_DEFAULT_LEVEL)?),
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
