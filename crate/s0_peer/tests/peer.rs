use std::{
  fs,
  iter::repeat_n,
  net::{Ipv4Addr, Ipv6Addr, SocketAddr},
  sync::Arc,
};

use aok::{OK, Void};
use ctor::ctor;
use log_init::init;
use s0_conf::GroupUri;
use s0_const::{Status, region::Region};
use s0_fs::{SharedSink, SourceReader, build_manifest_from_paths};
use s0_peer::{
  KeyMode, NodeConfig, PeerError, PeerNode, ShareConfig, TransferOpts, VOD_HEAD_WINDOW_BLOCKS,
  connect_ctrl_addrs, decode_id_b64, plan_stripes_range, register_iroh_endpoints,
  resolve_iroh_endpoints,
};
use s0_wire::{ProfileDraft, SwarmPeer};
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

#[test]
fn test_multi_group_multi_select_and_upload_prefix_permissions() -> Void {
  let (share_dir, models_dir, dst_dir, upload_src) =
    (tempdir()?, tempdir()?, tempdir()?, tempdir()?);

  fs::create_dir_all(share_dir.path().join("docs"))?;
  fs::create_dir_all(share_dir.path().join("assets/icons"))?;
  fs::create_dir_all(share_dir.path().join("assets/raw"))?;
  fs::write(share_dir.path().join("docs/readme.md"), b"hello p2p world")?;
  fs::write(share_dir.path().join("assets/icons/a.svg"), b"<svg>a</svg>")?;
  fs::write(share_dir.path().join("assets/icons/b.svg"), b"<svg>b</svg>")?;
  fs::write(share_dir.path().join("assets/raw/huge.bin"), b"unselected")?;
  fs::write(share_dir.path().join("top.txt"), b"top secret")?;
  fs::write(models_dir.path().join("llama.onnx"), b"onnx-weights-v1")?;

  let node_a = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Fixed([0x11; 32]),
    profile: ProfileDraft::new("alice_shanghai", Region::Shanghai)
      .with_email("alice@sync0.io")
      .with_url("https://alice.sync0.io"),
    ..Default::default()
  })?;
  let node_b = start_node("bob_shenzhen", Region::Shenzhen)?;
  let node_c = start_node("charlie_tokyo", Region::Tokyo)?;
  let node_d = start_node("dave_root_writer", Region::Singapore)?;

  // 通过 base64url iroh 地址解析为 32B 二进制 PeerId
  let alice_id = decode_id_b64(&node_a.iroh_addr_b64()).unwrap();
  assert_eq!(alice_id, node_a.endpoint_id());

  // 1. 单个用户 (node_a) 同时托管多个文件分享组，采用 NestedText (.nt) 格式配置：
  // 支持配置用户目录 `users_dir` 自动包含该目录下的全部 `.nt` 用户文件，用户与上传路径前缀均使用无序列表 (`-`)
  let cfg_dir = tempdir()?;
  let members_dir = cfg_dir.path().join("members");
  fs::create_dir_all(&members_dir)?;

  fs::write(
    members_dir.join("charlie.nt"),
    format!("id: {}\nrate: 80MB\n", node_c.iroh_id_b64()),
  )?;
  fs::write(
    members_dir.join("bob.nt"),
    format!(
      "id: {}\nperm: rw\nwrite_prefix:\n  - uploads/bob\n  - shared/common\n",
      node_b.iroh_id_b64()
    ),
  )?;
  fs::write(
    members_dir.join("dave.nt"),
    format!("id: {}\nperm: w\n", node_d.iroh_id_b64()),
  )?;

  let conf_nt = format!(
    "users_dir: members\ngroups:\n  pub-site:\n    path: {}\n    rate: 50MB\n  ai-models:\n    path: {}\n  priv-vault:\n    path: {}\n    rate: 10MB\n    users:\n      - charlie\n      - bob\n      - dave\n",
    share_dir.path().display(),
    models_dir.path().display(),
    share_dir.path().display(),
  );
  fs::write(cfg_dir.path().join("config.nt"), &conf_nt)?;
  let groups = ShareConfig::load_from_dir(cfg_dir.path())?;
  assert_eq!(groups.len(), 3);
  node_a.add_shares(groups);

  // 查询 node_a 托管的多个分享组列表
  let listed = node_b.list_shares(alice_id, None)?;
  assert_eq!(listed.len(), 3);

  // 2. 多选（Multi-select）同步：同时选择单文件 `docs/readme.md` 与子目录 `assets/icons`
  let uri = GroupUri::parse("pub-site://docs/readme.md,assets/icons").unwrap();
  let rep_multi = node_b.pull(
    alice_id,
    &uri.group,
    dst_dir.path(),
    TransferOpts::new().with_paths(&uri.paths),
  )?;
  assert_eq!(rep_multi.blocks_transferred, 3);
  assert_eq!(
    fs::read(dst_dir.path().join("docs/readme.md"))?,
    b"hello p2p world"
  );
  assert_eq!(
    fs::read(dst_dir.path().join("assets/icons/a.svg"))?,
    b"<svg>a</svg>"
  );
  assert_eq!(
    fs::read(dst_dir.path().join("assets/icons/b.svg"))?,
    b"<svg>b</svg>"
  );
  assert!(!dst_dir.path().join("assets/raw/huge.bin").exists());
  assert!(!dst_dir.path().join("top.txt").exists());

  let models_dst = tempdir()?;
  node_b.pull(
    alice_id,
    "ai-models",
    models_dst.path(),
    TransferOpts::new(),
  )?;
  assert_eq!(
    fs::read(models_dst.path().join("llama.onnx"))?,
    b"onnx-weights-v1"
  );

  // 3. 组员权限与可上传路径前缀校验
  // 3.1 只读成员 charlie (Perm::Ro) 可查看但不可上传
  let (resp_c, _) = node_c.stat(alice_id, "priv-vault", &TransferOpts::new())?;
  assert_eq!(resp_c.eff_rate, 10 * 1024 * 1024);
  let push_forbid_ro = node_c.push(alice_id, "priv-vault", dst_dir.path(), TransferOpts::new());
  assert!(matches!(
    push_forbid_ro,
    Err(PeerError::RemoteStatus(Status::Forbidden))
  ));

  // 3.2 受限上传成员 bob (Perm::Rw, write_prefixes = ["uploads/bob", "shared/common"])
  let bob_file = upload_src.path().join("report.txt");
  fs::write(&bob_file, b"bob-quarterly-report")?;

  // bob 上传到允许的前缀 `uploads/bob/report.txt` -> 成功
  node_b.push(
    alice_id,
    "priv-vault",
    &bob_file,
    TransferOpts::new().with_paths(["uploads/bob/report.txt"]),
  )?;
  assert_eq!(
    fs::read(share_dir.path().join("uploads/bob/report.txt"))?,
    b"bob-quarterly-report"
  );

  // bob 尝试越权上传到 `uploads/alice/hack.txt` 或根目录 `root_hack.txt` -> 被拒绝 (Status::Forbidden)
  for bad_sub in ["uploads/alice/hack.txt", "root_hack.txt"] {
    let res = node_b.push(
      alice_id,
      "priv-vault",
      &bob_file,
      TransferOpts::new().with_paths([bad_sub]),
    );
    assert!(matches!(
      res,
      Err(PeerError::RemoteStatus(Status::Forbidden))
    ));
  }

  // 3.3 仅写成员 dave (Perm::W, write_prefixes = [] 即默认分享根目录) -> 可上传但不可读取
  node_d.push(
    alice_id,
    "priv-vault",
    &bob_file,
    TransferOpts::new().with_paths(["root_allowed.txt"]),
  )?;
  assert_eq!(
    fs::read(share_dir.path().join("root_allowed.txt"))?,
    b"bob-quarterly-report"
  );
  let stat_forbid_w = node_d.stat(alice_id, "priv-vault", &TransferOpts::new());
  assert!(matches!(
    stat_forbid_w,
    Err(PeerError::RemoteStatus(Status::Forbidden))
  ));

  // 3.4 未授权公网节点 (node_e) 无法枚举或读取 `priv-vault`（源节点与做种订阅节点均拒绝 `Status::Unauth`）
  let node_e = start_node("eve_stranger", Region::Frankfurt)?;
  assert_eq!(node_e.list_shares(alice_id, None)?.len(), 2);
  let unauth_pull = node_e.pull(
    alice_id,
    "priv-vault",
    tempdir()?.path(),
    TransferOpts::new(),
  );
  assert!(matches!(
    unauth_pull,
    Err(PeerError::RemoteStatus(Status::Unauth))
  ));
  let sub_c_dir = tempdir()?;
  node_c.subscribe(
    alice_id,
    "priv-vault",
    sub_c_dir.path().to_path_buf(),
    TransferOpts::new(),
  )?;
  assert!(matches!(
    node_e.pull(
      node_c.endpoint_id(),
      "priv-vault",
      tempdir()?.path(),
      TransferOpts::new()
    ),
    Err(PeerError::RemoteStatus(Status::Unauth))
  ));

  OK
}

#[test]
fn test_long_lived_subscribers_act_as_dragonfly_p2p_swarm_seeds() -> Void {
  let (origin_dir, sub_full_dir, sub_part_dir, client_dir) =
    (tempdir()?, tempdir()?, tempdir()?, tempdir()?);
  let mut big_data = vec![0u8; 600_000];
  let mut s = 0xCAFE_BABEu64;
  for b in &mut big_data {
    s ^= s << 13;
    s ^= s >> 7;
    s ^= s << 17;
    *b = s as u8;
  }
  fs::create_dir_all(origin_dir.path().join("sub"))?;
  fs::create_dir_all(origin_dir.path().join("extra"))?;
  fs::write(origin_dir.path().join("big.bin"), &big_data)?;
  fs::write(origin_dir.path().join("sub/part.txt"), b"v1-sub")?;
  fs::write(origin_dir.path().join("extra/note.md"), b"v1-note")?;

  let origin = start_node("origin_beijing", Region::Beijing)?;
  origin.add_share(ShareConfig::public_r("repo", origin_dir.path(), 0));
  let origin_id = origin.endpoint_id();

  let sub_peer1 = start_node("seed1_shanghai", Region::Shanghai)?;
  let sub_peer2 = start_node("seed2_shanghai", Region::Shanghai)?;
  sub_peer1.subscribe(
    origin_id,
    "repo",
    sub_full_dir.path().to_path_buf(),
    TransferOpts::new(),
  )?;
  sub_peer2.subscribe(
    origin_id,
    "repo",
    sub_part_dir.path().to_path_buf(),
    TransferOpts::new().with_paths(["sub", "extra/note.md"]),
  )?;

  assert_eq!(fs::read(sub_full_dir.path().join("big.bin"))?, big_data);
  assert_eq!(
    fs::read(sub_part_dir.path().join("sub/part.txt"))?,
    b"v1-sub"
  );
  assert_eq!(
    fs::read(sub_part_dir.path().join("extra/note.md"))?,
    b"v1-note"
  );
  assert!(!sub_part_dir.path().join("big.bin").exists());

  let downloader = start_node("dl_shanghai", Region::Shanghai)?;
  let rep = downloader.pull(
    origin_id,
    "repo",
    client_dir.path(),
    TransferOpts::new().with_delete(true),
  )?;
  assert!(rep.peers_used >= 2);
  assert_eq!(fs::read(client_dir.path().join("big.bin"))?, big_data);

  // 恶意节点篡改本地做种文件（保持相同长度伪造内容）：下载端逐块校验 gxhash128 自动拦截脏块、剔除恶意源并从诚实源重拉补齐
  let (bad_seed, bad_dir, clean_dst) = (
    start_node("malicious_seed", Region::Shanghai)?,
    tempdir()?,
    tempdir()?,
  );
  bad_seed.subscribe(
    origin_id,
    "repo",
    bad_dir.path().to_path_buf(),
    TransferOpts::new(),
  )?;
  fs::write(bad_dir.path().join("big.bin"), vec![0xFFu8; big_data.len()])?;
  downloader.pull(origin_id, "repo", clean_dst.path(), TransferOpts::new())?;
  assert_eq!(fs::read(clean_dst.path().join("big.bin"))?, big_data);

  fs::write(origin_dir.path().join("root_new.txt"), b"root-v2")?;
  fs::write(origin_dir.path().join("sub/part.txt"), b"v2-sub-updated")?;
  assert!(origin.publish_updates("repo")? >= 2);
  assert_eq!(
    fs::read(sub_full_dir.path().join("root_new.txt"))?,
    b"root-v2"
  );
  assert_eq!(
    fs::read(sub_full_dir.path().join("sub/part.txt"))?,
    b"v2-sub-updated"
  );
  assert_eq!(
    fs::read(sub_part_dir.path().join("sub/part.txt"))?,
    b"v2-sub-updated"
  );
  assert!(!sub_part_dir.path().join("root_new.txt").exists());
  OK
}

#[test]
fn test_group_mutual_backup_full_and_multi_select() -> Void {
  let (dir1, dir2, dir3) = (tempdir()?, tempdir()?, tempdir()?);
  let p1 = start_node("p1", Region::Shenzhen)?;
  let p2 = start_node("p2", Region::HongKong)?;
  let p3 = start_node("p3", Region::Taipei)?;

  let peers = [p1.endpoint_id(), p2.endpoint_id(), p3.endpoint_id()];
  let sel_p3 = vec!["wal".to_owned(), "meta.json".to_owned()];
  p1.join_group("db-cluster", dir1.path().to_path_buf(), &[], &peers)?;
  p2.join_group("db-cluster", dir2.path().to_path_buf(), &[], &peers)?;
  p3.join_group("db-cluster", dir3.path().to_path_buf(), &sel_p3, &peers)?;
  p1.join_group("db-cluster", dir1.path().to_path_buf(), &[], &peers)?;
  p2.join_group("db-cluster", dir2.path().to_path_buf(), &[], &peers)?;

  fs::create_dir_all(dir1.path().join("wal"))?;
  fs::create_dir_all(dir1.path().join("cache"))?;
  fs::write(dir1.path().join("wal/01.log"), b"tx-commit-1")?;
  fs::write(dir1.path().join("meta.json"), b"{\"epoch\":1}")?;
  fs::write(dir1.path().join("cache/tmp.bin"), b"ephemeral-cache")?;
  p1.publish_updates("db-cluster")?;

  assert_eq!(fs::read(dir2.path().join("wal/01.log"))?, b"tx-commit-1");
  assert_eq!(
    fs::read(dir2.path().join("cache/tmp.bin"))?,
    b"ephemeral-cache"
  );
  assert_eq!(fs::read(dir3.path().join("wal/01.log"))?, b"tx-commit-1");
  assert_eq!(fs::read(dir3.path().join("meta.json"))?, b"{\"epoch\":1}");
  assert!(!dir3.path().join("cache/tmp.bin").exists());

  fs::write(dir3.path().join("wal/02.log"), b"tx-commit-2-from-p3")?;
  p3.publish_updates("db-cluster")?;
  assert_eq!(
    fs::read(dir1.path().join("wal/02.log"))?,
    b"tx-commit-2-from-p3"
  );
  assert_eq!(
    fs::read(dir2.path().join("wal/02.log"))?,
    b"tx-commit-2-from-p3"
  );

  OK
}

fn mock_swarm_peer(id: u8, blk_hi: u32) -> SwarmPeer {
  SwarmPeer {
    endpoint_id: [id; 32],
    region: Region::Shanghai,
    weight_rate: 100_000_000,
    blk_lo: 0,
    blk_hi,
    peer_addrs: Vec::new(),
    profile_card: Vec::new(),
  }
}

#[test]
fn test_byte_weighted_stripes_for_mixed_file_sizes() -> Void {
  // 构造 2 个同地域等权重节点与混合负载：前 8 块为 128KiB 大文件块（共 1MiB），后 256 块为 4KiB 小文件块（共 1MiB）
  let peers = vec![mock_swarm_peer(1, 264), mock_swarm_peer(2, 264)];
  let mut weights = Vec::with_capacity(264);
  weights.extend(repeat_n(131_072u64, 8));
  weights.extend(repeat_n(4_096u64, 256));

  let stripes = plan_stripes_range(
    Region::Shanghai,
    &peers,
    &weights,
    0,
    weights.len() as u32,
    false,
  );
  assert_eq!(stripes, vec![(0, 0, 8), (1, 8, 264)]);
  OK
}

#[test]
fn test_vod_byte_range_crash_resume_and_dualstack_happy_eyeballs() -> Void {
  let (src_dir, vod_dir, resume_dir, v6_dst_dir) = (tempdir()?, tempdir()?, tempdir()?, tempdir()?);

  let mut video_data = vec![0u8; 800_000];
  let mut rng = 0x9E37_79B9_7F4A_7C15u64;
  for b in &mut video_data {
    rng ^= rng << 13;
    rng ^= rng >> 7;
    rng ^= rng << 17;
    *b = rng as u8;
  }
  fs::write(src_dir.path().join("movie.mp4"), &video_data)?;

  // 1. 验证 VoD Head-First Priority 条带规划：首窗口 16 块优先分配给第 0 号最低 RTT 节点
  let dummy_peers = vec![mock_swarm_peer(1, 60), mock_swarm_peer(2, 60)];
  let vod_stripes = plan_stripes_range(
    Region::Shanghai,
    &dummy_peers,
    &vec![32_768u64; 60],
    10,
    50,
    true,
  );
  assert_eq!(vod_stripes[0], (0, 10, 10 + VOD_HEAD_WINDOW_BLOCKS));
  assert_eq!(vod_stripes[1], (1, 10 + VOD_HEAD_WINDOW_BLOCKS, 50));

  // 2. 启动 IPv6 ([::1]) 双栈源节点，验证双栈路由注册、Happy Eyeballs 竞速连接与 FLAG_BYTE_RANGE 点播偏移拉取
  let seed_v6 = PeerNode::start(NodeConfig {
    bind_addr: Some(SocketAddr::from((Ipv6Addr::LOCALHOST, 0))),
    profile: ProfileDraft::new("seed_dualstack", Region::Shanghai),
    ..Default::default()
  })?;
  seed_v6.add_share(ShareConfig::public_r("vod", src_dir.path(), 0));
  let seed_id = seed_v6.endpoint_id();

  // 注册一个不可达 IPv6 端口 + 真实双栈端点，验证 Happy Eyeballs 竞速回退
  let mut race_addrs = vec![SocketAddr::from((Ipv6Addr::LOCALHOST, 9))];
  race_addrs.extend_from_slice(seed_v6.ctrl_addrs());
  register_iroh_endpoints(seed_id, &race_addrs);
  assert!(resolve_iroh_endpoints(&seed_id).len() >= 2);
  assert!(
    connect_ctrl_addrs(&resolve_iroh_endpoints(&seed_id))?
      .peer_addr()
      .is_ok()
  );
  register_iroh_endpoints(seed_id, seed_v6.ctrl_addrs());

  let client = PeerNode::start(NodeConfig {
    bind_addr: Some(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))),
    profile: ProfileDraft::new("vod_client", Region::Shanghai),
    ..Default::default()
  })?;

  let (byte_lo, byte_hi) = (220_000u64, 480_000u64);
  let rep_vod = client.pull(
    seed_id,
    "vod",
    vod_dir.path(),
    TransferOpts::new().with_byte_range(byte_lo, byte_hi),
  )?;
  assert!(rep_vod.blocks_transferred > 0 && rep_vod.blocks_transferred < rep_vod.total_blocks);

  let mf = build_manifest_from_paths::<&str>(src_dir.path(), &[], false)?;
  let vod_sink = SharedSink::prepare(vod_dir.path(), mf.clone())?;
  let mut vod_slice = vec![0u8; (byte_hi - byte_lo) as usize];
  assert_eq!(
    vod_sink.read_at_offset(0, byte_lo, &mut vod_slice)?,
    vod_slice.len()
  );
  assert_eq!(
    &vod_slice[..],
    &video_data[byte_lo as usize..byte_hi as usize]
  );

  // 3. 验证 .s0_stage.<filename> 崩溃中断后的零状态文件断点续传（仅传输剩余未完成块）
  let (total_blks, half_blks) = (mf.total_blocks(), mf.total_blocks() / 2);
  {
    let reader = SourceReader::open(src_dir.path(), mf.clone())?;
    let crashed_sink = SharedSink::prepare(resume_dir.path(), mf)?;
    let mut blk_buf = Vec::new();
    for b in 0..half_blks {
      reader.read_block(b, &mut blk_buf)?;
      assert!(crashed_sink.write_verified_block(b, &blk_buf)?);
    }
  }
  assert!(resume_dir.path().join(".s0_stage.movie.mp4").exists());
  let rep_resume = client.pull(seed_id, "vod", resume_dir.path(), TransferOpts::new())?;
  assert_eq!(rep_resume.blocks_transferred, total_blks - half_blks);
  assert_eq!(fs::read(resume_dir.path().join("movie.mp4"))?, video_data);

  // 4. 验证纯 IPv6 节点到节点的全量高速拉取
  let client_v6 = PeerNode::start(NodeConfig {
    bind_addr: Some(SocketAddr::from((Ipv6Addr::LOCALHOST, 0))),
    profile: ProfileDraft::new("client_v6", Region::Shanghai),
    ..Default::default()
  })?;
  let rep_v6 = client_v6.pull(seed_id, "vod", v6_dst_dir.path(), TransferOpts::new())?;
  assert_eq!(rep_v6.blocks_transferred, total_blks);
  assert_eq!(fs::read(v6_dst_dir.path().join("movie.mp4"))?, video_data);
  OK
}
