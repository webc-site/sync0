use std::{fs, sync::Arc, thread};

use aok::{OK, Void};
use s0_udp::fs::{SharedSink, SourceReader, build_manifest_from_path};

#[ctor::ctor(unsafe)]
fn _log_init() {
  log_init::init();
}

#[test]
fn test_concurrent_shared_sink_and_basis_reuse() -> Void {
  let src_dir = tempfile::tempdir()?;
  let dst_dir = tempfile::tempdir()?;

  let mut big = vec![0u8; 260_000];
  for (i, b) in big.iter_mut().enumerate() {
    *b = (i.wrapping_mul(73) ^ (i >> 5)) as u8;
  }
  fs::create_dir_all(src_dir.path().join("sub"))?;
  fs::write(src_dir.path().join("sub/big.bin"), &big)?;
  fs::write(src_dir.path().join("sub/empty.txt"), b"")?;

  let mf = build_manifest_from_path(src_dir.path(), "", true)?;
  let reader = Arc::new(SourceReader::open(src_dir.path(), mf.clone())?);
  let sink = Arc::new(SharedSink::prepare(dst_dir.path(), mf.clone())?);

  let total = reader.total_blocks();
  assert!(total >= 2);

  // 模拟 3 个对等节点线程并发向同一个 SharedSink 写入不相交的块区间
  let mut handles = Vec::new();
  for tid in 0..3u32 {
    let r = Arc::clone(&reader);
    let s = Arc::clone(&sink);
    handles.push(thread::spawn(move || {
      let mut buf = Vec::new();
      for blk in (tid..total).step_by(3) {
        r.read_block(blk, &mut buf).unwrap();
        s.write_verified_block(blk, &buf).unwrap();
      }
    }));
  }
  for h in handles {
    h.join().unwrap();
  }

  sink.commit()?;
  assert_eq!(fs::read(dst_dir.path().join("sub/big.bin"))?, big);
  assert_eq!(fs::read(dst_dir.path().join("sub/empty.txt"))?, b"");

  // 再次对已同步目录调用 SharedSink::prepare，应 100% 命中本地基准块
  let sink2 = SharedSink::prepare(dst_dir.path(), mf)?;
  assert!(sink2.is_range_complete(0, total));
  OK
}
