#[cfg(not(miri))]
use std::{
  env::temp_dir,
  fs::{File, remove_file, write},
};

use aok::{OK, Void};
use ctor::ctor;
use log_init::init;
#[cfg(not(miri))]
use s0_cdc::{BasisIndex, chunk_file, chunk_file_range_with_buf, match_basis_file};
use s0_cdc::{
  CdcConfig, Manifest, ManifestEntry, Rd, WireError, Wr, bitset_to_roaring, chunk_slice,
  decode_roaring_bits, encode_roaring_bits, for_each_missing_span, is_roaring_full_frame,
  roaring_runs, roaring_to_bitset, set_bit_usize,
};

#[ctor(unsafe)]
fn _log_init() {
  init();
}

#[test]
fn test_varint_and_front_coding_bijection() -> Void {
  let mut buf = Vec::new();
  let mut w = Wr::new(&mut buf);
  for &v in &[0usize, 1, 127, 128, 255, 16384, 32767] {
    w.u15(v)?;
  }
  assert_eq!(w.u15(32768), Err(WireError::Overflow));
  for &v in &[0u64, 1, 127, 128, 16383, 16384, u32::MAX as u64, u64::MAX] {
    w.uvar(v);
  }
  let mut r = Rd::new(&buf);
  for &v in &[0u16, 1, 127, 128, 255, 16384, 32767] {
    assert_eq!(r.u15()?, v);
  }
  for &v in &[0u64, 1, 127, 128, 16383, 16384, u32::MAX as u64, u64::MAX] {
    assert_eq!(r.uvar()?, v);
  }
  assert!(r.is_empty());

  // 10 字节 ULEB128 最高字节 > 1 必须判定为 u64 溢出
  let bad_uvar = [0x80u8, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x02];
  assert_eq!(Rd::new(&bad_uvar).uvar(), Err(WireError::Overflow));
  OK
}

#[test]
fn test_fastcdc_and_manifest_subpath() -> Void {
  #[cfg(miri)]
  let len = 40_000;
  #[cfg(not(miri))]
  let len = 300_000;
  let mut data = vec![0u8; len];
  for (i, b) in data.iter_mut().enumerate() {
    *b = (i.wrapping_mul(131) ^ (i >> 7)) as u8;
  }
  let chunks = chunk_slice(&data, CdcConfig::DEFAULT);
  assert!(chunks.len() >= 2);
  let total_len: u64 = chunks.iter().map(|c| u64::from(c.len)).sum();
  assert_eq!(total_len, data.len() as u64);

  let mf = Manifest {
    entries: vec![
      ManifestEntry {
        path: "a/b/c.bin".into(),
        size: data.len() as u64,
        mode: 0o755,
        mtime: 1_700_000_000_123,
        chunks: chunks.clone(),
        deleted: false,
      },
      ManifestEntry {
        path: "a/b/d.txt".into(),
        size: 0,
        mode: 0o644,
        mtime: 0,
        chunks: vec![],
        deleted: false,
      },
      ManifestEntry {
        path: "other/x.log".into(),
        size: 0,
        mode: 0o644,
        mtime: 0,
        chunks: vec![],
        deleted: true,
      },
    ],
    delete_extraneous: true,
    partial: false,
  };

  let raw = mf.encode()?;
  let dec = Manifest::decode(&raw)?;
  assert_eq!(dec, mf);
  assert_eq!(dec.root_hash()?, mf.root_hash()?);

  // 测试子路径选择性过滤
  let sub = mf.filter_paths(&["a/b"]);
  assert_eq!(sub.entries.len(), 2);
  let single = mf.filter_paths(&["a/b/c.bin"]);
  assert_eq!(single.entries.len(), 1);

  // 测试缺失块区间计算与 RoaringBitmap 编解码/游程提取双射
  let mut have = vec![0u8; 2];
  set_bit_usize(&mut have, 1);
  set_bit_usize(&mut have, 2);
  set_bit_usize(&mut have, 5);
  let mut spans = Vec::new();
  for_each_missing_span(&have, 0, 8, |lo, hi| spans.push((lo, hi)));
  assert_eq!(spans, vec![(0, 1), (3, 5), (6, 8)]);
  let rb_small = bitset_to_roaring(&have, 8);
  assert_eq!(roaring_runs(&rb_small, 8), vec![(1, 3), (5, 6)]);
  assert_eq!(roaring_to_bitset(&rb_small, 8), vec![have[0]]);

  let total_large = 4096u32;
  let empty_bits = vec![0u8; (total_large as usize) / 8];
  let enc_empty = encode_roaring_bits(&empty_bits, total_large);
  assert_eq!(enc_empty.len(), 1);
  assert_eq!(decode_roaring_bits(&enc_empty, total_large)?, empty_bits);

  let full_bits = vec![0xFFu8; (total_large as usize) / 8];
  let enc_full = encode_roaring_bits(&full_bits, total_large);
  assert_eq!(enc_full.len(), 1);
  assert!(is_roaring_full_frame(&enc_full));
  assert_eq!(decode_roaring_bits(&enc_full, total_large)?, full_bits);

  let mut macro_bits = vec![0u8; (total_large as usize) / 8];
  for b in (0..256usize).chain(1024..1536) {
    set_bit_usize(&mut macro_bits, b);
  }
  let enc_macro = encode_roaring_bits(&macro_bits, total_large);
  assert!(enc_macro.len() < 40);
  assert_eq!(decode_roaring_bits(&enc_macro, total_large)?, macro_bits);

  #[cfg(not(miri))]
  {
    // 测试 >= 8 MiB 大文件多线程收敛并行 FastCDC 与单线程切片分块 100% 比特级一致
    let mut big = vec![0u8; 9 * 1024 * 1024];
    for (i, b) in big.iter_mut().enumerate() {
      *b = (i.wrapping_mul(257) ^ (i >> 9) ^ (i >> 15)) as u8;
    }
    let p = temp_dir().join("s0_cdc_par_test.bin");
    write(&p, &big)?;
    let f = File::open(&p)?;
    let par_chunks = chunk_file(&f, big.len() as u64, CdcConfig::DEFAULT)?;
    let seq_chunks = chunk_slice(&big, CdcConfig::DEFAULT);
    assert_eq!(par_chunks, seq_chunks);

    // 测试任意子区间 [start_off, end_off) 多线程收敛 FastCDC 与切片分块一致
    let sub_lo = 512 * 1024usize;
    let sub_hi = big.len() - 256 * 1024;
    let mut buf = Vec::new();
    let range_chunks = chunk_file_range_with_buf(
      &f,
      sub_lo as u64,
      sub_hi as u64,
      CdcConfig::DEFAULT,
      &mut buf,
    )?;
    let mut expected_sub = chunk_slice(&big[sub_lo..sub_hi], CdcConfig::DEFAULT);
    for c in &mut expected_sub {
      c.offset += sub_lo as u64;
    }
    assert_eq!(range_chunks, expected_sub);

    // 测试双阶段多线程 match_basis_file：原位局部修改 + 尾部追加（is_in_place() == true）
    let mut modified = big.clone();
    modified[3 * 1024 * 1024..3 * 1024 * 1024 + 4096].fill(0xA5);
    modified.extend_from_slice(&[0x5A; 65536]);
    let mod_chunks = chunk_slice(&modified, CdcConfig::DEFAULT);
    let mut have_bits = vec![0u8; mod_chunks.len().div_ceil(8)];
    let (mut bc, mut dc) = (0usize, 0u64);
    let mut basis_idx = BasisIndex::new();
    let delta_in_place = match_basis_file(
      &mod_chunks,
      (&f, big.len() as u64),
      CdcConfig::DEFAULT,
      (&mut basis_idx, &mut buf),
      &mut have_bits,
      &mut bc,
      &mut dc,
    )?;
    let _ = remove_file(&p);
    assert!(delta_in_place.is_in_place());
    assert!(delta_in_place.reused_bytes > (big.len() as u64) - 256 * 1024);
    assert_eq!(
      delta_in_place.reused_bytes + delta_in_place.delta_bytes,
      modified.len() as u64
    );

    // 测试长周期重复文本（Gear 滚哈全程触发 max_size 边界且总长非 64 KiB 整数倍）在多线程下与单线程切片 100% 一致
    let pat = b"static inline int sync0_kernel_sched_tick(unsigned long cpu, unsigned long flags, unsigned long jiffies) { return 0; }\n";
    let periodic_len = 9 * 1024 * 1024 + 35_840;
    let mut periodic = Vec::with_capacity(periodic_len);
    while periodic.len() < periodic_len {
      let rem = periodic_len - periodic.len();
      periodic.extend_from_slice(&pat[..rem.min(pat.len())]);
    }
    let p_per = temp_dir().join("s0_cdc_periodic_test.bin");
    write(&p_per, &periodic)?;
    let f_per = File::open(&p_per)?;
    let par_per = chunk_file(&f_per, periodic.len() as u64, CdcConfig::DEFAULT)?;
    let seq_per = chunk_slice(&periodic, CdcConfig::DEFAULT);
    let _ = remove_file(&p_per);
    assert_eq!(par_per, seq_per);
  }
  OK
}
