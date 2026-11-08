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

use std::{
  hint::spin_loop,
  io::ErrorKind,
  net::{SocketAddr, UdpSocket},
  sync::OnceLock,
  thread::sleep,
  time::{Duration, Instant},
};

pub use probe::{
  handle_probe, handle_probe_observed, make_probe, parse_probe_ack, parse_probe_ack_observed,
};
pub use rx::{recv, recv_on_sock, recv_on_sock_opt, recv_opt};
pub use stats::Stats;
pub use tx::{send, send_changes, send_files, send_on_sock_opt, send_opt};

pub(super) use crate::fs::{RootBuf, par_chunk_plan};
use crate::{
  UdpError, bind_std_udp,
  consts::{
    ACK_BLOCKS, ACK_SCAN_BLOCKS, DEFAULT_PKT as PKT, DIR_ACK_MAGIC as DIR_ACK, DIR_MAGIC,
    FIN_MAGIC, MANIFEST_CHUNK, MANIFEST_ZSTD, MAX_BLOCK as BLK, MAX_CNT, MAX_DATAGRAM as DGRAM,
    Role, TUNNEL_PKT, WINDOW_BLOCKS, ZSTD_DEFAULT_LEVEL as ZSTD_LEVEL,
  },
};

/// 空槽标记
pub(super) const EMPTY: u32 = u32::MAX;
pub(super) const IDLE_US: u64 = 15_000_000;

/// 混合等待：亚毫秒区间 (< 1.5ms) 自旋等待，避免 macOS/Linux 内核定时器过睡拉低吞吐
pub(crate) fn wait_until(target: u64) {
  loop {
    let now = now_us();
    if now >= target {
      return;
    }
    let d = target - now;
    if d > 1_500 {
      sleep(Duration::from_micros(d - 1_000));
    } else {
      spin_loop();
    }
  }
}

pub(crate) fn now_us() -> u64 {
  static EPOCH: OnceLock<Instant> = OnceLock::new();
  EPOCH.get_or_init(Instant::now).elapsed().as_micros() as u64 + 1
}

/// 绑定 UDP 并放大收发缓冲，避免突发 ACK 被内核丢弃
pub fn udp_bind(addr: SocketAddr) -> Result<UdpSocket, UdpError> {
  Ok(bind_std_udp(addr, false)?)
}

pub(crate) fn send_udp(sock: &UdpSocket, data: &[u8], peer: SocketAddr) -> Result<(), UdpError> {
  for _ in 0..200 {
    match sock.send_to(data, peer) {
      Ok(_) => return Ok(()),
      Err(e) if e.kind() == ErrorKind::WouldBlock => {
        sleep(Duration::from_micros(100));
      }
      Err(e) => return Err(e.into()),
    }
  }
  Err(UdpError::StreamTimeout)
}
