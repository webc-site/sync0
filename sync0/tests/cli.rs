use std::process::Command;

use aok::{OK, Void};

#[ctor::ctor(unsafe)]
fn _log_init() {
  log_init::init();
}

#[test]
fn test_cli_id_and_profile() -> Void {
  let bin = env!("CARGO_BIN_EXE_sync0");
  let cfg_dir = tempfile::tempdir()?;

  let out_id = Command::new(bin)
    .args(["id", "--config", cfg_dir.path().to_str().unwrap()])
    .output()?;
  assert!(out_id.status.success());
  let id_str = String::from_utf8(out_id.stdout)?.trim().to_owned();
  assert_eq!(id_str.len(), 43);

  let out_prof = Command::new(bin)
    .args([
      "profile",
      "--config",
      cfg_dir.path().to_str().unwrap(),
      "--name",
      "dev_node",
      "--region",
      "SHA",
      "--email",
      "dev@sync0.io",
      "--url",
      "https://sync0.io",
    ])
    .output()?;
  assert!(out_prof.status.success());
  let prof_str = String::from_utf8(out_prof.stdout)?;
  assert!(prof_str.contains("dev_node [上海/SHA] <dev@sync0.io> (https://sync0.io)"));
  OK
}
