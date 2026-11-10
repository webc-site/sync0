use std::{
  env::temp_dir,
  fs::{File, OpenOptions, read, remove_file, write},
  process,
  sync::atomic::{AtomicU64, Ordering},
};

use fastrand::Rng;
use s0_cdc::{
  BasisIndex, CdcConfig, FileAt, chunk_file, copy_file_spans, for_each_missing_data_span,
  match_basis_file,
};

use crate::{SimLink, SimReport, run_wan_sim};

static SIM_SEQ: AtomicU64 = AtomicU64::new(1);

/// 大文件部分内容改动（CDC 增量同步）离散事件仿真规格
#[derive(Clone, Copy, Debug)]
pub struct DeltaSimSpec {
  /// 广域网物理链路参数
  pub link: SimLink,
  /// 接收端已有基准文件字节大小（`>= 8 MiB` 时触发双阶段多线程原位校验与并行收敛 FastCDC）
  pub basis_size: usize,
  /// 可选：在指定偏移 `(offset, len)` 插入字节，引发后续全量字节偏移（Byte-Shift）
  pub insert_at: Option<(usize, usize)>,
  /// 可选：在指定偏移 `(offset, len)` 原位覆写字节（不改变后续块偏移）
  pub patch_at: Option<(usize, usize)>,
  /// 尾部追加字节数（模拟 `tar -rf` 归档追加或日志追加）
  pub append_len: usize,
}

/// 大文件部分内容改动（CDC 增量同步）离散事件仿真报告
#[derive(Clone, Copy, Debug)]
pub struct DeltaSimReport {
  /// 目标文件完整逻辑字节数
  pub logical_bytes: u64,
  /// 从接收端本地基准文件复用的字节数
  pub reused_bytes: u64,
  /// 实际经由广域网传输的增量字节数
  pub delta_bytes: u64,
  /// 增量重删率百分比（`reused_bytes / logical_bytes * 100.0`）
  pub dedup_ratio_pct: f64,
  /// 是否触发零拷贝原位补丁（`FileDelta::is_in_place`）
  pub in_place: bool,
  /// 计入 1-RTT 清单握手与增量流传输总耗时后的等效逻辑吞吐率（bit/s）
  pub effective_goodput_bps: u64,
  /// 底层增量流广域网传输仿真报告
  pub wire_report: SimReport,
}

fn fill_noise(buf: &mut [u8], rng: &mut Rng) {
  let (chunks, tail) = buf.as_chunks_mut::<8>();
  for c in chunks {
    c.copy_from_slice(&rng.u64(..).to_le_bytes());
  }
  for b in tail {
    *b = rng.u8(..);
  }
}

/// 运行大文件局部改动（原位覆写、`tar -rf` 追加、中间字节插入偏移）端到端 CDC + WAN 离散事件仿真
pub fn run_delta_wan_sim(spec: DeltaSimSpec) -> DeltaSimReport {
  let mut rng = Rng::with_seed(spec.link.seed ^ 0x9E37_79B9_7F4A_7C15);
  let mut basis_bytes = vec![0u8; spec.basis_size];
  fill_noise(&mut basis_bytes, &mut rng);

  let mut target_bytes = basis_bytes.clone();
  if let Some((off, len)) = spec.patch_at
    && off < target_bytes.len()
  {
    let end = (off + len).min(target_bytes.len());
    fill_noise(&mut target_bytes[off..end], &mut rng);
  }
  if let Some((off, len)) = spec.insert_at
    && len > 0
  {
    let pos = off.min(target_bytes.len());
    let mut ins = vec![0u8; len];
    fill_noise(&mut ins, &mut rng);
    let mut shifted = Vec::with_capacity(target_bytes.len() + len);
    shifted.extend_from_slice(&target_bytes[..pos]);
    shifted.extend_from_slice(&ins);
    shifted.extend_from_slice(&target_bytes[pos..]);
    target_bytes = shifted;
  }
  if spec.append_len > 0 {
    let cur = target_bytes.len();
    target_bytes.resize(cur + spec.append_len, 0);
    fill_noise(&mut target_bytes[cur..], &mut rng);
  }

  let seq = SIM_SEQ.fetch_add(1, Ordering::Relaxed);
  let pid = process::id();
  let basis_path = temp_dir().join(format!("s0_delta_sim_basis_{pid}_{seq}.bin"));
  let target_path = temp_dir().join(format!("s0_delta_sim_target_{pid}_{seq}.bin"));
  let out_path = temp_dir().join(format!("s0_delta_sim_out_{pid}_{seq}.bin"));

  write(&basis_path, &basis_bytes).expect("write basis");
  write(&target_path, &target_bytes).expect("write target");

  let logical_bytes = target_bytes.len() as u64;
  let target_file = File::open(&target_path).expect("open target");
  let chunks =
    chunk_file(&target_file, logical_bytes, CdcConfig::DEFAULT).expect("chunk target file");

  let basis_file = File::open(&basis_path).expect("open basis");
  let mut have_bits = vec![0u8; chunks.len().div_ceil(8)];
  let (mut bit_cursor, mut delta_cursor) = (0usize, 0u64);
  let (mut basis_idx, mut io_buf) = (BasisIndex::new(), Vec::new());
  let plan = match_basis_file(
    &chunks,
    (&basis_file, basis_bytes.len() as u64),
    CdcConfig::DEFAULT,
    (&mut basis_idx, &mut io_buf),
    &mut have_bits,
    &mut bit_cursor,
    &mut delta_cursor,
  )
  .expect("match basis file");
  drop(basis_file);

  // 发送端根据 have_bits 提取缺失块增量流，验证与接收端 data_spans 完全一致
  let mut tx_spans = Vec::new();
  let (mut tx_bc, mut tx_dc) = (0usize, 0u64);
  let mut delta_stream = Vec::with_capacity(plan.delta_bytes as usize);
  let mut read_buf = Vec::new();
  for_each_missing_data_span(&chunks, &have_bits, &mut tx_bc, &mut tx_dc, |sp| {
    tx_spans.push(sp);
    let n = sp.len as usize;
    read_buf.resize(n, 0);
    target_file
      .read_exact_at_pos(&mut read_buf, sp.file_off)
      .expect("read missing span");
    delta_stream.extend_from_slice(&read_buf);
  });
  assert_eq!(tx_spans, plan.data_spans);
  assert_eq!(delta_stream.len() as u64, plan.delta_bytes);

  // 接收端执行增量还原：in_place 时直接在 basis_path 原位补丁（0 字节拷贝），否则走暂存拷贝
  let in_place = plan.is_in_place();
  let verify_path = if in_place {
    let rw = OpenOptions::new()
      .read(true)
      .write(true)
      .open(&basis_path)
      .expect("open basis rw");
    rw.set_len(logical_bytes).expect("set_len in_place");
    for sp in &plan.data_spans {
      let s = sp.delta_off as usize;
      let e = s + (sp.len as usize);
      rw.write_all_at_pos(&delta_stream[s..e], sp.file_off)
        .expect("write in_place span");
    }
    &basis_path
  } else {
    let out_f = OpenOptions::new()
      .create(true)
      .truncate(true)
      .read(true)
      .write(true)
      .open(&out_path)
      .expect("create out");
    out_f.set_len(logical_bytes).expect("set_len out");
    let bf = File::open(&basis_path).expect("reopen basis");
    let mut copy_buf = vec![0u8; 256 * 1024];
    copy_file_spans(&bf, &out_f, &plan.copy_spans, &mut copy_buf).expect("copy spans");
    for sp in &plan.data_spans {
      let s = sp.delta_off as usize;
      let e = s + (sp.len as usize);
      out_f
        .write_all_at_pos(&delta_stream[s..e], sp.file_off)
        .expect("write data span");
    }
    &out_path
  };

  let reconstructed = read(verify_path).expect("read reconstructed");
  let _ = remove_file(&basis_path);
  let _ = remove_file(&target_path);
  let _ = remove_file(&out_path);
  assert_eq!(reconstructed, target_bytes);

  let wire_report = if plan.delta_bytes > 0 {
    run_wan_sim(spec.link, plan.delta_bytes)
  } else {
    SimReport {
      goodput_bps: 0,
      theoretical_max_bps: spec.link.bw_bps,
      efficiency_pct: 100.0,
      sim_elapsed_us: 1,
      total_packets: 0,
      retx_packets: 0,
      srtt_us: spec.link.rtt_us,
      rtt_min_us: spec.link.rtt_us,
    }
  };
  let total_us = spec.link.rtt_us.max(1)
    + if plan.delta_bytes > 0 {
      wire_report.sim_elapsed_us
    } else {
      0
    };
  let effective_goodput_bps =
    ((logical_bytes as u128) * 8 * 1_000_000 / u128::from(total_us.max(1))) as u64;
  let dedup_ratio_pct = if logical_bytes > 0 {
    (plan.reused_bytes as f64) * 100.0 / (logical_bytes as f64)
  } else {
    100.0
  };

  DeltaSimReport {
    logical_bytes,
    reused_bytes: plan.reused_bytes,
    delta_bytes: plan.delta_bytes,
    dedup_ratio_pct,
    in_place,
    effective_goodput_bps,
    wire_report,
  }
}
