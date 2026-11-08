mod ackgen;
pub mod bench;
pub mod peer;
mod progress;
pub mod range;
mod recv;
mod sender;

use std::{
  hint::spin_loop,
  io::{self, ErrorKind},
  net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, UdpSocket},
  sync::OnceLock,
  thread::sleep,
  time::{Duration, Instant},
};

pub use ackgen::{AckGen, AckView};
use bench::{make_probe, parse_probe_ack_observed};
use codec::Error as CodecError;
pub use fs::BlockSource;
use fs::FsError;
pub use progress::Progress;
pub use range::{Pacer, RangeSpec, UdpStats, recv_range_udp, send_range_udp};
pub use recv::{Config, MAX_SLOTS, Outcome, Receiver, RecvStats};
use s0_cdc::{CdcError, WireError};
pub use s0_codec as codec;
pub use s0_fs::{compio_io, fs};
pub use s0_wire::{consts, profile, region, wire};
pub use sender::{AckStats, MIN_CAP, Sender, SentInfo};
use socket2::{Domain, Protocol, Socket, Type};
use wire::{decode_socket_addrs, encode_punch, encode_punch_padded, encode_socket_addrs};

/// 收发缓冲目标值（32 MiB，内核可能截断）
pub const BUF_BYTES: usize = 32 * 1024 * 1024;
/// 默认 IPv4 任意绑定地址 (`0.0.0.0:0`)
pub const BIND_V4: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0));
/// 默认 IPv6 任意绑定地址 (`[::]:0`)
pub const BIND_V6: SocketAddr = SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0));

/// 进程级单调递增微秒时钟（起始 `>= 1`）
pub fn now_us() -> u64 {
  static EPOCH: OnceLock<Instant> = OnceLock::new();
  EPOCH.get_or_init(Instant::now).elapsed().as_micros() as u64 + 1
}

/// 混合等待：亚毫秒区间 (< 1.5ms) 自旋等待，避免 macOS/Linux 内核定时器过睡拉低吞吐
pub fn wait_until(target: u64) {
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

/// 带非阻塞退避重试的 UDP 数据报发送辅助函数
pub fn send_udp(sock: &UdpSocket, data: &[u8], peer: SocketAddr) -> Result<(), UdpError> {
  for _ in 0..200 {
    match sock.send_to(data, peer) {
      Ok(_) => return Ok(()),
      Err(e) if e.kind() == ErrorKind::WouldBlock => sleep(Duration::from_micros(100)),
      Err(e) => return Err(e.into()),
    }
  }
  Err(UdpError::StreamTimeout)
}

#[derive(Debug, thiserror::Error)]
pub enum UdpError {
  #[error("IO 错误: {0}")]
  Io(#[from] io::Error),
  #[error("线协议错误: {0}")]
  Wire(#[from] WireError),
  #[error("CDC 错误: {0}")]
  Cdc(#[from] CdcError),
  #[error("压缩编解码错误: {0}")]
  Codec(#[from] CodecError),
  #[error("文件存储错误: {0}")]
  Fs(#[from] FsError),
  #[error("参数错误: {0}")]
  Arg(&'static str),
  #[error("数据损坏或校验失败")]
  Corrupt,
  #[error("流传输超时")]
  StreamTimeout,
  #[error("UDP 区间传输超时 ({0}..{1})")]
  Timeout(u32, u32),
  #[error("无可用对等 UDP 地址")]
  NoPeerAddr,
  #[error("对等节点存在恶意分块投毒行为 (block {0})")]
  PoisonedPeer(u32),
}

/// 绑定非阻塞标准库 UDP 套接字并配置最大可用收发缓冲（兼容 macOS 7MiB 限制与 Linux 32MiB）
pub fn bind_std_udp(addr: SocketAddr, reuse: bool) -> io::Result<UdpSocket> {
  let s = Socket::new(Domain::for_address(addr), Type::DGRAM, Some(Protocol::UDP))?;
  if reuse {
    s.set_reuse_address(true)?;
  }
  for sz in [
    BUF_BYTES,
    16 * 1024 * 1024,
    7 * 1024 * 1024,
    4 * 1024 * 1024,
    2 * 1024 * 1024,
  ] {
    if s.set_recv_buffer_size(sz).is_ok() {
      break;
    }
  }
  for sz in [
    BUF_BYTES,
    16 * 1024 * 1024,
    7 * 1024 * 1024,
    4 * 1024 * 1024,
    2 * 1024 * 1024,
  ] {
    if s.set_send_buffer_size(sz).is_ok() {
      break;
    }
  }
  s.set_nonblocking(true)?;
  s.bind(&addr.into())?;
  Ok(s.into())
}

/// 向远端节点发送 `PROBE` 并通过 `PROBE_ACK`（类似 `iroh` QAD / STUN `XOR-MAPPED-ADDRESS`）测量本机 UDP 套接字经 NAT 映射后的真实外部 `SocketAddr` 与 RTT（微秒）
pub fn probe_external_udp_addr(
  sock: &UdpSocket,
  peer: SocketAddr,
  timeout: Duration,
) -> Option<(SocketAddr, u64)> {
  let start = Instant::now();
  let _ = sock.set_read_timeout(Some(Duration::from_millis(25)));
  let mut buf = [0u8; 64];
  while start.elapsed() < timeout {
    let ts_us = start.elapsed().as_micros() as u64;
    let _ = sock.send_to(&make_probe(ts_us), peer);
    if let Ok((n, from)) = sock.recv_from(&mut buf)
      && from == peer
      && let Some((rtt, obs)) =
        parse_probe_ack_observed(&buf[..n], start.elapsed().as_micros() as u64)
    {
      return Some((obs.unwrap_or(from), rtt));
    }
  }
  None
}

fn cached_outbound_v4() -> Option<IpAddr> {
  static OUTBOUND_V4: OnceLock<Option<IpAddr>> = OnceLock::new();
  *OUTBOUND_V4.get_or_init(|| {
    let probe = UdpSocket::bind("0.0.0.0:0").ok()?;
    probe.connect("8.8.8.8:80").ok()?;
    let ip = probe.local_addr().ok()?.ip();
    (!ip.is_unspecified() && !ip.is_loopback()).then_some(ip)
  })
}

fn cached_outbound_v6() -> Option<Ipv6Addr> {
  static OUTBOUND_V6: OnceLock<Option<Ipv6Addr>> = OnceLock::new();
  *OUTBOUND_V6.get_or_init(|| {
    let probe = UdpSocket::bind("[::]:0").ok()?;
    probe.connect("[2606:4700:4700::1111]:80").ok()?;
    let IpAddr::V6(v6) = probe.local_addr().ok()?.ip() else {
      return None;
    };
    ((v6.segments()[0] & 0xE000) == 0x2000).then_some(v6)
  })
}

#[inline]
const fn is_ula_v6(ip: Ipv6Addr) -> bool {
  (ip.segments()[0] & 0xFE00) == 0xFC00
}

/// 判断 IPv4 地址是否为可直接路由的公网地址（排除 RFC1918 私网、回环、链路本地与 `100.64.0.0/10` CGNAT/Tailscale 隧道网段）
#[inline]
pub const fn is_global_v4(ip: Ipv4Addr) -> bool {
  let oct = ip.octets();
  !ip.is_private()
    && !ip.is_loopback()
    && !ip.is_link_local()
    && !ip.is_unspecified()
    && !ip.is_broadcast()
    && !(oct[0] == 100 && (oct[1] & 0xC0) == 64)
}

/// 判断地址是否位于 Overlay 虚拟隧道网段（`100.64.0.0/10` 或 IPv6 ULA `fc00::/7`）
#[inline]
pub const fn is_tunnel_addr(addr: SocketAddr) -> bool {
  match addr.ip() {
    IpAddr::V4(v4) => {
      let oct = v4.octets();
      oct[0] == 100 && (oct[1] & 0xC0) == 64
    }
    IpAddr::V6(v6) => is_ula_v6(v6),
  }
}

/// 判断 IO 错误是否为报文超过路径 MTU (`EMSGSIZE` / `WSAEMSGSIZE`)
#[inline]
pub(crate) fn is_emsgsize(err: &io::Error) -> bool {
  #[cfg(target_os = "linux")]
  const EMSGSIZE_OS: i32 = 90;
  #[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
  ))]
  const EMSGSIZE_OS: i32 = 40;
  #[cfg(target_os = "windows")]
  const EMSGSIZE_OS: i32 = 10040;
  #[cfg(not(any(
    target_os = "linux",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "windows"
  )))]
  const EMSGSIZE_OS: i32 = 90;

  err.raw_os_error() == Some(EMSGSIZE_OS)
}

#[cfg(target_os = "linux")]
fn linux_sock_mtu(sock: &UdpSocket) -> Option<usize> {
  use std::os::fd::AsRawFd;
  unsafe extern "C" {
    fn getsockopt(sockfd: i32, level: i32, optname: i32, optval: *mut u8, optlen: *mut u32) -> i32;
  }
  const IPPROTO_IP: i32 = 0;
  const IP_MTU: i32 = 14;
  const IPPROTO_IPV6: i32 = 41;
  const IPV6_MTU: i32 = 24;

  let fd = sock.as_raw_fd();
  for (lvl, opt) in [(IPPROTO_IP, IP_MTU), (IPPROTO_IPV6, IPV6_MTU)] {
    let mut mtu: i32 = 0;
    let mut len: u32 = 4;
    let rc = unsafe { getsockopt(fd, lvl, opt, (&raw mut mtu).cast::<u8>(), &raw mut len) };
    if rc == 0 && mtu > 0 {
      return Some(mtu as usize);
    }
  }
  None
}

/// 综合对端/本地 Overlay 地址特征与内核套接字路径 MTU 判断是否应起步使用 1200B 隧道安全分片
#[inline]
pub fn is_tunnel_path(sock: &UdpSocket, peer: SocketAddr) -> bool {
  #[cfg(target_os = "linux")]
  if linux_sock_mtu(sock).is_some_and(|m| m < 1420) {
    return true;
  }
  is_tunnel_addr(peer) || sock.local_addr().is_ok_and(is_tunnel_addr)
}

/// 根据控制流本地 IP 与已绑定 UDP 端口生成候选打洞地址列表编码
pub fn encode_udp_addrs_for_ip(local_ip: IpAddr, local_port: u16) -> Vec<u8> {
  let mut addrs = Vec::with_capacity(4);
  if !local_ip.is_unspecified() {
    addrs.push(SocketAddr::new(local_ip, local_port));
  }
  if let IpAddr::V6(v6) = local_ip
    && is_ula_v6(v6)
    && let Some(ext_v6) = cached_outbound_v6()
  {
    let ext = SocketAddr::new(IpAddr::V6(ext_v6), local_port);
    if !addrs.contains(&ext) {
      addrs.push(ext);
    }
  }
  if local_ip.is_loopback() || local_ip.is_unspecified() {
    if local_ip.is_ipv6() {
      let lo6 = SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), local_port);
      if !addrs.contains(&lo6) {
        addrs.push(lo6);
      }
    } else {
      let lo4 = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), local_port);
      if !addrs.contains(&lo4) {
        addrs.push(lo4);
      }
    }
  } else if local_ip.is_ipv4()
    && let Some(ext_ip) = cached_outbound_v4()
  {
    let ext = SocketAddr::new(ext_ip, local_port);
    if !addrs.contains(&ext) {
      addrs.push(ext);
    }
  }
  encode_socket_addrs(&addrs)
}

/// 根据 TCP 控制端口推导确定性预绑定 UDP 端口（落在 `60000..61000` Mosh/QUIC 常见防火墙放行区间）
#[inline]
pub const fn default_udp_port(ctrl_port: u16) -> u16 {
  if ctrl_port >= 60000 && ctrl_port < 61000 {
    ctrl_port
  } else {
    60100 + (ctrl_port % 890)
  }
}

/// 绑定本地临时 UDP 端口并返回 `(UdpSocket, 候选打洞地址编码)`
pub fn bind_udp_ephemeral() -> Result<(UdpSocket, Vec<u8>), UdpError> {
  bind_udp_for_ip(IpAddr::V4(Ipv4Addr::LOCALHOST))
}

/// 根据控制流本地 IP 族自适应绑定 IPv4/IPv6 临时 UDP 套接字并生成候选打洞地址列表
pub fn bind_udp_for_ip(local_ip: IpAddr) -> Result<(UdpSocket, Vec<u8>), UdpError> {
  let unspec_ip = if local_ip.is_ipv6() {
    IpAddr::V6(Ipv6Addr::UNSPECIFIED)
  } else {
    IpAddr::V4(Ipv4Addr::UNSPECIFIED)
  };
  let mut sock_opt = None;
  if !local_ip.is_loopback() {
    for _ in 0..8 {
      let port = fastrand::u16(60100..60990);
      if let Ok(s) = bind_std_udp(SocketAddr::new(unspec_ip, port), false) {
        sock_opt = Some(s);
        break;
      }
    }
  }
  let sock = match sock_opt {
    Some(s) => s,
    None => bind_std_udp(SocketAddr::new(unspec_ip, 0), false)?,
  };
  sock.set_nonblocking(false)?;
  let local_port = sock.local_addr()?.port();
  Ok((sock, encode_udp_addrs_for_ip(local_ip, local_port)))
}

/// 向对端所有候选地址发送打洞探测包（含 1 个 1400B DPLPMTUD 填充探测包与 2 个 5B 保底打洞包）并返回与本地套接字地址族匹配的候选地址列表
pub fn punch_udp(
  sock: &UdpSocket,
  peer_addrs_raw: &[u8],
  session_id: u32,
) -> Result<Vec<SocketAddr>, UdpError> {
  let raw_addrs = decode_socket_addrs(peer_addrs_raw)?;
  if raw_addrs.is_empty() {
    return Err(UdpError::NoPeerAddr);
  }
  let sock_is_v6 = sock.local_addr().is_ok_and(|a| a.is_ipv6());
  let mut addrs: Vec<SocketAddr> = raw_addrs
    .iter()
    .copied()
    .filter(|a| a.is_ipv6() == sock_is_v6)
    .collect();
  if addrs.is_empty() {
    addrs = raw_addrs;
  }
  let padded = encode_punch_padded(session_id);
  let pkt = encode_punch(session_id);
  for &addr in &addrs {
    if !is_tunnel_addr(addr) {
      let _ = sock.send_to(&padded, addr);
    }
    let _ = sock.send_to(&pkt, addr);
    let _ = sock.send_to(&pkt, addr);
  }
  Ok(addrs)
}
