use std::{
  net::{AddrParseError, SocketAddr},
  path::PathBuf,
  process::ExitCode,
  sync::Arc,
  thread,
  time::Duration,
};

use clap_args::clap::{Arg, ArgAction, ArgMatches, Command, arg, value_parser};
use rusty_alloc_default as _;
use s0_i18n::{I18n, t};
use s0_udp::{
  Role, UdpError, bench_recv_opt, bench_send_opt,
  s0_peer::{KeyMode, NodeConfig, PeerError, PeerNode, ShareConfig, TransferOpts, encode_id_b64},
  s0_profile::{ProfileDraft, ProfileError},
  s0_region::Region,
};

#[derive(Debug, thiserror::Error)]
enum CliError {
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
  if let Err(e) = run() {
    let i18n = t();
    eprintln!("sync0 {}: {e}", i18n.err_prefix);
    return ExitCode::FAILURE;
  }
  ExitCode::SUCCESS
}

fn run() -> Result<(), CliError> {
  let i18n = t();
  let m = clap_args::parse!(|cmd| {
    cmd
      .about(i18n.about)
      .arg_required_else_help(true)
      .subcommand(bench_send_cmd("bench-send", i18n))
      .subcommand(bench_send_cmd("send", i18n))
      .subcommand(bench_recv_cmd("bench-recv", i18n))
      .subcommand(bench_recv_cmd("recv", i18n))
      .subcommand(node_args(Command::new("id").about(i18n.id_about), i18n))
      .subcommand(node_args(
        Command::new("profile").about(i18n.profile_about),
        i18n,
      ))
      .subcommand(node_args(
        Command::new("share")
          .about(i18n.share_about)
          .arg(arg!(<share_name>).help(i18n.arg_share_name))
          .arg(
            arg!(<dir>)
              .help(i18n.arg_local_dir)
              .value_parser(value_parser!(PathBuf)),
          )
          .arg(
            arg!(--rate <bps>)
              .help(i18n.arg_rate)
              .value_parser(value_parser!(u64))
              .default_value("0"),
          )
          .arg(arg!(--auth).help(i18n.arg_auth).action(ArgAction::SetTrue)),
        i18n,
      ))
      .subcommand(node_args(
        Command::new("pull")
          .about(i18n.pull_about)
          .arg(arg!(<peer_addrs_csv>).help(i18n.arg_peer_addrs_csv))
          .arg(arg!(<share_name>).help(i18n.arg_share_name))
          .arg(
            arg!(<dst_dir>)
              .help(i18n.arg_dst_dir)
              .value_parser(value_parser!(PathBuf)),
          )
          .arg(
            arg!(--subpath <path>)
              .help(i18n.arg_subpath)
              .default_value(""),
          )
          .arg(
            arg!(--delete)
              .help(i18n.arg_delete)
              .action(ArgAction::SetTrue),
          )
          .arg(
            arg!(--rate <bps>)
              .help(i18n.arg_rate)
              .value_parser(value_parser!(u64))
              .default_value("0"),
          ),
        i18n,
      ))
      .subcommand(node_args(
        Command::new("push")
          .about(i18n.push_about)
          .arg(
            arg!(<peer_addr>)
              .help(i18n.arg_peer_addr)
              .value_parser(value_parser!(SocketAddr)),
          )
          .arg(arg!(<share_name>).help(i18n.arg_share_name))
          .arg(
            arg!(<src_dir>)
              .help(i18n.arg_src_dir)
              .value_parser(value_parser!(PathBuf)),
          )
          .arg(
            arg!(--subpath <path>)
              .help(i18n.arg_subpath)
              .default_value(""),
          )
          .arg(
            arg!(--delete)
              .help(i18n.arg_delete)
              .action(ArgAction::SetTrue),
          )
          .arg(
            arg!(--rate <bps>)
              .help(i18n.arg_rate)
              .value_parser(value_parser!(u64))
              .default_value("0"),
          ),
        i18n,
      ))
      .subcommand(node_args(
        Command::new("sub")
          .about(i18n.sub_about)
          .arg(
            arg!(<origin_addr>)
              .help(i18n.arg_origin_addr)
              .value_parser(value_parser!(SocketAddr)),
          )
          .arg(arg!(<share_name>).help(i18n.arg_share_name))
          .arg(
            arg!(<local_dir>)
              .help(i18n.arg_local_dir)
              .value_parser(value_parser!(PathBuf)),
          )
          .arg(
            arg!(--prefix <subpath>)
              .help(i18n.arg_subpath)
              .default_value(""),
          )
          .arg(
            arg!(--rate <bps>)
              .help(i18n.arg_rate)
              .value_parser(value_parser!(u64))
              .default_value("0"),
          ),
        i18n,
      ))
      .subcommand(node_args(
        Command::new("group")
          .about(i18n.group_about)
          .arg(arg!(<group_name>).help(i18n.arg_group_name))
          .arg(
            arg!(<local_dir>)
              .help(i18n.arg_local_dir)
              .value_parser(value_parser!(PathBuf)),
          )
          .arg(arg!(<peer_addrs_csv>).help(i18n.arg_peer_addrs_csv))
          .arg(
            arg!(--prefix <subpath>)
              .help(i18n.arg_subpath)
              .default_value(""),
          ),
        i18n,
      ))
  });
  let Some(m) = m else {
    return Ok(());
  };

  match m.subcommand() {
    Some(("bench-send" | "send", sub)) => {
      let path = sub.get_one::<PathBuf>("file_or_dir").unwrap();
      let peer = *sub.get_one::<SocketAddr>("peer_addr").unwrap();
      let rate = *sub.get_one::<u64>("rate").unwrap();
      let show = !sub.get_flag("quiet");
      let delete = sub.get_flag("delete");
      let st = bench_send_opt(path, peer, rate, show, delete)?;
      println!("{}", st.line(Role::Send));
    }
    Some(("bench-recv" | "recv", sub)) => {
      let listen = *sub.get_one::<SocketAddr>("listen_addr").unwrap();
      let out = sub.get_one::<PathBuf>("out_path").unwrap();
      let show = !sub.get_flag("quiet");
      let delete = sub.get_flag("delete");
      let st = bench_recv_opt(listen, out, show, delete)?;
      println!("{}", st.line(Role::Recv));
    }
    Some(("id", sub)) => {
      let mode = parse_key_mode(sub);
      let sk = mode.resolve_secret_key()?;
      let id_b64 = encode_id_b64(sk.public().as_bytes());
      println!("{id_b64}");
    }
    Some(("profile", sub)) => {
      let mode = parse_key_mode(sub);
      let draft = parse_profile_draft(sub);
      let sk = mode.resolve_secret_key()?;
      let signed = draft.sign(&sk)?;
      println!("{signed}");
    }
    Some(("share", sub)) => {
      let share_name = sub.get_one::<String>("share_name").unwrap();
      let dir = sub.get_one::<PathBuf>("dir").unwrap().clone();
      let rate = *sub.get_one::<u64>("rate").unwrap();
      let auth = sub.get_flag("auth");
      let node = start_node(sub)?;
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
    Some(("pull", sub)) => {
      let addrs = parse_addrs_csv(sub.get_one::<String>("peer_addrs_csv").unwrap())?;
      let share_name = sub.get_one::<String>("share_name").unwrap();
      let dst = sub.get_one::<PathBuf>("dst_dir").unwrap();
      let opts = TransferOpts::new()
        .with_subpath(sub.get_one::<String>("subpath").unwrap())
        .with_delete(sub.get_flag("delete"))
        .with_rate(*sub.get_one::<u64>("rate").unwrap());

      let node = start_node(sub)?;
      let rep = node.swarm_pull(&addrs, share_name, dst, opts)?;
      println!(
        "pull ok: blocks={}/{} bytes={} peers={}",
        rep.blocks_transferred, rep.total_blocks, rep.bytes_transferred, rep.peers_used
      );
    }
    Some(("push", sub)) => {
      let addr = *sub.get_one::<SocketAddr>("peer_addr").unwrap();
      let share_name = sub.get_one::<String>("share_name").unwrap();
      let src = sub.get_one::<PathBuf>("src_dir").unwrap();
      let opts = TransferOpts::new()
        .with_subpath(sub.get_one::<String>("subpath").unwrap())
        .with_delete(sub.get_flag("delete"))
        .with_rate(*sub.get_one::<u64>("rate").unwrap());

      let node = start_node(sub)?;
      let rep = node.push(addr, share_name, src, opts)?;
      println!(
        "push ok: blocks={}/{} bytes={}",
        rep.blocks_transferred, rep.total_blocks, rep.bytes_transferred
      );
    }
    Some(("sub", sub)) => {
      let addr = *sub.get_one::<SocketAddr>("origin_addr").unwrap();
      let share_name = sub.get_one::<String>("share_name").unwrap();
      let dst = sub.get_one::<PathBuf>("local_dir").unwrap().clone();
      let opts = TransferOpts::new()
        .with_subpath(sub.get_one::<String>("prefix").unwrap())
        .with_rate(*sub.get_one::<u64>("rate").unwrap());

      let node = start_node(sub)?;
      let rep = node.subscribe(addr, share_name, dst, opts)?;
      println!(
        "sub ready & seeding: addr={} blocks={}",
        node.ctrl_addr(),
        rep.total_blocks
      );
      park_forever();
    }
    Some(("group", sub)) => {
      let group_name = sub.get_one::<String>("group_name").unwrap();
      let dir = sub.get_one::<PathBuf>("local_dir").unwrap().clone();
      let addrs = parse_addrs_csv(sub.get_one::<String>("peer_addrs_csv").unwrap())?;
      let prefix = sub.get_one::<String>("prefix").unwrap();

      let node = start_node(sub)?;
      node.join_group(group_name, dir, prefix, &addrs)?;
      println!(
        "joined group={} addr={} prefix={}",
        group_name,
        node.ctrl_addr(),
        prefix
      );
      park_forever();
    }
    _ => {}
  }
  Ok(())
}

fn bench_send_cmd(name: &'static str, i18n: &'static I18n) -> Command {
  Command::new(name)
    .about(i18n.send_about)
    .arg(
      arg!(<file_or_dir>)
        .help(i18n.arg_file_or_dir)
        .value_parser(value_parser!(PathBuf)),
    )
    .arg(
      arg!(<peer_addr>)
        .help(i18n.arg_peer_addr)
        .value_parser(value_parser!(SocketAddr)),
    )
    .arg(
      arg!(--rate <bps>)
        .help(i18n.arg_rate)
        .value_parser(value_parser!(u64))
        .default_value("0"),
    )
    .arg(
      arg!(--delete)
        .help(i18n.arg_delete)
        .action(ArgAction::SetTrue),
    )
    .arg(
      arg!(-q - -quiet)
        .help(i18n.arg_quiet)
        .action(ArgAction::SetTrue),
    )
}

fn bench_recv_cmd(name: &'static str, i18n: &'static I18n) -> Command {
  Command::new(name)
    .about(i18n.recv_about)
    .arg(
      arg!(<listen_addr>)
        .help(i18n.arg_listen_addr)
        .value_parser(value_parser!(SocketAddr)),
    )
    .arg(
      arg!(<out_path>)
        .help(i18n.arg_out_path)
        .value_parser(value_parser!(PathBuf)),
    )
    .arg(
      arg!(--delete)
        .help(i18n.arg_delete)
        .action(ArgAction::SetTrue),
    )
    .arg(
      arg!(-q - -quiet)
        .help(i18n.arg_quiet)
        .action(ArgAction::SetTrue),
    )
}

fn node_args(cmd: Command, i18n: &'static I18n) -> Command {
  cmd
    .arg(
      Arg::new("config")
        .short('c')
        .long("config")
        .visible_alias("config-dir")
        .value_name("dir")
        .help(i18n.arg_config)
        .value_parser(value_parser!(PathBuf)),
    )
    .arg(
      Arg::new("name")
        .short('u')
        .long("name")
        .visible_alias("user")
        .value_name("user")
        .default_value("peer")
        .help(i18n.arg_name),
    )
    .arg(arg!(--region <city>).help(i18n.arg_region))
    .arg(arg!(--email <email>).help(i18n.arg_email))
    .arg(arg!(--url <url>).help(i18n.arg_url))
    .arg(
      Arg::new("bio")
        .short('b')
        .long("bio")
        .visible_aliases(["motto", "sign"])
        .value_name("bio")
        .help(i18n.arg_bio),
    )
}

fn start_node(m: &ArgMatches) -> Result<Arc<PeerNode>, CliError> {
  Ok(PeerNode::start(NodeConfig {
    key_mode: parse_key_mode(m),
    profile: parse_profile_draft(m),
    enable_iroh: false,
  })?)
}

fn park_forever() -> ! {
  loop {
    thread::sleep(Duration::from_secs(3600));
  }
}

fn parse_key_mode(m: &ArgMatches) -> KeyMode {
  if let Some(dir) = m.get_one::<PathBuf>("config") {
    KeyMode::Persistent(dir.clone())
  } else {
    KeyMode::Ephemeral
  }
}

fn parse_profile_draft(m: &ArgMatches) -> ProfileDraft {
  let name = m.get_one::<String>("name").map_or("peer", String::as_str);
  let region = m
    .get_one::<String>("region")
    .and_then(|s| Region::parse_str(s))
    .unwrap_or(Region::Unknown);
  let mut d = ProfileDraft::new(name, region);
  if let Some(email) = m.get_one::<String>("email") {
    d = d.with_email(email);
  }
  if let Some(url) = m.get_one::<String>("url") {
    d = d.with_url(url);
  }
  if let Some(bio) = m.get_one::<String>("bio") {
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
