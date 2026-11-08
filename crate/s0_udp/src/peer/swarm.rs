use std::cmp::Reverse;

use crate::{
  consts::DEFAULT_SEG_PAYLOAD,
  range::seg_payload_for_addr,
  region::Region,
  wire::{SwarmPeer, decode_socket_addrs},
};

/// 点播首屏优先窗口默认块数（16 块 = 1 个 RTT 滑窗快速起播）
pub const VOD_HEAD_WINDOW_BLOCKS: u32 = 16;

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

/// 将总块数 `[0, total_blocks)` 按候选对等节点的可用区间与得分比例切分为不相交的条带区间 `[(peer_idx, blk_lo, blk_hi)]`
pub fn plan_stripes(
  local_region: Region,
  peers: &[SwarmPeer],
  total_blocks: u32,
) -> Vec<(usize, u32, u32)> {
  plan_stripes_weighted(local_region, peers, &vec![1u64; total_blocks as usize])
}

/// 按各块的真实字节权重 `block_weights` 与节点得分比例将 `[0, block_weights.len())` 切分为不相交的条带区间 `[(peer_idx, blk_lo, blk_hi)]`
pub fn plan_stripes_weighted(
  local_region: Region,
  peers: &[SwarmPeer],
  block_weights: &[u64],
) -> Vec<(usize, u32, u32)> {
  plan_stripes_range(
    local_region,
    peers,
    block_weights,
    0,
    block_weights.len() as u32,
    false,
  )
}

/// 将任意目标块子区间 `[req_lo, req_hi)` 按各块真实字节权重与节点得分切分为不相交条带区间 `[(peer_idx, blk_lo, blk_hi)]`
///
/// 当启用 `vod_head_first`（点播首屏优先调度）且候选节点数 `> 1` 时：
/// - 首窗口 `[req_lo, (req_lo + 16).min(req_hi))` 优先分配给排序第 1 的最低 RTT 节点，使其在 1 个 RTT 内率先就绪；
/// - 其余区间 `[head_hi, req_hi)` 按字节权重条带化并发分配。
pub fn plan_stripes_range(
  local_region: Region,
  peers: &[SwarmPeer],
  block_weights: &[u64],
  req_lo: u32,
  req_hi: u32,
  vod_head_first: bool,
) -> Vec<(usize, u32, u32)> {
  let max_blks = block_weights.len() as u32;
  let hi = req_hi.min(max_blks);
  let lo = req_lo.min(hi);
  if lo >= hi || peers.is_empty() {
    return Vec::new();
  }

  let mut stripes = Vec::new();
  let mut slice_lo = lo;
  let mut peer_offset = 0usize;

  if vod_head_first && peers.len() > 1 {
    let head_hi = (lo + VOD_HEAD_WINDOW_BLOCKS).min(hi);
    let head_peer = peers
      .iter()
      .position(|p| p.blk_lo <= lo && p.blk_hi >= head_hi)
      .unwrap_or(0);
    stripes.push((head_peer, lo, head_hi));
    slice_lo = head_hi;
    if slice_lo >= hi {
      return stripes;
    }
    if head_peer == 0
      && peers[1..]
        .iter()
        .any(|p| p.blk_lo <= slice_lo && p.blk_hi >= hi)
    {
      peer_offset = 1;
    }
  }

  let sub_peers = &peers[peer_offset..];
  let sub_weights = &block_weights[slice_lo as usize..hi as usize];
  let span_blocks = hi - slice_lo;
  let k = sub_peers.len().min(span_blocks as usize);
  let sum_score: u128 = sub_peers[..k]
    .iter()
    .map(|p| peer_score(local_region, p).max(1))
    .sum();

  let mut prefix_w = Vec::with_capacity(sub_weights.len() + 1);
  prefix_w.push(0u64);
  let mut acc = 0u64;
  for &w in sub_weights {
    acc = acc.saturating_add(w.max(1));
    prefix_w.push(acc);
  }
  let total_w = u128::from(acc.max(1));

  let mut prefix_score = 0u128;
  let mut cur = slice_lo;
  for (i, peer) in sub_peers[..k].iter().enumerate() {
    prefix_score += peer_score(local_region, peer).max(1);
    let next = if i + 1 == k {
      hi
    } else {
      let target_w = ((total_w * prefix_score + (sum_score >> 1)) / sum_score) as u64;
      let pos = prefix_w.partition_point(|&pw| pw < target_w);
      let rel_next = if pos > 0
        && pos <= sub_weights.len()
        && target_w.saturating_sub(prefix_w[pos - 1]) < prefix_w[pos].saturating_sub(target_w)
      {
        (pos - 1) as u32
      } else {
        pos as u32
      };
      let max_next = hi - (k - 1 - i) as u32;
      (slice_lo + rel_next).clamp(cur + 1, max_next)
    };
    if cur < next {
      let global_i = peer_offset + i;
      let chosen = if peer.blk_lo <= cur && peer.blk_hi >= next {
        global_i
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
        && !vod_head_first
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
