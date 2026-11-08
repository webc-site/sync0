use std::{
  cell::OnceCell,
  future::Future,
  io,
  net::SocketAddr,
  path::{Path, PathBuf},
};

use compio::{
  buf::{IntoInner, IoBuf},
  fs::{File, OpenOptions},
  io::{AsyncReadAtExt, AsyncWriteAtExt},
  net::UdpSocket,
  runtime::Runtime,
};
use s0_wire::{
  consts::{DATA_HDR_LEN, PKT_ACK, PKT_PUNCH},
  wire::{AckPkt, DataHeader, decode_punch, encode_punch},
};

use crate::FsError;

/// 单线程 `compio` 异步完成流并发度上限（控制同时在飞的 `io_uring`/`kqueue`/`IOCP` 文件句柄数，防 FD 耗尽与内存爆炸）
const COMPIO_BATCH_CONCURRENCY: usize = 32;

thread_local! {
  static LOCAL_RT: OnceCell<Runtime> = const { OnceCell::new() };
}

/// 在线程局部 `compio` Runtime 上执行异步 I/O 任务（每线程仅创建一次 `io_uring` / `kqueue` / `IOCP` 实例）
pub fn run_compio<T, F: Future<Output = io::Result<T>>>(fut: F) -> io::Result<T> {
  LOCAL_RT.with(|cell| {
    if let Some(rt) = cell.get() {
      return rt.block_on(fut);
    }
    let rt = Runtime::new()?;
    cell.get_or_init(|| rt).block_on(fut)
  })
}

/// 利用 `compio` 异步 `ReadAt` (`pread` / `io_uring`) 在指定文件偏移处精确读取 `len` 字节
pub fn read_exact_at_compio(path: &Path, offset: u64, len: usize) -> io::Result<Vec<u8>> {
  if len == 0 {
    return Ok(Vec::new());
  }
  run_compio(async move {
    let file = OpenOptions::new().read(true).open(path).await?;
    let buf = Vec::with_capacity(len);
    let compio::BufResult(res, slice_buf) = file.read_exact_at(buf.slice(..len), offset).await;
    res?;
    Ok(slice_buf.into_inner())
  })
}

/// 利用 `compio` 异步 `WriteAt` (`pwrite` / `io_uring`) 在指定文件偏移处写入完整切片
pub fn write_all_at_compio(
  path: &Path,
  offset: u64,
  data: Vec<u8>,
  truncate: bool,
) -> io::Result<()> {
  run_compio(async move {
    let mut file = OpenOptions::new()
      .create(true)
      .truncate(truncate)
      .write(true)
      .open(path)
      .await?;
    let compio::BufResult(res, _) = file.write_all_at(data, offset).await;
    res
  })
}

/// 利用 `compio` 跨平台完成式异步 I/O (`io_uring` / `kqueue` / `IOCP`) 批量并发创建并定位写入 (`WriteAt`) 小文件切片，全程复用单块缓冲区
pub fn flush_memory_slices_compio<'a, F>(count: usize, get_item: F) -> io::Result<()>
where
  F: Fn(usize) -> (&'a Path, &'a [u8]),
{
  if count == 0 {
    return Ok(());
  }
  run_compio(async move {
    let mut reusable_buf = Vec::with_capacity(64 * 1024);
    let mut idx = 0usize;
    while idx < count {
      let end = (idx + COMPIO_BATCH_CONCURRENCY).min(count);
      for i in idx..end {
        let (path, data) = get_item(i);
        reusable_buf.clear();
        reusable_buf.extend_from_slice(data);
        let mut file = OpenOptions::new()
          .create(true)
          .truncate(true)
          .write(true)
          .open(path)
          .await?;
        let compio::BufResult(res, returned_buf) = file.write_all_at(reusable_buf, 0).await;
        res?;
        reusable_buf = returned_buf;
      }
      idx = end;
    }
    Ok(())
  })
}

/// 利用 `compio` 异步 `ReadAt` + `WriteAt` 从聚合流文件 `.s0_pack.tmp` 按偏移读取并批量解包落盘，全程复用定长缓冲，杜绝内存爆炸
pub fn flush_pack_stream_compio(pack_path: &Path, jobs: &[(PathBuf, u64, u32)]) -> io::Result<()> {
  if jobs.is_empty() {
    return Ok(());
  }
  run_compio(async move {
    let pack_file = OpenOptions::new().read(true).open(pack_path).await?;
    let mut reusable_buf = Vec::with_capacity(64 * 1024);
    for &(ref dst_path, off, len) in jobs {
      let n = len as usize;
      reusable_buf.clear();
      if reusable_buf.capacity() < n {
        reusable_buf.reserve(n - reusable_buf.capacity());
      }
      let compio::BufResult(read_res, slice_buf) =
        pack_file.read_exact_at(reusable_buf.slice(..n), off).await;
      read_res?;
      let buf = slice_buf.into_inner();
      let mut dst_file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(dst_path)
        .await?;
      let compio::BufResult(write_res, returned_buf) = dst_file.write_all_at(buf, 0).await;
      write_res?;
      reusable_buf = returned_buf;
    }
    Ok(())
  })
}

/// `compio` 异步单块流发送参数
#[derive(Debug, Clone, Copy)]
pub struct CompioBlockSpec {
  pub session_id: u32,
  pub block_idx: u32,
  pub file_offset: u64,
  pub block_len: usize,
  pub seg_payload: usize,
}

/// 基于 `compio` 异步 `File` (`ReadAt`) 与 `UdpSocket` 的零拷贝流式单文件分块发送器（直接从磁盘完成式读取分片并发送 UDP 数据报，内存占用恒定 $O(1)$）
pub async fn stream_file_block_compio(
  sock: &UdpSocket,
  peer: SocketAddr,
  file: &File,
  spec: CompioBlockSpec,
) -> Result<usize, FsError> {
  if spec.block_len == 0 || spec.seg_payload == 0 {
    return Ok(0);
  }
  let seg_cnt = spec.block_len.div_ceil(spec.seg_payload) as u16;
  let sid24 = spec.session_id & 0x00FF_FFFF;
  let mut pkt_buf = vec![0u8; DATA_HDR_LEN + spec.seg_payload];
  let mut sent_total = 0usize;

  for seg_idx in 0..seg_cnt {
    let off_in_blk = (seg_idx as usize) * spec.seg_payload;
    let payload_len = (spec.block_len - off_in_blk).min(spec.seg_payload);
    let hdr = DataHeader {
      session_id: sid24,
      block_idx: spec.block_idx,
      seg_idx,
      seg_cnt,
      offset_in_block: off_in_blk as u32,
      payload_len: payload_len as u16,
    };
    let mut hdr_bytes = [0u8; DATA_HDR_LEN];
    hdr.encode(&mut hdr_bytes);

    let mut payload_vec = pkt_buf.split_off(DATA_HDR_LEN);
    payload_vec.clear();
    if payload_vec.capacity() < payload_len {
      payload_vec.reserve(payload_len - payload_vec.capacity());
    }
    let compio::BufResult(read_res, filled_slice) = file
      .read_exact_at(
        payload_vec.slice(..payload_len),
        spec.file_offset + off_in_blk as u64,
      )
      .await;
    read_res?;
    let mut filled_payload = filled_slice.into_inner();

    pkt_buf.clear();
    pkt_buf.extend_from_slice(&hdr_bytes);
    pkt_buf.append(&mut filled_payload);

    let compio::BufResult(send_res, returned_pkt) = sock.send_to(pkt_buf, peer).await;
    send_res?;
    pkt_buf = returned_pkt;
    sent_total += payload_len;
  }
  Ok(sent_total)
}

/// 基于 `compio` 异步 `UdpSocket` 的轻量级打洞与 ACK 探测握手
pub async fn punch_and_verify_compio(
  sock: &UdpSocket,
  peer: SocketAddr,
  session_id: u32,
) -> Result<SocketAddr, FsError> {
  let sid24 = session_id & 0x00FF_FFFF;
  let pkt = encode_punch(sid24).to_vec();
  let compio::BufResult(res, _) = sock.send_to(pkt, peer).await;
  res?;

  let rx_buf = vec![0u8; 64];
  let compio::BufResult(recv_res, rx_buf) = sock.recv_from(rx_buf).await;
  let (n, from) = recv_res?;
  if n > 0 {
    let valid = match rx_buf[0] {
      PKT_PUNCH => decode_punch(&rx_buf[..n]) == Ok(sid24),
      PKT_ACK => AckPkt::decode(&rx_buf[..n]).is_ok_and(|a| a.session_id == sid24),
      _ => false,
    };
    if valid {
      return Ok(from);
    }
  }
  Err(FsError::Io(io::Error::new(
    io::ErrorKind::InvalidData,
    "invalid compio punch/ack packet",
  )))
}
