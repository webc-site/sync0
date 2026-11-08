use std::{
  fs,
  net::{Ipv4Addr, SocketAddr, UdpSocket},
  sync::Arc,
  thread,
  time::Duration,
};

use aok::{OK, Void};
use s0_udp::{
  RangeSpec,
  bench::handle_probe_observed,
  bind_udp_ephemeral,
  consts::PKT_PUNCH,
  fs::{SharedSink, SourceReader, build_manifest_from_path},
  probe_external_udp_addr, punch_udp, recv_range_udp, send_range_udp,
};

#[ctor::ctor(unsafe)]
fn _log_init() {
  log_init::init();
}

#[test]
fn test_multi_peer_striped_udp_range_transfer() -> Void {
  let src_dir = tempfile::tempdir()?;
  let dst_dir = tempfile::tempdir()?;

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

  let mf = build_manifest_from_path(src_dir.path(), "", true)?;
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
  let src_dir = tempfile::tempdir()?;
  let dst_dir = tempfile::tempdir()?;
  let payload = vec![0x5Au8; 131_072];
  fs::write(src_dir.path().join("data.bin"), &payload)?;

  let mf = build_manifest_from_path(src_dir.path(), "", true)?;
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
    assert_eq!(n, 5);
    assert_eq!(buf[0], PKT_PUNCH);
  }

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
fn test_anti_poisoning_and_in_progress_seeding() -> Void {
  use s0_udp::{UdpError, consts::DATA_HDR_LEN, wire::DataHeader};

  let src_dir = tempfile::tempdir()?;
  let leecher_dir = tempfile::tempdir()?;
  let peer3_dir = tempfile::tempdir()?;

  let mut payload = vec![0u8; 320_000];
  for (i, b) in payload.iter_mut().enumerate() {
    *b = ((i * 31 + 17) & 0xFF) as u8;
  }
  fs::write(src_dir.path().join("bundle.bin"), &payload)?;

  let mf = build_manifest_from_path(src_dir.path(), "", true)?;
  let total = mf.total_blocks();
  assert!(total >= 4);
  let half = total / 2;

  // 1. 恶意节点投毒测试：连续发送 3 个伪造/篡改载荷的分块，接收端通过 BLAKE3-128 摘要校验拦截（零落盘污染）并驱逐该恶意节点 (`UdpError::PoisonedPeer`)
  let sink_poison = SharedSink::prepare(leecher_dir.path(), mf.clone())?;
  let (mal_sock, mal_addrs) = bind_udp_ephemeral()?;
  let (rx_sock, rx_addrs) = bind_udp_ephemeral()?;
  let rx_peers = punch_udp(&mal_sock, &rx_addrs, 909)?;
  let mal_peers = punch_udp(&rx_sock, &mal_addrs, 909)?;

  let t_mal = thread::spawn(move || {
    let target = rx_peers[0];
    let mut pkt = [0xEEu8; DATA_HDR_LEN + 1024];
    for strike in 0..3u32 {
      let hdr = DataHeader {
        session_id: 909,
        block_idx: strike % half,
        seg_idx: 0,
        seg_cnt: 1,
        offset_in_block: 0,
        payload_len: 1024,
      };
      let hdr_slice: &mut [u8; DATA_HDR_LEN] = (&mut pkt[..DATA_HDR_LEN]).try_into().unwrap();
      hdr.encode(hdr_slice);
      pkt[DATA_HDR_LEN] = strike as u8;
      let _ = mal_sock.send_to(&pkt, target);
      thread::sleep(Duration::from_millis(5));
    }
  });

  let poison_res = recv_range_udp(
    &rx_sock,
    &mal_peers,
    &sink_poison,
    RangeSpec {
      session_id: 909,
      blk_lo: 0,
      blk_hi: half,
      eff_rate: 0,
      timeout: Duration::from_secs(2),
    },
  );
  t_mal.join().unwrap();
  assert!(matches!(poison_res, Err(UdpError::PoisonedPeer(_))));
  assert!(!sink_poison.has_block(0));

  // 2. 边下载边上传测试 (`BlockSource::InProgress(&SharedSink)`)：
  // 节点 B 仅下载了前半部分 `[0, half)` 且尚未 `commit()`，即可直接从 `.s0tmp` 暂存池向节点 C 上传已校验的 `[0, half)` 块
  let reader_origin = Arc::new(SourceReader::open(src_dir.path(), mf.clone())?);
  let sink_b = Arc::new(sink_poison);
  let (s_orig, orig_addrs) = bind_udp_ephemeral()?;
  let (s_b_rx, b_rx_addrs) = bind_udp_ephemeral()?;
  let spec_first_half = RangeSpec {
    session_id: 910,
    blk_lo: 0,
    blk_hi: half,
    eff_rate: 0,
    timeout: Duration::from_secs(5),
  };
  let r_orig = Arc::clone(&reader_origin);
  let t_orig = thread::spawn(move || {
    let peers = punch_udp(&s_orig, &b_rx_addrs, 910).unwrap();
    send_range_udp(&s_orig, &peers, &r_orig, &[], spec_first_half).unwrap()
  });
  let peers_orig = punch_udp(&s_b_rx, &orig_addrs, 910)?;
  let st_b = recv_range_udp(&s_b_rx, &peers_orig, &sink_b, spec_first_half)?;
  t_orig.join().unwrap();
  assert_eq!(st_b.blocks_transferred, half);
  assert!(sink_b.is_range_complete(0, half));
  assert!(!sink_b.is_range_complete(half, total));

  // 节点 B 在未 commit() 的情况下，直接作为上传源向节点 C 供应 `[0, half)`
  let sink_c = Arc::new(SharedSink::prepare(peer3_dir.path(), mf)?);
  let (s_b_tx, b_tx_addrs) = bind_udp_ephemeral()?;
  let (s_c_rx, c_rx_addrs) = bind_udp_ephemeral()?;
  let spec_b_to_c = RangeSpec {
    session_id: 911,
    blk_lo: 0,
    blk_hi: half,
    eff_rate: 0,
    timeout: Duration::from_secs(5),
  };
  let sink_b_seed = Arc::clone(&sink_b);
  let t_b_seed = thread::spawn(move || {
    let peers = punch_udp(&s_b_tx, &c_rx_addrs, 911).unwrap();
    send_range_udp(&s_b_tx, &peers, &sink_b_seed, &[], spec_b_to_c).unwrap()
  });
  let peers_b = punch_udp(&s_c_rx, &b_tx_addrs, 911)?;
  let st_c1 = recv_range_udp(&s_c_rx, &peers_b, &sink_c, spec_b_to_c)?;
  t_b_seed.join().unwrap();
  assert_eq!(st_c1.blocks_transferred, half);
  assert!(sink_c.is_range_complete(0, half));

  // 节点 C 从源节点补齐后半部分 `[half, total)` 并提交验证完整性
  let (s_orig2, orig2_addrs) = bind_udp_ephemeral()?;
  let (s_c_rx2, c_rx2_addrs) = bind_udp_ephemeral()?;
  let spec_second_half = RangeSpec {
    session_id: 912,
    blk_lo: half,
    blk_hi: total,
    eff_rate: 0,
    timeout: Duration::from_secs(5),
  };
  let t_orig2 = thread::spawn(move || {
    let peers = punch_udp(&s_orig2, &c_rx2_addrs, 912).unwrap();
    send_range_udp(&s_orig2, &peers, &reader_origin, &[], spec_second_half).unwrap()
  });
  let peers_orig2 = punch_udp(&s_c_rx2, &orig2_addrs, 912)?;
  let st_c2 = recv_range_udp(&s_c_rx2, &peers_orig2, &sink_c, spec_second_half)?;
  t_orig2.join().unwrap();
  assert_eq!(st_c2.blocks_transferred, total - half);
  sink_c.commit()?;
  assert_eq!(fs::read(peer3_dir.path().join("bundle.bin"))?, payload);
  OK
}

#[test]
fn test_small_file_superblock_and_pack_stream() -> Void {
  let src_dir = tempfile::tempdir()?;
  let dst_dir = tempfile::tempdir()?;

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

  let mf = build_manifest_from_path(src_dir.path(), "", true)?;
  let total = mf.total_blocks();
  assert_eq!(total, 750);

  let reader = Arc::new(SourceReader::open(src_dir.path(), mf.clone())?);
  let sink = Arc::new(SharedSink::prepare(dst_dir.path(), mf)?);
  let (s_tx, tx_addrs) = bind_udp_ephemeral()?;
  let (s_rx, rx_addrs) = bind_udp_ephemeral()?;
  let spec = RangeSpec {
    session_id: 950,
    blk_lo: 0,
    blk_hi: total,
    eff_rate: 0,
    timeout: Duration::from_secs(8),
  };

  let r_tx = Arc::clone(&reader);
  let t_tx = thread::spawn(move || {
    let peers = punch_udp(&s_tx, &rx_addrs, 950).unwrap();
    send_range_udp(&s_tx, &peers, &r_tx, &[], spec).unwrap()
  });
  let peers_tx = punch_udp(&s_rx, &tx_addrs, 950)?;
  let st_rx = recv_range_udp(&s_rx, &peers_tx, &sink, spec)?;
  let st_tx = t_tx.join().unwrap();
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
