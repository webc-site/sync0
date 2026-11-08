use std::cmp::Reverse;

use crate::{
  consts::DEFAULT_SEG_PAYLOAD,
  range::seg_payload_for_addr,
  region::Region,
  wire::{SwarmPeer, decode_socket_addrs},
};

/// Dragonfly 风格 `ParentSelector`：结合城市级地域亲和度 (`Region::proximity_score`) 与可用带宽权重对候选对等节点排序
pub fn rank_swarm_peers(local_region: Region, mut peers: Vec<SwarmPeer>) -> Vec<SwarmPeer> {
  if peers.len() > 1 {
    peers.sort_by_cached_key(|p| Reverse(peer_score(local_region, p)));
  }
  peers
}

#[inline]
fn peer_score(local_region: Region, peer: &SwarmPeer) -> u128 {
  let prox = u128::from(local_region.proximity_score(peer.region));
  let bw = u128::from(if peer.weight_rate == 0 {
    let is_direct = decode_socket_addrs(&peer.peer_addrs)
      .ok()
      .and_then(|v| v.first().copied())
      .is_none_or(|a| seg_payload_for_addr(a) == DEFAULT_SEG_PAYLOAD);
    if is_direct { 140_000_000 } else { 100_000_000 }
  } else {
    peer.weight_rate
  });
  prox * bw
}

/// 将总块数 `[0, total_blocks)` 按候选对等节点的得分比例切分为不相交的条带区间 `[(peer_idx, blk_lo, blk_hi)]`
/// 采用无堆分配的累积比例舍入法（Bresenham 精确整数分摊），保证每个被选中节点至少分配 1 块且总误差 $\le 1$ 块
pub fn plan_stripes(
  local_region: Region,
  peers: &[SwarmPeer],
  total_blocks: u32,
) -> Vec<(usize, u32, u32)> {
  if total_blocks == 0 || peers.is_empty() {
    return Vec::new();
  }
  let k = peers.len().min(total_blocks as usize);
  let sum_score: u128 = peers[..k]
    .iter()
    .map(|p| peer_score(local_region, p).max(1))
    .sum();
  let surplus = u128::from(total_blocks - k as u32);

  let mut stripes = Vec::with_capacity(k);
  let mut prefix_score = 0u128;
  let mut cur = 0u32;
  for (i, peer) in peers[..k].iter().enumerate() {
    prefix_score += peer_score(local_region, peer).max(1);
    let next = if i + 1 == k {
      total_blocks
    } else {
      let extra = ((surplus * prefix_score + (sum_score >> 1)) / sum_score) as u32;
      ((i as u32 + 1) + extra).clamp(cur + 1, total_blocks)
    };
    if cur < next {
      stripes.push((i, cur, next));
      cur = next;
    }
  }
  stripes
}
