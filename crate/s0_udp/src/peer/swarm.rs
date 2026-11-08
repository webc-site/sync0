use std::cmp::Reverse;

use crate::{
  consts::DEFAULT_SEG_PAYLOAD,
  range::seg_payload_for_addr,
  region::Region,
  wire::{SwarmPeer, decode_socket_addrs},
};

/// Dragonfly 风格 `ParentSelector`：结合城市级地域亲和度 (`Region::proximity_score`)、已就绪区间跨度与可用带宽权重对候选对等节点排序
pub fn rank_swarm_peers(local_region: Region, mut peers: Vec<SwarmPeer>) -> Vec<SwarmPeer> {
  if peers.len() > 1 {
    peers.sort_by_cached_key(|p| Reverse(peer_score(local_region, p)));
  }
  peers
}

#[inline]
pub(crate) fn peer_score(local_region: Region, peer: &SwarmPeer) -> u128 {
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
  let span = u128::from(peer.blk_hi.saturating_sub(peer.blk_lo).max(1));
  prox * bw * span
}

/// 将总块数 `[0, total_blocks)` 按候选对等节点的可用区间 `[peer.blk_lo, peer.blk_hi)` 与得分比例切分为不相交的条带区间 `[(peer_idx, blk_lo, blk_hi)]`
///
/// 数学完备性保证：
/// - 尊重部分做种/边下边传节点的真实可用区间 `[peer.blk_lo, peer.blk_hi)`；
/// - 采用累积比例舍入法（Bresenham 精确整数分摊），当某节点不覆盖当前子区间时自动分配给覆盖该区间的最高分节点，保证 `[0, total_blocks)` 100% 完备覆盖且不相交。
pub fn plan_stripes(
  local_region: Region,
  peers: &[SwarmPeer],
  total_blocks: u32,
) -> Vec<(usize, u32, u32)> {
  plan_stripes_weighted(local_region, peers, &vec![1u64; total_blocks as usize])
}

/// 按各块的真实字节权重 `block_weights` 与节点得分比例将 `[0, block_weights.len())` 切分为不相交的条带区间 `[(peer_idx, blk_lo, blk_hi)]`
///
/// 消除「大文件 + 海量小文件」混合负载下按块数均分导致的 32 倍字节偏斜：
/// - 构造严格单调递增的累积权重前缀和 `prefix_w`；
/// - 对每个节点的累积得分比例目标 `target_w` 执行 $O(\log N)$ 二分查找 (`partition_point`) 并约束在 `[cur + 1, max_next]`，确保 `[0, total_blocks)` 100% 完备覆盖且各条带字节量与节点带宽得分精确成正比。
pub fn plan_stripes_weighted(
  local_region: Region,
  peers: &[SwarmPeer],
  block_weights: &[u64],
) -> Vec<(usize, u32, u32)> {
  let total_blocks = block_weights.len() as u32;
  if total_blocks == 0 || peers.is_empty() {
    return Vec::new();
  }
  let k = peers.len().min(total_blocks as usize);
  let sum_score: u128 = peers[..k]
    .iter()
    .map(|p| peer_score(local_region, p).max(1))
    .sum();

  let mut prefix_w = Vec::with_capacity(block_weights.len() + 1);
  prefix_w.push(0u64);
  let mut acc = 0u64;
  for &w in block_weights {
    acc = acc.saturating_add(w.max(1));
    prefix_w.push(acc);
  }
  let total_w = u128::from(acc.max(1));

  let mut stripes: Vec<(usize, u32, u32)> = Vec::with_capacity(k);
  let mut prefix_score = 0u128;
  let mut cur = 0u32;
  for (i, peer) in peers[..k].iter().enumerate() {
    prefix_score += peer_score(local_region, peer).max(1);
    let next = if i + 1 == k {
      total_blocks
    } else {
      let target_w = ((total_w * prefix_score + (sum_score >> 1)) / sum_score) as u64;
      let pos = prefix_w.partition_point(|&pw| pw < target_w);
      let raw_next = if pos > 0
        && pos <= block_weights.len()
        && target_w.saturating_sub(prefix_w[pos - 1]) < prefix_w[pos].saturating_sub(target_w)
      {
        (pos - 1) as u32
      } else {
        pos as u32
      };
      let max_next = total_blocks - (k - 1 - i) as u32;
      raw_next.clamp(cur + 1, max_next)
    };
    if cur < next {
      let chosen = if peer.blk_lo <= cur && peer.blk_hi >= next {
        i
      } else {
        peers
          .iter()
          .enumerate()
          .find(|(_, p)| p.blk_lo <= cur && p.blk_hi >= next)
          .map_or(0, |(idx, _)| idx)
      };
      if let Some(last) = stripes.last_mut()
        && last.0 == chosen
        && last.2 == cur
      {
        last.2 = next;
      } else {
        stripes.push((chosen, cur, next));
      }
      cur = next;
    }
  }
  stripes
}
