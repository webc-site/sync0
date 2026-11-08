use std::fs;

use aok::{OK, Void};
use s0_conf::GroupUri;
use s0_udp::{
  consts::Status,
  peer::{KeyMode, NodeConfig, PeerError, PeerNode, ShareConfig, TransferOpts, decode_id_b64},
  profile::ProfileDraft,
  region::Region,
};

#[ctor::ctor(unsafe)]
fn _log_init() {
  log_init::init();
}

#[test]
fn test_multi_group_multi_select_and_upload_prefix_permissions() -> Void {
  let share_dir = tempfile::tempdir()?;
  let models_dir = tempfile::tempdir()?;
  let dst_dir = tempfile::tempdir()?;
  let upload_src = tempfile::tempdir()?;

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

  let node_b = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Ephemeral,
    profile: ProfileDraft::new("bob_shenzhen", Region::Shenzhen),
    ..Default::default()
  })?;

  let node_c = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Fixed([0x33; 32]),
    profile: ProfileDraft::new("charlie_tokyo", Region::Tokyo),
    ..Default::default()
  })?;

  let node_d = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Fixed([0x44; 32]),
    profile: ProfileDraft::new("dave_root_writer", Region::Singapore),
    ..Default::default()
  })?;

  // 通过 base64url iroh 地址解析为 32B 二进制 PeerId
  let alice_id = decode_id_b64(&node_a.iroh_addr_b64()).unwrap();
  assert_eq!(alice_id, node_a.endpoint_id());

  // 1. 单个用户 (node_a) 同时托管多个文件分享组，采用 NestedText (.nt) 格式配置：
  // 支持配置用户目录 `users_dir` 自动包含该目录下的全部 `.nt` 用户文件，用户与上传路径前缀均使用无序列表 (`-`)
  let cfg_dir = tempfile::tempdir()?;
  let members_dir = cfg_dir.path().join("members");
  fs::create_dir_all(&members_dir)?;

  fs::write(
    members_dir.join("charlie.nt"),
    format!("id: {}\nperm: ro\nrate: 80MB\n", node_c.iroh_id_b64()),
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
    format!("id: {}\nperm: rw\n", node_d.iroh_id_b64()),
  )?;

  let conf_nt = format!(
    "users_dir: members\ngroups:\n  pub-site:\n    path: {}\n    auth: false\n    perm: ro\n    rate: 50MB\n  ai-models:\n    path: {}\n    auth: false\n    perm: ro\n  priv-vault:\n    path: {}\n    auth: true\n    perm: ro\n    rate: 10MB\n    users:\n      - charlie\n      - bob\n      - dave\n",
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

  // 拉取同一用户下的第二个分享组 `ai-models`
  let models_dst = tempfile::tempdir()?;
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
    TransferOpts::new().with_subpath("uploads/bob/report.txt"),
  )?;
  assert_eq!(
    fs::read(share_dir.path().join("uploads/bob/report.txt"))?,
    b"bob-quarterly-report"
  );

  // bob 尝试越权上传到 `uploads/alice/hack.txt` 或根目录 `root_hack.txt` -> 被拒绝 (Status::Forbidden)
  let bob_bad_prefix = node_b.push(
    alice_id,
    "priv-vault",
    &bob_file,
    TransferOpts::new().with_subpath("uploads/alice/hack.txt"),
  );
  assert!(matches!(
    bob_bad_prefix,
    Err(PeerError::RemoteStatus(Status::Forbidden))
  ));
  let bob_bad_root = node_b.push(
    alice_id,
    "priv-vault",
    &bob_file,
    TransferOpts::new().with_subpath("root_hack.txt"),
  );
  assert!(matches!(
    bob_bad_root,
    Err(PeerError::RemoteStatus(Status::Forbidden))
  ));

  // 3.3 默认根目录上传成员 dave (Perm::Rw, write_prefixes = [] 即默认分享根目录) -> 可上传至任意路径
  node_d.push(
    alice_id,
    "priv-vault",
    &bob_file,
    TransferOpts::new().with_subpath("root_allowed.txt"),
  )?;
  assert_eq!(
    fs::read(share_dir.path().join("root_allowed.txt"))?,
    b"bob-quarterly-report"
  );

  OK
}

#[test]
fn test_long_lived_subscribers_act_as_dragonfly_p2p_swarm_seeds() -> Void {
  let origin_dir = tempfile::tempdir()?;
  let sub_full_dir = tempfile::tempdir()?;
  let sub_part_dir = tempfile::tempdir()?;
  let client_dir = tempfile::tempdir()?;

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

  let origin = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Ephemeral,
    profile: ProfileDraft::new("origin_beijing", Region::Beijing),
    ..Default::default()
  })?;
  origin.add_share(ShareConfig::public_ro("repo", origin_dir.path(), 0));
  let origin_id = origin.endpoint_id();

  let sub_peer1 = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Ephemeral,
    profile: ProfileDraft::new("seed1_shanghai", Region::Shanghai),
    ..Default::default()
  })?;
  let sub_peer2 = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Ephemeral,
    profile: ProfileDraft::new("seed2_shanghai", Region::Shanghai),
    ..Default::default()
  })?;

  sub_peer1.subscribe(
    origin_id,
    "repo",
    sub_full_dir.path().to_path_buf(),
    TransferOpts::new(),
  )?;
  // 多选订阅目录 `sub` 与单文件 `extra/note.md`
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

  let downloader = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Ephemeral,
    profile: ProfileDraft::new("dl_shanghai", Region::Shanghai),
    ..Default::default()
  })?;
  let rep = downloader.pull(
    origin_id,
    "repo",
    client_dir.path(),
    TransferOpts::new().with_delete(true),
  )?;
  assert!(rep.peers_used >= 2);
  assert_eq!(fs::read(client_dir.path().join("big.bin"))?, big_data);

  fs::write(origin_dir.path().join("root_new.txt"), b"root-v2")?;
  fs::write(origin_dir.path().join("sub/part.txt"), b"v2-sub-updated")?;
  let pushed = origin.publish_updates("repo")?;
  assert_eq!(pushed, 2);

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
  let dir1 = tempfile::tempdir()?;
  let dir2 = tempfile::tempdir()?;
  let dir3 = tempfile::tempdir()?;

  let p1 = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Ephemeral,
    profile: ProfileDraft::new("p1", Region::Shenzhen),
    ..Default::default()
  })?;
  let p2 = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Ephemeral,
    profile: ProfileDraft::new("p2", Region::HongKong),
    ..Default::default()
  })?;
  let p3 = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Ephemeral,
    profile: ProfileDraft::new("p3", Region::Taipei),
    ..Default::default()
  })?;

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
