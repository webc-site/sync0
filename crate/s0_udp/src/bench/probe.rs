use std::net::SocketAddr;

use crate::{
  consts::{PROBE_ACK_MAGIC, PROBE_LEN, PROBE_MAGIC},
  wire::{decode_socket_addrs, encode_socket_addrs},
};

/// 构造 12B 探测包（`PROBE_MAGIC (u32 LE) ‖ ts_us (u64 LE)`）
pub const fn make_probe(ts_us: u64) -> [u8; PROBE_LEN] {
  pack_probe(PROBE_MAGIC, ts_us)
}

/// 若输入为 12B `PROBE_MAGIC` 包，构造对应的 12B `PROBE_ACK_MAGIC` 回包
pub fn handle_probe(pkt: &[u8]) -> Option<[u8; PROBE_LEN]> {
  let (head, tail) = pkt.split_first_chunk::<4>()?;
  if u32::from_le_bytes(*head) != PROBE_MAGIC {
    return None;
  }
  let ts: &[u8; 8] = tail.try_into().ok()?;
  Some(pack_probe(PROBE_ACK_MAGIC, u64::from_le_bytes(*ts)))
}

/// 若输入为 12B `PROBE_MAGIC` 包，构造附带观测到的对端真实公网/NAT 映射 `SocketAddr`（类似 `iroh` QAD / STUN `XOR-MAPPED-ADDRESS`）的回包（总长 19B IPv4 / 31B IPv6，严格 ≤ 3×12B 抗放大上限）
pub fn handle_probe_observed(pkt: &[u8], observed: SocketAddr) -> Option<Vec<u8>> {
  let base = handle_probe(pkt)?;
  let addr_bytes = encode_socket_addrs(&[observed]);
  let mut out = Vec::with_capacity(PROBE_LEN + addr_bytes.len());
  out.extend_from_slice(&base);
  out.extend_from_slice(&addr_bytes);
  Some(out)
}

/// 解析 `PROBE_ACK_MAGIC` 回包并计算 RTT（微秒）
pub fn parse_probe_ack(pkt: &[u8], now_us: u64) -> Option<u64> {
  parse_probe_ack_observed(pkt, now_us).map(|(rtt, _)| rtt)
}

/// 解析 `PROBE_ACK_MAGIC` 回包，返回 `(rtt_us, 对端观测到的本机真实外部 SocketAddr)`
pub fn parse_probe_ack_observed(pkt: &[u8], now_us: u64) -> Option<(u64, Option<SocketAddr>)> {
  let (head, tail) = pkt.split_first_chunk::<4>()?;
  if u32::from_le_bytes(*head) != PROBE_ACK_MAGIC {
    return None;
  }
  let (ts, rest) = tail.split_first_chunk::<8>()?;
  let rtt = now_us.saturating_sub(u64::from_le_bytes(*ts));
  let observed = if rest.is_empty() {
    None
  } else {
    decode_socket_addrs(rest)
      .ok()
      .and_then(|v| v.first().copied())
  };
  Some((rtt, observed))
}

const fn pack_probe(magic: u32, ts_us: u64) -> [u8; PROBE_LEN] {
  let m = magic.to_le_bytes();
  let t = ts_us.to_le_bytes();
  [
    m[0], m[1], m[2], m[3], t[0], t[1], t[2], t[3], t[4], t[5], t[6], t[7],
  ]
}
