use std::{fs::File, mem::swap, sync::Arc};

use s0_cdc::{FileSpan, for_each_missing_data_span};
use s0_codec::{Codec, Detector, LevelChoice, Policy};
use s0_const::{MAX_BLOCK, MAX_CNT, WINDOW_BLOCKS, ZSTD_DEFAULT_LEVEL};
use s0_fs::RootBuf;

use crate::{
  UdpError,
  bench::{
    EMPTY,
    manifest::Entry,
    prefetch::{LVL_OFF, Prefetcher, SrcKind, read_into},
  },
  now_us,
};

/// 分片数对应的低 `cnt` 位全 1 掩码
pub(crate) const fn cnt_mask(cnt: u8) -> u128 {
  if cnt >= 128 {
    u128::MAX
  } else {
    (1u128 << cnt) - 1
  }
}

/// 窗口内一个块的线路载荷（可能已压缩）与分片级确认/重传状态
pub(crate) struct Blk {
  pub(crate) blk: u32,
  pub(crate) data: Vec<u8>,
  pub(crate) cnt: u8,
  pub(crate) zstd: bool,
  pub(crate) have: u128,
  pub(crate) last_tx_us: u64,
  pub(crate) last_retx: [u64; MAX_CNT],
}

impl Blk {
  #[inline]
  fn reset_meta(&mut self, blk: u32, zstd: bool, pkt_size: usize) {
    self.blk = blk;
    self.zstd = zstd;
    self.cnt = self.data.len().div_ceil(pkt_size) as u8;
    self.have = 0;
    self.last_tx_us = 0;
    self.last_retx[..usize::from(self.cnt)].fill(0);
  }
}

/// 根据清单与接收端 `have_bits` 构建缺失数据的发送区间表与需传总字节数
pub(crate) fn build_tx_spans(entries: &[Entry], have_bits: &[u8]) -> (Vec<FileSpan>, u64) {
  if have_bits.is_empty() {
    let mut spans = Vec::with_capacity(entries.len());
    let mut delta_total = 0u64;
    for (idx, e) in entries.iter().enumerate() {
      if e.size > 0 {
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
      |s| spans.push(FileSpan::from_data(idx, s)),
    );
  }
  (spans, delta_total)
}

#[inline]
fn observe_comp(
  policy: &mut Policy,
  lvl: i32,
  raw_len: usize,
  dt_us: u64,
  comp_len: Option<usize>,
) {
  let speed_bps = (raw_len as f64) * 1_000_000.0 / (dt_us.max(1) as f64);
  let ratio = comp_len.map_or(1.0, |c| (c as f64) / (raw_len.max(1) as f64));
  policy.observe(lvl, speed_bps, ratio);
}

/// 流式块源：后台多线程前瞻预取 + 并行 Zstd 压缩流水线 + `s0_codec::{Detector, Policy}` 带宽×CPU 自适应压缩
pub(crate) struct Src {
  kind: Arc<SrcKind>,
  entries: Arc<Vec<Entry>>,
  spans: Arc<Vec<FileSpan>>,
  span_hint: usize,
  root_buf: RootBuf,
  epoch: u64,
  total: u64,
  nblk: u32,
  pkt_size: usize,
  codec: Option<Codec>,
  detector: Detector,
  policy: Policy,
  line_bps: u64,
  open: Option<(usize, File)>,
  prefetcher: Prefetcher,
  slots: Box<[Blk]>,
}

impl Src {
  pub(crate) fn new(
    kind: SrcKind,
    entries: Vec<Entry>,
    spans: Vec<FileSpan>,
    total: u64,
    skip_all: bool,
    pkt_size: usize,
  ) -> Result<Self, UdpError> {
    let check_magic = matches!(kind, SrcKind::File(_));
    let root_buf = kind.root_buf();
    let kind_arc = Arc::new(kind);
    let entries_arc = Arc::new(entries);
    let spans_arc = Arc::new(spans);
    let codec = (!skip_all)
      .then(|| Codec::new(ZSTD_DEFAULT_LEVEL))
      .transpose()?;
    let detector = Detector::with_flags(skip_all, check_magic);
    let prefetcher = Prefetcher::new(
      Arc::clone(&kind_arc),
      Arc::clone(&entries_arc),
      Arc::clone(&spans_arc),
      total,
      skip_all,
      check_magic,
    );
    let policy = Policy::new(prefetcher.worker_count(), 0);
    let slots = (0..WINDOW_BLOCKS)
      .map(|_| Blk {
        blk: EMPTY,
        data: Vec::new(),
        cnt: 0,
        zstd: false,
        have: 0,
        last_tx_us: 0,
        last_retx: [0; MAX_CNT],
      })
      .collect();
    Ok(Self {
      kind: kind_arc,
      entries: entries_arc,
      spans: spans_arc,
      span_hint: 0,
      root_buf,
      epoch: 1,
      total,
      nblk: total.div_ceil(MAX_BLOCK as u64) as u32,
      pkt_size,
      codec,
      detector,
      policy,
      line_bps: 0,
      open: None,
      prefetcher,
      slots,
    })
  }

  /// 动态降级分片大小（如 1400B -> 1200B）：直接复用窗口槽位内已读取/压缩的数据重新计算分片数 `cnt`，零二次磁盘读取与零重压缩
  pub(crate) fn downgrade_pkt_size(&mut self, new_pkt: usize) {
    if new_pkt < self.pkt_size {
      self.pkt_size = new_pkt;
      for s in &mut self.slots {
        if s.blk != EMPTY {
          s.reset_meta(s.blk, s.zstd, new_pkt);
        }
      }
    }
  }

  /// 当接收端回传非空 `have_bits`（增量同步）时，重置虚拟流区间并失效预热槽位
  pub(crate) fn rebuild_spans(&mut self, have_bits: &[u8]) -> u64 {
    let (spans, total) = build_tx_spans(&self.entries, have_bits);
    let spans_arc = Arc::new(spans);
    self.epoch += 1;
    self.spans = Arc::clone(&spans_arc);
    self.span_hint = 0;
    self.total = total;
    self.nblk = total.div_ceil(MAX_BLOCK as u64) as u32;
    self.prefetcher.reset_plan(self.epoch, spans_arc, total);
    for s in &mut self.slots {
      s.blk = EMPTY;
    }
    total
  }

  #[inline]
  fn active_zstd_level(&self) -> i32 {
    if self.codec.is_none() {
      return LVL_OFF;
    }
    match self.policy.choose(self.line_bps) {
      LevelChoice::Level(l) => l.clamp(1, ZSTD_DEFAULT_LEVEL),
      LevelChoice::Off => 1,
    }
  }

  pub(crate) fn set_net_state(&mut self, line_bps: u64, peer_dec_mbps: u16) {
    self.line_bps = line_bps;
    if peer_dec_mbps > 0 {
      self
        .policy
        .set_peer_bps(u64::from(peer_dec_mbps) * 1_000_000);
    }
    self.prefetcher.set_level(self.active_zstd_level());
  }

  /// 取块；调用方保证 `blk` 位于发送窗口内
  pub(crate) fn get(&mut self, blk: u32) -> Result<&mut Blk, UdpError> {
    let idx = (blk % WINDOW_BLOCKS) as usize;
    if self.slots[idx].blk != blk {
      self.prefetcher.request_ahead(blk, self.nblk);
      let mut ready_buf = Vec::new();
      swap(&mut ready_buf, &mut self.slots[idx].data);
      if let Some(mut ready) = self.prefetcher.take_ready(self.epoch, blk, &mut ready_buf) {
        if ready.attempted_zstd {
          observe_comp(
            &mut self.policy,
            ready.lvl,
            ready.raw_len,
            ready.comp_us,
            ready.zstd.then_some(ready.data.len()),
          );
          self.prefetcher.set_level(self.active_zstd_level());
        }
        let slot = &mut self.slots[idx];
        swap(&mut slot.data, &mut ready.data);
        slot.reset_meta(blk, ready.zstd, self.pkt_size);
        return Ok(&mut self.slots[idx]);
      }
      swap(&mut ready_buf, &mut self.slots[idx].data);

      let off = u64::from(blk) * MAX_BLOCK as u64;
      let len = self.total.saturating_sub(off).min(MAX_BLOCK as u64) as usize;
      let lvl = self.active_zstd_level();
      let slot = &mut self.slots[idx];
      slot.data.resize(len, 0);
      let (all_skip_ext, entered_compressible) = read_into(
        &self.kind,
        &self.entries,
        &self.spans,
        (&mut self.span_hint, &mut self.root_buf, &mut self.open),
        (&mut slot.data, off),
      )?;
      if entered_compressible && self.detector.skip() {
        self.detector.reset_file();
      }
      let zstd = if !all_skip_ext
        && lvl != LVL_OFF
        && let Some(codec) = &mut self.codec
      {
        codec.set_level(lvl)?;
        let was_skip = self.detector.skip();
        let t0 = now_us();
        if let Some(comp) = self.detector.compress(&slot.data, codec)? {
          observe_comp(
            &mut self.policy,
            lvl,
            len,
            now_us().saturating_sub(t0),
            Some(comp.len()),
          );
          slot.data.clear();
          slot.data.extend_from_slice(comp);
          true
        } else {
          if !was_skip {
            observe_comp(
              &mut self.policy,
              lvl,
              len,
              now_us().saturating_sub(t0),
              None,
            );
          }
          false
        }
      } else {
        false
      };
      slot.reset_meta(blk, zstd, self.pkt_size);
    }
    Ok(&mut self.slots[idx])
  }

  pub(crate) fn slot(&self, blk: u32) -> Option<&Blk> {
    let s = &self.slots[(blk % WINDOW_BLOCKS) as usize];
    (s.blk == blk).then_some(s)
  }

  pub(crate) fn slot_mut(&mut self, blk: u32) -> Option<&mut Blk> {
    let s = &mut self.slots[(blk % WINDOW_BLOCKS) as usize];
    (s.blk == blk).then_some(s)
  }
}
