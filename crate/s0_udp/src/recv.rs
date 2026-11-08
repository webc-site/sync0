use gxhash::gxhash64;

use crate::{
  UdpError,
  consts::{DEFAULT_PKT, MAX_BLOCK, MAX_CNT, TUNNEL_PKT, WINDOW_BLOCKS},
  wire::StreamDataHeader,
};

/// 槽位上限
const MAX_SLOTS: usize = 65_536;
/// 单会话块池内存上限（1 GiB）
const MAX_POOL: usize = 1024 * 1024 * 1024;
const MIN_PKT: usize = 256;

const fn check_slots(slots: usize, name: &'static str) -> Result<usize, UdpError> {
  if slots == 0 || !slots.is_power_of_two() || slots > MAX_SLOTS {
    return Err(UdpError::Arg(name));
  }
  Ok(slots - 1)
}

/// 接收端配置
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
  pub slots: usize,
  pub block: usize,
  pub pkt: usize,
  pub accept_zstd: bool,
}

impl Config {
  pub fn new(slots: usize, block: usize, pkt: usize) -> Result<Self, UdpError> {
    check_slots(slots, "slots")?;
    if block == 0 || block > MAX_BLOCK {
      return Err(UdpError::Arg("block"));
    }
    if pkt < MIN_PKT || pkt > block {
      return Err(UdpError::Arg("pkt"));
    }
    if block.div_ceil(pkt) > MAX_CNT {
      return Err(UdpError::Arg("block/pkt"));
    }
    slots
      .checked_mul(block)
      .filter(|&m| m <= MAX_POOL)
      .ok_or(UdpError::Arg("mem"))?;
    Ok(Self {
      slots,
      block,
      pkt,
      accept_zstd: false,
    })
  }

  const fn pool_bytes(&self) -> usize {
    self.slots * self.block
  }

  pub const fn with_zstd(mut self) -> Self {
    self.accept_zstd = true;
    self
  }

  const fn slots_mask(&self) -> usize {
    self.slots - 1
  }
}

impl Default for Config {
  fn default() -> Self {
    Self {
      slots: WINDOW_BLOCKS as usize,
      block: MAX_BLOCK,
      pkt: 1416,
      accept_zstd: false,
    }
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum State {
  #[default]
  Empty,
  Filling,
  Ready,
  Done,
}

#[derive(Clone, Copy, Debug, Default)]
struct Slot {
  blk: u32,
  have: u128,
  cnt: u8,
  zstd: bool,
  pkt: u16,
  last: u32,
  len: u32,
  state: State,
}

impl Slot {
  fn reset(&mut self, blk: u32, cnt: u8, zstd: bool, pkt: u16) {
    *self = Self {
      blk,
      cnt,
      zstd,
      pkt,
      state: State::Filling,
      ..Self::default()
    };
  }
}

/// 单包处理结果
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
  Stored,
  BlockReady { blk: u32, len: usize, sum: u64 },
  Duplicate,
  Stale,
  NoCredit,
  Ignored,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RecvStats {
  pub packets: u64,
  pub bytes: u64,
  pub dup: u64,
  pub stale: u64,
  pub no_credit: u64,
  pub blocks: u64,
}

/// 接收端块组装（sans-IO，内存预分配）
pub struct Receiver {
  cfg: Config,
  buf: Box<[u8]>,
  meta: Box<[Slot]>,
  done_lo: u32,
  end: u32,
  cur_pkt: usize,
  seed: i64,
  stats: RecvStats,
}

impl Receiver {
  pub fn new(cfg: Config, seed: u64) -> Result<Self, UdpError> {
    let cfg = Config {
      accept_zstd: cfg.accept_zstd,
      ..Config::new(cfg.slots, cfg.block, cfg.pkt)?
    };
    let buf = vec![0u8; cfg.pool_bytes()].into_boxed_slice();
    let meta = vec![Slot::default(); cfg.slots].into_boxed_slice();
    Ok(Self {
      cfg,
      buf,
      meta,
      done_lo: 0,
      end: 0,
      cur_pkt: cfg.pkt,
      seed: seed as i64,
      stats: RecvStats::default(),
    })
  }

  pub const fn config(&self) -> Config {
    self.cfg
  }

  pub const fn done_lo(&self) -> u32 {
    self.done_lo
  }

  pub const fn credit(&self) -> u32 {
    self.done_lo.saturating_add(self.cfg.slots as u32)
  }

  pub const fn stats(&self) -> RecvStats {
    self.stats
  }

  /// 处理一个 DATA 包
  pub fn on_data(&mut self, h: &StreamDataHeader, payload: &[u8]) -> Result<Outcome, UdpError> {
    if h.is_par() {
      return Ok(Outcome::Ignored);
    }
    if h.is_zstd() && !self.cfg.accept_zstd {
      return Err(UdpError::Arg("ZSTD 块"));
    }
    if h.cnt == 0 || usize::from(h.cnt) > MAX_CNT || h.idx >= h.cnt {
      return Err(UdpError::Arg("DATA idx/cnt"));
    }
    let blk = h.blk;
    if blk < self.done_lo {
      self.stats.stale += 1;
      return Ok(Outcome::Stale);
    }
    if blk >= self.credit() {
      self.stats.no_credit += 1;
      return Ok(Outcome::NoCredit);
    }
    if payload.is_empty() {
      return Err(UdpError::Corrupt);
    }
    let tail = h.idx + 1 == h.cnt;
    let allow_tunnel = self.cfg.pkt == DEFAULT_PKT;
    let max_default_cnt = self.cfg.block.div_ceil(self.cfg.pkt);
    let i = blk as usize & self.cfg.slots_mask();
    let start = i * self.cfg.block;

    let pkt_step = if !tail {
      if payload.len() != self.cfg.pkt && !(allow_tunnel && payload.len() == TUNNEL_PKT) {
        return Err(UdpError::Corrupt);
      }
      self.cur_pkt = payload.len();
      payload.len()
    } else if allow_tunnel && usize::from(h.cnt) > max_default_cnt {
      self.cur_pkt = TUNNEL_PKT;
      TUNNEL_PKT
    } else {
      let existing = &self.meta[i];
      if matches!(existing.state, State::Filling | State::Ready) && existing.blk == blk {
        if existing.cnt == h.cnt && existing.pkt > 0 {
          usize::from(existing.pkt)
        } else if allow_tunnel && h.cnt > existing.cnt {
          self.cur_pkt = TUNNEL_PKT;
          TUNNEL_PKT
        } else {
          self.cur_pkt
        }
      } else {
        self.cur_pkt
      }
    };

    if payload.len() > pkt_step.max(if h.cnt == 1 { self.cfg.pkt } else { pkt_step }) {
      return Err(UdpError::Corrupt);
    }

    let slot = &mut self.meta[i];
    match slot.state {
      State::Filling | State::Ready => {
        if slot.blk != blk || slot.zstd != h.is_zstd() {
          return Err(UdpError::Corrupt);
        }
        if slot.cnt != h.cnt {
          if allow_tunnel && (pkt_step == TUNNEL_PKT || usize::from(h.cnt) > max_default_cnt) {
            slot.reset(blk, h.cnt, h.is_zstd(), TUNNEL_PKT as u16);
          } else if allow_tunnel && slot.pkt == TUNNEL_PKT as u16 && h.cnt < slot.cnt {
            self.stats.dup += 1;
            return Ok(Outcome::Duplicate);
          } else {
            return Err(UdpError::Corrupt);
          }
        } else if !tail && slot.pkt != pkt_step as u16 {
          let tail_bit = 1u128 << (h.cnt - 1);
          if allow_tunnel && slot.have == tail_bit {
            let old_off = start + usize::from(h.cnt - 1) * usize::from(slot.pkt);
            let new_off = start + usize::from(h.cnt - 1) * pkt_step;
            let last_len = slot.last as usize;
            if new_off + last_len > start + self.cfg.block {
              return Err(UdpError::Corrupt);
            }
            self.buf.copy_within(old_off..old_off + last_len, new_off);
            slot.pkt = pkt_step as u16;
          } else if allow_tunnel && slot.pkt == TUNNEL_PKT as u16 && pkt_step > TUNNEL_PKT {
            self.stats.dup += 1;
            return Ok(Outcome::Duplicate);
          } else {
            return Err(UdpError::Corrupt);
          }
        }
        if slot.have >> h.idx & 1 == 1 {
          self.stats.dup += 1;
          return Ok(Outcome::Duplicate);
        }
      }
      State::Done if slot.blk == blk => {
        self.stats.dup += 1;
        return Ok(Outcome::Duplicate);
      }
      _ => slot.reset(blk, h.cnt, h.is_zstd(), pkt_step as u16),
    }

    let eff_pkt = usize::from(slot.pkt);
    let idx = usize::from(h.idx);
    let pkt_off = idx * eff_pkt;
    let end_off = pkt_off + payload.len();
    if end_off > self.cfg.block {
      return Err(UdpError::Corrupt);
    }
    self.end = self.end.max(blk + 1);
    if tail {
      slot.last = payload.len() as u32;
    }

    let base = start + pkt_off;
    self.buf[base..base + payload.len()].copy_from_slice(payload);
    slot.have |= 1u128 << h.idx;
    self.stats.packets += 1;
    self.stats.bytes += payload.len() as u64;

    if slot.have.count_ones() != u32::from(h.cnt) {
      return Ok(Outcome::Stored);
    }

    let len = (usize::from(h.cnt) - 1) * eff_pkt + slot.last as usize;
    slot.len = len as u32;
    slot.state = State::Ready;
    self.stats.blocks += 1;
    let sum = gxhash64(&self.buf[start..start + len], self.seed);
    Ok(Outcome::BlockReady { blk, len, sum })
  }

  /// 已收齐但未释放的块数据
  pub fn data(&self, blk: u32) -> Option<&[u8]> {
    let i = blk as usize & self.cfg.slots_mask();
    let slot = self.meta[i];
    if slot.blk != blk || slot.state != State::Ready {
      return None;
    }
    let base = i * self.cfg.block;
    Some(&self.buf[base..base + slot.len as usize])
  }

  /// 未收齐块已收包的位图（供 ACK 卡住块上报）
  pub fn partial(&self, blk: u32) -> Option<u128> {
    let slot = self.meta[blk as usize & self.cfg.slots_mask()];
    (slot.blk == blk && slot.state == State::Filling).then_some(slot.have)
  }

  pub fn release(&mut self, blk: u32) -> Result<(), UdpError> {
    if blk < self.done_lo || blk >= self.credit() {
      return Err(UdpError::Corrupt);
    }
    let mask = self.cfg.slots_mask();
    let slot = &mut self.meta[blk as usize & mask];
    if slot.blk != blk || slot.state != State::Ready {
      return Err(UdpError::Corrupt);
    }
    slot.state = State::Done;

    while self.done_lo < u32::MAX {
      let s = self.meta[self.done_lo as usize & mask];
      if s.state != State::Done || s.blk != self.done_lo {
        break;
      }
      self.done_lo += 1;
    }
    Ok(())
  }
}
