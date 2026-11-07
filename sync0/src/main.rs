use std::{
  env,
  net::{AddrParseError, SocketAddr},
  path::PathBuf,
  process::ExitCode,
  sync::Arc,
  thread,
  time::Duration,
};

use rusty_alloc_default as _;
use s0_const::Role;
use s0_peer::{KeyMode, NodeConfig, PeerError, PeerNode, ShareConfig, TransferOpts, encode_id_b64};
use s0_profile::{ProfileDraft, ProfileError};
use s0_region::Region;
use s0_udp::{UdpError, bench_recv_opt, bench_send_opt};

#[derive(Debug, thiserror::Error)]
enum CliError {
  #[error("用法: {0}")]
  Usage(&'static str),
  #[error(transparent)]
  Addr(#[from] AddrParseError),
  #[error(transparent)]
  Profile(#[from] ProfileError),
  #[error(transparent)]
  Peer(#[from] PeerError),
  #[error(transparent)]
  Udp(#[from] UdpError),
}

fn main() -> ExitCode {
  log_init::init();
  let args: Vec<String> = env::args().skip(1).collect();
  if let Err(e) = run(&args) {
    eprintln!("sync0 error: {e}");
    return ExitCode::FAILURE;
  }
  ExitCode::SUCCESS
}

fn run(args: &[String]) -> Result<(), CliError> {
  let Some(cmd) = args.first().map(String::as_str) else {
    print_usage();
    return Ok(());
  };
  let rest = &args[1..];

  match cmd {
    "bench-send" | "send" => {
      let pos = positional_args(rest);
      if pos.len() < 2 {
        return Err(CliError::Usage(
          "sync0 bench-send <file_or_dir> <peer_addr> [--rate <bps>] [--delete] [-q|--quiet]",
        ));
      }
      let path = PathBuf::from(pos[0]);
      let peer: SocketAddr = pos[1].parse()?;
      let rate = flag_u64(rest, "--rate").unwrap_or(0);
      let show = !has_flag(rest, "-q") && !has_flag(rest, "--quiet");
      let delete = has_flag(rest, "--delete");
      let st = bench_send_opt(&path, peer, rate, show, delete)?;
      println!("{}", st.line(Role::Send));
    }
    "bench-recv" | "recv" => {
      let pos = positional_args(rest);
      if pos.len() < 2 {
        return Err(CliError::Usage(
          "sync0 bench-recv <listen_addr> <out_path> [--delete] [-q|--quiet]",
        ));
      }
      let listen: SocketAddr = pos[0].parse()?;
      let out = PathBuf::from(pos[1]);
      let show = !has_flag(rest, "-q") && !has_flag(rest, "--quiet");
      let delete = has_flag(rest, "--delete");
      let st = bench_recv_opt(listen, &out, show, delete)?;
      println!("{}", st.line(Role::Recv));
    }
    "id" => {
      let mode = parse_key_mode(rest);
      let sk = mode.resolve_secret_key()?;
      let id_b64 = encode_id_b64(sk.public().as_bytes());
      println!("{id_b64}");
    }
    "profile" => {
      let mode = parse_key_mode(rest);
      let draft = parse_profile_draft(rest);
      let sk = mode.resolve_secret_key()?;
      let signed = draft.sign(&sk)?;
      println!("{signed}");
    }
    "share" => {
      if rest.len() < 2 {
        return Err(CliError::Usage(
          "sync0 share <share_name> <dir> [--rate <bps>] [--auth] [--name <user>] [--region <city>]",
        ));
      }
      let share_name = &rest[0];
      let dir = PathBuf::from(&rest[1]);
      let rate = flag_u64(rest, "--rate").unwrap_or(0);
      let auth = has_flag(rest, "--auth");
      let node = start_node(rest)?;
      let cfg = ShareConfig::public_ro(share_name, dir, rate).with_auth(auth);
      node.add_share(cfg);
      println!(
        "peer={} addr={} sharing={}",
        node.profile(),
        node.ctrl_addr(),
        share_name
      );
      park_forever();
    }
    "pull" => {
      if rest.len() < 3 {
        return Err(CliError::Usage(
          "sync0 pull <peer_addrs_csv> <share_name> <dst_dir> [--subpath <path>] [--delete] [--rate <bps>]",
        ));
      }
      let addrs = parse_addrs_csv(&rest[0])?;
      let share_name = &rest[1];
      let dst = PathBuf::from(&rest[2]);
      let opts = TransferOpts::new()
        .with_subpath(flag_str(rest, "--subpath").unwrap_or(""))
        .with_delete(has_flag(rest, "--delete"))
        .with_rate(flag_u64(rest, "--rate").unwrap_or(0));

      let node = start_node(rest)?;
      let rep = node.swarm_pull(&addrs, share_name, &dst, opts)?;
      println!(
        "pull ok: blocks={}/{} bytes={} peers={}",
        rep.blocks_transferred, rep.total_blocks, rep.bytes_transferred, rep.peers_used
      );
    }
    "push" => {
      if rest.len() < 3 {
        return Err(CliError::Usage(
          "sync0 push <peer_addr> <share_name> <src_dir> [--subpath <path>] [--delete] [--rate <bps>]",
        ));
      }
      let addr: SocketAddr = rest[0].parse()?;
      let share_name = &rest[1];
      let src = PathBuf::from(&rest[2]);
      let opts = TransferOpts::new()
        .with_subpath(flag_str(rest, "--subpath").unwrap_or(""))
        .with_delete(has_flag(rest, "--delete"))
        .with_rate(flag_u64(rest, "--rate").unwrap_or(0));

      let node = start_node(rest)?;
      let rep = node.push(addr, share_name, &src, opts)?;
      println!(
        "push ok: blocks={}/{} bytes={}",
        rep.blocks_transferred, rep.total_blocks, rep.bytes_transferred
      );
    }
    "sub" => {
      if rest.len() < 3 {
        return Err(CliError::Usage(
          "sync0 sub <origin_addr> <share_name> <local_dir> [--prefix <subpath>] [--rate <bps>]",
        ));
      }
      let addr: SocketAddr = rest[0].parse()?;
      let share_name = &rest[1];
      let dst = PathBuf::from(&rest[2]);
      let opts = TransferOpts::new()
        .with_subpath(flag_str(rest, "--prefix").unwrap_or(""))
        .with_rate(flag_u64(rest, "--rate").unwrap_or(0));

      let node = start_node(rest)?;
      let rep = node.subscribe(addr, share_name, dst, opts)?;
      println!(
        "sub ready & seeding: addr={} blocks={}",
        node.ctrl_addr(),
        rep.total_blocks
      );
      park_forever();
    }
    "group" => {
      if rest.len() < 3 {
        return Err(CliError::Usage(
          "sync0 group <group_name> <local_dir> <peer_addrs_csv> [--prefix <subpath>]",
        ));
      }
      let group_name = &rest[0];
      let dir = PathBuf::from(&rest[1]);
      let addrs = parse_addrs_csv(&rest[2])?;
      let prefix = flag_str(rest, "--prefix").unwrap_or("");

      let node = start_node(rest)?;
      node.join_group(group_name, dir, prefix, &addrs)?;
      println!(
        "joined group={} addr={} prefix={}",
        group_name,
        node.ctrl_addr(),
        prefix
      );
      park_forever();
    }
    _ => {
      print_usage();
    }
  }
  Ok(())
}

fn positional_args(args: &[String]) -> Vec<&str> {
  let mut out = Vec::new();
  let mut i = 0usize;
  while i < args.len() {
    let a = args[i].as_str();
    if matches!(
      a,
      "--rate"
        | "--config"
        | "--config-dir"
        | "-c"
        | "--name"
        | "--region"
        | "--email"
        | "--url"
        | "--bio"
        | "--subpath"
        | "--prefix"
        | "-u"
        | "--user"
    ) {
      i += 2;
      continue;
    }
    if a.starts_with('-') {
      i += 1;
      continue;
    }
    out.push(a);
    i += 1;
  }
  out
}

fn start_node(args: &[String]) -> Result<Arc<PeerNode>, CliError> {
  Ok(PeerNode::start(NodeConfig {
    key_mode: parse_key_mode(args),
    profile: parse_profile_draft(args),
    enable_iroh: false,
  })?)
}

fn park_forever() -> ! {
  loop {
    thread::sleep(Duration::from_secs(3600));
  }
}

fn parse_key_mode(args: &[String]) -> KeyMode {
  if let Some(dir) = flag_str(args, "--config").or_else(|| flag_str(args, "--config-dir")) {
    KeyMode::Persistent(PathBuf::from(dir))
  } else {
    KeyMode::Ephemeral
  }
}

fn parse_profile_draft(args: &[String]) -> ProfileDraft {
  let name = flag_str(args, "--name").unwrap_or("peer");
  let region = flag_str(args, "--region")
    .and_then(Region::parse_str)
    .unwrap_or(Region::Unknown);
  let mut d = ProfileDraft::new(name, region);
  if let Some(email) = flag_str(args, "--email") {
    d = d.with_email(email);
  }
  if let Some(url) = flag_str(args, "--url") {
    d = d.with_url(url);
  }
  if let Some(bio) = flag_str(args, "--bio") {
    d = d.with_bio(bio);
  }
  d
}

fn parse_addrs_csv(s: &str) -> Result<Vec<SocketAddr>, AddrParseError> {
  s.split(',')
    .filter(|p| !p.trim().is_empty())
    .map(|p| p.trim().parse())
    .collect()
}

fn flag_str<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
  args
    .windows(2)
    .find(|w| w[0] == flag)
    .map(|w| w[1].as_str())
}

fn flag_u64(args: &[String], flag: &str) -> Option<u64> {
  flag_str(args, flag).and_then(|v| v.parse().ok())
}

fn has_flag(args: &[String], flag: &str) -> bool {
  args.iter().any(|a| a == flag)
}

fn print_usage() {
  eprintln!(
    "sync0 — 对等文件同步与 Dragonfly P2P 分发系统\n\
     子命令:\n  \
       id         [--config <dir>]\n  \
       profile    --name <name> --region <city> [--email <e>] [--url <u>] [--bio <b>]\n  \
       bench-send <file_or_dir> <peer_addr> [--rate <bps>] [--delete] [-q|--quiet]\n  \
       bench-recv <listen_addr> <out_path> [--delete] [-q|--quiet]\n  \
       share      <share_name> <dir> [--rate <bps>] [--auth] [--name <user>] [--region <city>]\n  \
       pull       <peer_addrs_csv> <share_name> <dst_dir> [--subpath <path>] [--delete] [--rate <bps>]\n  \
       push       <peer_addr> <share_name> <src_dir> [--subpath <path>] [--delete] [--rate <bps>]\n  \
       sub        <origin_addr> <share_name> <local_dir> [--prefix <subpath>] [--rate <bps>]\n  \
       group      <group_name> <local_dir> <peer_addrs_csv> [--prefix <subpath>]"
  );
}
