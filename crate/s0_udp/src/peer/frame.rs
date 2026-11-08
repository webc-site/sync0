use std::{
  io::{self, Read, Write},
  net::TcpStream,
};

use s0_cdc::{Rd, Wr};

use super::PeerError;
use crate::{
  codec::{Codec, SLACK},
  consts::{
    CTRL_LEN_HDR, HAVE_ALL_ONES, MANIFEST_ZSTD, MAX_CTRL_FRAME_LEN, MIN_ZSTD_BYTES,
    ZSTD_DEFAULT_LEVEL,
  },
  wire::{P2pReq, P2pResp},
};

#[inline]
pub(crate) fn is_have_all_ones_frame(buf: &[u8]) -> bool {
  buf.first_chunk::<CTRL_LEN_HDR>().is_some_and(|&b| {
    buf.len() == CTRL_LEN_HDR && u32::from_le_bytes(b) == HAVE_ALL_ONES
  })
}

fn append_framed_payload(out: &mut Vec<u8>, payload: &[u8]) {
  if payload.len() >= MIN_ZSTD_BYTES
    && let Ok(mut codec) = Codec::new(ZSTD_DEFAULT_LEVEL)
  {
    let mut comp = vec![0u8; payload.len() + SLACK];
    if let Ok(Some(clen)) = codec.try_compress_block(payload, &mut comp)
      && clen + 5 < payload.len()
    {
      let mut body = Vec::with_capacity(5 + clen);
      Wr::new(&mut body).uvar(payload.len() as u64);
      body.extend_from_slice(&comp[..clen]);
      let hdr = (body.len() as u32) | MANIFEST_ZSTD;
      out.extend_from_slice(&hdr.to_le_bytes());
      out.extend_from_slice(&body);
      return;
    }
  }
  out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
  out.extend_from_slice(payload);
}

pub(crate) fn write_frame(stream: &mut TcpStream, payload: &[u8]) -> io::Result<()> {
  let mut buf = Vec::with_capacity(CTRL_LEN_HDR + payload.len());
  append_framed_payload(&mut buf, payload);
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

pub(crate) fn read_frame(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
  let mut len_buf = [0u8; CTRL_LEN_HDR];
  stream.read_exact(&mut len_buf)?;
  let hdr = u32::from_le_bytes(len_buf);
  let zstd = (hdr & MANIFEST_ZSTD) != 0;
  let len = (hdr & !MANIFEST_ZSTD) as usize;
  if len > MAX_CTRL_FRAME_LEN {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "control frame exceeds maximum length",
    ));
  }
  let mut raw = vec![0u8; len];
  stream.read_exact(&mut raw)?;
  if !zstd {
    return Ok(raw);
  }
  let mut rd = Rd::new(&raw);
  let dec_len = rd
    .uvar_usize()
    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "corrupt zstd frame len"))?;
  if dec_len > MAX_CTRL_FRAME_LEN {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "decompressed control frame exceeds maximum length",
    ));
  }
  let mut codec = Codec::new(ZSTD_DEFAULT_LEVEL)
    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "zstd codec init failed"))?;
  let mut out = vec![0u8; dec_len];
  let actual = codec
    .decompress_block(rd.rest(), &mut out)
    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "zstd frame decompress failed"))?;
  if actual != dec_len {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "zstd frame length mismatch",
    ));
  }
  Ok(out)
}
