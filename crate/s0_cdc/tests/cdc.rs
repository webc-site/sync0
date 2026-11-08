use aok::{OK, Void};
use ctor::ctor;
use log_init::init;
use s0_cdc::{CdcConfig, Manifest, ManifestEntry, chunk_slice, for_each_missing_span, set_bit};

#[ctor(unsafe)]
fn _log_init() {
  init();
}

#[test]
fn test_fastcdc_and_manifest_subpath() -> Void {
  let mut data = vec![0u8; 300_000];
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
        chunks: chunks.clone(),
        deleted: false,
      },
      ManifestEntry {
        path: "a/b/d.txt".into(),
        size: 0,
        chunks: vec![],
        deleted: false,
      },
      ManifestEntry {
        path: "other/x.log".into(),
        size: 0,
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
  let sub = mf.filter_subpath("a/b");
  assert_eq!(sub.entries.len(), 2);
  let single = mf.filter_subpath("a/b/c.bin");
  assert_eq!(single.entries.len(), 1);

  // 测试缺失块区间计算
  let mut have = vec![0u8; 2];
  set_bit(&mut have, 1);
  set_bit(&mut have, 2);
  set_bit(&mut have, 5);
  let mut spans = Vec::new();
  for_each_missing_span(&have, 0, 8, |lo, hi| spans.push((lo, hi)));
  assert_eq!(spans, vec![(0, 1), (3, 5), (6, 8)]);
  OK
}
