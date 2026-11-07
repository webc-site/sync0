mod ackgen;
mod bench;
mod progress;
mod recv;
mod sender;

use std::{
  collections::hash_map::Entry,
  io,
  net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, UdpSocket},
  slice::from_ref,
  thread,
  time::{Duration, Instant},
};

pub use ackgen::{AckGen, AckView};
pub use bench::{
  PROBE_ACK_MAGIC, PROBE_LEN, PROBE_MAGIC, Role, Stats, handle_probe, handle_probe_observed,
  make_probe, parse_probe_ack, parse_probe_ack_observed, recv as bench_recv,
  recv_on_sock as bench_recv_sock, recv_on_sock_opt as bench_recv_sock_opt,
  recv_opt as bench_recv_opt, send as bench_send, send_changes as bench_send_changes,
  send_files as bench_send_files, send_on_sock_opt as bench_send_sock_opt,
  send_opt as bench_send_opt, udp_bind as bench_udp_bind,
};
use coarsetime::{Duration as CoarseDuration, Instant as CoarseInstant};
use compio::net::UdpSocket as CompioUdpSocket;
use gxhash::HashMap;
pub use progress::Progress;
pub use recv::{BLOCK, Config, MAX_CNT, MAX_SLOTS, Outcome, Receiver, RecvStats, SLOTS};
use s0_cc::AntiAmp;
use s0_cdc::{CdcError, for_each_missing_span, get_bit};
use s0_codec::{Codec, DEFAULT_LEVEL as ZSTD_DEFAULT_LEVEL, Error as CodecError, SLACK};
pub use s0_const::MAX_DATAGRAM;
use s0_const::{
  DATA_FLAG_ZSTD, DATA_HDR_LEN, DEFAULT_SEG_PAYLOAD, MAX_UDP_PKT, PKT_ACK, PKT_NACK, PKT_PUNCH,
};
use s0_fs::{FsError, SharedSink, SourceReader};
use s0_wire::{
  AckPkt, DataHeader, NackPkt, WireError, decode_punch, decode_socket_addrs, encode_punch,
  encode_socket_addrs,
};
pub use sender::{AckStats, MIN_CAP, Sender, SentInfo};
use socket2::{Domain, Protocol, Socket, Type};

/// 接收端滑动窗口最大并发组装块槽位数（$64 \times 64\text{KiB} = 4\text{MiB}$ 内存上限）
const MAX_WINDOW_SLOTS: usize = 64;
/// 收发缓冲目标值（32 MiB，内核可能截断）
pub const BUF_BYTES: usize = 32 * 1024 * 1024;
/// 默认 IPv4 任意绑定地址 (`0.0.0.0:0`)
pub const BIND_V4: SocketAddr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0));
/// 默认 IPv6 任意绑定地址 (`[::]:0`)
pub const BIND_V6: SocketAddr = SocketAddr::V6(SocketAddrV6::new(Ipv6Addr::UNSPECIFIED, 0, 0, 0));

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
}

/// 绑定非阻塞标准库 UDP 套接字并配置 32 MiB 收发缓冲
pub fn bind_std_udp(addr: SocketAddr, reuse: bool) -> io::Result<UdpSocket> {
  let s = Socket::new(Domain::for_address(addr), Type::DGRAM, Some(Protocol::UDP))?;
  if reuse {
    s.set_reuse_address(true)?;
  }
  let _ = s.set_recv_buffer_size(BUF_BYTES);
  let _ = s.set_send_buffer_size(BUF_BYTES);
  s.set_nonblocking(true)?;
  s.bind(&addr.into())?;
  Ok(s.into())
}

/// 基于 `compio` 的异步 UDP 封装（零 `tokio` 依赖）
pub struct AsyncUdp {
  s: CompioUdpSocket,
}

impl AsyncUdp {
  pub fn bind(addr: SocketAddr) -> Result<Self, UdpError> {
    let std_sock = bind_std_udp(addr, true)?;
    Ok(Self {
      s: CompioUdpSocket::from_std(std_sock)?,
    })
  }

  pub fn local_addr(&self) -> Result<SocketAddr, UdpError> {
    Ok(self.s.local_addr()?)
  }

  pub async fn recv(&self, buf: Vec<u8>) -> Result<(Vec<u8>, SocketAddr), UdpError> {
    let r = self.s.recv_from(buf).await;
    let (n, peer) = r.0?;
    let mut buf = r.1;
    buf.truncate(n);
    Ok((buf, peer))
  }

  pub async fn send(&self, data: Vec<u8>, peer: SocketAddr) -> Result<(usize, Vec<u8>), UdpError> {
    let r = self.s.send_to(data, peer).await;
    Ok((r.0?, r.1))
  }
}

/// 原生 UDP 块区间传输参数
#[derive(Debug, Clone, Copy)]
pub struct RangeSpec {
  pub session_id: u32,
  pub blk_lo: u32,
  pub blk_hi: u32,
  pub eff_rate: u64,
  pub timeout: Duration,
}

/// 原生 UDP 区间传输统计信息
#[derive(Debug, Default, Clone, Copy)]
pub struct UdpStats {
  pub bytes_transferred: u64,
  pub blocks_transferred: u32,
  pub elapsed_us: u64,
}

/// 微秒级流量步进器（支持按 `eff_rate` 字节/秒平滑限速）
pub struct Pacer {
  rate_bps: u64,
  start: Instant,
  bytes_sent: u64,
}

impl Pacer {
  #[inline]
  pub fn new(rate_bps: u64) -> Self {
    Self {
      rate_bps,
      start: Instant::now(),
      bytes_sent: 0,
    }
  }

  #[inline]
  pub fn on_send(&mut self, pkt_bytes: usize) {
    self.bytes_sent = self.bytes_sent.saturating_add(pkt_bytes as u64);
    let eff_rate = if self.rate_bps == 0 {
      500_000_000
    } else {
      self.rate_bps
    };
    let expected_us = (self.bytes_sent as u128 * 1_000_000) / (eff_rate as u128);
    let elapsed_us = self.start.elapsed().as_micros();
    if expected_us > elapsed_us {
      let wait_us = (expected_us - elapsed_us) as u64;
      if wait_us >= 100 {
        thread::sleep(Duration::from_micros(wait_us));
      }
    }
  }
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

/// 绑定本地临时 UDP 端口并返回 `(UdpSocket, 候选打洞地址编码)`
pub fn bind_udp_ephemeral() -> Result<(UdpSocket, Vec<u8>), UdpError> {
  let sock = bind_std_udp(BIND_V4, false)?;
  sock.set_nonblocking(false)?;
  let local_port = sock.local_addr()?.port();
  let mut addrs = Vec::with_capacity(4);
  addrs.push(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), local_port));
  if let Ok(probe) = UdpSocket::bind("0.0.0.0:0")
    && probe.connect("8.8.8.8:80").is_ok()
    && let Ok(la) = probe.local_addr()
    && !la.ip().is_unspecified()
    && !la.ip().is_loopback()
  {
    addrs.push(SocketAddr::new(la.ip(), local_port));
  }
  Ok((sock, encode_socket_addrs(&addrs)))
}

/// 向对端所有候选地址发送打洞探测包并返回候选地址列表
pub fn punch_udp(
  sock: &UdpSocket,
  peer_addrs_raw: &[u8],
  session_id: u32,
) -> Result<Vec<SocketAddr>, UdpError> {
  let addrs = decode_socket_addrs(peer_addrs_raw)?;
  if addrs.is_empty() {
    return Err(UdpError::NoPeerAddr);
  }
  let pkt = encode_punch(session_id);
  for addr in &addrs {
    let _ = sock.send_to(&pkt, addr);
  }
  Ok(addrs)
}

struct SendCtx<'a> {
  sock: &'a UdpSocket,
  peer_addr: SocketAddr,
  session_id: u32,
  reader: &'a SourceReader,
  codec: Codec,
  comp_buf: Vec<u8>,
  block_buf: Vec<u8>,
  pkt_buf: [u8; MAX_UDP_PKT],
  pacer: Pacer,
}

impl<'a> SendCtx<'a> {
  fn send_block(&mut self, block_idx: u32) -> Result<u64, UdpError> {
    self.reader.read_block(block_idx, &mut self.block_buf)?;
    let raw_len = self.block_buf.len();
    if self.comp_buf.len() < raw_len + SLACK {
      self.comp_buf.resize(raw_len + SLACK, 0);
    }
    let (wire_slice, zstd) = if raw_len >= 256
      && let Ok(Some(clen)) = self
        .codec
        .try_compress_block(&self.block_buf, &mut self.comp_buf)
    {
      (&self.comp_buf[..clen], true)
    } else {
      (&self.block_buf[..], false)
    };
    let sid = self.session_id & 0x00FF_FFFF;
    let total_len = wire_slice.len();
    let seg_cnt = total_len.div_ceil(DEFAULT_SEG_PAYLOAD).max(1) as u16;
    let flagged_cnt = if zstd {
      seg_cnt | DATA_FLAG_ZSTD
    } else {
      seg_cnt
    };
    for seg_idx in 0..seg_cnt {
      let off = (seg_idx as usize) * DEFAULT_SEG_PAYLOAD;
      let end = (off + DEFAULT_SEG_PAYLOAD).min(total_len);
      let payload = &wire_slice[off..end];
      let hdr = DataHeader {
        session_id: sid,
        block_idx,
        seg_idx,
        seg_cnt: flagged_cnt,
        offset_in_block: off as u32,
        payload_len: payload.len() as u16,
      };
      let hdr_slice: &mut [u8; DATA_HDR_LEN] =
        (&mut self.pkt_buf[..DATA_HDR_LEN]).try_into().unwrap();
      hdr.encode(hdr_slice);
      self.pkt_buf[DATA_HDR_LEN..DATA_HDR_LEN + payload.len()].copy_from_slice(payload);
      let pkt_len = DATA_HDR_LEN + payload.len();
      self
        .sock
        .send_to(&self.pkt_buf[..pkt_len], self.peer_addr)?;
      self.pacer.on_send(pkt_len);
    }
    Ok(raw_len as u64)
  }
}

/// 原生 UDP 区间发送引擎（支持 `mzstd` 块级自适应压缩 + `AntiAmp` 源地址往返验证防反射放大 + NAT 真实端口锁定）
pub fn send_range_udp(
  sock: &UdpSocket,
  peer_addrs: &[SocketAddr],
  reader: &SourceReader,
  have_bits: &[u8],
  spec: RangeSpec,
) -> Result<UdpStats, UdpError> {
  let start = Instant::now();
  let peer_addr = peer_addrs.first().copied().ok_or(UdpError::NoPeerAddr)?;
  let hi = spec.blk_hi.min(reader.total_blocks());
  if spec.blk_lo >= hi {
    return Ok(UdpStats::default());
  }

  let mut ctx = SendCtx {
    sock,
    peer_addr,
    session_id: spec.session_id,
    reader,
    codec: Codec::new(ZSTD_DEFAULT_LEVEL)?,
    comp_buf: Vec::with_capacity(64 * 1024 + SLACK),
    block_buf: Vec::with_capacity(64 * 1024),
    pkt_buf: [0u8; MAX_UDP_PKT],
    pacer: Pacer::new(spec.eff_rate),
  };
  let mut ctrl_buf = [0u8; MAX_UDP_PKT];
  let mut bytes_transferred = 0u64;
  let mut blocks_transferred = 0u32;
  let sid24 = spec.session_id & 0x00FF_FFFF;
  let mut amp: AntiAmp<SocketAddr> = AntiAmp::default();
  let timeout_coarse = CoarseDuration::from_millis(spec.timeout.as_millis() as u64);
  let deadline = CoarseInstant::now() + timeout_coarse;

  // 协议层抗反射放大与真实 NAT 端口锁定：在收到对端携带匹配 24-bit session_id 的 PKT_PUNCH / PKT_NACK / PKT_ACK 前绝不发送 64KiB 数据块
  sock.set_nonblocking(false)?;
  sock.set_read_timeout(Some(Duration::from_millis(15)))?;
  let punch_pkt = encode_punch(sid24);
  while !amp.is_validated() {
    if CoarseInstant::now() >= deadline {
      return Err(UdpError::Timeout(spec.blk_lo, hi));
    }
    for addr in peer_addrs {
      let _ = sock.send_to(&punch_pkt, addr);
    }
    match sock.recv_from(&mut ctrl_buf) {
      Ok((n, from)) if n > 0 => {
        if let Some(ack) = handle_probe_observed(&ctrl_buf[..n], from) {
          let _ = sock.send_to(&ack, from);
          continue;
        }
        let valid_sid = match ctrl_buf[0] {
          PKT_PUNCH => decode_punch(&ctrl_buf[..n]) == Ok(sid24),
          PKT_NACK => NackPkt::decode(&ctrl_buf[..n]).is_ok_and(|p| p.session_id == sid24),
          PKT_ACK => AckPkt::decode(&ctrl_buf[..n]).is_ok_and(|p| p.session_id == sid24),
          _ => false,
        };
        if valid_sid {
          let _ = amp.on_recv(from, n);
          amp.mark_validated();
          ctx.peer_addr = from;
          if ctrl_buf[0] == PKT_ACK {
            return Ok(UdpStats {
              bytes_transferred: 0,
              blocks_transferred: 0,
              elapsed_us: start.elapsed().as_micros() as u64,
            });
          }
        }
      }
      Ok(_) => {}
      Err(e) if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut => {}
      Err(e) => return Err(UdpError::Io(e)),
    }
  }

  sock.set_nonblocking(true)?;
  for blk in spec.blk_lo..hi {
    if get_bit(have_bits, blk) {
      continue;
    }
    bytes_transferred += ctx.send_block(blk)?;
    blocks_transferred += 1;

    while let Ok((n, from)) = sock.recv_from(&mut ctrl_buf) {
      if n >= 13
        && ctrl_buf[0] == PKT_ACK
        && let Ok(ack) = AckPkt::decode(&ctrl_buf[..n])
        && ack.session_id == sid24
      {
        ctx.peer_addr = from;
        sock.set_nonblocking(false)?;
        return Ok(UdpStats {
          bytes_transferred,
          blocks_transferred,
          elapsed_us: start.elapsed().as_micros() as u64,
        });
      }
    }
  }
  sock.set_nonblocking(false)?;
  sock.set_read_timeout(Some(Duration::from_millis(15)))?;

  while CoarseInstant::now() < deadline {
    match sock.recv_from(&mut ctrl_buf) {
      Ok((n, from)) => {
        if n == 0 {
          continue;
        }
        match ctrl_buf[0] {
          PKT_ACK => {
            if let Ok(ack) = AckPkt::decode(&ctrl_buf[..n])
              && ack.session_id == sid24
            {
              ctx.peer_addr = from;
              return Ok(UdpStats {
                bytes_transferred,
                blocks_transferred,
                elapsed_us: start.elapsed().as_micros() as u64,
              });
            }
          }
          PKT_NACK => {
            if let Ok(nack) = NackPkt::decode(&ctrl_buf[..n])
              && nack.session_id == sid24
            {
              ctx.peer_addr = from;
              let mut resent = 0u32;
              'nack_loop: for (s_lo, s_hi) in nack.spans {
                let r_lo = s_lo.max(spec.blk_lo);
                let r_hi = s_hi.min(hi);
                for blk in r_lo..r_hi {
                  bytes_transferred += ctx.send_block(blk)?;
                  resent += 1;
                  if resent >= 4 {
                    break 'nack_loop;
                  }
                }
              }
            }
          }
          PKT_PUNCH if decode_punch(&ctrl_buf[..n]) == Ok(sid24) => {
            ctx.peer_addr = from;
          }
          _ => {}
        }
      }
      Err(e) if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut => {
        continue;
      }
      Err(e) => return Err(UdpError::Io(e)),
    }
  }

  Err(UdpError::Timeout(spec.blk_lo, hi))
}

struct BlockSlot {
  buf: Vec<u8>,
  seg_mask: u64,
  seg_cnt: u16,
  received_segs: u16,
  zstd: bool,
}

/// 原生 UDP 区间接收引擎（支持 `mzstd` 块解压，逐块校验 BLAKE3 并无锁落盘）
pub fn recv_range_udp(
  sock: &UdpSocket,
  peer_addrs: &[SocketAddr],
  sink: &SharedSink,
  spec: RangeSpec,
) -> Result<UdpStats, UdpError> {
  let start = Instant::now();
  if peer_addrs.is_empty() {
    return Err(UdpError::NoPeerAddr);
  }
  let mut locked_peer: Option<SocketAddr> = None;
  let hi = spec.blk_hi.min(sink.total_blocks());
  let sid24 = spec.session_id & 0x00FF_FFFF;

  if sink.is_range_complete(spec.blk_lo, hi) {
    send_ack_burst(sock, peer_addrs, sid24, spec.blk_lo, hi);
    return Ok(UdpStats::default());
  }

  sock.set_read_timeout(Some(Duration::from_millis(20)))?;
  let mut slots: HashMap<u32, BlockSlot> = HashMap::default();
  let mut free_bufs: Vec<Vec<u8>> = Vec::with_capacity(MAX_WINDOW_SLOTS);
  let mut codec = Codec::new(ZSTD_DEFAULT_LEVEL)?;
  let mut dec_buf = vec![0u8; 64 * 1024];
  let mut pkt_buf = [0u8; MAX_UDP_PKT];
  let mut nack_buf = Vec::with_capacity(256);
  let mut bytes_transferred = 0u64;
  let mut blocks_transferred = 0u32;

  for addr in peer_addrs {
    let _ = sock.send_to(&encode_punch(sid24), addr);
  }
  send_nack_for_range(
    sock,
    peer_addrs,
    sid24,
    sink,
    (spec.blk_lo, hi),
    &mut nack_buf,
  )?;

  let timeout_coarse = CoarseDuration::from_millis(spec.timeout.as_millis() as u64);
  let deadline = CoarseInstant::now() + timeout_coarse;
  while CoarseInstant::now() < deadline {
    match sock.recv_from(&mut pkt_buf) {
      Ok((n, from)) => {
        if n < DATA_HDR_LEN || (pkt_buf[0] & 0x80) != 0 {
          continue;
        }
        let Ok(hdr) = DataHeader::decode(&pkt_buf[..n]) else {
          continue;
        };
        let pkt_sid24 = hdr.session_id & 0x00FF_FFFF;
        let zstd = (hdr.seg_cnt & DATA_FLAG_ZSTD) != 0;
        let seg_cnt = hdr.seg_cnt & !DATA_FLAG_ZSTD;
        if pkt_sid24 != sid24
          || hdr.block_idx < spec.blk_lo
          || hdr.block_idx >= hi
          || seg_cnt == 0
          || hdr.seg_idx >= seg_cnt
          || seg_cnt > 64
        {
          continue;
        }
        locked_peer = Some(from);
        let payload_end = DATA_HDR_LEN + (hdr.payload_len as usize);
        if payload_end > n || sink.has_block(hdr.block_idx) {
          continue;
        }

        if !slots.contains_key(&hdr.block_idx)
          && slots.len() >= MAX_WINDOW_SLOTS
          && let Some(&evict_k) = slots.keys().next()
          && let Some(mut evicted) = slots.remove(&evict_k)
          && free_bufs.len() < MAX_WINDOW_SLOTS
        {
          evicted.buf.clear();
          free_bufs.push(evicted.buf);
        }

        let mut occ = match slots.entry(hdr.block_idx) {
          Entry::Occupied(o) => o,
          Entry::Vacant(v) => v.insert_entry(BlockSlot {
            buf: free_bufs
              .pop()
              .unwrap_or_else(|| Vec::with_capacity(64 * 1024)),
            seg_mask: 0,
            seg_cnt,
            received_segs: 0,
            zstd,
          }),
        };
        let slot = occ.get_mut();

        let bit = 1u64 << hdr.seg_idx;
        if (slot.seg_mask & bit) == 0 {
          let req_end = (hdr.offset_in_block as usize) + (hdr.payload_len as usize);
          if slot.buf.len() < req_end {
            slot.buf.resize(req_end, 0);
          }
          slot.buf[hdr.offset_in_block as usize..req_end]
            .copy_from_slice(&pkt_buf[DATA_HDR_LEN..payload_end]);
          slot.seg_mask |= bit;
          slot.received_segs += 1;
        }

        if slot.received_segs == slot.seg_cnt {
          let mut completed = occ.remove();
          let write_res = if completed.zstd {
            match codec.decompress_block(&completed.buf, &mut dec_buf) {
              Ok(raw_len) => sink
                .write_verified_block(hdr.block_idx, &dec_buf[..raw_len])
                .map(|ok| (ok, raw_len)),
              Err(_) => Err(FsError::BlockCorrupted(hdr.block_idx)),
            }
          } else {
            sink
              .write_verified_block(hdr.block_idx, &completed.buf)
              .map(|ok| (ok, completed.buf.len()))
          };
          if free_bufs.len() < MAX_WINDOW_SLOTS {
            completed.buf.clear();
            free_bufs.push(completed.buf);
          }
          if let Ok((_, raw_len)) = write_res {
            bytes_transferred += raw_len as u64;
            blocks_transferred += 1;
            if sink.is_range_complete(spec.blk_lo, hi) {
              let targets = if let Some(ref p) = locked_peer {
                from_ref(p)
              } else {
                peer_addrs
              };
              send_ack_burst(sock, targets, sid24, spec.blk_lo, hi);
              return Ok(UdpStats {
                bytes_transferred,
                blocks_transferred,
                elapsed_us: start.elapsed().as_micros() as u64,
              });
            }
          }
        }
      }
      Err(e) if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut => {
        let targets = if let Some(ref p) = locked_peer {
          from_ref(p)
        } else {
          peer_addrs
        };
        if sink.is_range_complete(spec.blk_lo, hi) {
          send_ack_burst(sock, targets, sid24, spec.blk_lo, hi);
          return Ok(UdpStats {
            bytes_transferred,
            blocks_transferred,
            elapsed_us: start.elapsed().as_micros() as u64,
          });
        }
        send_nack_for_range(sock, targets, sid24, sink, (spec.blk_lo, hi), &mut nack_buf)?;
      }
      Err(e) => return Err(UdpError::Io(e)),
    }
  }

  Err(UdpError::Timeout(spec.blk_lo, hi))
}

fn send_nack_for_range(
  sock: &UdpSocket,
  targets: &[SocketAddr],
  session_id: u32,
  sink: &SharedSink,
  range: (u32, u32),
  nack_buf: &mut Vec<u8>,
) -> Result<(), UdpError> {
  let have = sink.snapshot_have_bits();
  let mut spans = Vec::new();
  for_each_missing_span(&have, range.0, range.1, |lo, hi| {
    if spans.len() < 64 {
      spans.push((lo, hi));
    }
  });
  if !spans.is_empty() {
    nack_buf.clear();
    NackPkt { session_id, spans }.encode(nack_buf)?;
    for addr in targets {
      let _ = sock.send_to(nack_buf, addr);
    }
  }
  Ok(())
}

fn send_ack_burst(
  sock: &UdpSocket,
  targets: &[SocketAddr],
  session_id: u32,
  blk_lo: u32,
  blk_hi: u32,
) {
  let mut raw = [0u8; 13];
  AckPkt {
    session_id,
    blk_lo,
    blk_hi,
  }
  .encode(&mut raw);
  for _ in 0..3 {
    for addr in targets {
      let _ = sock.send_to(&raw, addr);
    }
  }
}
