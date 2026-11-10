use aok::{OK, Void};
use ctor::ctor;
use fastrand::Rng;
use log_init::init;
use s0_codec::{Codec, Detector, Error, HEADER, LevelChoice, Policy, SLACK, skip_magic, skip_name};
use s0_const::{MAX_BLOCK, ZSTD_DEFAULT_LEVEL as DEFAULT_LEVEL};

#[ctor(unsafe)]
fn log_init() {
  init();
}

const LEVEL: i32 = 3;

fn buf() -> Vec<u8> {
  vec![0u8; MAX_BLOCK + SLACK]
}

fn text(len: usize) -> Vec<u8> {
  let line = b"the quick brown fox jumps over the lazy dog 0123456789\n";
  line.iter().copied().cycle().take(len).collect()
}

fn noise(len: usize) -> Vec<u8> {
  let mut rng = Rng::with_seed(0x1234_5678_9abc_def0);
  (0..len).map(|_| rng.u8(..)).collect()
}

#[test]
fn text_roundtrip_and_ratio() -> Void {
  let raw = text(64 * 1024);
  let mut c = Codec::new(LEVEL)?;
  let mut out = buf();
  let len = c.try_compress_block(&raw, &mut out)?.unwrap();
  assert!(
    len * 100 / raw.len() < 20,
    "ratio={}",
    len * 100 / raw.len()
  );
  assert_eq!(&out[..HEADER], &((len - HEADER) as u32).to_le_bytes());

  let mut back = buf();
  let n = c.decompress_block(&out[..len], &mut back)?;
  assert_eq!(n, raw.len());
  assert_eq!(&back[..n], &raw[..]);
  OK
}

#[test]
fn incompressible_falls_back() -> Void {
  let raw = noise(64 * 1024);
  let mut c = Codec::new(LEVEL)?;
  let mut out = buf();
  assert!(c.try_compress_block(&raw, &mut out)?.is_none());
  OK
}

#[test]
fn bad_frame_rejected() -> Void {
  let mut c = Codec::new(LEVEL)?;
  let mut out = buf();
  let raw = text(1024);
  let len = c.try_compress_block(&raw, &mut out)?.unwrap();

  assert!(matches!(
    c.decompress_block(&[1, 2], &mut buf()),
    Err(Error::Frame(_))
  ));
  let mut bad = out[..len].to_vec();
  bad[..4].copy_from_slice(&u32::MAX.to_le_bytes());
  assert!(matches!(
    c.decompress_block(&bad, &mut buf()),
    Err(Error::Frame(_))
  ));
  OK
}

#[test]
fn level_reconfigure() -> Void {
  let mut c = Codec::new(-7)?;
  let raw = text(4096);
  let mut out = buf();
  let a = c.try_compress_block(&raw, &mut out)?.unwrap();
  c.set_level(9)?;
  let b = c.try_compress_block(&raw, &mut out)?.unwrap();
  assert!(b <= a);
  OK
}

#[test]
fn name_and_magic_detection() {
  assert!(skip_name("a/b/photo.JPG"));
  assert!(skip_name("x.zip"));
  assert!(!skip_name("src/main.rs"));
  assert!(!skip_name("noext"));
  assert!(skip_magic(b"\x89PNG\r\n\x1a\n123"));
  assert!(skip_magic(b"PK\x03\x04rest"));
  assert!(!skip_magic(b"hello world"));
}

#[test]
fn detector_samples_and_reprobes() -> Void {
  let mut c = Codec::new(LEVEL).unwrap();

  let mut d = Detector::new("a.zip");
  assert!(d.compress(&text(4096), &mut c).unwrap().is_none());

  let mut d = Detector::new("a.txt");
  assert!(d.compress(&text(MAX_BLOCK), &mut c).unwrap().is_some());
  assert!(!d.skip());

  let mut d = Detector::new("");
  let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
  assert!(d.compress(png, &mut c).unwrap().is_none());
  assert!(d.skip());
  assert!(d.compress(&text(MAX_BLOCK), &mut c).unwrap().is_none());

  let mut d = Detector::new("blob");
  let raw = noise(MAX_BLOCK);
  assert!(d.compress(&raw, &mut c).unwrap().is_none());
  assert!(!d.skip());
  assert!(d.compress(&raw, &mut c).unwrap().is_none());
  assert!(d.skip());
  for _ in 0..61 {
    assert!(d.compress(&raw, &mut c).unwrap().is_none());
  }
  assert!(d.compress(&raw, &mut c).unwrap().is_none());

  // 4 KiB 周期可压缩数据与首块局部高熵头 + 4 KiB 周期数据：首块完整试压不误跳过
  let period_4k: Vec<u8> = noise(4096).into_iter().cycle().take(MAX_BLOCK).collect();
  let mut head_plus_4k = noise(16 * 1024);
  head_plus_4k.extend_from_slice(&period_4k[16 * 1024..]);
  let mut d_4k = Detector::new("periodic.bin");
  assert!(d_4k.compress(&head_plus_4k, &mut c)?.is_some());
  assert!(!d_4k.skip());
  assert!(d_4k.compress(&period_4k, &mut c)?.is_some());
  assert!(d_4k.is_compressible());

  let mut d = Detector::new("git-cargo.tar");
  let txt = text(MAX_BLOCK);
  assert!(d.compress(&txt, &mut c)?.is_some());
  assert!(d.compress(&txt, &mut c)?.is_some());
  assert!(d.compress(&raw, &mut c)?.is_none());
  assert!(!d.skip());
  assert!(d.compress(&txt, &mut c)?.is_some());

  assert!(d.compress(&raw, &mut c)?.is_none());
  assert!(!d.skip());
  assert!(d.compress(&raw, &mut c)?.is_none());
  assert!(d.skip());
  for _ in 7..64 {
    assert!(d.compress(&txt, &mut c)?.is_none());
  }
  assert!(d.compress(&txt, &mut c)?.is_some());
  assert!(!d.skip());
  OK
}

#[test]
fn policy_choices() {
  let p = Policy::new(1, 0);
  assert_eq!(p.choose(0), LevelChoice::Level(DEFAULT_LEVEL));
  assert_eq!(p.choose(12_500_000), LevelChoice::Level(6));
  assert_eq!(p.choose(1_250_000_000), LevelChoice::Off);
  assert_eq!(p.choose(125_000_000), LevelChoice::Level(-3));

  let p4 = Policy::new(4, 0);
  assert_ne!(p4.choose(1_250_000_000), LevelChoice::Off);

  let slow = Policy::new(1, 5_000_000);
  assert_eq!(slow.choose(12_500_000), LevelChoice::Off);

  let mut p = Policy::new(1, 0);
  p.observe(6, 1.0e6, 0.42);
  assert_ne!(p.choose(12_500_000), LevelChoice::Level(6));
  assert!(p.stat(6).unwrap().0 < 3.0e7);

  let mut p_text = Policy::new(1, 0);
  for _ in 0..8 {
    p_text.observe(3, 7.0e7, 0.15);
  }
  assert!(p_text.stat(6).unwrap().1 <= p_text.stat(3).unwrap().1);
}
