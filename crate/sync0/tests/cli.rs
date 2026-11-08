use std::{fs, process::Command};

use aok::{OK, Void};
use ctor::ctor;
use log_init::init;
use s0_udp::{
  consts::Perm,
  peer::{KeyMode, NodeConfig, PeerNode, ShareConfig},
  profile::ProfileDraft,
  region::Region,
};
use tempfile::tempdir;

#[ctor(unsafe)]
fn _log_init() {
  init();
}

#[test]
fn test_cli_id_and_profile() -> Void {
  let bin = env!("CARGO_BIN_EXE_sync0");
  let temp_home = tempdir()?;

  // 默认无需 -c，自动使用 $HOME/.config/sync0
  let out_id = Command::new(bin)
    .env("HOME", temp_home.path())
    .args(["id"])
    .output()?;
  assert!(out_id.status.success());
  let id_str = String::from_utf8(out_id.stdout)?.trim().to_owned();
  assert_eq!(id_str.len(), 43);
  assert!(temp_home.path().join(".config/sync0/iroh.key").is_file());

  let out_prof = Command::new(bin)
    .env("HOME", temp_home.path())
    .args([
      "profile",
      "--name",
      "dev_node",
      "--region",
      "SHA",
      "--email",
      "dev@sync0.io",
      "--url",
      "https://sync0.io",
      "--motto",
      "Zero-copy P2P node",
    ])
    .output()?;
  assert!(out_prof.status.success());
  let prof_str = String::from_utf8(out_prof.stdout)?;
  assert!(
    prof_str
      .contains("dev_node [上海/SHA] <dev@sync0.io> (https://sync0.io) \"Zero-copy P2P node\"")
  );
  OK
}

#[test]
fn test_cli_pull_and_push_with_base64url_iroh_and_group_uri() -> Void {
  let bin = env!("CARGO_BIN_EXE_sync0");
  let share_dir = tempdir()?;
  let pull_dst = tempdir()?;
  let push_src = tempdir()?;
  let client_cfg = tempdir()?;

  fs::create_dir_all(share_dir.path().join("docs"))?;
  fs::create_dir_all(share_dir.path().join("pkg"))?;
  fs::write(share_dir.path().join("docs/guide.md"), b"cli-guide-v1")?;
  fs::write(share_dir.path().join("pkg/lib.rs"), b"pub fn ok() {}")?;
  fs::write(share_dir.path().join("skip.bin"), b"ignored")?;

  let out_client_id = Command::new(bin)
    .env("HOME", client_cfg.path())
    .args(["id"])
    .output()?;
  assert!(out_client_id.status.success());
  let client_id_b64 = String::from_utf8(out_client_id.stdout)?.trim().to_owned();

  let server = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Ephemeral,
    profile: ProfileDraft::new("srv_node", Region::Shanghai),
    ..Default::default()
  })?;
  let cfg = ShareConfig::public_ro("team-repo", share_dir.path(), 0)
    .grant_peer_b64(&client_id_b64, Perm::Rw, 0, ["uploads/client"])
    .unwrap();
  server.add_share(cfg);
  let srv_iroh = server.iroh_addr_b64();

  // 1. CLI 多选拉取：`sync0 pull team-repo://docs/guide.md,pkg <dst> -p <iroh_b64>`
  let out_pull = Command::new(bin)
    .env("HOME", client_cfg.path())
    .args([
      "pull",
      "team-repo://docs/guide.md,pkg",
      pull_dst.path().to_str().unwrap(),
      "-p",
      &srv_iroh,
    ])
    .output()?;
  assert!(out_pull.status.success());
  assert_eq!(
    fs::read(pull_dst.path().join("docs/guide.md"))?,
    b"cli-guide-v1"
  );
  assert_eq!(
    fs::read(pull_dst.path().join("pkg/lib.rs"))?,
    b"pub fn ok() {}"
  );
  assert!(!pull_dst.path().join("skip.bin").exists());

  // 2. CLI 按允许路径前缀上传（默认读取 $HOME/.config/sync0 密钥，无需 -c）
  let note_file = push_src.path().join("note.txt");
  fs::write(&note_file, b"uploaded-by-client")?;
  let out_push_ok = Command::new(bin)
    .env("HOME", client_cfg.path())
    .args([
      "push",
      "team-repo://uploads/client/note.txt",
      note_file.to_str().unwrap(),
      "-p",
      &srv_iroh,
    ])
    .output()?;
  assert!(out_push_ok.status.success());
  assert_eq!(
    fs::read(share_dir.path().join("uploads/client/note.txt"))?,
    b"uploaded-by-client"
  );

  // 3. CLI 越权上传到非授权路径前缀被拒绝
  let out_push_deny = Command::new(bin)
    .env("HOME", client_cfg.path())
    .args([
      "push",
      "team-repo://uploads/other/hack.txt",
      note_file.to_str().unwrap(),
      "-p",
      &srv_iroh,
    ])
    .output()?;
  assert!(!out_push_deny.status.success());

  // 4. 对等种子节点发现与全对称做种（每个节点既是服务器也是客户端）：
  // 节点 B 加入节点 A 的分享组后自动成为做种服务器，节点 C 从 A 或 B 均可获取全部可用种子节点并并发拉取
  let peer_b_dir = tempdir()?;
  let peer_b = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Ephemeral,
    profile: ProfileDraft::new("peer_b", Region::Shanghai),
    ..Default::default()
  })?;
  peer_b.join_group(
    "team-repo",
    peer_b_dir.path().to_path_buf(),
    &[],
    &[server.endpoint_id()],
  )?;

  let peer_c_dir = tempdir()?;
  let peer_c = PeerNode::start(NodeConfig {
    key_mode: KeyMode::Ephemeral,
    profile: ProfileDraft::new("peer_c", Region::Beijing),
    ..Default::default()
  })?;
  let seeds_from_a = peer_c.discover_seeds(server.endpoint_id(), "team-repo")?;
  assert!(
    seeds_from_a
      .iter()
      .any(|s| s.endpoint_id == server.endpoint_id())
  );
  assert!(
    seeds_from_a
      .iter()
      .any(|s| s.endpoint_id == peer_b.endpoint_id())
  );

  let seeds_from_b = peer_c.discover_seeds(peer_b.endpoint_id(), "team-repo")?;
  assert!(
    seeds_from_b
      .iter()
      .any(|s| s.endpoint_id == server.endpoint_id())
  );
  assert!(
    seeds_from_b
      .iter()
      .any(|s| s.endpoint_id == peer_b.endpoint_id())
  );

  let rep_c = peer_c.pull(
    peer_b.endpoint_id(),
    "team-repo",
    peer_c_dir.path(),
    Default::default(),
  )?;
  assert!(rep_c.total_blocks > 0);
  assert_eq!(
    fs::read(peer_c_dir.path().join("docs/guide.md"))?,
    b"cli-guide-v1"
  );

  OK
}
