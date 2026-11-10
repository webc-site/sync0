#[cfg(not(miri))]
use std::fs;
use std::{sync::Arc, thread};

use aok::{OK, Void};
use ctor::ctor;
use log_init::init;
use s0_fs::{AtomicBitset, DisjointArena};
#[cfg(not(miri))]
use s0_fs::{SharedSink, SourceReader, build_manifest_from_paths};
#[cfg(not(miri))]
use tempfile::tempdir;

#[ctor(unsafe)]
fn _log_init() {
  init();
}

#[test]
fn test_disjoint_arena_and_atomic_bitset_concurrency() -> Void {
  let slots = 16usize;
  let slot_len = 32usize;
  let arena = Arc::new(DisjointArena::new(slots * slot_len));
  let bitset = Arc::new(AtomicBitset::from_bits(&[0u8; 2], slots as u32));

  thread::scope(|s| {
    for tid in 0..4usize {
      let a = Arc::clone(&arena);
      let b = Arc::clone(&bitset);
      s.spawn(move || {
        for idx in (tid..slots).step_by(4) {
          let payload = [idx as u8 ^ 0x5A; 32];
          unsafe { a.write_at(idx * slot_len, &payload) };
          assert!(b.mark(idx as u32));
        }
      });
    }
  });

  assert!(bitset.is_range_complete(0, slots as u32));
  for idx in 0..slots {
    assert!(bitset.has(idx as u32));
    let slice = unsafe { arena.slice_at(idx * slot_len, slot_len) };
    assert_eq!(slice, &[idx as u8 ^ 0x5A; 32]);
  }
  OK
}

#[test]
#[cfg(not(miri))]
fn test_concurrent_shared_sink_and_basis_reuse() -> Void {
  let src_dir = tempdir()?;
  let dst_dir = tempdir()?;

  let mut big = vec![0u8; 260_000];
  for (i, b) in big.iter_mut().enumerate() {
    *b = (i.wrapping_mul(73) ^ (i >> 5)) as u8;
  }
  fs::create_dir_all(src_dir.path().join("sub"))?;
  fs::write(src_dir.path().join("sub/big.bin"), &big)?;
  fs::write(src_dir.path().join("sub/empty.txt"), b"")?;
  for i in 0..12u32 {
    fs::write(
      src_dir.path().join(format!("sub/s_{i:02}.txt")),
      format!("small_arena_slot_{i}").as_bytes(),
    )?;
  }

  let mf = build_manifest_from_paths::<&str>(src_dir.path(), &[], true)?;
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
