use std::fs;

use aok::{OK, Void};
use s0_testkit::{
  s0_const::{Perm, Status},
  s0_peer::{KeyMode, NodeConfig, PeerError, PeerNode, ShareConfig, TransferOpts},
  s0_profile::ProfileDraft,
  s0_region::Region,
};

#[ctor::ctor(unsafe)]
fn _log_init() {
  log_init::init();
}

#[test]
fn test_anonymous_and_authenticated_share_with_signed_profile() -> Void {
  let share_dir = tempfile::tempdir()?;
  let dst_dir = tempfile::tempdir()?;
  fs::create_dir_all(share_dir.path().join("docs"))?;
  fs::write(share_dir.path().join("docs/readme.md"), b"hello p2p world")?;
  fs::write(share_dir.path().join("top.txt"), b"top secret")?;

  let node_a = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Fixed([0x11; 32]),
    profile: ProfileDraft::new("alice_shanghai", Region::Shanghai)
      .with_email("alice@sync0.io")
      .with_url("https://alice.sync0.io"),
    enable_iroh: false,
  })?;

  let node_b = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Ephemeral,
    profile: ProfileDraft::new("bob_shenzhen", Region::Shenzhen),
    enable_iroh: false,
  })?;

  let node_c = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Fixed([0x33; 32]),
    profile: ProfileDraft::new("charlie_tokyo", Region::Tokyo),
    enable_iroh: false,
  })?;

  // 1. 默认匿名公开分享：无需密码即可拉取单个文件或子目录，适用默认带宽限额
  node_a.add_share(ShareConfig::public_ro(
    "pub-site",
    share_dir.path(),
    50_000_000,
  ));

  let rep_file = node_b.pull(
    node_a.ctrl_addr(),
    "pub-site",
    dst_dir.path(),
    TransferOpts::new().with_subpath("docs/readme.md"),
  )?;
  assert_eq!(rep_file.blocks_transferred, 1);
  assert_eq!(
    fs::read(dst_dir.path().join("docs/readme.md"))?,
    b"hello p2p world"
  );
  assert!(!dst_dir.path().join("top.txt").exists());

  let alice_card = node_b.get_peer_profile(&node_a.endpoint_id()).unwrap();
  assert_eq!(alice_card.name, "alice_shanghai");
  assert_eq!(alice_card.region, Region::Shanghai);
  assert_eq!(alice_card.email, "alice@sync0.io");

  let bob_card = node_a.get_peer_profile(&node_b.endpoint_id()).unwrap();
  assert_eq!(bob_card.name, "bob_shenzhen");
  assert_eq!(bob_card.region, Region::Shenzhen);

  // 2. 开启认证模式 (auth_required = true)
  let auth_cfg = ShareConfig::public_ro("priv-vault", share_dir.path(), 10_000_000)
    .with_auth(true)
    .grant_peer(node_c.endpoint_id(), Perm::Ro, 80_000_000);
  node_a.add_share(auth_cfg);

  let unauth_res = node_b.stat(node_a.ctrl_addr(), "priv-vault", TransferOpts::new());
  assert!(matches!(
    unauth_res,
    Err(PeerError::RemoteStatus(Status::Unauth))
  ));

  let (resp_c, _) = node_c.stat(node_a.ctrl_addr(), "priv-vault", TransferOpts::new())?;
  assert_eq!(resp_c.eff_rate, 10_000_000);
  let push_forbid = node_c.push(
    node_a.ctrl_addr(),
    "priv-vault",
    dst_dir.path(),
    TransferOpts::new(),
  );
  assert!(matches!(
    push_forbid,
    Err(PeerError::RemoteStatus(Status::Forbidden))
  ));

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
  fs::write(origin_dir.path().join("big.bin"), &big_data)?;
  fs::write(origin_dir.path().join("sub/part.txt"), b"v1-sub")?;

  let origin = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Ephemeral,
    profile: ProfileDraft::new("origin_beijing", Region::Beijing),
    enable_iroh: false,
  })?;
  origin.add_share(ShareConfig::public_ro("repo", origin_dir.path(), 0));

  let sub_peer1 = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Ephemeral,
    profile: ProfileDraft::new("seed1_shanghai", Region::Shanghai),
    enable_iroh: false,
  })?;
  let sub_peer2 = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Ephemeral,
    profile: ProfileDraft::new("seed2_shanghai", Region::Shanghai),
    enable_iroh: false,
  })?;

  sub_peer1.subscribe(
    origin.ctrl_addr(),
    "repo",
    sub_full_dir.path().to_path_buf(),
    TransferOpts::new(),
  )?;
  sub_peer2.subscribe(
    origin.ctrl_addr(),
    "repo",
    sub_part_dir.path().to_path_buf(),
    TransferOpts::new().with_subpath("sub"),
  )?;

  assert_eq!(fs::read(sub_full_dir.path().join("big.bin"))?, big_data);
  assert_eq!(
    fs::read(sub_part_dir.path().join("sub/part.txt"))?,
    b"v1-sub"
  );
  assert!(!sub_part_dir.path().join("big.bin").exists());

  let downloader = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Ephemeral,
    profile: ProfileDraft::new("dl_shanghai", Region::Shanghai),
    enable_iroh: false,
  })?;
  let rep = downloader.pull(
    origin.ctrl_addr(),
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
fn test_group_mutual_backup_full_and_prefix() -> Void {
  let dir1 = tempfile::tempdir()?;
  let dir2 = tempfile::tempdir()?;
  let dir3 = tempfile::tempdir()?;

  let p1 = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Ephemeral,
    profile: ProfileDraft::new("p1", Region::Shenzhen),
    enable_iroh: false,
  })?;
  let p2 = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Ephemeral,
    profile: ProfileDraft::new("p2", Region::HongKong),
    enable_iroh: false,
  })?;
  let p3 = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Ephemeral,
    profile: ProfileDraft::new("p3", Region::Taipei),
    enable_iroh: false,
  })?;

  let addrs = [p1.ctrl_addr(), p2.ctrl_addr(), p3.ctrl_addr()];
  p1.join_group("db-cluster", dir1.path().to_path_buf(), "", &addrs)?;
  p2.join_group("db-cluster", dir2.path().to_path_buf(), "", &addrs)?;
  p3.join_group("db-cluster", dir3.path().to_path_buf(), "wal", &addrs)?;
  p1.join_group("db-cluster", dir1.path().to_path_buf(), "", &addrs)?;
  p2.join_group("db-cluster", dir2.path().to_path_buf(), "", &addrs)?;

  fs::create_dir_all(dir1.path().join("wal"))?;
  fs::create_dir_all(dir1.path().join("cache"))?;
  fs::write(dir1.path().join("wal/01.log"), b"tx-commit-1")?;
  fs::write(dir1.path().join("cache/tmp.bin"), b"ephemeral-cache")?;
  p1.publish_updates("db-cluster")?;

  assert_eq!(fs::read(dir2.path().join("wal/01.log"))?, b"tx-commit-1");
  assert_eq!(
    fs::read(dir2.path().join("cache/tmp.bin"))?,
    b"ephemeral-cache"
  );
  assert_eq!(fs::read(dir3.path().join("wal/01.log"))?, b"tx-commit-1");
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
