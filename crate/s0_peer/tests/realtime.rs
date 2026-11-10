use std::{
  fs::{self, OpenOptions},
  io::Write,
  path::Path,
  sync::Arc,
  thread,
  time::{Duration, Instant},
};

use aok::{OK, Void};
use ctor::ctor;
use log_init::init;
use s0_cdc::{ChunkSig, Manifest, PathChange};
use s0_const::{FsOp, MANIFEST_ZSTD, Perm, region::Region};
use s0_fs::{
  SUBS_FILE, SharedSink, build_manifest_from_paths, meta_mode, meta_mtime_ms, set_file_meta,
};
use s0_peer::{
  KeyMode, NodeConfig, PeerError, PeerNode, ShareConfig, SubscriberRecord, TransferOpts,
  append_subs_aof_delta, decode_zstd_frame, encode_zstd_frame, load_subs_file, save_subs_file,
};
use s0_wire::{ProfileDraft, SubPush};
use tempfile::tempdir;

#[ctor(unsafe)]
fn _log_init() {
  init();
}

fn start_node(name: &str, region: Region) -> Result<Arc<PeerNode>, PeerError> {
  PeerNode::start(NodeConfig {
    profile: ProfileDraft::new(name, region),
    ..Default::default()
  })
}

fn wait_until(timeout: Duration, mut cond: impl FnMut() -> bool) -> bool {
  let deadline = Instant::now() + timeout;
  while Instant::now() < deadline {
    if cond() {
      return true;
    }
    thread::sleep(Duration::from_millis(20));
  }
  cond()
}

fn file_has(path: &Path, expected: &[u8]) -> bool {
  fs::read(path).is_ok_and(|b| b == expected)
}

#[test]
fn test_realtime_folder_sync_create_modify_delete_and_crash_recovery() -> Void {
  let origin_dir = tempdir()?;
  let sub_full_dir = tempdir()?;
  let sub_folder_dir = tempdir()?;

  let mut large_v1 = vec![0u8; 600_000];
  let mut rng = 0x1234_5678_9ABC_DEF0u64;
  for b in &mut large_v1 {
    rng ^= rng << 13;
    rng ^= rng >> 7;
    rng ^= rng << 17;
    *b = rng as u8;
  }

  fs::create_dir_all(origin_dir.path().join("sub/nested"))?;
  fs::write(origin_dir.path().join("sub/alpha.txt"), b"alpha-v1")?;
  fs::write(origin_dir.path().join("sub/large.bin"), &large_v1)?;
  fs::write(origin_dir.path().join("top.txt"), b"top-v1")?;

  let origin_key = [0x77; 32];
  let origin = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Fixed(origin_key),
    profile: ProfileDraft::new("origin_srv", Region::Shanghai),
    ..Default::default()
  })?;
  assert!(!origin.iroh_quic_addrs().is_empty());
  origin.add_share(ShareConfig::public_r("realtime-repo", origin_dir.path(), 0));
  let origin_id = origin.endpoint_id();

  let seed_full = start_node("seed_full", Region::Shenzhen)?;
  let seed_sub = start_node("seed_sub", Region::Beijing)?;

  // 全量做种订阅 + 子目录 `sub` 订阅
  seed_full.subscribe(
    origin_id,
    "realtime-repo",
    sub_full_dir.path().to_path_buf(),
    TransferOpts::new(),
  )?;
  seed_sub.subscribe(
    origin_id,
    "realtime-repo",
    sub_folder_dir.path().to_path_buf(),
    TransferOpts::new().with_paths(["sub"]),
  )?;

  // 验证服务器已在分享根目录下生成 `.s0_subs.bin` 持久化做种节点文件
  assert!(origin_dir.path().join(SUBS_FILE).is_file());

  // 0. 验证大变更 SubPush（含路径、权限、write_prefixes 与 ChunkSig）经 mzstd 自动压缩与还原
  let mut big_changes = Vec::with_capacity(64);
  for idx in 0..64u32 {
    big_changes.push(PathChange::upsert(
      format!("sub/nested/batch_file_{idx:04}.dat"),
      65_536,
      vec![ChunkSig {
        offset: 0,
        len: 65_536,
        hash: [0xAB; 16],
      }],
    ));
  }
  let push_sample = SubPush {
    flags: 0,
    share_id: [0x55; 16],
    perm: Perm::Rw,
    eff_rate: 50_000_000,
    write_prefixes: vec!["sub/nested".to_owned(), "uploads/team".to_owned()],
    root_hash: [0x99; 16],
    origin_addrs: Vec::new(),
    peers: Vec::new(),
    changes: big_changes,
  };
  let raw_push = push_sample.encode()?;
  let zstd_framed = encode_zstd_frame(&raw_push);
  let hdr = u32::from_le_bytes(*zstd_framed.first_chunk::<4>().unwrap());
  assert_ne!(hdr & MANIFEST_ZSTD, 0);
  assert!(zstd_framed.len() * 2 < raw_push.len());
  let decoded_push = SubPush::decode(&decode_zstd_frame(&zstd_framed)?)?;
  assert_eq!(decoded_push, push_sample);

  // 1. 块级真增量同步（拒绝删除全量重拉）：
  // 仅修改 600KiB 多块文件 `sub/large.bin` 尾部 64 字节，验证本地 Basis 块复用且仅拉取尾部改动块
  let mut large_v2 = large_v1.clone();
  let tail_start = large_v2.len() - 64;
  large_v2[tail_start..].fill(0xFE);
  fs::write(origin_dir.path().join("sub/large.bin"), &large_v2)?;

  // 校验针对修改后的 `sub/large.bin` 构造增量 Manifest 时，本地未修改的前序 FastCDC 块全部直接命中复用
  let large_mf = build_manifest_from_paths(origin_dir.path(), &["sub/large.bin"], false)?;
  let total_large_blks = large_mf.total_blocks();
  assert!(total_large_blks >= 8);
  {
    let probe_sink = SharedSink::prepare(
      sub_full_dir.path(),
      Manifest {
        entries: large_mf.entries.clone(),
        delete_extraneous: false,
        partial: true,
      },
    )?;
    let reused_blks = (0..total_large_blks)
      .filter(|&b| probe_sink.has_block(b))
      .count() as u32;
    assert!(reused_blks >= total_large_blks - 2);
  }

  // 实时文件创建与修改（无需手动调用 publish_updates）：
  // 后台监控通过 iroh 自动推送含 ChunkSig 的 SubPush，全量订阅者与 `sub` 目录订阅者仅拉取增量差异块
  fs::write(
    origin_dir.path().join("sub/nested/created.txt"),
    b"realtime-created",
  )?;
  fs::write(origin_dir.path().join("sub/alpha.txt"), b"alpha-v2")?;
  fs::write(origin_dir.path().join("outside.txt"), b"only-full")?;

  assert!(wait_until(Duration::from_secs(3), || {
    file_has(
      &sub_full_dir.path().join("sub/nested/created.txt"),
      b"realtime-created",
    ) && file_has(&sub_full_dir.path().join("sub/alpha.txt"), b"alpha-v2")
      && file_has(&sub_full_dir.path().join("sub/large.bin"), &large_v2)
      && file_has(&sub_full_dir.path().join("outside.txt"), b"only-full")
      && file_has(
        &sub_folder_dir.path().join("sub/nested/created.txt"),
        b"realtime-created",
      )
      && file_has(&sub_folder_dir.path().join("sub/alpha.txt"), b"alpha-v2")
      && file_has(&sub_folder_dir.path().join("sub/large.bin"), &large_v2)
  }));
  assert!(!sub_folder_dir.path().join("outside.txt").exists());
  assert!(!sub_full_dir.path().join(SUBS_FILE).exists());

  // 2. 仅修改文件权限 (mode) 与最后修改时间 (mtime)，内容未变（验证 FsOp::Meta = 3 零块签名、零 UDP 传输原语）
  let prev_mf = build_manifest_from_paths::<&str>(origin_dir.path(), &[], false)?;
  let target_mtime_ms = 1_715_000_000_500u64;
  set_file_meta(
    &origin_dir.path().join("sub/large.bin"),
    0o755,
    target_mtime_ms,
  )?;
  let curr_mf = build_manifest_from_paths::<&str>(origin_dir.path(), &[], false)?;
  let meta_diff = prev_mf.diff(&curr_mf);
  assert_eq!(meta_diff.len(), 1);
  assert_eq!(meta_diff[0].op, FsOp::Meta);
  assert_eq!(meta_diff[0].path, "sub/large.bin");
  assert_eq!(meta_diff[0].mode, 0o755);
  assert_eq!(meta_diff[0].mtime, target_mtime_ms);
  assert!(meta_diff[0].chunks.is_empty());

  assert!(wait_until(Duration::from_secs(3), || {
    let p1 = sub_full_dir.path().join("sub/large.bin");
    let p2 = sub_folder_dir.path().join("sub/large.bin");
    p1.metadata()
      .is_ok_and(|m| meta_mode(&m) == 0o755 && meta_mtime_ms(&m) == target_mtime_ms)
      && p2
        .metadata()
        .is_ok_and(|m| meta_mode(&m) == 0o755 && meta_mtime_ms(&m) == target_mtime_ms)
  }));

  // 3. 实时文件删除（FsOp::Remove = 2）：全量与子目录订阅者同步删除本地文件并清理空父目录
  fs::remove_file(origin_dir.path().join("sub/nested/created.txt"))?;
  assert!(wait_until(Duration::from_secs(3), || {
    !sub_full_dir.path().join("sub/nested/created.txt").exists()
      && !sub_folder_dir
        .path()
        .join("sub/nested/created.txt")
        .exists()
  }));
  assert!(!sub_full_dir.path().join("sub/nested").exists());
  assert!(!sub_folder_dir.path().join("sub/nested").exists());

  // 4. 服务器宕机重启恢复：
  // 模拟服务器挂掉 (drop)，停机期间目录发生新增与删除；
  // 重启后服务器读取 `.s0_subs.bin`，通过 iroh 重新通知全部做种节点完成同步
  drop(origin);

  fs::write(
    origin_dir.path().join("sub/after_crash.txt"),
    b"recovered-via-iroh",
  )?;
  fs::remove_file(origin_dir.path().join("sub/alpha.txt"))?;

  let origin_restarted = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Fixed(origin_key),
    profile: ProfileDraft::new("origin_srv", Region::Shanghai),
    ..Default::default()
  })?;
  origin_restarted.add_share(ShareConfig::public_r("realtime-repo", origin_dir.path(), 0));

  assert!(wait_until(Duration::from_secs(3), || {
    file_has(
      &sub_full_dir.path().join("sub/after_crash.txt"),
      b"recovered-via-iroh",
    ) && !sub_full_dir.path().join("sub/alpha.txt").exists()
      && file_has(
        &sub_folder_dir.path().join("sub/after_crash.txt"),
        b"recovered-via-iroh",
      )
      && !sub_folder_dir.path().join("sub/alpha.txt").exists()
  }));

  OK
}

#[test]
fn test_massive_10k_files_changes_mzstd_compression_and_unbounded_primitives() -> Void {
  const TOTAL_FILES: usize = 10_500;

  // 1. 构造 10,500 个文件的初始清单与一次性混合大变更（Upsert + Meta + Remove）
  let mut base_changes = Vec::with_capacity(TOTAL_FILES);
  for i in 0..TOTAL_FILES {
    base_changes.push(PathChange::upsert_meta(
      format!("shard_{:03}/item_{i:05}.dat", i / 100),
      4096,
      0o644,
      1_700_000_000_000 + (i as u64),
      vec![ChunkSig {
        offset: 0,
        len: 4096,
        hash: [(i & 0xFF) as u8; 16],
      }],
    ));
  }
  let mut mf = Manifest::default();
  mf.apply_changes(&base_changes);
  assert_eq!(mf.entries.len(), TOTAL_FILES);

  // 2. 一次性产生 10,500 条变更：
  // - 前 4,000 个文件仅修改权限 (0o755) 与 mtime（FsOp::Meta = 3，零块签名）
  // - 中间 5,000 个文件修改内容与权限（FsOp::Upsert = 1）
  // - 末尾 1,500 个文件删除（FsOp::Remove = 2）
  // - 额外新增 500 个新文件（FsOp::Upsert = 1），总计 11,000 条变更
  let mut batch_changes = Vec::with_capacity(11_000);
  for i in 0..4_000usize {
    batch_changes.push(PathChange::meta(
      format!("shard_{:03}/item_{i:05}.dat", i / 100),
      0o755,
      1_720_000_000_000 + (i as u64),
    ));
  }
  for i in 4_000..9_000usize {
    batch_changes.push(PathChange::upsert_meta(
      format!("shard_{:03}/item_{i:05}.dat", i / 100),
      8192,
      0o750,
      1_720_000_000_000 + (i as u64),
      vec![ChunkSig {
        offset: 0,
        len: 8192,
        hash: [((i >> 2) & 0xFF) as u8 ^ 0x5A; 16],
      }],
    ));
  }
  for i in 9_000..TOTAL_FILES {
    batch_changes.push(PathChange::remove(format!(
      "shard_{:03}/item_{i:05}.dat",
      i / 100
    )));
  }
  for i in TOTAL_FILES..(TOTAL_FILES + 500) {
    batch_changes.push(PathChange::upsert_meta(
      format!("shard_{:03}/item_{i:05}.dat", i / 100),
      2048,
      0o600,
      1_720_000_999_000 + (i as u64),
      vec![ChunkSig {
        offset: 0,
        len: 2048,
        hash: [0xCC; 16],
      }],
    ));
  }
  assert_eq!(batch_changes.len(), 11_000);

  let push = SubPush {
    flags: 0,
    share_id: [0x42; 16],
    perm: Perm::Rw,
    eff_rate: 100_000_000,
    write_prefixes: (0..50).map(|k| format!("shard_{k:03}")).collect(),
    root_hash: [0x77; 16],
    origin_addrs: Vec::new(),
    peers: Vec::new(),
    changes: batch_changes.clone(),
  };

  // 3. 验证 11,000 条文件变更编码、mzstd 压缩率（压缩后 < 原始体积 25%）与无上限解码还原
  let raw_bytes = push.encode()?;
  let compressed_frame = encode_zstd_frame(&raw_bytes);
  let hdr = u32::from_le_bytes(*compressed_frame.first_chunk::<4>().unwrap());
  assert_ne!(hdr & MANIFEST_ZSTD, 0);
  assert!(compressed_frame.len() * 4 < raw_bytes.len());

  let decompressed = decode_zstd_frame(&compressed_frame)?;
  let decoded = SubPush::decode(&decompressed)?;
  assert_eq!(decoded, push);

  // 4. 验证 O(N + M) 内存原位合并 11,000 条变更及 O(N log M) 多选路径过滤性能与准确性
  mf.apply_changes(&decoded.changes);
  assert_eq!(mf.entries.len(), 9_500);
  assert_eq!(mf.entries[0].mode, 0o755);
  assert_eq!(mf.entries[0].mtime, 1_720_000_000_000);
  assert_eq!(mf.entries[4_000].size, 8192);
  assert_eq!(mf.entries[4_000].mode, 0o750);
  assert_eq!(mf.entries.last().unwrap().mode, 0o600);

  let sel_paths: Vec<String> = (0..2_000)
    .map(|i| format!("shard_{:03}/item_{i:05}.dat", i / 100))
    .collect();
  let filtered = mf.filter_paths(&sel_paths);
  assert_eq!(filtered.entries.len(), 2_000);

  // 5. 验证 Redis RDB + AOF 混合持久化 (.s0_subs.bin)：
  //    9,500 个文件的基准清单单次修改仅追加 < 100B 的 AOF 帧（免全量序列化），
  //    支持掉电半包截断恢复 (aof-load-truncated) 并在加载后自动紧缩整理为纯净 RDB
  let store_dir = tempdir()?;
  let subs = vec![SubscriberRecord {
    endpoint_id: [0x11; 32],
    ctrl_addrs: vec!["127.0.0.1:19001".parse()?],
    paths: Vec::new(),
    eff_rate: 50_000_000,
  }];
  let rdb_len = save_subs_file(store_dir.path(), Some(&mf), &subs);
  assert!(rdb_len > 100_000);

  let single_delta = vec![PathChange::meta(
    "shard_000/item_00000.dat".into(),
    0o700,
    1_730_000_000_000,
  )];
  let (aof_written, aof_ops) = append_subs_aof_delta(store_dir.path(), &single_delta, &[]).unwrap();
  assert_eq!(aof_ops, 1);
  assert!(aof_written < 100);
  let hybrid_len = fs::metadata(store_dir.path().join(SUBS_FILE))?.len();
  assert_eq!(hybrid_len, rdb_len + aof_written);

  // 模拟尾部掉电写入半包脏数据，验证 CRC16 校验自动截断并恢复已提交 AOF 记录 + 自动重写紧缩
  {
    let mut f = OpenOptions::new()
      .append(true)
      .open(store_dir.path().join(SUBS_FILE))?;
    f.write_all(&[0x50, 0x00, 0x00, 0x00, 0xDE, 0xAD, 0x03, 0xFF])?;
  }
  let (loaded_mf_opt, loaded_subs, compacted_len) = load_subs_file(store_dir.path()).unwrap();
  let loaded_mf = loaded_mf_opt.unwrap();
  assert_eq!(loaded_subs, subs);
  assert_eq!(loaded_mf.entries.len(), 9_500);
  assert_eq!(loaded_mf.entries[0].mode, 0o700);
  assert_eq!(loaded_mf.entries[0].mtime, 1_730_000_000_000);
  assert_eq!(
    fs::metadata(store_dir.path().join(SUBS_FILE))?.len(),
    compacted_len
  );

  OK
}

#[test]
fn test_leecher_seeding_simultaneous_download_and_upload() -> Void {
  let (origin_dir, leech_a_dir, leech_b_dir) = (tempdir()?, tempdir()?, tempdir()?);

  // 构造 1.5 MiB 高熵数据（约 48 个 FastCDC 块，每块 16 KiB ~ 64 KiB，均值 32 KiB） + 若干小文件（落入 DisjointArena）
  let mut payload = vec![0u8; 1_500_000];
  let mut rng = 0xD1B5_4A32_D192_ED03u64;
  for b in &mut payload {
    rng ^= rng << 13;
    rng ^= rng >> 7;
    rng ^= rng << 17;
    *b = rng as u8;
  }
  fs::write(origin_dir.path().join("stream.bin"), &payload)?;
  for i in 0..8u32 {
    fs::write(
      origin_dir.path().join(format!("tiny_{i}.dat")),
      format!("small-pack-block-{i}").as_bytes(),
    )?;
  }

  // 限制源节点单流速率为 1.5 MB/s（总耗时约 1s），使 leecher_a 在 ~100ms 下载满 4 块并通过 Op::Have 上报已就绪块区间，
  // 供随后启动的 leecher_b 直接从 leecher_a 的未落盘 SharedSink (BlockSource::InProgress) 并发边下边传
  let origin = start_node("origin_slow", Region::Shanghai)?;
  origin.add_share(ShareConfig::public_r(
    "leech-swarm",
    origin_dir.path(),
    1_500_000,
  ));
  let origin_id = origin.endpoint_id();

  let leecher_a = start_node("leecher_a", Region::Shanghai)?;
  let leecher_b = start_node("leecher_b", Region::Shanghai)?;

  let (rep_a, rep_b) = thread::scope(|s| {
    let a_path = leech_a_dir.path();
    let b_path = leech_b_dir.path();
    let h_a = s.spawn(|| leecher_a.pull(origin_id, "leech-swarm", a_path, TransferOpts::new()));
    // 等待 leecher_a 下载满至少 4 个块并通过 30ms 心跳向源节点上报 Op::Have，但此时尚未完成全量下载（stream.bin 尚未 rename）
    assert!(wait_until(Duration::from_secs(2), || {
      origin
        .stat(origin_id, "leech-swarm", &TransferOpts::new())
        .is_ok_and(|(r, _)| {
          r.peers
            .iter()
            .any(|p| p.endpoint_id == leecher_a.endpoint_id() && p.blk_hi >= 4)
        })
    }));
    assert!(!a_path.join("stream.bin").exists());
    let h_b = s.spawn(|| leecher_b.pull(origin_id, "leech-swarm", b_path, TransferOpts::new()));
    (h_a.join().unwrap(), h_b.join().unwrap())
  });

  let (rep_a, rep_b) = (rep_a?, rep_b?);
  assert!(rep_a.total_blocks >= 40);
  assert_eq!(rep_b.blocks_transferred, rep_a.total_blocks);
  assert!(rep_b.peers_used >= 2);
  assert_eq!(fs::read(leech_a_dir.path().join("stream.bin"))?, payload);
  assert_eq!(fs::read(leech_b_dir.path().join("stream.bin"))?, payload);
  for i in 0..8u32 {
    assert_eq!(
      fs::read(leech_b_dir.path().join(format!("tiny_{i}.dat")))?,
      format!("small-pack-block-{i}").as_bytes()
    );
  }
  OK
}
