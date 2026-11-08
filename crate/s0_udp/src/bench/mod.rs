mod manifest;
mod manifest_net;
mod probe;
mod rx;
mod sink;
mod sink_basis;
mod source;
mod stats;
mod tx;
mod tx_state;

use std::net::{SocketAddr, UdpSocket};

pub use probe::{
  handle_probe, handle_probe_observed, make_probe, parse_probe_ack, parse_probe_ack_observed,
};
pub use rx::{recv, recv_on_sock_opt, recv_opt};
pub use stats::Stats;
pub use tx::{send, send_changes, send_on_sock_opt, send_opt};

use crate::{UdpError, bind_std_udp};
pub(super) use crate::{
  consts::{
    ACK_BLOCKS, ACK_SCAN_BLOCKS, DEFAULT_PKT, DIR_ACK_MAGIC, DIR_MAGIC, FIN_MAGIC, MANIFEST_CHUNK,
    MANIFEST_ZSTD, MAX_BLOCK, MAX_CNT, MAX_DATAGRAM, Role, TUNNEL_PKT, WINDOW_BLOCKS,
    ZSTD_DEFAULT_LEVEL,
  },
  fs::{RootBuf, par_chunk_plan},
  now_us, send_udp, wait_until,
};

/// 空槽标记
pub(super) const EMPTY: u32 = u32::MAX;
pub(super) const IDLE_US: u64 = 15_000_000;

/// 虚拟增量流区间与目标文件/条目的映射
#[derive(Clone, Copy, Debug)]
pub(super) struct FileSpan {
  pub(super) idx: usize,
  pub(super) file_off: u64,
  pub(super) delta_off: u64,
  pub(super) len: u64,
}

/// 利用单调游标提示 `hint` 在区间表中定位包含或紧接 `off` 的起始下标（顺序访问 $O(1)$，乱序访问回退二分 $O(\log N)$）
#[inline]
pub(super) fn locate_span(spans: &[FileSpan], hint: &mut usize, off: u64) -> usize {
  let start = if let Some(sp) = spans.get(*hint)
    && sp.delta_off <= off
    && (*hint == 0 || spans[*hint - 1].delta_off + spans[*hint - 1].len <= off)
  {
    let mut idx = *hint;
    while idx < spans.len() && spans[idx].delta_off + spans[idx].len <= off {
      idx += 1;
    }
    idx
  } else {
    spans.partition_point(|sp| sp.delta_off + sp.len <= off)
  };
  *hint = start;
  start
}

/// 绑定 UDP 并放大收发缓冲，避免突发 ACK 被内核丢弃
pub fn udp_bind(addr: SocketAddr) -> Result<UdpSocket, UdpError> {
  Ok(bind_std_udp(addr, false)?)
}
