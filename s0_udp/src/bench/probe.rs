pub use s0_const::{PROBE_ACK_MAGIC, PROBE_LEN, PROBE_MAGIC};

/// 构造 12B 探测包（`PROBE_MAGIC (u32 LE) ‖ ts_us (u64 LE)`）
pub const fn make_probe(ts_us: u64) -> [u8; PROBE_LEN] {
  pack_probe(PROBE_MAGIC, ts_us)
}

/// 若输入为 12B `PROBE_MAGIC` 包，构造对应的 `PROBE_ACK_MAGIC` 回包
pub fn handle_probe(pkt: &[u8]) -> Option<[u8; PROBE_LEN]> {
  let (head, tail) = pkt.split_first_chunk::<4>()?;
  if u32::from_le_bytes(*head) != PROBE_MAGIC {
    return None;
  }
  let ts: &[u8; 8] = tail.try_into().ok()?;
  Some(pack_probe(PROBE_ACK_MAGIC, u64::from_le_bytes(*ts)))
}

/// 解析 12B `PROBE_ACK_MAGIC` 回包并计算 RTT（微秒）
pub fn parse_probe_ack(pkt: &[u8], now_us: u64) -> Option<u64> {
  let (head, tail) = pkt.split_first_chunk::<4>()?;
  if u32::from_le_bytes(*head) != PROBE_ACK_MAGIC {
    return None;
  }
  let ts = tail.first_chunk::<8>()?;
  Some(now_us.saturating_sub(u64::from_le_bytes(*ts)))
}

const fn pack_probe(magic: u32, ts_us: u64) -> [u8; PROBE_LEN] {
  let m = magic.to_le_bytes();
  let t = ts_us.to_le_bytes();
  [
    m[0], m[1], m[2], m[3], t[0], t[1], t[2], t[3], t[4], t[5], t[6], t[7],
  ]
}
