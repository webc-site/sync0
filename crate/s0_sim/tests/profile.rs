use aok::{OK, Void};
use s0_udp::{
  profile::{ProfileDraft, ProfileError, SecretKey, SignedProfile},
  region::Region,
};

#[ctor::ctor(unsafe)]
fn _log_init() {
  log_init::init();
}

#[test]
fn test_signed_profile_roundtrip_and_tamper_reject() -> Void {
  let sk = SecretKey::from_bytes(&[0x42; 32]);
  let card = ProfileDraft::new("alice_节点_01", Region::Shanghai)
    .with_email("alice@sync0.dev")
    .with_url("https://sync0.dev/alice")
    .with_bio("East Asia Seed Peer")
    .sign(&sk)?;

  let raw = card.encode()?;
  let verified = SignedProfile::decode_and_verify(&raw)?;
  assert_eq!(verified, card);
  assert!(verified.matches_endpoint(sk.public().as_bytes()));
  assert!(verified.to_string().contains("alice_节点_01 [上海/SHA]"));

  // 篡改任意字节（如将地域或用户名篡改）必须触发验签失败
  let mut tampered = raw.clone();
  tampered[50] ^= 0x01;
  assert!(SignedProfile::decode_and_verify(&tampered).is_err());

  // 超过 64 个字符的用户名必须拒绝签名
  let long_name = "中".repeat(65);
  let err = ProfileDraft::new(long_name, Region::Shenzhen).sign(&sk);
  assert_eq!(err, Err(ProfileError::NameTooLong(65)));

  // 恰好 64 个字符的用户名允许签名
  let exact_64 = "中".repeat(64);
  let ok_64 = ProfileDraft::new(exact_64, Region::HongKong).sign(&sk)?;
  let dec_64 = SignedProfile::decode_and_verify(&ok_64.encode()?)?;
  assert_eq!(dec_64.name.chars().count(), 64);
  assert_eq!(dec_64.region, Region::HongKong);

  // 超过 256 个字符的个性签名必须拒绝签名
  let long_bio = "签".repeat(257);
  let err_bio = ProfileDraft::new("bob", Region::Tokyo)
    .with_motto(long_bio)
    .sign(&sk);
  assert_eq!(err_bio, Err(ProfileError::BioTooLong(257)));

  // 恰好 256 个字符的个性签名允许签名
  let exact_256 = "签".repeat(256);
  let ok_256 = ProfileDraft::new("bob", Region::Tokyo)
    .with_motto(&exact_256)
    .sign(&sk)?;
  let dec_256 = SignedProfile::decode_and_verify(&ok_256.encode()?)?;
  assert_eq!(dec_256.bio.chars().count(), 256);
  assert_eq!(dec_256.bio, exact_256);

  OK
}
