use std::{
  fs,
  net::{Ipv4Addr, SocketAddr, UdpSocket},
  sync::Arc,
  thread,
  time::Duration,
};

use aok::{OK, Void};
use s0_udp::{
  RangeSpec, bind_udp_ephemeral, handle_probe_observed, probe_external_udp_addr, punch_udp,
  recv_range_udp,
  s0_const::PKT_PUNCH,
  s0_fs::{SharedSink, SourceReader, build_manifest_from_path},
  send_range_udp,
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
