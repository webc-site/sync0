use std::cmp::Reverse;

use s0_const::{DEFAULT_SEG_PAYLOAD, ZERO_PEER_ID, region::Region};
use s0_udp::seg_payload_for_addr;
use s0_wire::{SwarmPeer, decode_socket_addrs};

use crate::{PeerId, register_encoded_endpoints};

/// 点播首屏优先窗口默认块数（16 块 = 1 个 RTT 滑窗快速起播）
pub const VOD_HEAD_WINDOW_BLOCKS: u32 = 16;
/// 未声明带宽权重时物理直连路径默认估算带宽（140 MB/s）
const DEFAULT_DIRECT_BW: u64 = 140_000_000;
/// 未声明带宽权重时隧道或回退路径默认估算带宽（100 MB/s）
const DEFAULT_TUNNEL_BW: u64 = 100_000_000;
/// RTT 带宽加权基准微秒数（100 ms）
const RTT_WEIGHT_REF_US: u64 = 100_000;
/// RTT 带宽加权最小夹逼微秒数（2 ms）
const RTT_WEIGHT_MIN_US: u64 = 2_000;
/// RTT 带宽加权最大夹逼微秒数（1 s）
const RTT_WEIGHT_MAX_US: u64 = 1_000_000;
/// 串行回退探测默认估算 RTT（50 ms）
pub(crate) const FALLBACK_RTT_US: u64 = 50_000;
/// 跨洋高时延二次衰减触发阈值（140 ms）与衰减参考分母（70 ms）
const TRANSOCEANIC_RTT_THRESHOLD_US: u64 = 140_000;
const TRANSOCEANIC_DECAY_REF_US: u64 = 70_000;

/// 按实测控制流 RTT 对节点带宽权重进行反比加权（高时延跨洋链路二次衰减，避免慢速跨洋尾块拖累多源并发）
#[inline]
pub(crate) fn rtt_weighted_rate(base_rate: u64, rtt_us: u64) -> u64 {
  let base = if base_rate > 0 {
    base_rate
  } else {
    DEFAULT_TUNNEL_BW
  };
  let clamped = rtt_us.clamp(RTT_WEIGHT_MIN_US, RTT_WEIGHT_MAX_US);
  let linear = base.saturating_mul(RTT_WEIGHT_REF_US) / clamped;
  if base_rate == 0 && clamped >= TRANSOCEANIC_RTT_THRESHOLD_US {
    linear.saturating_mul(TRANSOCEANIC_DECAY_REF_US) / clamped
  } else {
    linear
  }
}

/// 就近父节点选择器：结合城市级地域亲和度 (`Region::proximity_score`)、已就绪区间跨度与可用带宽权重对候选对等节点排序
pub(crate) fn rank_swarm_peers(local_region: Region, mut peers: Vec<SwarmPeer>) -> Vec<SwarmPeer> {
  if peers.len() > 1 {
    peers.sort_by_cached_key(|p| Reverse(peer_score(local_region, p)));
  }
  peers
}

/// 注册远端 `SwarmPeer` 地址并将拥有有效块区间的非自身节点合并到候选列表（相邻或重叠区间自动合并，不相交游程保留独立候选）
pub(crate) fn merge_swarm_candidates(
  dst: &mut Vec<SwarmPeer>,
  incoming: impl IntoIterator<Item = SwarmPeer>,
  my_id: PeerId,
) {
  for sp in incoming {
    register_encoded_endpoints(sp.endpoint_id, &sp.peer_addrs);
    if sp.blk_hi > sp.blk_lo && sp.endpoint_id != my_id {
      if sp.endpoint_id != ZERO_PEER_ID
        && let Some(ex) = dst.iter_mut().find(|p| {
          p.endpoint_id == sp.endpoint_id && p.blk_lo <= sp.blk_hi && sp.blk_lo <= p.blk_hi
        })
      {
        ex.blk_lo = ex.blk_lo.min(sp.blk_lo);
        ex.blk_hi = ex.blk_hi.max(sp.blk_hi);
        ex.weight_rate = sp.weight_rate;
        ex.peer_addrs = sp.peer_addrs;
        if !sp.profile_card.is_empty() {
          ex.profile_card = sp.profile_card;
        }
      } else {
        dst.push(sp);
      }
    }
  }
}

#[inline]
fn peer_score(local_region: Region, peer: &SwarmPeer) -> u128 {
  let prox = u128::from(local_region.proximity_score(peer.region));
  let bw = u128::from(if peer.weight_rate == 0 {
    let is_direct = decode_socket_addrs(&peer.peer_addrs)
      .ok()
      .and_then(|v| v.first().copied())
      .is_none_or(|a| seg_payload_for_addr(a) == DEFAULT_SEG_PAYLOAD);
    if is_direct {
      DEFAULT_DIRECT_BW
    } else {
      DEFAULT_TUNNEL_BW
    }
  } else {
    peer.weight_rate
  });
  let span = u128::from(peer.blk_hi.saturating_sub(peer.blk_lo).max(1));
  prox * bw * span
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
  // 按可用区间右边界 `blk_hi` 升序排列（Most-Constrained-First）：
  // 正在下载的半程 Leecher（`[0, blk_hi)`，`blk_hi < hi`）优先分配头部已就绪区间，全量 Seeder（`blk_hi >= hi`）殿后覆盖剩余尾部区间
  let mut order: Vec<usize> = (0..k).collect();
  order.sort_by_key(|&idx| (sub_peers[idx].blk_hi.min(hi), idx));
  let sum_score: u128 = order
    .iter()
    .map(|&idx| peer_score(local_region, &sub_peers[idx]).max(1))
    .sum();

  let has_missing_w = sub_weights.iter().any(|&w| w > 1);
  let mut prefix_w = Vec::with_capacity(sub_weights.len() + 1);
  prefix_w.push(0u64);
  let mut acc = 0u64;
  for &w in sub_weights {
    let eff_w = if has_missing_w && w <= 1 { 0 } else { w.max(1) };
    acc = acc.saturating_add(eff_w);
    prefix_w.push(acc);
  }
  let total_w = u128::from(acc.max(1));

  let mut prefix_score = 0u128;
  let mut cur = slice_lo;
  for (step, &i) in order.iter().enumerate() {
    let peer = &sub_peers[i];
    prefix_score += peer_score(local_region, peer).max(1);
    let next = if step + 1 == k {
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
      let max_next = hi - (k - 1 - step) as u32;
      let cap_next = if peer.blk_lo <= cur && peer.blk_hi > cur {
        max_next.min(peer.blk_hi)
      } else {
        max_next
      };
      (slice_lo + rel_next).clamp(cur + 1, cap_next)
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
