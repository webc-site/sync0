mod ackgen;
pub mod bench;
mod progress;
pub mod range;
mod recv;
mod sender;

#[cfg(target_os = "linux")]
use std::os::fd::AsRawFd;
use std::{
  hint::spin_loop,
  io::{self, ErrorKind},
  net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, UdpSocket},
  sync::OnceLock,
  thread::sleep,
  time::{Duration, Instant},
};

pub use ackgen::AckGen;
use bench::{make_probe, parse_probe_ack_observed};
use fastrand::u16 as rand_u16;
use progress::Progress;
pub use range::{RangeSpec, UdpStats, recv_range_udp, seg_payload_for_addr, send_range_udp};
pub use recv::{Config, Outcome, Receiver};
use s0_cdc::WireError;
use s0_codec::Error as CodecError;
use s0_fs::FsError;
use s0_wire::{decode_socket_addrs, encode_punch, encode_punch_padded, encode_socket_addrs};
pub use sender::Sender;
use socket2::{Domain, Protocol, Socket, Type};
use thiserror::Error;

/// 收发缓冲目标值（32 MiB，内核可能截断）
const BUF_BYTES: usize = 32 * 1024 * 1024;
/// 默认 IPv4 任意绑定地址 (`0.0.0.0:0`)
pub(crate) const BIND_V4: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0));
/// 默认 IPv6 任意绑定地址 (`[::]:0`)
pub(crate) const BIND_V6: SocketAddr =
  SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0));

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

/// 参数与边界校验错误码（1 字节数值枚举，零字符串指针开销）
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ArgError {
  #[error("非法槽位数")]
  Slots = 1,
  #[error("非法块大小")]
  BlockSize = 2,
  #[error("非法分片大小")]
  PktSize = 3,
  #[error("单块分片数超限")]
  SegCnt = 4,
  #[error("块池内存超限")]
  PoolMem = 5,
  #[error("未启用 Zstd 却收到压缩块")]
  UnexpectedZstd = 6,
  #[error("非法 DATA 分片头 idx/cnt")]
  BadDataHdr = 7,
  #[error("非法在途环容量")]
  RingCap = 8,
  #[error("在途环已满")]
  RingFull = 9,
  #[error("清单路径重复或未排序")]
  UnsortedPath = 10,
  #[error("虚拟流读取越界")]
  StreamBounds = 11,
}

#[derive(Debug, Error)]
pub enum UdpError {
  #[error(transparent)]
  Io(#[from] io::Error),
  #[error(transparent)]
  Wire(#[from] WireError),
  #[error(transparent)]
  Codec(#[from] CodecError),
  #[error(transparent)]
  Fs(#[from] FsError),
  #[error(transparent)]
  Arg(#[from] ArgError),
  #[error("数据损坏或校验失败")]
  Corrupt,
  #[error("流传输超时")]
  StreamTimeout,
  #[error("UDP 区间传输超时 ({0}..{1})")]
  Timeout(u32, u32),
  #[error("无可用对等 UDP 地址")]
  NoPeerAddr,
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
    let IpAddr::V4(v4) = probe.local_addr().ok()?.ip() else {
      return None;
    };
    let oct = v4.octets();
    let is_fake_or_link_local =
      (oct[0] == 198 && (oct[1] & 0xFE) == 18) || (oct[0] == 169 && oct[1] == 254);
    (!v4.is_unspecified() && !v4.is_loopback() && !v4.is_private() && !is_fake_or_link_local)
      .then_some(IpAddr::V4(v4))
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
fn encode_udp_addrs_for_ip(local_ip: IpAddr, local_port: u16) -> Vec<u8> {
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
      if is_tunnel_addr(SocketAddr::new(local_ip, local_port)) && !is_tunnel_addr(ext) {
        addrs.insert(0, ext);
      } else {
        addrs.push(ext);
      }
    }
  }
  encode_socket_addrs(&addrs)
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
      let port = rand_u16(60100..60990);
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

/// 向给定目标地址列表发送一轮 UDP 打洞突发（每个地址 1 个 1400B DPLPMTUD 填充包 + 2 个 5B 保底打洞包）
pub(crate) fn send_punch_burst(sock: &UdpSocket, addrs: &[SocketAddr], session_id: u32) {
  let padded = encode_punch_padded(session_id);
  let pkt = encode_punch(session_id);
  for &addr in addrs {
    if !is_tunnel_addr(addr) {
      let _ = sock.send_to(&padded, addr);
    }
    let _ = sock.send_to(&pkt, addr);
    let _ = sock.send_to(&pkt, addr);
  }
}

/// 向对端所有候选地址发送打洞探测包（含 1 个 1400B DPLPMTUD 填充探测包与 2 个 5B 保底打洞包）并返回与本地套接字地址族匹配的候选地址列表
pub fn punch_udp(
  sock: &UdpSocket,
  peer_addrs_raw: &[u8],
  session_id: u32,
) -> Result<Vec<SocketAddr>, UdpError> {
  punch_udp_with_peer_ip(sock, peer_addrs_raw, None, session_id)
}

/// 结合控制流 TCP 远端真实 IP (`tcp_peer_ip`) 与候选地址列表执行 UDP 打洞（优先锁定已连通的控制面 IP，规避 Fake-IP / 虚拟网卡干扰）
pub fn punch_udp_with_peer_ip(
  sock: &UdpSocket,
  peer_addrs_raw: &[u8],
  tcp_peer_ip: Option<IpAddr>,
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
  if let Some(ip) = tcp_peer_ip
    && ip.is_ipv6() == sock_is_v6
    && let Some(port) = addrs.first().map(SocketAddr::port)
  {
    let direct = SocketAddr::new(ip, port);
    let has_public_local = if sock_is_v6 {
      cached_outbound_v6().is_some()
    } else {
      cached_outbound_v4().is_some_and(|a| !is_tunnel_addr(SocketAddr::new(a, 0)))
    };
    let keep_public_first = has_public_local
      && is_tunnel_addr(direct)
      && addrs.first().is_some_and(|&a| !is_tunnel_addr(a));
    if keep_public_first {
      if !addrs.contains(&direct) {
        addrs.push(direct);
      }
    } else {
      if !has_public_local && is_tunnel_addr(direct) {
        addrs.retain(|&a| is_tunnel_addr(a));
      }
      if let Some(pos) = addrs.iter().position(|&a| a == direct) {
        addrs.swap(0, pos);
      } else {
        addrs.insert(0, direct);
      }
    }
  }
  send_punch_burst(sock, &addrs[..1], session_id);
  Ok(addrs)
}
