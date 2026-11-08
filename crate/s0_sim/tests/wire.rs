use std::net::SocketAddr;

use aok::{OK, Void};
use s0_cdc::FLAG_DELETE;
use s0_udp::{
  consts::{DATA_HDR_LEN, Op, Status},
  region::Region,
  wire::{
    AckPkt, DataHeader, NackPkt, P2pReq, P2pResp, SwarmPeer, decode_socket_addrs,
    encode_socket_addrs,
  },
};

#[ctor::ctor(unsafe)]
fn _log_init() {
  log_init::init();
}

#[test]
fn test_data_hdr_and_p2p_frames_roundtrip() -> Void {
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
  let mut raw_ack = [0u8; 13];
  ack.encode(&mut raw_ack);
  assert_eq!(AckPkt::decode(&raw_ack)?, ack);

  let addrs: Vec<SocketAddr> = vec!["127.0.0.1:9001".parse()?, "[::1]:9002".parse()?];
  let enc_addrs = encode_socket_addrs(&addrs);
  assert_eq!(decode_socket_addrs(&enc_addrs)?, addrs);

  let req = P2pReq {
    op: Op::Pull,
    flags: FLAG_DELETE,
    share_id: [7u8; 16],
    req_rate: 100_000_000,
    paths: vec!["docs/arch".into()],
    peer_addrs: enc_addrs.clone(),
    profile_card: vec![1, 2, 3],
    range: Some(([9u8; 16], 10, 50)),
    token: Some([0xAA; 16]),
  };
  let req_bytes = req.encode()?;
  let dec_req = P2pReq::decode(&req_bytes)?;
  assert_eq!(dec_req.op, Op::Pull);
  assert_eq!(dec_req.paths, vec!["docs/arch".to_owned()]);
  assert_eq!(dec_req.range, Some(([9u8; 16], 10, 50)));
  assert_eq!(dec_req.token, Some([0xAA; 16]));

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
  let resp_bytes = resp.encode()?;
  assert_eq!(P2pResp::decode(&resp_bytes)?, resp);
  OK
}
