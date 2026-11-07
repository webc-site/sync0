mod cursor;
mod p2p;
mod pkt;
mod tack;

pub use cursor::{Rd, SliceWr, Wr};
pub use p2p::{P2pReq, P2pResp, SubPush, SwarmPeer, decode_socket_addrs, encode_socket_addrs};
pub use pkt::{AckPkt, DataHeader, NackPkt, encode_punch};
pub use s0_const::{
  ACK_BLOCKS, ACK_DELAY_US, DATA_HLEN, MAX_CNT, MAX_RANGES, RETX, SESSION_HLEN, ZSTD,
};
pub use tack::{
  Ack, AckOut, Blocks, Items, Kind, Ranges, SessionHeader, StreamDataHeader, check_ranges,
  encode_ack, expand_pn,
};

/// 紧凑 1 字节线协议错误类型
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WireError {
  #[error("帧数据截断")]
  Truncated = 1,
  #[error("魔数不匹配")]
  BadMagic = 2,
  #[error("协议版本不支持")]
  BadVersion = 3,
  #[error("未知操作码")]
  BadOp = 4,
  #[error("未知状态码")]
  BadStatus = 5,
  #[error("非法 UTF-8 编码")]
  InvalidUtf8 = 6,
  #[error("字段长度超出 u15 上限")]
  Overflow = 7,
  #[error("包头 CRC16 校验失败")]
  CrcMismatch = 8,
}
