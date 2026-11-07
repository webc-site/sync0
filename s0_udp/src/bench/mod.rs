mod manifest;
mod probe;
mod rx;
mod sink;
mod source;
mod stats;
mod tx;

use std::{
  hint::spin_loop,
  io::ErrorKind,
  net::{SocketAddr, UdpSocket},
  path::{Path, PathBuf},
  sync::OnceLock,
  thread::sleep,
  time::{Duration, Instant},
};

pub use probe::{
  PROBE_ACK_MAGIC, PROBE_LEN, PROBE_MAGIC, handle_probe, handle_probe_observed, make_probe,
  parse_probe_ack, parse_probe_ack_observed,
};
pub use rx::{recv, recv_on_sock, recv_on_sock_opt, recv_opt};
pub use s0_const::{
  ACK_BLOCKS, ACK_SCAN_BLOCKS, DEFAULT_PKT as PKT, DIR_ACK_MAGIC as DIR_ACK, DIR_MAGIC, FIN_MAGIC,
  MANIFEST_CHUNK, MANIFEST_ZSTD, MAX_BLOCK as BLK, MAX_CNT, MAX_DATAGRAM as DGRAM, Role,
  WINDOW_BLOCKS, ZSTD_DEFAULT_LEVEL as ZSTD_LEVEL,
};
pub use stats::Stats;
pub use tx::{send, send_changes, send_files, send_on_sock_opt, send_opt};

use crate::{UdpError, bind_std_udp};

/// 空槽标记
pub(super) const EMPTY: u32 = u32::MAX;
pub(super) const IDLE_US: u64 = 15_000_000;

/// 复用单块堆内存的根目录路径拼接器，消除海量小文件热路径 `Path::join` 重复堆分配
pub(super) struct RootBuf {
  buf: PathBuf,
  base_len: usize,
}

impl RootBuf {
  pub(super) fn new(root: &Path) -> Self {
    let mut buf = PathBuf::with_capacity(root.as_os_str().len() + 64);
    buf.push(root);
    buf.push("x");
    buf.pop();
    let base_len = buf.as_os_str().len();
    Self { buf, base_len }
  }

  pub(super) fn join(&mut self, rel: impl AsRef<Path>) -> &Path {
    while self.buf.as_os_str().len() > self.base_len {
      if !self.buf.pop() {
        break;
      }
    }
    self.buf.push(rel);
    &self.buf
  }
}

/// 混合等待：亚毫秒区间 (< 1.5ms) 自旋等待，避免 macOS/Linux 内核定时器过睡拉低吞吐
pub(super) fn wait_until(target: u64) {
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

pub(super) fn now_us() -> u64 {
  static EPOCH: OnceLock<Instant> = OnceLock::new();
  EPOCH.get_or_init(Instant::now).elapsed().as_micros() as u64 + 1
}

/// 绑定 UDP 并放大收发缓冲，避免突发 ACK 被内核丢弃
pub fn udp_bind(addr: SocketAddr) -> Result<UdpSocket, UdpError> {
  Ok(bind_std_udp(addr, false)?)
}

pub(super) fn send_udp(sock: &UdpSocket, data: &[u8], peer: SocketAddr) -> Result<(), UdpError> {
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
