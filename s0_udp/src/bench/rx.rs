use std::{
  io::ErrorKind,
  net::{SocketAddr, UdpSocket},
  path::Path,
  thread::sleep,
  time::Duration,
};

use s0_cc::{AntiAmp, MAX_TACK_US};
use s0_wire::{
  Ack, AckOut, Kind, Rd, SessionHeader, SliceWr as Wr, StreamDataHeader as DataHeader, encode_ack,
};

use super::{
  ACK_BLOCKS, ACK_SCAN_BLOCKS, BLK, DGRAM, DIR_MAGIC, FIN_MAGIC, IDLE_US, PKT, Role,
  manifest::{DIR_HDR_LEN, ManifestRx, parse_manifest},
  now_us,
  probe::handle_probe,
  send_udp,
  sink::{Dec, Sink},
  stats::{Stats, done_bytes},
  udp_bind,
};
use crate::{AckGen, Config, Outcome, Progress, Receiver, UdpError};

/// 接收端
pub fn recv(listen: SocketAddr, out: &Path, show: bool) -> Result<Stats, UdpError> {
  recv_opt(listen, out, show, false)
}

/// 支持可选接收端强制 `delete` 的接收端
pub fn recv_opt(
  listen: SocketAddr,
  out: &Path,
  show: bool,
  delete: bool,
) -> Result<Stats, UdpError> {
  let sock = udp_bind(listen)?;
  recv_on_sock_opt(&sock, out, show, delete)
}

#[derive(Clone, Copy, Default)]
struct RxTs {
  pn: Option<u32>,
  ts: u32,
  rx_us: u64,
}

impl RxTs {
  const fn on_data(&mut self, pn: u32, ts: u32, rx_us: u64) {
    if match self.pn {
      Some(prev) => pn.wrapping_sub(prev) < (1 << 31),
      None => true,
    } {
      self.pn = Some(pn);
      self.ts = ts;
      self.rx_us = rx_us;
    }
  }

  const fn delay_units(&self, now_us: u64) -> u16 {
    if self.pn.is_none() || self.ts == 0 {
      return 0;
    }
    let d = now_us.saturating_sub(self.rx_us) / Ack::DELAY_US;
    if d > u16::MAX as u64 {
      u16::MAX
    } else {
      d as u16
    }
  }

  const fn clear_echo(&mut self) {
    self.ts = 0;
  }
}

const ACK_RTT_US: u64 = 16_000;

struct AckCtx {
  ackgen: AckGen,
  rx_ts: RxTs,
  amp: AntiAmp<SocketAddr>,
  dec_mbps: u16,
  buf: [u8; DGRAM],
}

impl AckCtx {
  fn new(first_us: u64) -> Self {
    let mut ackgen = AckGen::new();
    ackgen.mark_sent(first_us);
    Self {
      ackgen,
      rx_ts: RxTs::default(),
      amp: AntiAmp::default(),
      dec_mbps: 2_000,
      buf: [0u8; DGRAM],
    }
  }

  fn send_guarded(&mut self, sock: &UdpSocket, pkt: &[u8], peer: SocketAddr) {
    if self.amp.can_send(pkt.len()) && send_udp(sock, pkt, peer).is_ok() {
      self.amp.on_send(pkt.len());
    }
  }

  fn send_ack(
    &mut self,
    sock: &UdpSocket,
    peer: SocketAddr,
    recv: &Receiver,
  ) -> Result<Option<u64>, UdpError> {
    let now = now_us();
    if !self.ackgen.should_send(now, ACK_RTT_US) {
      return Ok(None);
    }
    self.send_ack_at(sock, peer, recv, now, 1).map(Some)
  }

  fn send_ack_force(
    &mut self,
    sock: &UdpSocket,
    peer: SocketAddr,
    recv: &Receiver,
  ) -> Result<u64, UdpError> {
    self.send_ack_at(sock, peer, recv, now_us(), 1)
  }

  fn send_ack_burst(
    &mut self,
    sock: &UdpSocket,
    peer: SocketAddr,
    recv: &Receiver,
    copies: usize,
  ) -> Result<u64, UdpError> {
    self.send_ack_at(sock, peer, recv, now_us(), copies)
  }

  fn send_ack_at(
    &mut self,
    sock: &UdpSocket,
    peer: SocketAddr,
    recv: &Receiver,
    now: u64,
    copies: usize,
  ) -> Result<u64, UdpError> {
    let (largest, flag, ranges) = self
      .ackgen
      .build()
      .map_or((0, 0, &[][..]), |v| (v.largest, v.flag, v.ranges));
    let mut blocks = [(0u32, 0u128); ACK_BLOCKS];
    let mut nb = 0usize;
    let lo = recv.done_lo();
    let hi = (lo + ACK_SCAN_BLOCKS).min(recv.credit());
    for (dst, item) in blocks
      .iter_mut()
      .zip((lo..hi).filter_map(|b| recv.partial(b).map(|bits| (b, bits))))
    {
      *dst = item;
      nb += 1;
    }
    let hdr = SessionHeader {
      kind: Kind::Ack,
      flag: 0,
      x: 0,
      sid: 1,
      pn: 0,
    };
    let mut w = Wr::new(&mut self.buf);
    encode_ack(
      &mut w,
      &hdr,
      &AckOut {
        largest,
        delay: self.rx_ts.delay_units(now),
        ts_echo: self.rx_ts.ts,
        rx_pkts: 0,
        credit: recv.credit(),
        done_lo: lo,
        dec: self.dec_mbps,
        flag,
        ranges,
        blocks: &blocks[..nb],
      },
    )?;
    let n = w.pos();
    let mut sent = false;
    for _ in 0..copies {
      if !self.amp.can_send(n) {
        break;
      }
      send_udp(sock, &self.buf[..n], peer)?;
      self.amp.on_send(n);
      sent = true;
    }
    if sent {
      self.ackgen.mark_sent(now);
      self.rx_ts.clear_echo();
    }
    Ok(now)
  }
}

/// 在已绑定的 UDP 套接字上接收一轮传输
pub fn recv_on_sock(sock: &UdpSocket, out: &Path, show: bool) -> Result<Stats, UdpError> {
  recv_on_sock_opt(sock, out, show, false)
}

/// 在已绑定的 UDP 套接字上接收一轮传输（支持接收端配置级 `delete` 策略）
pub fn recv_on_sock_opt(
  sock: &UdpSocket,
  out: &Path,
  show: bool,
  delete: bool,
) -> Result<Stats, UdpError> {
  let mut recv = Receiver::new(Config::new(512, BLK, PKT)?.with_zstd(), 0)?;
  let first_us = now_us();
  let mut ctx = AckCtx::new(first_us);
  let mut sink: Option<Sink> = None;
  let mut manifest: Option<ManifestRx> = None;
  let mut dir_ack_done: Option<Vec<u8>> = None;
  let mut total: Option<u64> = None;
  let mut logical_bytes = 0u64;
  let mut dec = Dec::default();
  let mut bar = show.then(|| Progress::new(Role::Recv, 0));
  let mut buf = [0u8; DGRAM];
  let fin_le = FIN_MAGIC.to_le_bytes();
  let dir_le = DIR_MAGIC.to_le_bytes();
  let mut packets = 0u64;
  let mut last_data = first_us;
  let mut last_ack_us = first_us;
  let mut unacked = false;
  let mut complete_at: Option<u64> = None;

  loop {
    match sock.recv_from(&mut buf) {
      Ok((n, addr)) => {
        if let Some(ack) = handle_probe(&buf[..n]) {
          let _ = send_udp(sock, &ack, addr);
          continue;
        }
        if n == 4 && buf[..4] == fin_le {
          if ctx.amp.primary() == Some(addr) && complete_at.is_some() {
            break;
          }
          continue;
        }
        if n >= DIR_HDR_LEN && buf[..4] == dir_le {
          let mut hdr_rd = Rd::new(&buf[4..DIR_HDR_LEN]);
          let Ok(total_len) = hdr_rd.u32().map(|v| v as usize) else {
            continue;
          };
          let Ok(mut idx) = hdr_rd.u32().map(|v| v as usize) else {
            continue;
          };
          let payload = &buf[DIR_HDR_LEN..n];
          if !ManifestRx::valid_chunk(total_len, idx, payload.len()) || !ctx.amp.on_recv(addr, n) {
            continue;
          }
          last_data = now_us();
          if let Some(ack) = &dir_ack_done {
            ctx.send_guarded(sock, ack, addr);
            if total == Some(0) {
              complete_at = Some(last_data);
            }
            continue;
          }
          let m = match &mut manifest {
            Some(m) if m.data.len() == total_len => m,
            _ => {
              let Some(rx) = ManifestRx::new(total_len) else {
                continue;
              };
              manifest.insert(rx)
            }
          };
          m.on_chunk(idx, payload);
          // 非阻塞排空内核接收缓冲中已到达的后续清单分片，确保整批清单在单次 RTT 内收齐而免发多余部分 DIR_ACK
          while m.rem > 0 {
            let Ok((n2, addr2)) = sock.recv_from(&mut buf) else {
              break;
            };
            if n2 >= DIR_HDR_LEN && buf[..4] == dir_le {
              let mut r2 = Rd::new(&buf[4..DIR_HDR_LEN]);
              if let (Ok(t2), Ok(i2)) = (r2.u32(), r2.u32()) {
                let (t2, i2) = (t2 as usize, i2 as usize);
                let p2 = &buf[DIR_HDR_LEN..n2];
                if t2 == total_len
                  && ManifestRx::valid_chunk(t2, i2, p2.len())
                  && ctx.amp.on_recv(addr2, n2)
                {
                  last_data = now_us();
                  idx = i2;
                  m.on_chunk(i2, p2);
                }
              }
            }
          }
          if let Some(m) = manifest.take_if(|m| m.rem == 0) {
            let (entries, l_total, mode) = parse_manifest(&m.data)?;
            ctx.amp.mark_validated();
            let do_delete = !mode.partial && (mode.delete || delete);
            let (mut sk, delta_total, final_ack) = Sink::prepare(out, entries, &m.ack, do_delete)?;
            for _ in 0..3 {
              ctx.send_guarded(sock, &final_ack, addr);
            }
            sk.stage_copies()?;
            if delta_total == 0 {
              sk.finalize()?;
              complete_at = Some(now_us());
            }
            logical_bytes = l_total;
            sink = Some(sk);
            total = Some(delta_total);
            if let Some(b) = bar.as_mut() {
              b.set_total(delta_total);
            }
            dir_ack_done = Some(final_ack);
          } else if let Some(m) = &manifest
            && (idx + 1 == m.chunks || m.rx.is_multiple_of(8))
          {
            ctx.send_guarded(sock, &m.ack, addr);
            last_ack_us = now_us();
          }
          continue;
        }
        let Ok((h, payload)) = DataHeader::parse(&buf[..n]) else {
          continue;
        };
        let (Some(sk), Some(t)) = (&mut sink, total) else {
          continue;
        };
        if !ctx.amp.on_recv(addr, n) {
          continue;
        }
        last_data = now_us();
        let out_res = recv.on_data(&h, payload)?;
        ctx.ackgen.on_packet(h.pn);
        ctx.rx_ts.on_data(h.pn, h.ts, last_data);
        unacked = true;
        let mut force_ack = h.pn == 0 || h.idx + 1 == h.cnt;
        packets += 1;
        if let Outcome::BlockReady { blk, .. } = out_res {
          ctx.amp.mark_validated();
          let mut ready = [(0u32, false); 32];
          ready[0] = (blk, h.is_zstd());
          let mut n_ready = 1usize;
          let mut drained = 0u32;
          while n_ready < ready.len() && drained < AckGen::MAX_PACKETS {
            match sock.recv_from(&mut buf) {
              Ok((n2, addr2)) => {
                if let Some(ack) = handle_probe(&buf[..n2]) {
                  let _ = send_udp(sock, &ack, addr2);
                  continue;
                }
                let Ok((h2, payload2)) = DataHeader::parse(&buf[..n2]) else {
                  continue;
                };
                if !ctx.amp.on_recv(addr2, n2) {
                  continue;
                }
                last_data = now_us();
                let res2 = recv.on_data(&h2, payload2)?;
                ctx.ackgen.on_packet(h2.pn);
                ctx.rx_ts.on_data(h2.pn, h2.ts, last_data);
                unacked = true;
                force_ack |= h2.pn == 0 || h2.idx + 1 == h2.cnt;
                packets += 1;
                drained += 1;
                if let Outcome::BlockReady { blk: blk2, .. } = res2 {
                  ready[n_ready] = (blk2, h2.is_zstd());
                  n_ready += 1;
                }
              }
              Err(_) => break,
            }
          }
          if n_ready > 1 {
            ready[..n_ready].sort_unstable_by_key(|&(b, _)| b);
          }
          for &(b, zstd) in &ready[..n_ready] {
            dec.write(&mut recv, sk, b, zstd, t)?;
          }
          ctx.dec_mbps = dec.dec_mbps();
        }
        if let Some(b) = bar.as_mut() {
          b.tick(done_bytes(recv.done_lo(), t), 0, packets);
        }
        if u64::from(recv.done_lo()) >= t.div_ceil(BLK as u64) {
          if complete_at.is_none() {
            sk.finalize()?;
          }
          let done_now = ctx.send_ack_burst(sock, addr, &recv, 3)?;
          unacked = false;
          last_ack_us = done_now;
          if complete_at.is_none() {
            complete_at = Some(done_now);
            if let Some(b) = bar.as_mut() {
              b.mark_done();
            }
          }
          last_data = done_now;
        } else if unacked {
          let ack_now = if force_ack {
            Some(ctx.send_ack_force(sock, addr, &recv)?)
          } else {
            ctx.send_ack(sock, addr, &recv)?
          };
          if let Some(ack_t) = ack_now {
            unacked = false;
            last_ack_us = ack_t;
          }
        }
      }
      Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
        let now = now_us();
        if let Some(addr) = ctx.amp.primary() {
          if let Some(m) = &manifest {
            if now.saturating_sub(last_data) >= 20_000 && now.saturating_sub(last_ack_us) >= 50_000
            {
              ctx.send_guarded(sock, &m.ack, addr);
              last_ack_us = now;
            }
          } else if total.is_some_and(|t| t > 0)
            && (unacked || now.saturating_sub(last_ack_us) >= MAX_TACK_US)
          {
            last_ack_us = ctx.send_ack_force(sock, addr, &recv)?;
            unacked = false;
          }
        }
        if let Some(at) = complete_at {
          if now.saturating_sub(at) > 2_000_000 {
            break;
          }
        } else if now.saturating_sub(last_data) > IDLE_US && packets > 0 {
          break;
        }
        sleep(Duration::from_micros(200));
      }
      Err(e) => return Err(e.into()),
    }
  }

  let delta_bytes = total.ok_or(UdpError::Corrupt)?;
  if u64::from(recv.done_lo()) < delta_bytes.div_ceil(BLK as u64) {
    return Err(UdpError::Corrupt);
  }
  if let Some(b) = bar.as_mut() {
    b.set_total(delta_bytes);
    b.finish(delta_bytes, 0, packets);
  }
  Ok(Stats {
    bytes: logical_bytes,
    us: complete_at.unwrap_or_else(now_us).saturating_sub(first_us),
    packets,
    retx: 0,
    blocks: delta_bytes.div_ceil(BLK as u64) as u32,
    srtt_us: 0,
    rtt_min_us: 0,
  })
}
