use crate::{s0_region::Region, s0_wire::SwarmPeer};

/// Dragonfly 风格 `ParentSelector`：结合城市级地域亲和度 (`Region::proximity_score`) 与可用带宽权重对候选对等节点排序
pub fn rank_swarm_peers(local_region: Region, mut peers: Vec<SwarmPeer>) -> Vec<SwarmPeer> {
  peers.sort_unstable_by(|a, b| {
    let score_a = peer_score(local_region, a);
    let score_b = peer_score(local_region, b);
    score_b.cmp(&score_a)
  });
  peers
}

#[inline]
fn peer_score(local_region: Region, peer: &SwarmPeer) -> u128 {
  let prox = u128::from(local_region.proximity_score(peer.region));
  let bw = u128::from(if peer.weight_rate == 0 {
    100_000_000
  } else {
    peer.weight_rate
  });
  prox * bw
}

/// 将总块数 `[0, total_blocks)` 按候选对等节点的得分比例切分为不相交的条带区间 `[(peer_idx, blk_lo, blk_hi)]`
pub fn plan_stripes(
  local_region: Region,
  peers: &[SwarmPeer],
  total_blocks: u32,
) -> Vec<(usize, u32, u32)> {
  if total_blocks == 0 || peers.is_empty() {
    return Vec::new();
  }
  let k = (peers.len() as u32).min(total_blocks) as usize;
  let scores: Vec<u128> = peers[..k]
    .iter()
    .map(|p| peer_score(local_region, p).max(1))
    .collect();
  let sum_score: u128 = scores.iter().sum();

  let mut stripes = Vec::with_capacity(k);
  let mut cur = 0u32;
  for (i, &sc) in scores.iter().enumerate() {
    if cur >= total_blocks {
      break;
    }
    let next = if i + 1 == k {
      total_blocks
    } else {
      let share = ((u128::from(total_blocks) * sc) / sum_score).max(1) as u32;
      (cur + share).min(total_blocks)
    };
    if cur < next {
      stripes.push((i, cur, next));
      cur = next;
    }
  }
  if cur < total_blocks
    && let Some(last) = stripes.last_mut()
  {
    last.2 = total_blocks;
  }
  stripes
}
