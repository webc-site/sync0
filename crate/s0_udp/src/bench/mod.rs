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

/// 绑定 UDP 并放大收发缓冲，避免突发 ACK 被内核丢弃
pub fn udp_bind(addr: SocketAddr) -> Result<UdpSocket, UdpError> {
  Ok(bind_std_udp(addr, false)?)
}
