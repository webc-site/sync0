use std::{
  io::{self, Read, Write},
  net::TcpStream,
};

use s0_cdc::{Rd, WireError, Wr};
use s0_codec::{Codec, SLACK};
use s0_const::{
  CTRL_LEN_HDR, MANIFEST_ZSTD, MAX_CTRL_FRAME_LEN, MIN_ZSTD_BYTES, ZSTD_DEFAULT_LEVEL,
};
use s0_udp::UdpError;
use s0_wire::{P2pReq, P2pResp};

use crate::PeerError;

fn append_framed_payload(out: &mut Vec<u8>, payload: &[u8]) {
  if payload.len() >= MIN_ZSTD_BYTES
    && let Ok(mut codec) = Codec::new(ZSTD_DEFAULT_LEVEL)
  {
    let mut comp = vec![0u8; payload.len() + SLACK];
    if let Ok(Some(clen)) = codec.try_compress_block(payload, &mut comp)
      && clen + 5 < payload.len()
    {
      let hdr_pos = out.len();
      out.extend_from_slice(&[0u8; CTRL_LEN_HDR]);
      let body_pos = out.len();
      Wr::new(out).uvar(payload.len() as u64);
      out.extend_from_slice(&comp[..clen]);
      let hdr = ((out.len() - body_pos) as u32) | MANIFEST_ZSTD;
      out[hdr_pos..body_pos].copy_from_slice(&hdr.to_le_bytes());
      return;
    }
  }
  out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
  out.extend_from_slice(payload);
}

/// 将控制面或 `iroh` 变更推送帧封装为 4B 长度头 + 载荷（当载荷 `>= MIN_ZSTD_BYTES` 时自动经 `mzstd` 压缩并置位 `MANIFEST_ZSTD`）
pub fn encode_zstd_frame(payload: &[u8]) -> Vec<u8> {
  let mut buf = Vec::with_capacity(CTRL_LEN_HDR + payload.len());
  append_framed_payload(&mut buf, payload);
  buf
}

/// 解码带 4B 长度头（含 `MANIFEST_ZSTD` 标志位）的完整帧切片，自动使用 `mzstd` 解压
pub fn decode_zstd_frame(framed: &[u8]) -> Result<Vec<u8>, PeerError> {
  let (&len_buf, body) = framed
    .split_first_chunk::<CTRL_LEN_HDR>()
    .ok_or(WireError::Truncated)?;
  let (hdr, len) = parse_zstd_frame_hdr(len_buf)?;
  let raw = body.get(..len).ok_or(WireError::Truncated)?;
  decode_zstd_frame_body(hdr, raw.to_vec())
}

#[inline]
pub(crate) fn parse_zstd_frame_hdr(len_buf: [u8; CTRL_LEN_HDR]) -> Result<(u32, usize), WireError> {
  let hdr = u32::from_le_bytes(len_buf);
  let len = (hdr & !MANIFEST_ZSTD) as usize;
  if len > MAX_CTRL_FRAME_LEN {
    return Err(WireError::Overflow);
  }
  Ok((hdr, len))
}

pub(crate) fn decode_zstd_frame_body(hdr: u32, raw: Vec<u8>) -> Result<Vec<u8>, PeerError> {
  if (hdr & MANIFEST_ZSTD) == 0 {
    return Ok(raw);
  }
  let mut rd = Rd::new(&raw);
  let dec_len = rd.uvar_usize()?;
  if dec_len > MAX_CTRL_FRAME_LEN {
    return Err(WireError::Overflow.into());
  }
  let mut codec = Codec::new(ZSTD_DEFAULT_LEVEL).map_err(UdpError::Codec)?;
  let mut out = vec![0u8; dec_len];
  let actual = codec
    .decompress_block(rd.rest(), &mut out)
    .map_err(UdpError::Codec)?;
  if actual != dec_len {
    return Err(UdpError::Corrupt.into());
  }
  Ok(out)
}

pub(crate) fn write_frame(stream: &mut TcpStream, payload: &[u8]) -> io::Result<()> {
  let buf = encode_zstd_frame(payload);
  stream.write_all(&buf)?;
  stream.flush()
}

pub(crate) fn write_resp(stream: &mut TcpStream, resp: &P2pResp) -> Result<(), PeerError> {
  write_frame(stream, &resp.encode()?)?;
  Ok(())
}

pub(crate) fn send_req_recv_resp(
  stream: &mut TcpStream,
  req: &P2pReq,
) -> Result<P2pResp, PeerError> {
  write_frame(stream, &req.encode()?)?;
  Ok(P2pResp::decode(&read_frame(stream)?)?)
}

pub(crate) fn write_two_frames(stream: &mut TcpStream, f1: &[u8], f2: &[u8]) -> io::Result<()> {
  let mut buf = Vec::with_capacity(CTRL_LEN_HDR * 2 + f1.len() + f2.len());
  append_framed_payload(&mut buf, f1);
  append_framed_payload(&mut buf, f2);
  stream.write_all(&buf)?;
  stream.flush()
}

pub(crate) fn read_frame(stream: &mut TcpStream) -> Result<Vec<u8>, PeerError> {
  let mut len_buf = [0u8; CTRL_LEN_HDR];
  stream.read_exact(&mut len_buf)?;
  let (hdr, len) = parse_zstd_frame_hdr(len_buf)?;
  let mut raw = vec![0u8; len];
  stream.read_exact(&mut raw)?;
  decode_zstd_frame_body(hdr, raw)
}
