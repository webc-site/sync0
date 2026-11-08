mod rx;
mod tx;

use std::{
  net::{SocketAddr, UdpSocket},
  time::Duration,
};

pub use rx::recv_range_udp;
pub use tx::send_range_udp;

pub(crate) use crate::consts::TUNNEL_SEG_PAYLOAD;
use crate::{
  consts::{DEFAULT_SEG_PAYLOAD, PKT_ACK, PKT_NACK, PKT_PUNCH},
  is_tunnel_addr, is_tunnel_path, now_us, wait_until,
  wire::{AckPkt, NackPkt, decode_punch},
};

#[inline]
pub(crate) const fn seg_payload_for_addr(addr: SocketAddr) -> usize {
  if is_tunnel_addr(addr) {
    TUNNEL_SEG_PAYLOAD
  } else {
    DEFAULT_SEG_PAYLOAD
  }
}

#[inline]
pub(crate) fn seg_payload_for_sock(sock: &UdpSocket, peer: SocketAddr) -> usize {
  if is_tunnel_path(sock, peer) {
    TUNNEL_SEG_PAYLOAD
  } else {
    DEFAULT_SEG_PAYLOAD
  }
}

#[inline]
pub(super) fn is_valid_ctrl_sid(pkt: &[u8], sid24: u32) -> bool {
  match pkt.first().copied() {
    Some(PKT_PUNCH) => decode_punch(pkt) == Ok(sid24),
    Some(PKT_NACK) => NackPkt::decode(pkt).is_ok_and(|p| p.session_id == sid24),
    Some(PKT_ACK) => AckPkt::decode(pkt).is_ok_and(|p| p.session_id == sid24),
    _ => false,
  }
}

/// 单区间滑动窗口缓存槽位数（2048 槽位完整覆盖单区间块与 SuperBlock，使 NACK 重传零磁盘读取、零二次压缩）
pub(crate) const TX_CACHE_SLOTS: usize = 2048;
/// 单次 NACK 批处理最大重传块数
pub(crate) const MAX_NACK_RESEND_BATCH: u32 = 512;
/// 单个对等节点允许的最大块摘要校验失败次数（达到阈值判定为恶意投毒并拉黑驱逐）
pub(crate) const MAX_POISON_STRIKES: u8 = 3;
/// 广域网单节点默认平滑发送速率（48 MB/s = 384 Mbps，匹配高 BDP 跨国链路并由微突发 Pacer 平滑防溢出）
pub(crate) const WAN_DEFAULT_RATE_BPS: u64 = 48_000_000;
/// SuperBlock 聚合小文件与可压小分块的单块最大触发阈值（32 KiB）
pub(crate) const SUPER_SMALL_LIMIT: u32 = 32 * 1024;
/// SuperBlock 聚合后的最大原始字节数（64 KiB）
pub(crate) const SUPER_MAX_BYTES: usize = 64 * 1024;
/// SuperBlock 最多聚合的额外块数（`offset_in_block >> 24`，最多 1 + 63 = 64 个小块）
pub(crate) const SUPER_MAX_EXTRA: u8 = 63;

/// 令牌桶微突发平滑限速器（限制空闲累积令牌上限为 2 个 MTU 包并强制最小包间隔，防止高压缩小包突发冲垮路由器 PPS 队列）
pub struct Pacer {
  rate_bps: u64,
  next_send_us: u64,
}

impl Pacer {
  pub fn new(rate_bps: u64) -> Self {
    Self {
      rate_bps,
      next_send_us: now_us(),
    }
  }

  #[inline]
  pub fn on_send(&mut self, bytes: usize) {
    if self.rate_bps == 0 {
      return;
    }
    let now = now_us();
    let max_burst_bytes = if bytes < 1000 { 16_384u128 } else { 65_536u128 };
    let max_burst_us = ((max_burst_bytes * 1_000_000) / u128::from(self.rate_bps)) as u64;
    if self.next_send_us.saturating_add(max_burst_us) < now {
      self.next_send_us = now.saturating_sub(max_burst_us);
    }
    let eff_bytes = bytes.max(1024) as u128;
    let step_us = ((eff_bytes * 1_000_000) / u128::from(self.rate_bps)).max(1) as u64;
    self.next_send_us = self.next_send_us.saturating_add(step_us);
    if self.next_send_us > now + 200 {
      wait_until(self.next_send_us);
    }
  }
}

/// 区间传输统计信息
#[derive(Debug, Clone, Copy, Default)]
pub struct UdpStats {
  pub bytes_transferred: u64,
  pub blocks_transferred: u32,
  pub elapsed_us: u64,
}

/// UDP 区间收发任务参数
#[derive(Debug, Clone, Copy)]
pub struct RangeSpec {
  pub session_id: u32,
  pub blk_lo: u32,
  pub blk_hi: u32,
  pub eff_rate: u64,
  pub timeout: Duration,
}

/// 发送端滑动窗口缓存槽位（复用已压缩字节流，NACK 重传直接发内存分片）
pub(crate) struct TxCacheSlot {
  pub(crate) block_idx: u32,
  pub(crate) extra_blocks: u8,
  pub(crate) raw_len: u32,
  pub(crate) zstd: bool,
  pub(crate) wire_data: Vec<u8>,
}

impl Default for TxCacheSlot {
  fn default() -> Self {
    Self {
      block_idx: u32::MAX,
      extra_blocks: 0,
      raw_len: 0,
      zstd: false,
      wire_data: Vec::new(),
    }
  }
}

impl TxCacheSlot {
  #[inline]
  pub(crate) fn seg_cnt(&self, seg_payload: usize) -> u16 {
    self.wire_data.len().div_ceil(seg_payload).max(1) as u16
  }
}
