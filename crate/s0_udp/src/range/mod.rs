mod rx;
mod tx;

use std::{
  net::{SocketAddr, UdpSocket},
  time::Duration,
};

use gxhash::HashMap;
pub use rx::recv_range_udp;
use s0_cc::AntiAmp;
use s0_codec::{Codec, Detector};
use s0_const::{
  ACK_BURST_COUNT, ACK_PKT_LEN, DATA_FLAG_ZSTD, DATA_HDR_LEN, DEFAULT_SEG_PAYLOAD, MAX_UDP_PKT,
  MIN_ZSTD_BYTES, NACK_MAX_PARTIALS, NACK_MAX_SPANS, PKT_ACK, PKT_NACK, PKT_PUNCH, SID24_MASK,
  SUPER_EXTRA_SHIFT, TUNNEL_SEG_PAYLOAD,
};
use s0_fs::{FsError, SharedSink};
use s0_wire::{AckPkt, DataHeader, NackPkt, decode_punch};
pub use tx::send_range_udp;

use crate::{UdpError, is_emsgsize, is_tunnel_addr, is_tunnel_path, now_us, send_udp, wait_until};

/// 根据目标 `SocketAddr` 判断单包有效载荷上限（物理网卡 `1380B` / Overlay 隧道网卡 `1200B`）
#[inline]
pub const fn seg_payload_for_addr(addr: SocketAddr) -> usize {
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
pub(crate) fn is_valid_ctrl_sid(pkt: &[u8], sid24: u32) -> bool {
  match pkt.first().copied() {
    Some(PKT_PUNCH) => decode_punch(pkt) == Ok(sid24),
    Some(PKT_NACK) => NackPkt::decode(pkt).is_ok_and(|p| p.session_id == sid24),
    Some(PKT_ACK) => AckPkt::decode(pkt).is_ok_and(|p| p.session_id == sid24),
    _ => false,
  }
}

/// 单区间滑动窗口缓存槽位数（4096 槽位配合斐波那契乘法散列，消除 SuperBlock 64 步长别名冲突，使 NACK 重传零磁盘读取、零二次压缩）
pub(crate) const TX_CACHE_SLOTS: usize = 4096;

#[inline]
pub(crate) const fn cache_slot_idx(block_idx: u32) -> usize {
  (block_idx.wrapping_mul(0x9E37_79B9) as usize) & (TX_CACHE_SLOTS - 1)
}
/// 单次 NACK 批处理最大重传块数
pub(crate) const MAX_NACK_RESEND_BATCH: u32 = 512;
/// 广域网单节点默认平滑发送速率（48 MB/s = 384 Mbps，匹配高 BDP 跨国链路并由微突发 Pacer 平滑防溢出）
pub(crate) const WAN_DEFAULT_RATE_BPS: u64 = 48_000_000;
/// 高时延跨洋链路（RTT >= 80 ms）慢启动初始发送速率（4 MB/s = 32 Mbps，避免首轮突发挤爆跨洋瓶颈队列，随后根据进度 ACK 实测交付速率动态伸缩）
pub(crate) const WAN_HIGH_RTT_INIT_RATE_BPS: u64 = 4_000_000;
/// SuperBlock 聚合小文件与可压小分块的单块最大触发阈值（32 KiB）
pub(crate) const SUPER_SMALL_LIMIT: u32 = 32 * 1024;
/// 不可压小文件 SuperBlock 聚合单块上限（16_000 B）
pub(crate) const SUPER_INCOMP_LIMIT: u32 = 16_000;
/// SuperBlock 聚合后的最大原始字节数（64 KiB）
pub(crate) const SUPER_MAX_BYTES: usize = 64 * 1024;
/// SuperBlock 最多聚合的额外块数（`offset_in_block >> 24`，最多 1 + 63 = 64 个小块）
pub(crate) const SUPER_MAX_EXTRA: u8 = 63;
/// 回环链路初始重传与连续重发冷却间隔（10 ms）
pub(crate) const LOOPBACK_COOLDOWN_US: u64 = 10_000;
/// 广域网首次发送后的默认重传冷却间隔（85 ms）
pub(crate) const WAN_INIT_COOLDOWN_US: u64 = 85_000;
/// 广域网连续重传之间的默认冷却间隔（110 ms）
pub(crate) const WAN_RETRANS_COOLDOWN_US: u64 = 110_000;
/// 回环链路周期性与空闲 NACK 间隔（15 ms）
pub(crate) const LOOPBACK_NACK_US: u64 = 15_000;
/// 广域网周期性 NACK 默认间隔（140 ms）
pub(crate) const WAN_NACK_US: u64 = 140_000;
/// 广域网空闲尾包 NACK 默认触发间隔（36 ms）
pub(crate) const WAN_IDLE_NACK_US: u64 = 36_000;
/// 广域网乱序空洞 NACK 默认最小间隔（20 ms）
pub(crate) const WAN_GAP_NACK_US: u64 = 20_000;

/// 根据实测 RTT 计算发送端 `(首次重传冷却微秒, 连续重传冷却微秒)`
#[inline]
pub(crate) fn cooldowns_for_rtt(is_loopback: bool, rtt_us: u64) -> (u64, u64) {
  if is_loopback {
    return (LOOPBACK_COOLDOWN_US, LOOPBACK_COOLDOWN_US);
  }
  if rtt_us == 0 {
    return (WAN_INIT_COOLDOWN_US, WAN_RETRANS_COOLDOWN_US);
  }
  let init = (rtt_us * 3 / 4).clamp(20_000, 30_000_000);
  let retrans = (rtt_us + (rtt_us >> 4)).clamp(35_000, 30_000_000);
  (init, retrans)
}

/// 根据实测 RTT 计算接收端 `(周期 NACK 微秒, 空闲尾包 NACK 微秒, 乱序空洞 NACK 微秒)`
#[inline]
pub(crate) fn nack_timers_for_rtt(is_loopback: bool, rtt_us: u64) -> (u64, u64, u64) {
  if is_loopback {
    return (LOOPBACK_NACK_US, LOOPBACK_NACK_US, 2_000);
  }
  if rtt_us == 0 {
    return (WAN_NACK_US, WAN_IDLE_NACK_US, WAN_GAP_NACK_US);
  }
  let nack = (rtt_us + (rtt_us >> 2)).clamp(60_000, 30_000_000);
  let idle = (rtt_us >> 1).clamp(25_000, 15_000_000);
  let gap = (rtt_us >> 2).clamp(12_000, 5_000_000);
  (nack, idle, gap)
}

/// 令牌桶微突发平滑限速器（限制空闲累积令牌上限为 2 个 MTU 包并强制最小包间隔，防止高压缩小包突发冲垮路由器 PPS 队列）
pub(crate) struct Pacer {
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
  pub const fn rate(&self) -> u64 {
    self.rate_bps
  }

  #[inline]
  pub const fn set_rate(&mut self, rate_bps: u64) {
    self.rate_bps = rate_bps;
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

  pub(crate) fn store(
    &mut self,
    block_idx: u32,
    extra_blocks: u8,
    raw_buf: &[u8],
    detector: &mut Detector,
    codec: &mut Codec,
  ) {
    self.block_idx = block_idx;
    self.extra_blocks = extra_blocks;
    self.raw_len = raw_buf.len() as u32;
    self.wire_data.clear();
    if raw_buf.len() >= MIN_ZSTD_BYTES
      && let Ok(Some(comp)) = detector.compress(raw_buf, codec)
    {
      self.wire_data.extend_from_slice(comp);
      self.zstd = true;
    } else {
      self.wire_data.extend_from_slice(raw_buf);
      self.zstd = false;
    }
  }

  pub(crate) fn transmit(
    &self,
    (sock, peer_addr, session_id): (&UdpSocket, SocketAddr, u32),
    seg_payload: &mut usize,
    mut have_seg_mask: u64,
    dup_tail: bool,
    pkt_buf: &mut [u8; MAX_UDP_PKT],
    pacer: &mut Pacer,
  ) -> Result<(), UdpError> {
    'retry_mtu: loop {
      let (sid, sp) = (session_id & SID24_MASK, *seg_payload);
      let (block_idx, total_len) = (self.block_idx, self.wire_data.len());
      let extra_prefix = u32::from(self.extra_blocks) << SUPER_EXTRA_SHIFT;
      let seg_cnt = self.seg_cnt(sp);
      let flagged_cnt = seg_cnt | if self.zstd { DATA_FLAG_ZSTD } else { 0 };
      for seg_idx in 0..seg_cnt {
        if seg_idx < 64 && (have_seg_mask & (1u64 << seg_idx)) != 0 {
          continue;
        }
        let off = (seg_idx as usize) * sp;
        let payload = &self.wire_data[off..(off + sp).min(total_len)];
        DataHeader {
          session_id: sid,
          block_idx,
          seg_idx,
          seg_cnt: flagged_cnt,
          offset_in_block: extra_prefix | (off as u32),
          payload_len: payload.len() as u16,
        }
        .encode((&mut pkt_buf[..DATA_HDR_LEN]).try_into().unwrap());
        pkt_buf[DATA_HDR_LEN..DATA_HDR_LEN + payload.len()].copy_from_slice(payload);
        let pkt_len = DATA_HDR_LEN + payload.len();
        if let Err(e) = send_udp(sock, &pkt_buf[..pkt_len], peer_addr) {
          if let UdpError::Io(ref io_err) = e
            && is_emsgsize(io_err)
            && *seg_payload > TUNNEL_SEG_PAYLOAD
          {
            *seg_payload = TUNNEL_SEG_PAYLOAD;
            have_seg_mask = 0;
            continue 'retry_mtu;
          }
          return Err(e);
        }
        pacer.on_send(pkt_len);
        if dup_tail && seg_idx + 2 >= seg_cnt {
          let _ = send_udp(sock, &pkt_buf[..pkt_len], peer_addr);
        }
      }
      return Ok(());
    }
  }
}

/// 活跃分片重组槽位（仅在块尚未收齐分片期间驻留，收齐后立即回收 `buf` 到缓冲池，空间复杂度 $O(\text{乱序活跃块})$）
pub(crate) struct PartialSlot {
  pub(crate) extra_blocks: u8,
  pub(crate) seg_mask: u64,
  pub(crate) seg_cnt: u16,
  pub(crate) received_segs: u16,
  pub(crate) zstd: bool,
  pub(crate) buf: Vec<u8>,
}

/// 解压并落盘校验块长（支持单块与 SuperBlock 跨小文件批量解包，长度或压缩帧异常立即返回 `BlockCorrupted`）
pub(crate) fn unpack_and_verify(
  sink: &SharedSink,
  codec: &mut Codec,
  dec_buf: &mut [u8],
  start_blk: u32,
  extra_blocks: u8,
  is_zstd: bool,
  wire_bytes: &[u8],
) -> Result<(usize, u32), FsError> {
  let raw_slice = if is_zstd {
    let raw_len = codec
      .decompress_block(wire_bytes, dec_buf)
      .map_err(|_| FsError::BlockCorrupted(start_blk))?;
    &dec_buf[..raw_len]
  } else {
    wire_bytes
  };
  if extra_blocks == 0 {
    let newly_written = sink.write_verified_block(start_blk, raw_slice)?;
    return Ok(if newly_written {
      (raw_slice.len(), 1)
    } else {
      (0, 0)
    });
  }
  sink.write_superblock(start_blk, 1 + u32::from(extra_blocks), raw_slice)
}

pub(crate) fn send_nack_for_range(
  (sock, targets, session_id): (&UdpSocket, &[SocketAddr], u32),
  sink: &SharedSink,
  range: (u32, u32),
  partials_map: &HashMap<u32, PartialSlot>,
  nack_buf: &mut Vec<u8>,
  mut amp: Option<&mut AntiAmp<SocketAddr>>,
) -> Result<(), UdpError> {
  let mut spans = Vec::new();
  sink.for_each_missing_span(range.0, range.1, |lo, hi| {
    spans.push((lo, hi));
  });
  let mut partials = Vec::with_capacity(partials_map.len().min(NACK_MAX_PARTIALS));
  for chunk in spans.chunks(NACK_MAX_SPANS) {
    partials.clear();
    if !partials_map.is_empty() {
      'outer: for &(lo, hi) in chunk {
        for blk in lo..hi {
          if let Some(slot) = partials_map.get(&blk)
            && slot.seg_mask != 0
          {
            partials.push((blk, slot.seg_mask));
            if partials.len() >= NACK_MAX_PARTIALS {
              break 'outer;
            }
          }
        }
      }
    }
    nack_buf.clear();
    NackPkt::encode_parts(session_id, chunk, &partials, nack_buf)?;
    for addr in targets {
      if let Some(a) = amp.as_deref_mut() {
        if !a.can_send(nack_buf.len()) {
          return Ok(());
        }
        a.on_send(nack_buf.len());
      }
      let _ = sock.send_to(nack_buf, addr);
    }
  }
  Ok(())
}

pub(crate) fn send_ack_pkts(
  sock: &UdpSocket,
  targets: &[SocketAddr],
  session_id: u32,
  blk_lo: u32,
  blk_hi: u32,
  count: usize,
) {
  let mut raw = [0u8; ACK_PKT_LEN];
  AckPkt {
    session_id,
    blk_lo,
    blk_hi,
  }
  .encode(&mut raw);
  for _ in 0..count {
    for addr in targets {
      let _ = sock.send_to(&raw, addr);
    }
  }
}

#[inline]
pub(crate) fn send_ack_burst(
  sock: &UdpSocket,
  targets: &[SocketAddr],
  session_id: u32,
  blk_lo: u32,
  blk_hi: u32,
) {
  send_ack_pkts(sock, targets, session_id, blk_lo, blk_hi, ACK_BURST_COUNT);
}
