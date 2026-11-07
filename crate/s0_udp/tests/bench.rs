use std::{
  fs,
  net::{SocketAddr, UdpSocket},
  path::Path,
  thread,
  time::Duration,
};

use aok::{OK, Void};
use ctor::ctor;
use fastrand::Rng;
use log_init::init;
use s0_udp::{
  Role, Stats, UdpError, bench_recv, bench_send, bench_send_opt, handle_probe, make_probe,
  parse_probe_ack,
};
use tempfile::tempdir;

#[ctor(unsafe)]
fn log_init() {
  init();
}

fn free_addr() -> SocketAddr {
  let sock = UdpSocket::bind("127.0.0.1:0").unwrap();
  sock.local_addr().unwrap()
}

fn run_roundtrip(src: &Path, dst: &Path) -> Result<(Stats, Stats), UdpError> {
  let addr = free_addr();
  let dst_rx = dst.to_path_buf();
  let rx = thread::spawn(move || bench_recv(addr, &dst_rx, false).unwrap());
  thread::sleep(Duration::from_millis(20));
  let st_tx = bench_send(src, addr, 0, false)?;
  let st_rx = rx.join().unwrap();
  Ok((st_tx, st_rx))
}

#[test]
fn transfer_single_files_compressible_and_incompressible() -> Void {
  let dir = tempdir()?;

  let src_empty = dir.path().join("empty.bin");
  let dst_empty = dir.path().join("out_empty.bin");
  fs::write(&src_empty, b"")?;
  let (st_tx, st_rx) = run_roundtrip(&src_empty, &dst_empty)?;
  assert_eq!(st_tx.bytes, 0);
  assert_eq!(st_rx.bytes, 0);
  assert_eq!(fs::read(&dst_empty)?, b"");

  let src_txt = dir.path().join("data.tar");
  let dst_txt = dir.path().join("out.tar");
  let payload: Vec<u8> = (0..(300 * 1024)).map(|i| (i % 29) as u8).collect();
  fs::write(&src_txt, &payload)?;

  let (st_tx, st_rx) = run_roundtrip(&src_txt, &dst_txt)?;
  assert_eq!(st_tx.bytes, payload.len() as u64);
  assert_eq!(st_rx.bytes, payload.len() as u64);
  assert_eq!(fs::read(&dst_txt)?, payload);

  let src_mp4 = dir.path().join("video.mp4");
  let dst_mp4 = dir.path().join("out.mp4");
  let mut rng = Rng::with_seed(0x1234_5678_9abc_def0);
  let raw_mp4: Vec<u8> = (0..(300 * 1024)).map(|_| rng.u8(..)).collect();
  fs::write(&src_mp4, &raw_mp4)?;

  let (st_tx, st_rx) = run_roundtrip(&src_mp4, &dst_mp4)?;
  assert_eq!(st_tx.bytes, raw_mp4.len() as u64);
  assert_eq!(st_rx.bytes, raw_mp4.len() as u64);
  assert_eq!(fs::read(&dst_mp4)?, raw_mp4);
  OK
}

#[test]
fn transfer_directory_small_and_cross_block_files() -> Void {
  let dir = tempdir()?;
  let src = dir.path().join("src");
  let dst = dir.path().join("dst");
  fs::create_dir_all(src.join("a/b"))?;
  fs::create_dir_all(src.join("c"))?;

  fs::write(src.join("empty.txt"), b"")?;
  for i in 0..120u32 {
    let sub = if i % 2 == 0 { "a/b" } else { "c" };
    let content = format!("file-{i}-content-{}", "x".repeat((i as usize % 50) * 17));
    fs::write(
      src.join(sub).join(format!("f_{i:04}.txt")),
      content.as_bytes(),
    )?;
  }
  let big: Vec<u8> = (0..(200 * 1024)).map(|i| (i * 7 + 3) as u8).collect();
  fs::write(src.join("a/cross_block.bin"), &big)?;

  let (st_tx, st_rx) = run_roundtrip(&src, &dst)?;
  assert_eq!(st_tx.bytes, st_rx.bytes);

  assert_eq!(fs::read(dst.join("empty.txt"))?, b"");
  assert_eq!(fs::read(dst.join("a/cross_block.bin"))?, big);
  for i in 0..120u32 {
    let sub = if i % 2 == 0 { "a/b" } else { "c" };
    let p = format!("{sub}/f_{i:04}.txt");
    assert_eq!(fs::read(dst.join(&p))?, fs::read(src.join(&p))?);
  }
  OK
}

#[test]
fn cdc_incremental_file_and_dir_sync() -> Void {
  let dir = tempdir()?;
  let mut rng = Rng::with_seed(0x9876_5432_10fe_dcba);

  let src_file = dir.path().join("asset.pak");
  let dst_file = dir.path().join("out_asset.pak");
  let orig: Vec<u8> = (0..(2 * 1024 * 1024)).map(|_| rng.u8(..)).collect();
  fs::write(&src_file, &orig)?;

  let (st_full, _) = run_roundtrip(&src_file, &dst_file)?;
  assert_eq!(st_full.blocks, 16);
  assert_eq!(fs::read(&dst_file)?, orig);

  let (st_noop, st_noop_rx) = run_roundtrip(&src_file, &dst_file)?;
  assert_eq!(st_noop.bytes, orig.len() as u64);
  assert_eq!(st_noop_rx.bytes, orig.len() as u64);
  assert_eq!(st_noop.blocks, 0);
  assert_eq!(st_noop.packets, 0);
  assert_eq!(fs::read(&dst_file)?, orig);

  let ins_at = 600 * 1024;
  let del_at = 1500 * 1024;
  let ins_bytes: Vec<u8> = (0..5000).map(|_| rng.u8(..)).collect();
  let mut modified = Vec::with_capacity(orig.len() + ins_bytes.len() - 2000);
  modified.extend_from_slice(&orig[..ins_at]);
  modified.extend_from_slice(&ins_bytes);
  modified.extend_from_slice(&orig[ins_at..del_at]);
  modified.extend_from_slice(&orig[del_at + 2000..]);
  fs::write(&src_file, &modified)?;

  let (st_delta, st_delta_rx) = run_roundtrip(&src_file, &dst_file)?;
  assert_eq!(st_delta.bytes, modified.len() as u64);
  assert_eq!(st_delta_rx.bytes, modified.len() as u64);
  assert!(
    st_delta.blocks <= 2,
    "CDC 增量仅需传输 ≤ 2 块，实际 blocks={}",
    st_delta.blocks
  );
  assert_eq!(fs::read(&dst_file)?, modified);

  let src_dir = dir.path().join("sdir");
  let dst_dir = dir.path().join("ddir");
  fs::create_dir_all(&src_dir)?;
  for i in 0..50u32 {
    fs::write(
      src_dir.join(format!("s_{i:02}.txt")),
      format!("small-file-{i}").as_bytes(),
    )?;
  }
  fs::write(src_dir.join("big_a.bin"), &orig)?;
  run_roundtrip(&src_dir, &dst_dir)?;

  fs::write(src_dir.join("s_00.txt"), b"modified-small-file-0")?;
  fs::remove_file(src_dir.join("big_a.bin"))?;
  let mut big_b = Vec::with_capacity(orig.len() + 1024);
  big_b.extend_from_slice(&ins_bytes[..1024]);
  big_b.extend_from_slice(&orig);
  fs::write(src_dir.join("big_b.bin"), &big_b)?;

  let (st_dir_delta, _) = run_roundtrip(&src_dir, &dst_dir)?;
  assert!(
    st_dir_delta.blocks <= 2,
    "目录改名+微改增量块数应 ≤ 2，实际 blocks={}",
    st_dir_delta.blocks
  );
  assert_eq!(
    fs::read(dst_dir.join("s_00.txt"))?,
    b"modified-small-file-0"
  );
  assert_eq!(fs::read(dst_dir.join("big_b.bin"))?, big_b);
  assert!(dst_dir.join("big_a.bin").exists());

  fs::create_dir_all(src_dir.join("sub"))?;
  fs::rename(src_dir.join("big_b.bin"), src_dir.join("sub/big_c.bin"))?;
  fs::create_dir_all(dst_dir.join("stale_dir"))?;
  fs::write(dst_dir.join("stale_dir/orphan.txt"), b"remote-only")?;

  let addr = free_addr();
  let dst_rx = dst_dir.clone();
  let rx = thread::spawn(move || bench_recv(addr, &dst_rx, false).unwrap());
  thread::sleep(Duration::from_millis(20));
  let st_del = bench_send_opt(&src_dir, addr, 0, false, true)?;
  rx.join().unwrap();

  assert_eq!(
    st_del.blocks, 0,
    "纯改名 + delete 应 100% 复用本地 CDC 块，0 网络块"
  );
  assert_eq!(fs::read(dst_dir.join("sub/big_c.bin"))?, big_b);
  assert!(!dst_dir.join("big_a.bin").exists());
  assert!(!dst_dir.join("big_b.bin").exists());
  assert!(!dst_dir.join("stale_dir").exists());
  OK
}

#[test]
fn probe_and_stats_roundtrip() {
  let pkt = make_probe(1_000);
  let ack = handle_probe(&pkt).unwrap();
  assert_eq!(parse_probe_ack(&ack, 1_450), Some(450));
  assert!(handle_probe(&ack).is_none());
  assert!(parse_probe_ack(&pkt, 2_000).is_none());

  let mut st = Stats {
    bytes: 100,
    us: 50,
    packets: 2,
    retx: 1,
    blocks: 1,
    srtt_us: 12_000,
    rtt_min_us: 10_000,
  };
  st.merge(&Stats {
    bytes: 200,
    us: 30,
    packets: 3,
    retx: 0,
    blocks: 2,
    srtt_us: 15_000,
    rtt_min_us: 8_000,
  });
  assert_eq!(
    st.line(Role::Send),
    "{\"role\":\"send\",\"bytes\":300,\"us\":80,\"packets\":5,\"retx\":1,\"blocks\":3,\"srtt_us\":15000,\"rtt_min_us\":8000}"
  );
}
