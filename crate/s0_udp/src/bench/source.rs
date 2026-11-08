use std::fs::File;

use s0_cdc::{FileAt, for_each_missing_data_span};

use super::{
  EMPTY, FileSpan, MAX_BLOCK, MAX_CNT, RootBuf, WINDOW_BLOCKS, ZSTD_DEFAULT_LEVEL, locate_span,
  manifest::Entry, now_us,
};
use crate::{
  UdpError,
  codec::{Codec, Detector, LevelChoice, Policy, skip_name},
};

/// 分片数对应的低 `cnt` 位全 1 掩码
pub(super) const fn cnt_mask(cnt: u8) -> u128 {
  if cnt >= 128 {
    u128::MAX
  } else {
    (1u128 << cnt) - 1
  }
}

/// 窗口内一个块的线路载荷（可能已压缩）与分片级确认/重传状态
pub(super) struct Blk {
  pub(super) blk: u32,
  pub(super) data: Vec<u8>,
  pub(super) cnt: u8,
  pub(super) zstd: bool,
  pub(super) have: u128,
  pub(super) last_retx: [u64; MAX_CNT],
}

/// 根据清单与接收端 `have_bits` 构建缺失数据的发送区间表与需传总字节数
pub(super) fn build_tx_spans(entries: &[Entry], have_bits: &[u8]) -> (Vec<FileSpan>, u64) {
  if have_bits.is_empty() {
    let mut spans = Vec::with_capacity(entries.len());
    let mut delta_total = 0u64;
    for (idx, e) in entries.iter().enumerate() {
      if !e.del && e.size > 0 {
        spans.push(FileSpan {
          idx,
          file_off: 0,
          delta_off: delta_total,
          len: e.size,
        });
        delta_total = delta_total.saturating_add(e.size);
      }
    }
    return (spans, delta_total);
  }
  let total_chunks: usize = entries.iter().map(|e| e.chunks.len()).sum();
  if have_bits.len() * 8 >= total_chunks && have_bits.iter().all(|&b| b == 0xFF) {
    return (Vec::new(), 0);
  }
  let mut spans = Vec::with_capacity(entries.len());
  let mut bit_cursor = 0usize;
  let mut delta_total = 0u64;
  for (idx, e) in entries.iter().enumerate() {
    for_each_missing_data_span(
      &e.chunks,
      have_bits,
      &mut bit_cursor,
      &mut delta_total,
      |s| {
        spans.push(FileSpan {
          idx,
          file_off: s.file_off,
          delta_off: s.delta_off,
          len: s.len,
        });
      },
    );
  }
  (spans, delta_total)
}

/// 流式块源：按需直接读入槽位 + 复用 `s0_codec::{Detector, Policy}` 带宽×CPU 自适应压缩
pub(super) struct Src {
  kind: SrcKind,
  entries: Vec<Entry>,
  spans: Vec<FileSpan>,
  span_hint: usize,
  total: u64,
  pkt_size: usize,
  codec: Option<Codec>,
  detector: Detector,
  policy: Policy,
  line_bps: u64,
  open: Option<(usize, File)>,
  slots: Box<[Blk]>,
}

pub(super) enum SrcKind {
  File(File),
  Dir(RootBuf, Vec<u8>),
}

impl Src {
  pub(super) fn new(
    kind: SrcKind,
    entries: Vec<Entry>,
    spans: Vec<FileSpan>,
    total: u64,
    skip_all: bool,
    pkt_size: usize,
  ) -> Result<Self, UdpError> {
    let check_magic = matches!(kind, SrcKind::File(_));
    let codec = (!skip_all)
      .then(|| Codec::new(ZSTD_DEFAULT_LEVEL))
      .transpose()?;
    let detector = Detector::with_flags(skip_all, check_magic);
    let policy = Policy::new(1, 0);
    let slots = (0..WINDOW_BLOCKS)
      .map(|_| Blk {
        blk: EMPTY,
        data: Vec::new(),
        cnt: 0,
        zstd: false,
        have: 0,
        last_retx: [0; MAX_CNT],
      })
      .collect();
    Ok(Self {
      kind,
      entries,
      spans,
      span_hint: 0,
      total,
      pkt_size,
      codec,
      detector,
      policy,
      line_bps: 0,
      open: None,
      slots,
    })
  }

  /// 动态降级分片大小（如 1400B -> 1200B）：直接复用窗口槽位内已读取/压缩的数据重新计算分片数 `cnt`，零二次磁盘读取与零重压缩
  pub(super) fn downgrade_pkt_size(&mut self, new_pkt: usize) {
    if new_pkt < self.pkt_size {
      self.pkt_size = new_pkt;
      for s in &mut self.slots {
        if s.blk != EMPTY {
          s.cnt = s.data.len().div_ceil(new_pkt) as u8;
          s.have = 0;
          s.last_retx[..usize::from(s.cnt)].fill(0);
        }
      }
    }
  }

  /// 当接收端回传非空 `have_bits`（增量同步）时，重置虚拟流区间并失效预热槽位
  pub(super) fn rebuild_spans(&mut self, have_bits: &[u8]) -> u64 {
    let (spans, total) = build_tx_spans(&self.entries, have_bits);
    self.spans = spans;
    self.span_hint = 0;
    self.total = total;
    for s in &mut self.slots {
      s.blk = EMPTY;
    }
    total
  }

  pub(super) fn set_net_state(&mut self, line_bps: u64, peer_dec_mbps: u16) {
    self.line_bps = line_bps;
    if peer_dec_mbps > 0 {
      self
        .policy
        .set_peer_bps(u64::from(peer_dec_mbps) * 1_000_000);
    }
  }

  fn read_into(
    kind: &mut SrcKind,
    entries: &[Entry],
    spans: &[FileSpan],
    span_hint: &mut usize,
    open: &mut Option<(usize, File)>,
    raw: &mut [u8],
    off: u64,
  ) -> Result<(bool, bool), UdpError> {
    let len = raw.len();
    let end = off + len as u64;
    let mut done = 0usize;
    let mut all_skip_ext = matches!(kind, SrcKind::Dir(..));
    let mut entered_compressible = false;
    let start = locate_span(spans, span_hint, off);
    for (i, sp) in spans[start..].iter().enumerate() {
      if sp.delta_off >= end || done >= len {
        break;
      }
      if sp.delta_off + sp.len <= end {
        *span_hint = start + i + 1;
      } else {
        *span_hint = start + i;
      }
      let s = off.max(sp.delta_off);
      let t = end.min(sp.delta_off + sp.len);
      if t > s {
        let chunk = &mut raw[done..done + (t - s) as usize];
        let file_off = sp.file_off + (s - sp.delta_off);
        match kind {
          SrcKind::File(f) => {
            all_skip_ext = false;
            f.read_exact_at_pos(chunk, file_off)?;
          }
          SrcKind::Dir(root, arena) => {
            let idx = sp.idx;
            let e = &entries[idx];
            if !skip_name(&e.path) {
              all_skip_ext = false;
              if file_off == 0 {
                entered_compressible = true;
              }
            }
            if let Some(base) = e.cached {
              let start_off = (base as usize) + (file_off as usize);
              chunk.copy_from_slice(&arena[start_off..start_off + chunk.len()]);
            } else if let Some((cached_idx, f)) = open
              && *cached_idx == idx
            {
              f.read_exact_at_pos(chunk, file_off)?;
            } else {
              let f = File::open(root.join(&e.path))?;
              f.read_exact_at_pos(chunk, file_off)?;
              if file_off + (t - s) < e.size {
                *open = Some((idx, f));
              }
            }
          }
        }
        done += (t - s) as usize;
      }
    }
    if done != len {
      return Err(UdpError::Arg("虚拟流读取越界"));
    }
    Ok((all_skip_ext, entered_compressible))
  }

  /// 取块；调用方保证 `blk` 位于发送窗口内
  pub(super) fn get(&mut self, blk: u32) -> Result<&mut Blk, UdpError> {
    let idx = (blk % WINDOW_BLOCKS) as usize;
    if self.slots[idx].blk != blk {
      let off = u64::from(blk) * MAX_BLOCK as u64;
      let len = self.total.saturating_sub(off).min(MAX_BLOCK as u64) as usize;
      let slot = &mut self.slots[idx];
      slot.data.resize(len, 0);
      let (all_skip_ext, entered_compressible) = Self::read_into(
        &mut self.kind,
        &self.entries,
        &self.spans,
        &mut self.span_hint,
        &mut self.open,
        &mut slot.data,
        off,
      )?;
      if entered_compressible && self.detector.skip() {
        self.detector.reset_file();
      }
      let zstd = if !all_skip_ext
        && let Some(codec) = &mut self.codec
        && let LevelChoice::Level(lvl) = self.policy.choose(self.line_bps)
      {
        codec.set_level(lvl)?;
        let was_skip = self.detector.skip();
        let t0 = now_us();
        if let Some(comp) = self.detector.compress(&slot.data, codec)? {
          let dt_us = now_us().saturating_sub(t0).max(1);
          let clen = comp.len();
          let speed_bps = (len as f64) * 1_000_000.0 / (dt_us as f64);
          let ratio = (clen as f64) / (len.max(1) as f64);
          self.policy.observe(lvl, speed_bps, ratio);
          slot.data.clear();
          slot.data.extend_from_slice(comp);
          true
        } else {
          if !was_skip {
            let dt_us = now_us().saturating_sub(t0).max(1);
            let speed_bps = (len as f64) * 1_000_000.0 / (dt_us as f64);
            self.policy.observe(lvl, speed_bps, 1.0);
          }
          false
        }
      } else {
        false
      };
      slot.zstd = zstd;
      slot.cnt = slot.data.len().div_ceil(self.pkt_size) as u8;
      slot.have = 0;
      slot.last_retx[..usize::from(slot.cnt)].fill(0);
      slot.blk = blk;
    }
    Ok(&mut self.slots[idx])
  }

  pub(super) fn slot(&self, blk: u32) -> Option<&Blk> {
    let s = &self.slots[(blk % WINDOW_BLOCKS) as usize];
    (s.blk == blk).then_some(s)
  }

  pub(super) fn slot_mut(&mut self, blk: u32) -> Option<&mut Blk> {
    let s = &mut self.slots[(blk % WINDOW_BLOCKS) as usize];
    (s.blk == blk).then_some(s)
  }
}
