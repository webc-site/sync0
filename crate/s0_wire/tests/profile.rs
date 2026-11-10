use std::net::SocketAddr;

use aok::{OK, Void};
use ctor::ctor;
use log_init::init;
use s0_cdc::{SliceWr, WireError};
use s0_const::{ACK_PKT_LEN, DATA_HDR_LEN, FLAG_DELETE, Op, Status, region::Region};
use s0_wire::{
  Ack, AckOut, AckPkt, DataHeader, Kind, NackPkt, P2pReq, P2pResp, SessionHeader, StreamDataHeader,
  SwarmPeer, decode_punch, decode_socket_addrs, encode_ack, encode_punch_padded,
  encode_socket_addrs, expand_pn,
};
#[cfg(not(miri))]
use s0_wire::{ProfileDraft, ProfileError, SecretKey, SignedProfile};

#[ctor(unsafe)]
fn _log_init() {
  init();
}

#[test]
fn test_wire_packets_and_p2p_frames_bijection() -> Void {
  let hdr = DataHeader {
    session_id: 0x0012_3456,
    block_idx: 42,
    seg_idx: 3,
    seg_cnt: 12,
    offset_in_block: 4140,
    payload_len: 1380,
  };
  let mut raw_hdr = [0u8; DATA_HDR_LEN];
  hdr.encode(&mut raw_hdr);
  assert_eq!(DataHeader::decode(&raw_hdr)?, hdr);
  raw_hdr[5] ^= 0x01;
  assert_eq!(DataHeader::decode(&raw_hdr), Err(WireError::CrcMismatch));

  let nack = NackPkt {
    session_id: 0x0012_3456,
    spans: vec![(0, 5), (10, 20)],
    partials: vec![(7, 0b1011_0101), (15, 0xFFFF_0000_FFFF)],
  };
  let mut raw_nack = Vec::new();
  nack.encode(&mut raw_nack)?;
  assert_eq!(NackPkt::decode(&raw_nack)?, nack);

  let ack = AckPkt {
    session_id: 0x0012_3456,
    blk_lo: 0,
    blk_hi: 100,
  };
  let mut raw_ack = [0u8; ACK_PKT_LEN];
  ack.encode(&mut raw_ack);
  assert_eq!(AckPkt::decode(&raw_ack)?, ack);

  let mut padded = encode_punch_padded(0x00AB_CDEF);
  assert_eq!(decode_punch(&padded)?, 0x00AB_CDEF);
  padded[4] = 99;
  assert_eq!(decode_punch(&padded), Err(WireError::BadVersion));

  // RFC 9000 §A.3 64-bit 包号窗口回绕展开检验（32 位截断包号）
  assert_eq!(expand_pn(0xAA82_F30E_0000, 0xF30E_9B32), 0xAA82_F30E_9B32);
  assert_eq!(expand_pn(0xFFFF_FFFE, 0x0000_0002), 0x1_0000_0002);
  assert_eq!(expand_pn(0x1_0000_0002, 0xFFFF_FFFE), 0xFFFF_FFFE);

  // TACK 双层确认帧与 StreamDataHeader 边界校验
  let sh = SessionHeader {
    kind: Kind::Ack,
    flag: 0,
    x: 7,
    sid: 99,
    pn: 123,
  };
  let mut tack_buf = [0u8; 256];
  let mut sw = SliceWr::new(&mut tack_buf);
  let ranges = [(0u16, 4u16), (2, 3)];
  let blocks = [(5u32, 0xDEAD_BEEFu128)];
  encode_ack(
    &mut sw,
    &sh,
    &AckOut {
      largest: 50,
      delay: 12,
      ts_echo: 999,
      rx_pkts: 48,
      credit: 10,
      done_lo: 4,
      dec: 1,
      flag: Ack::DUP,
      ranges: &ranges,
      blocks: &blocks,
    },
  )?;
  let pos = sw.pos();
  let (dec_sh, body) = SessionHeader::parse(&tack_buf[..pos])?;
  assert_eq!(dec_sh, sh);
  let dec_ack = Ack::parse(&dec_sh, body)?;
  assert_eq!(dec_ack.reach, 9);
  assert_eq!(dec_ack.ranges().collect::<Vec<_>>(), ranges);
  assert_eq!(dec_ack.blocks().collect::<Vec<_>>(), blocks);

  let mut d_buf = [0u8; 32];
  let mut d_sw = SliceWr::new(&mut d_buf);
  let sdh = StreamDataHeader {
    flag: 0,
    idx: 2,
    cnt: 2,
    sid: 1,
    pn: 1,
    blk: 0,
    ts: 0,
  };
  sdh.write(&mut d_sw, b"x")?;
  let d_pos = d_sw.pos();
  assert_eq!(
    StreamDataHeader::parse(&d_buf[..d_pos]),
    Err(WireError::Overflow)
  );

  let addrs: Vec<SocketAddr> = vec!["127.0.0.1:9001".parse()?, "[::1]:9002".parse()?];
  let enc_addrs = encode_socket_addrs(&addrs);
  assert_eq!(decode_socket_addrs(&enc_addrs)?, addrs);

  let mut req = P2pReq::new(
    Op::Pull,
    FLAG_DELETE,
    [7u8; 16],
    enc_addrs.clone(),
    vec![1, 2, 3],
  );
  req.req_rate = 100_000_000;
  req.paths = vec!["docs/arch".into(), "docs/api".into()];
  req.range = Some(([9u8; 16], 10, 50));
  req.token = Some([0xAA; 16]);
  req.byte_range = Some((131_072, 524_288));
  let mut raw_req = req.encode()?;
  assert_eq!(P2pReq::decode(&raw_req)?, req);
  raw_req[4] = 99;
  assert_eq!(P2pReq::decode(&raw_req), Err(WireError::BadVersion));

  let resp = P2pResp {
    status: Status::Ok,
    flags: 0,
    eff_rate: 50_000_000,
    root_hash: [9u8; 16],
    peer_addrs: enc_addrs.clone(),
    profile_card: vec![4, 5],
    peers: vec![SwarmPeer {
      endpoint_id: [3u8; 32],
      region: Region::Shanghai,
      weight_rate: 80_000_000,
      blk_lo: 0,
      blk_hi: 100,
      peer_addrs: enc_addrs,
      profile_card: vec![],
    }],
    manifest: vec![10, 20, 30],
  };
  assert_eq!(P2pResp::decode(&resp.encode()?)?, resp);
  OK
}

#[test]
#[cfg(not(miri))]
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
  assert_eq!(&verified.pubkey, sk.public().as_bytes());
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
    .with_bio(long_bio)
    .sign(&sk);
  assert_eq!(err_bio, Err(ProfileError::BioTooLong(257)));

  // 恰好 256 个字符的个性签名允许签名
  let exact_256 = "签".repeat(256);
  let ok_256 = ProfileDraft::new("bob", Region::Tokyo)
    .with_bio(&exact_256)
    .sign(&sk)?;
  let dec_256 = SignedProfile::decode_and_verify(&ok_256.encode()?)?;
  assert_eq!(dec_256.bio.chars().count(), 256);
  assert_eq!(dec_256.bio, exact_256);

  OK
}
