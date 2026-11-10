use std::{
  fs,
  net::{Ipv4Addr, SocketAddr, UdpSocket},
  sync::Arc,
  thread,
  time::Duration,
};

use aok::{OK, Void};
use ctor::ctor;
use log_init::init;
use s0_cdc::{ChunkSig, Wr, hash128};
use s0_codec::Codec;
use s0_const::{
  DEFAULT_PKT, DIR_MAGIC, FIN_MAGIC, MANIFEST_ZSTD, MAX_BLOCK, PKT_PUNCH, PUNCH_PKT_LEN, TUNNEL_PKT,
};
use s0_fs::{BlockSource, SharedSink, SourceReader, build_manifest_from_paths};
use s0_udp::{
  Config, Outcome, RangeSpec, Receiver, UdpStats,
  bench::{handle_probe_observed, recv},
  bind_udp_ephemeral, probe_external_udp_addr, punch_udp, recv_range_udp, send_range_udp,
};
use s0_wire::{StreamDataHeader, decode_punch, encode_punch, encode_punch_padded};
use tempfile::tempdir;

#[ctor(unsafe)]
fn _log_init() {
  init();
}

fn run_range_pair<S>(
  source: Arc<S>,
  sink: &Arc<SharedSink>,
  sid: u32,
  blk_lo: u32,
  blk_hi: u32,
) -> (UdpStats, UdpStats)
where
  S: Send + Sync + 'static,
  for<'a> &'a S: Into<BlockSource<'a>>,
{
  let (s_tx, tx_addrs) = bind_udp_ephemeral().unwrap();
  let (s_rx, rx_addrs) = bind_udp_ephemeral().unwrap();
  let spec = RangeSpec {
    session_id: sid,
    blk_lo,
    blk_hi,
    eff_rate: 0,
    timeout: Duration::from_secs(8),
  };
  let t_tx = thread::spawn(move || {
    let peers = punch_udp(&s_tx, &rx_addrs, sid).unwrap();
    send_range_udp(&s_tx, &peers, &*source, &[], spec).unwrap()
  });
  let peers_tx = punch_udp(&s_rx, &tx_addrs, sid).unwrap();
  let st_rx = recv_range_udp(&s_rx, &peers_tx, sink, spec).unwrap();
  (t_tx.join().unwrap(), st_rx)
}

#[test]
fn test_multi_peer_striped_udp_range_transfer() -> Void {
  let src_dir = tempdir()?;
  let dst_dir = tempdir()?;

  let mut payload = vec![0u8; 600_000];
  let mut rng = 0x1234_5678u64;
  for (i, b) in payload.iter_mut().enumerate() {
    if i < 200_000 {
      *b = b"sync0_mzstd_compressible_block_pattern_"[i % 39];
    } else {
      rng ^= rng << 13;
      rng ^= rng >> 7;
      rng ^= rng << 17;
      *b = rng as u8;
    }
  }
  fs::write(src_dir.path().join("movie.bin"), &payload)?;

  let mf = build_manifest_from_paths::<&str>(src_dir.path(), &[], true)?;
  let total = mf.total_blocks();
  assert!(total >= 10);
  let mid = total / 2;

  let reader1 = Arc::new(SourceReader::open(src_dir.path(), mf.clone())?);
  let reader2 = Arc::clone(&reader1);
  let sink = Arc::new(SharedSink::prepare(dst_dir.path(), mf)?);
  let have_bits = sink.initial_have_bits().to_vec();

  let (s1_sock, s1_addrs) = bind_udp_ephemeral()?;
  let (c1_sock, c1_addrs) = bind_udp_ephemeral()?;
  let (s2_sock, s2_addrs) = bind_udp_ephemeral()?;
  let (c2_sock, c2_addrs) = bind_udp_ephemeral()?;

  let spec1 = RangeSpec {
    session_id: 101,
    blk_lo: 0,
    blk_hi: mid,
    eff_rate: 0,
    timeout: Duration::from_secs(5),
  };
  let spec2 = RangeSpec {
    session_id: 202,
    blk_lo: mid,
    blk_hi: total,
    eff_rate: 0,
    timeout: Duration::from_secs(5),
  };

  let have1 = have_bits.clone();
  let t_send1 = thread::spawn(move || {
    let peers = punch_udp(&s1_sock, &c1_addrs, 101).unwrap();
    send_range_udp(&s1_sock, &peers, &reader1, &have1, spec1).unwrap()
  });

  let have2 = have_bits;
  let t_send2 = thread::spawn(move || {
    let peers = punch_udp(&s2_sock, &c2_addrs, 202).unwrap();
    send_range_udp(&s2_sock, &peers, &reader2, &have2, spec2).unwrap()
  });

  let sink1 = Arc::clone(&sink);
  let t_recv1 = thread::spawn(move || {
    let peers = punch_udp(&c1_sock, &s1_addrs, 101).unwrap();
    recv_range_udp(&c1_sock, &peers, &sink1, spec1).unwrap()
  });

  let sink2 = Arc::clone(&sink);
  let t_recv2 = thread::spawn(move || {
    let peers = punch_udp(&c2_sock, &s2_addrs, 202).unwrap();
    recv_range_udp(&c2_sock, &peers, &sink2, spec2).unwrap()
  });

  t_send1.join().unwrap();
  t_send2.join().unwrap();
  let st1 = t_recv1.join().unwrap();
  let st2 = t_recv2.join().unwrap();
  assert_eq!(st1.blocks_transferred + st2.blocks_transferred, total);

  sink.commit()?;
  assert_eq!(fs::read(dst_dir.path().join("movie.bin"))?, payload);
  OK
}

#[test]
fn test_anti_amplification_and_nat_real_port_measurement() -> Void {
  let src_dir = tempdir()?;
  let dst_dir = tempdir()?;
  let payload = vec![0x5Au8; 131_072];
  fs::write(src_dir.path().join("data.bin"), &payload)?;

  let mf = build_manifest_from_paths::<&str>(src_dir.path(), &[], true)?;
  let total = mf.total_blocks();
  let reader = Arc::new(SourceReader::open(src_dir.path(), mf.clone())?);

  // 1. 抗反射放大验证：伪造受害者地址（不回送匹配 sid24 的 PKT_PUNCH/PKT_NACK），绝不向其发送任何 64KiB 数据块
  let victim_sock = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))?;
  victim_sock.set_nonblocking(true)?;
  let victim_addr = victim_sock.local_addr()?;
  let (sender_sock, _) = bind_udp_ephemeral()?;
  let res = send_range_udp(
    &sender_sock,
    &[victim_addr],
    &reader,
    &[],
    RangeSpec {
      session_id: 0x123456,
      blk_lo: 0,
      blk_hi: total,
      eff_rate: 0,
      timeout: Duration::from_millis(60),
    },
  );
  assert!(res.is_err());
  let mut buf = [0u8; 2048];
  while let Ok((n, _)) = victim_sock.recv_from(&mut buf) {
    assert_eq!(n, PUNCH_PKT_LEN);
    assert_eq!(buf[0], PKT_PUNCH);
  }

  // 1b. 接收端 (`recv_range_udp`) 抗反射放大验证：
  // 攻击者伪造受害者源地址发送 4B `PKT_PUNCH`，接收端回送给受害者的总字节数绝不超过 RFC 9000 `3 × rx_bytes` (12B) 上限
  let sink_amp = Arc::new(SharedSink::prepare(dst_dir.path(), mf.clone())?);
  let (rx_amp_sock, _) = bind_udp_ephemeral()?;
  let rx_amp_addr = SocketAddr::from((Ipv4Addr::LOCALHOST, rx_amp_sock.local_addr()?.port()));
  let dummy_peer = SocketAddr::from((Ipv4Addr::LOCALHOST, 9));
  let sink_amp_bg = Arc::clone(&sink_amp);
  let t_rx_amp = thread::spawn(move || {
    let _ = recv_range_udp(
      &rx_amp_sock,
      &[dummy_peer],
      &sink_amp_bg,
      RangeSpec {
        session_id: 0x234567,
        blk_lo: 0,
        blk_hi: total,
        eff_rate: 0,
        timeout: Duration::from_millis(60),
      },
    );
  });
  thread::sleep(Duration::from_millis(5));
  let spoofed_punch = encode_punch(0x234567);
  assert_eq!(spoofed_punch.len(), PUNCH_PKT_LEN);
  victim_sock.send_to(&spoofed_punch, rx_amp_addr)?;
  let _ = t_rx_amp.join();
  let mut reflected_bytes = 0usize;
  while let Ok((n, _)) = victim_sock.recv_from(&mut buf) {
    reflected_bytes += n;
  }
  assert!(
    reflected_bytes <= spoofed_punch.len() * 3,
    "reflection amplification detected: {} bytes sent for {}-byte spoofed punch ({}x)",
    reflected_bytes,
    spoofed_punch.len(),
    reflected_bytes / spoofed_punch.len()
  );

  // 1c. 单点接收端 (`bench::recv`) 抗反射放大验证：
  // 攻击者伪造受害者源地址发送单包高压缩比 `DIR_REQ`（Zstd 压缩后仅 ~80B，解压含 2000 个分块），
  // 接收端在收到后续数据块确认前绝不解除 `AntiAmp` 3× 限额
  let (bench_rx_sock, _) = bind_udp_ephemeral()?;
  let bench_rx_port = bench_rx_sock.local_addr()?.port();
  drop(bench_rx_sock);
  let bench_rx_addr = SocketAddr::from((Ipv4Addr::LOCALHOST, bench_rx_port));
  let bench_dst = dst_dir.path().join("bench_amp_out.bin");
  // 预置非空目标文件以触发接收端返回非全 0 的 have_bits 位图
  fs::write(&bench_dst, vec![0xAAu8; 65_536])?;
  let t_bench_rx = thread::spawn(move || {
    let _ = recv(bench_rx_addr, &bench_dst, false);
  });
  thread::sleep(Duration::from_millis(10));
  let mut raw_mf = Vec::new();
  raw_mf.extend_from_slice(&1u32.to_le_bytes()); // 1 entry
  raw_mf.push(0u8); // suf_len = 0 (single file)
  Wr::new(&mut raw_mf).uvar(2001); // nchunks + 1 = 2001 (2000 chunks)
  let dummy_sig = ChunkSig {
    offset: 0,
    len: 65_536,
    hash: hash128(&vec![0xAAu8; 65_536]),
  };
  let mut sigs = vec![dummy_sig; 2000];
  for (i, s) in sigs.iter_mut().enumerate() {
    s.offset = (i as u64) * 65_536;
  }
  ChunkSig::encode_slice(&sigs, &mut Wr::new(&mut raw_mf)).unwrap();
  let mut codec = Codec::new(3)?;
  let mut comp = vec![0u8; raw_mf.len() + 4096];
  let clen = codec.try_compress_block(&raw_mf, &mut comp)?.unwrap();
  let mut mf_payload = Vec::new();
  mf_payload.extend_from_slice(&(raw_mf.len() as u32).to_le_bytes());
  mf_payload.extend_from_slice(&((clen as u32) | MANIFEST_ZSTD).to_le_bytes());
  mf_payload.extend_from_slice(&comp[..clen]);
  let mut dir_pkt = Vec::new();
  dir_pkt.extend_from_slice(&DIR_MAGIC.to_le_bytes());
  dir_pkt.extend_from_slice(&(mf_payload.len() as u32).to_le_bytes());
  dir_pkt.extend_from_slice(&0u32.to_le_bytes());
  dir_pkt.extend_from_slice(&mf_payload);
  victim_sock.send_to(&dir_pkt, bench_rx_addr)?;
  thread::sleep(Duration::from_millis(40));
  let mut bench_reflected = 0usize;
  while let Ok((n, _)) = victim_sock.recv_from(&mut buf) {
    bench_reflected += n;
  }
  victim_sock.send_to(&FIN_MAGIC.to_le_bytes(), bench_rx_addr)?;
  let _ = t_bench_rx.join();
  assert!(
    bench_reflected <= dir_pkt.len() * 3,
    "bench::recv reflection amplification detected: {} bytes sent for {}-byte spoofed DIR_REQ ({:.1}x)",
    bench_reflected,
    dir_pkt.len(),
    bench_reflected as f64 / dir_pkt.len() as f64
  );

  // 2. QAD / STUN 式真实外部端口探测验证 (`probe_external_udp_addr`)
  let responder = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))?;
  let resp_addr = responder.local_addr()?;
  let t_resp = thread::spawn(move || {
    let mut b = [0u8; 64];
    if let Ok((n, from)) = responder.recv_from(&mut b)
      && let Some(ack) = handle_probe_observed(&b[..n], from)
    {
      let _ = responder.send_to(&ack, from);
    }
  });
  let client_sock = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))?;
  let expected_real_addr = client_sock.local_addr()?;
  let (observed_addr, _rtt) =
    probe_external_udp_addr(&client_sock, resp_addr, Duration::from_millis(200)).unwrap();
  assert_eq!(observed_addr, expected_real_addr);
  t_resp.join().unwrap();

  // 3. NAT 端口重写场景：控制面通告了错误的旧端口，但接收端从真实 NAT 映射端口发来打洞包，发送端自动锁定真实端口完成传输
  let sink = Arc::new(SharedSink::prepare(dst_dir.path(), mf)?);
  let (srv_sock, srv_addrs_raw) = bind_udp_ephemeral()?;
  let real_cli_sock = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))?;
  let dummy_port_addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 9));

  let reader_bg = Arc::clone(&reader);
  let t_srv = thread::spawn(move || {
    send_range_udp(
      &srv_sock,
      &[dummy_port_addr],
      &reader_bg,
      &[],
      RangeSpec {
        session_id: 777,
        blk_lo: 0,
        blk_hi: total,
        eff_rate: 0,
        timeout: Duration::from_secs(5),
      },
    )
    .unwrap()
  });

  let srv_peers = punch_udp(&real_cli_sock, &srv_addrs_raw, 777)?;
  let st = recv_range_udp(
    &real_cli_sock,
    &srv_peers,
    &sink,
    RangeSpec {
      session_id: 777,
      blk_lo: 0,
      blk_hi: total,
      eff_rate: 0,
      timeout: Duration::from_secs(5),
    },
  )?;
  assert_eq!(st.blocks_transferred, total);
  t_srv.join().unwrap();
  sink.commit()?;
  assert_eq!(fs::read(dst_dir.path().join("data.bin"))?, payload);
  OK
}

#[test]
fn test_in_progress_seeding() -> Void {
  let src_dir = tempdir()?;
  let leecher_dir = tempdir()?;
  let peer3_dir = tempdir()?;

  let mut payload = vec![0u8; 320_000];
  for (i, b) in payload.iter_mut().enumerate() {
    *b = ((i * 31 + 17) & 0xFF) as u8;
  }
  fs::write(src_dir.path().join("bundle.bin"), &payload)?;

  let mf = build_manifest_from_paths::<&str>(src_dir.path(), &[], true)?;
  let total = mf.total_blocks();
  assert!(total >= 4);
  let half = total / 2;

  // 边下载边上传测试 (`BlockSource::InProgress(&SharedSink)`)：
  // 节点 B 仅下载了前半部分 `[0, half)` 且尚未 `commit()`，即可直接从 `.s0_stage` 暂存池向节点 C 上传已落盘的 `[0, half)` 块
  let reader_origin = Arc::new(SourceReader::open(src_dir.path(), mf.clone())?);
  let sink_b = Arc::new(SharedSink::prepare(leecher_dir.path(), mf.clone())?);
  let (_, st_b) = run_range_pair(Arc::clone(&reader_origin), &sink_b, 910, 0, half);
  assert_eq!(st_b.blocks_transferred, half);
  assert!(sink_b.is_range_complete(0, half));
  assert!(!sink_b.is_range_complete(half, total));

  // 节点 B 在未 commit() 的情况下，直接作为上传源向节点 C 供应 `[0, half)`
  let sink_c = Arc::new(SharedSink::prepare(peer3_dir.path(), mf)?);
  let (_, st_c1) = run_range_pair(Arc::clone(&sink_b), &sink_c, 911, 0, half);
  assert_eq!(st_c1.blocks_transferred, half);
  assert!(sink_c.is_range_complete(0, half));

  // 节点 C 从源节点补齐后半部分 `[half, total)` 并提交验证完整性
  let (_, st_c2) = run_range_pair(reader_origin, &sink_c, 912, half, total);
  assert_eq!(st_c2.blocks_transferred, total - half);
  sink_c.commit()?;
  assert_eq!(fs::read(peer3_dir.path().join("bundle.bin"))?, payload);
  OK
}

#[test]
fn test_small_file_superblock_and_pack_stream() -> Void {
  let src_dir = tempdir()?;
  let dst_dir = tempdir()?;

  // 构造 > 8 MiB 的多小文件集合（750 个 12 KiB 小文件 = 9 MiB），同时触发：
  // 1. 发送端 SuperBlock 跨小文件合并压缩（每 5 个 12 KiB 小文件自动聚合为一个 <= 64 KiB SuperBlock）；
  // 2. 接收端 SmallPack::StreamFile 流式拼接暂存文件 `.s0_pack.tmp`（突破 8 MiB 阈值，零内存膨胀）与原子解包落盘。
  for i in 0..750u32 {
    let sub = src_dir.path().join(format!("dir_{:02}", i % 10));
    fs::create_dir_all(&sub)?;
    let mut content = Vec::with_capacity(12_000);
    let hdr = format!("file_idx={i:04}_header_");
    while content.len() < 12_000 {
      content.extend_from_slice(hdr.as_bytes());
    }
    content.truncate(12_000);
    fs::write(sub.join(format!("item_{i:04}.txt")), &content)?;
  }

  let mf = build_manifest_from_paths::<&str>(src_dir.path(), &[], true)?;
  let total = mf.total_blocks();
  assert_eq!(total, 750);

  let reader = Arc::new(SourceReader::open(src_dir.path(), mf.clone())?);
  let sink = Arc::new(SharedSink::prepare(dst_dir.path(), mf)?);
  let (st_tx, st_rx) = run_range_pair(reader, &sink, 950, 0, total);
  assert_eq!(st_tx.blocks_transferred, 750);
  assert_eq!(st_rx.blocks_transferred, 750);

  // commit 前验证从流式 `.s0_pack.tmp` 暂存池回读任意小文件块（支持边下载边做种）
  let mut check_buf = Vec::new();
  sink.read_verified_block(150, &mut check_buf)?;
  assert_eq!(check_buf.len(), 12_000);

  sink.commit()?;
  assert!(!dst_dir.path().join(".s0_pack.tmp").exists());
  for i in [0u32, 7, 150, 749] {
    let p = format!("dir_{:02}/item_{i:04}.txt", i % 10);
    assert_eq!(
      fs::read(dst_dir.path().join(&p))?,
      fs::read(src_dir.path().join(&p))?
    );
  }
  OK
}

#[test]
fn test_dplpmtud_adaptive_receiver_and_padded_punch() -> Void {
  // 1. 1400B DPLPMTUD 填充探测打洞包编解码验证
  let padded = encode_punch_padded(0x00AB_CDEF);
  assert_eq!(padded.len(), DEFAULT_PKT);
  assert_eq!(decode_punch(&padded).unwrap(), 0x00AB_CDEF);

  // 2. Receiver 动态适应 1400B -> 1200B 分片步长（含尾包乱序先到与中途降级重切分）
  let mut rx = Receiver::new(Config::new(16, MAX_BLOCK, DEFAULT_PKT)?.with_zstd())?;
  let mut raw_blk = vec![0u8; 25_000];
  for (i, b) in raw_blk.iter_mut().enumerate() {
    *b = ((i * 19 + 7) & 0xFF) as u8;
  }

  // 模拟发送端先按 1400B 发送了 idx=0，随后触发 MTU 降级改按 1200B 重新切分发送（且尾包先乱序到达）
  let cnt_1400 = raw_blk.len().div_ceil(DEFAULT_PKT) as u8;
  let mk_hdr = |idx, cnt, pn, ts| StreamDataHeader {
    flag: 0,
    idx,
    cnt,
    sid: 1,
    pn,
    blk: 0,
    ts,
  };
  assert_eq!(
    rx.on_data(&mk_hdr(0, cnt_1400, 1, 10), &raw_blk[..DEFAULT_PKT])?,
    Outcome::Stored
  );

  let cnt_1200 = raw_blk.len().div_ceil(TUNNEL_PKT) as u8;
  let (tail_idx, tail_off) = (cnt_1200 - 1, usize::from(cnt_1200 - 1) * TUNNEL_PKT);
  assert_eq!(
    rx.on_data(&mk_hdr(tail_idx, cnt_1200, 2, 20), &raw_blk[tail_off..])?,
    Outcome::Stored
  );

  for idx in 0..tail_idx {
    let off = usize::from(idx) * TUNNEL_PKT;
    let out = rx.on_data(
      &mk_hdr(idx, cnt_1200, 3 + u32::from(idx), 30),
      &raw_blk[off..off + TUNNEL_PKT],
    )?;
    if idx + 1 == tail_idx {
      assert!(matches!(
        out,
        Outcome::BlockReady {
          blk: 0,
          len: 25_000,
          ..
        }
      ));
    } else {
      assert_eq!(out, Outcome::Stored);
    }
  }
  assert_eq!(rx.data(0).unwrap(), raw_blk.as_slice());
  OK
}
