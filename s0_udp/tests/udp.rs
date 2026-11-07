use std::{fs, sync::Arc, thread, time::Duration};

use aok::{OK, Void};
use s0_fs::{SharedSink, SourceReader, build_manifest_from_path};
use s0_udp::{RangeSpec, bind_udp_ephemeral, punch_udp, recv_range_udp, send_range_udp};

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
