use std::fs;

use aok::{OK, Void};
use ctor::ctor;
use log_init::init;
use s0_conf::{GroupUri, parse_nt_groups, parse_rate, parse_rate_str};
use tempfile::tempdir;

#[ctor(unsafe)]
fn _log_init() {
  init();
}

#[test]
fn test_parse_rate_units_and_decimals() -> Void {
  assert_eq!(parse_rate("0"), Some(0));
  assert_eq!(parse_rate("1024"), Some(1024));
  assert_eq!(parse_rate("512B"), Some(512));
  assert_eq!(parse_rate("64KB"), Some(64 * 1024));
  assert_eq!(parse_rate("64K"), Some(64 * 1024));
  assert_eq!(parse_rate("64KiB"), Some(64 * 1024));
  assert_eq!(parse_rate("10MB"), Some(10 * 1024 * 1024));
  assert_eq!(parse_rate("10m"), Some(10 * 1024 * 1024));
  assert_eq!(parse_rate("10MB/s"), Some(10 * 1024 * 1024));
  assert_eq!(parse_rate("10mbps"), Some(10 * 1024 * 1024));
  assert_eq!(parse_rate("2GB"), Some(2 * 1024 * 1024 * 1024));
  assert_eq!(parse_rate("1.5GB"), Some(1536 * 1024 * 1024));
  assert_eq!(parse_rate("2.5MB"), Some(2560 * 1024));
  assert_eq!(parse_rate("1TB"), Some(1024 * 1024 * 1024 * 1024));
  assert!(parse_rate_str("100MB").is_ok());
  assert!(parse_rate("").is_none());
  assert!(parse_rate("abc").is_none());
  assert!(parse_rate("10XB").is_none());
  OK
}

#[test]
fn test_parse_nt_groups_with_unit_rates_and_users_dir() -> Void {
  let cfg_dir = tempdir()?;
  let users_dir = cfg_dir.path().join("users");
  fs::create_dir_all(&users_dir)?;

  fs::write(
    users_dir.join("alice.nt"),
    "id: alice_id_token\nperm: rw\nrate: 50MB\nwrite_prefix:\n  - docs\n  - assets/icons\n",
  )?;
  fs::write(
    users_dir.join("bob.nt"),
    "id: bob_id_token\nperm: rw\nrate: 512KB\ngroups:\n  project:\n    perm: rw\n    rate: 1.5GB\n    write_prefix:\n      - assets/raw\n",
  )?;

  let nt_text = "users_dir: ./users\nproject:\n  path: /srv/project\n  auth: true\n  rate: 100MB\n  users:\n    - alice\n    - bob\n";
  let groups = parse_nt_groups(nt_text, Some(cfg_dir.path()))?;
  assert_eq!(groups.len(), 1);
  let g = &groups[0];
  assert_eq!(g.name, "project");
  assert_eq!(g.default_max_rate, 100 * 1024 * 1024);
  assert_eq!(g.grants.len(), 2);
  let alice = g.grants.iter().find(|x| x.id == "alice_id_token").unwrap();
  assert_eq!(alice.rate, 50 * 1024 * 1024);
  assert_eq!(alice.write_prefixes, vec!["assets/icons", "docs"]);
  let bob = g.grants.iter().find(|x| x.id == "bob_id_token").unwrap();
  assert_eq!(bob.rate, 1536 * 1024 * 1024);
  assert_eq!(bob.write_prefixes, vec!["assets/raw"]);

  let uri = GroupUri::parse("project://docs/arch, /assets/icons/").unwrap();
  assert_eq!(uri.group, "project");
  assert_eq!(uri.paths, vec!["docs/arch", "assets/icons"]);
  OK
}
