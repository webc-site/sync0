use std::{
  alloc::{GlobalAlloc, Layout, System},
  hint::black_box,
  sync::atomic::{AtomicUsize, Ordering},
};

use ctor::ctor;
use log_init::init;
use s0_sim::s0_codec::{Codec, MAX_BLOCK, SLACK};

#[ctor(unsafe)]
fn log_init() {
  init();
}

struct Counting;

static ALLOCS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
  unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    ALLOCS.fetch_add(1, Ordering::Relaxed);
    unsafe { System.alloc(layout) }
  }

  unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
    unsafe { System.dealloc(ptr, layout) }
  }

  unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
    ALLOCS.fetch_add(1, Ordering::Relaxed);
    unsafe { System.alloc_zeroed(layout) }
  }

  unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
    ALLOCS.fetch_add(1, Ordering::Relaxed);
    unsafe { System.realloc(ptr, layout, new_size) }
  }
}

#[global_allocator]
static A: Counting = Counting;

/// 预热后 `compress_to_slice` / `decompress_to_slice` 不应再分配
#[test]
fn block_codec_zero_alloc_after_warmup() {
  let raw: Vec<u8> = b"sync0 zero-alloc block "
    .iter()
    .copied()
    .cycle()
    .take(64 * 1024)
    .collect();
  let mut c = Codec::new(3).unwrap();
  let mut enc = vec![0u8; MAX_BLOCK + SLACK];
  let mut dec = vec![0u8; MAX_BLOCK + SLACK];

  // 预热：让 mzstd 保留缓冲
  let e = c.compress_block(&raw, &mut enc).unwrap();
  assert!(e.compressed);
  for _ in 0..8 {
    let e = c.compress_block(&raw, &mut enc).unwrap();
    let n = c.decompress_block(&enc[..e.len], &mut dec).unwrap();
    assert_eq!(n, raw.len());
    black_box(n);
  }

  let before = ALLOCS.load(Ordering::Relaxed);
  for _ in 0..1000 {
    let e = c.compress_block(&raw, &mut enc).unwrap();
    let n = c.decompress_block(&enc[..e.len], &mut dec).unwrap();
    assert_eq!(n, raw.len());
    black_box(e);
  }
  let after = ALLOCS.load(Ordering::Relaxed);
  assert_eq!(after - before, 0, "块编解码发生堆分配");
}
