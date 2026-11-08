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
use s0_conf::{GroupUri, parse_rate_str};
use s0_i18n::{I18n, t};
use s0_udp::{
  UdpError,
  bench::{recv_opt as bench_recv_opt, send_opt as bench_send_opt},
  consts::{Perm, Role},
  peer::{
    KeyMode, NodeConfig, PeerError, PeerId, PeerNode, ShareConfig, TransferOpts, active_iroh_peers,
    decode_id_b64, default_config_dir, encode_id_b64, parse_iroh_ids_csv, resolve_iroh_endpoint,
  },
  profile::{ProfileDraft, ProfileError},
  region::Region,
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
          .arg(Arg::new("specs").help(i18n.arg_share_name).num_args(0..))
          .arg(
            arg!(--rate <bps>)
              .help(i18n.arg_rate)
              .value_parser(parse_rate_str)
              .default_value("0"),
          )
          .arg(arg!(--auth).help(i18n.arg_auth).action(ArgAction::SetTrue))
          .arg(arg!(--rw).help(i18n.arg_auth).action(ArgAction::SetTrue))
          .arg(
            Arg::new("allow_ro")
              .long("allow-ro")
              .value_name("iroh_b64_csv")
              .help(i18n.arg_peer_addrs_csv)
              .action(ArgAction::Append),
          )
          .arg(
            Arg::new("allow_rw")
              .long("allow-rw")
              .value_name("iroh_b64[:prefix_csv]")
              .help(i18n.arg_peer_addrs_csv)
              .action(ArgAction::Append),
          ),
        i18n,
      ))
      .subcommand(node_args(
        Command::new("pull")
          .about(i18n.pull_about)
          .arg(Arg::new("arg1").required(true).help(i18n.arg_share_name))
          .arg(Arg::new("arg2").required(true).help(i18n.arg_dst_dir))
          .arg(Arg::new("arg3").help(i18n.arg_dst_dir))
          .arg(
            Arg::new("peer")
              .short('p')
              .long("peer")
              .value_name("iroh_b64_csv")
              .help(i18n.arg_peer_addrs_csv),
          )
          .arg(
            Arg::new("subpath")
              .short('s')
              .long("subpath")
              .visible_alias("select")
              .value_name("paths_csv")
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
              .value_parser(parse_rate_str)
              .default_value("0"),
          ),
        i18n,
      ))
      .subcommand(node_args(
        Command::new("push")
          .about(i18n.push_about)
          .arg(Arg::new("arg1").required(true).help(i18n.arg_share_name))
          .arg(Arg::new("arg2").required(true).help(i18n.arg_src_dir))
          .arg(Arg::new("arg3").help(i18n.arg_src_dir))
          .arg(
            Arg::new("peer")
              .short('p')
              .long("peer")
              .value_name("iroh_b64")
              .help(i18n.arg_peer_addr),
          )
          .arg(
            Arg::new("subpath")
              .short('s')
              .long("subpath")
              .visible_alias("select")
              .value_name("paths_csv")
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
              .value_parser(parse_rate_str)
              .default_value("0"),
          ),
        i18n,
      ))
      .subcommand(node_args(
        Command::new("sub")
          .about(i18n.sub_about)
          .arg(Arg::new("arg1").required(true).help(i18n.arg_share_name))
          .arg(Arg::new("arg2").required(true).help(i18n.arg_local_dir))
          .arg(Arg::new("arg3").help(i18n.arg_local_dir))
          .arg(
            Arg::new("peer")
              .short('p')
              .long("peer")
              .value_name("iroh_b64")
              .help(i18n.arg_origin_addr),
          )
          .arg(
            Arg::new("prefix")
              .short('s')
              .long("prefix")
              .visible_aliases(["select", "subpath"])
              .value_name("paths_csv")
              .help(i18n.arg_subpath)
              .default_value(""),
          )
          .arg(
            arg!(--rate <bps>)
              .help(i18n.arg_rate)
              .value_parser(parse_rate_str)
              .default_value("0"),
          ),
        i18n,
      ))
      .subcommand(node_args(
        Command::new("group")
          .visible_alias("join")
          .about(i18n.group_about)
          .arg(
            Arg::new("group_uri")
              .required(true)
              .help(i18n.arg_group_name),
          )
          .arg(
            arg!(<local_dir>)
              .help(i18n.arg_local_dir)
              .value_parser(value_parser!(PathBuf)),
          )
          .arg(Arg::new("peer_addrs_csv").help(i18n.arg_peer_addrs_csv))
          .arg(
            Arg::new("peer")
              .short('p')
              .long("peer")
              .value_name("iroh_b64_csv")
              .help(i18n.arg_peer_addrs_csv),
          )
          .arg(
            Arg::new("prefix")
              .short('s')
              .long("prefix")
              .visible_aliases(["select", "subpath"])
              .value_name("paths_csv")
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
      let peer_str = sub.get_one::<String>("peer_addr").unwrap();
      let peer = resolve_send_target(peer_str)?;
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
      let rate = *sub.get_one::<u64>("rate").unwrap();
      let auth = sub.get_flag("auth");
      let is_rw = sub.get_flag("rw");
      let node = start_node(sub)?;

      let specs: Vec<&String> = sub
        .get_many::<String>("specs")
        .into_iter()
        .flatten()
        .collect();
      let mut shared_names = Vec::new();
      for pair in specs.chunks(2) {
        if pair.len() == 2 && !pair[0].contains(':') {
          let name = pair[0].as_str();
          let dir = PathBuf::from(pair[1].as_str());
          let cfg = build_cli_share_cfg(name, dir, rate, auth, is_rw, sub)?;
          shared_names.push(name.to_owned());
          node.add_share(cfg);
        } else {
          for item in pair {
            if let Some((name, dir_str)) = item.split_once(':') {
              let cfg = build_cli_share_cfg(
                name.trim(),
                PathBuf::from(dir_str.trim()),
                rate,
                auth,
                is_rw,
                sub,
              )?;
              shared_names.push(name.trim().to_owned());
              node.add_share(cfg);
            }
          }
        }
      }

      println!(
        "peer={} iroh={} sharing={}",
        node.profile(),
        node.iroh_addr_b64(),
        shared_names.join(",")
      );
      park_forever();
    }
    Some(("pull", sub)) => {
      let (peers, mut uri, dst) = parse_peer_uri_path_args(sub)?;
      uri.extend_paths_csv(sub.get_one::<String>("subpath").unwrap());
      let opts = TransferOpts::new()
        .with_paths(&uri.paths)
        .with_delete(sub.get_flag("delete"))
        .with_rate(*sub.get_one::<u64>("rate").unwrap());

      let node = start_node(sub)?;
      let rep = node.swarm_pull(&peers, &uri.group, &dst, opts)?;
      println!(
        "pull ok: group={} blocks={}/{} bytes={} peers={}",
        uri.group, rep.blocks_transferred, rep.total_blocks, rep.bytes_transferred, rep.peers_used
      );
    }
    Some(("push", sub)) => {
      let (peers, mut uri, src) = parse_peer_uri_path_args(sub)?;
      let peer_id = *peers.first().ok_or(PeerError::NoHealthyPeer)?;
      uri.extend_paths_csv(sub.get_one::<String>("subpath").unwrap());
      let opts = TransferOpts::new()
        .with_paths(&uri.paths)
        .with_delete(sub.get_flag("delete"))
        .with_rate(*sub.get_one::<u64>("rate").unwrap());

      let node = start_node(sub)?;
      let rep = node.push(peer_id, &uri.group, &src, opts)?;
      println!(
        "push ok: group={} blocks={}/{} bytes={}",
        uri.group, rep.blocks_transferred, rep.total_blocks, rep.bytes_transferred
      );
    }
    Some(("sub", sub)) => {
      let (peers, mut uri, dst) = parse_peer_uri_path_args(sub)?;
      let origin_id = *peers.first().ok_or(PeerError::NoHealthyPeer)?;
      uri.extend_paths_csv(sub.get_one::<String>("prefix").unwrap());
      let opts = TransferOpts::new()
        .with_paths(&uri.paths)
        .with_rate(*sub.get_one::<u64>("rate").unwrap());

      let node = start_node(sub)?;
      let rep = node.subscribe(origin_id, &uri.group, dst, opts)?;
      println!(
        "sub ready & seeding: group={} iroh={} blocks={}",
        uri.group,
        node.iroh_addr_b64(),
        rep.total_blocks
      );
      park_forever();
    }
    Some(("group", sub)) => {
      let raw_uri = sub.get_one::<String>("group_uri").unwrap();
      let mut uri = GroupUri::parse(raw_uri).ok_or(PeerError::BadAddress)?;
      uri.extend_paths_csv(sub.get_one::<String>("prefix").unwrap());
      let dir = sub.get_one::<PathBuf>("local_dir").unwrap().clone();

      let peers = if let Some(csv) = sub
        .get_one::<String>("peer")
        .or_else(|| sub.get_one::<String>("peer_addrs_csv"))
      {
        parse_iroh_ids_csv(csv).ok_or(PeerError::BadAddress)?
      } else {
        active_iroh_peers()
      };

      let node = start_node(sub)?;
      node.join_group(&uri.group, dir, &uri.paths, &peers)?;
      println!(
        "joined group={} iroh={} paths={}",
        uri.group,
        node.iroh_addr_b64(),
        uri.paths.join(",")
      );
      park_forever();
    }
    _ => {}
  }
  Ok(())
}

fn build_cli_share_cfg(
  name: &str,
  dir: PathBuf,
  rate: u64,
  auth: bool,
  is_rw: bool,
  sub: &ArgMatches,
) -> Result<ShareConfig, PeerError> {
  let mut cfg = if is_rw {
    ShareConfig::public_rw(name, dir, rate)
  } else {
    ShareConfig::public_ro(name, dir, rate)
  }
  .with_auth(auth);

  if let Some(ro_list) = sub.get_many::<String>("allow_ro") {
    for csv in ro_list {
      for item in csv.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let pid = decode_id_b64(item).ok_or(PeerError::BadAddress)?;
        cfg = cfg.grant_peer(pid, Perm::Ro, rate);
      }
    }
  }
  if let Some(rw_list) = sub.get_many::<String>("allow_rw") {
    for spec in rw_list {
      let (id_part, prefix_csv) = spec.split_once(':').unwrap_or((spec.as_str(), ""));
      let pid = decode_id_b64(id_part.trim()).ok_or(PeerError::BadAddress)?;
      let prefixes: Vec<&str> = prefix_csv
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
      cfg = cfg.grant_peer_with_prefixes(pid, Perm::Rw, rate, prefixes);
    }
  }
  Ok(cfg)
}

fn parse_peer_uri_path_args(
  sub: &ArgMatches,
) -> Result<(Vec<PeerId>, GroupUri, PathBuf), PeerError> {
  let a1 = sub.get_one::<String>("arg1").unwrap();
  let a2 = sub.get_one::<String>("arg2").unwrap();
  if let Some(a3) = sub.get_one::<String>("arg3") {
    let peers = parse_iroh_ids_csv(a1).ok_or(PeerError::BadAddress)?;
    let uri = GroupUri::parse(a2).ok_or(PeerError::BadAddress)?;
    Ok((peers, uri, PathBuf::from(a3)))
  } else {
    let uri = GroupUri::parse(a1).ok_or(PeerError::BadAddress)?;
    let peers = if let Some(csv) = sub.get_one::<String>("peer") {
      parse_iroh_ids_csv(csv).ok_or(PeerError::BadAddress)?
    } else {
      active_iroh_peers()
    };
    Ok((peers, uri, PathBuf::from(a2)))
  }
}

fn resolve_send_target(s: &str) -> Result<SocketAddr, CliError> {
  if let Some(pid) = decode_id_b64(s)
    && let Some(addr) = resolve_iroh_endpoint(&pid)
  {
    return Ok(addr);
  }
  Ok(s.parse::<SocketAddr>()?)
}

fn bench_send_cmd(name: &'static str, i18n: &'static I18n) -> Command {
  Command::new(name)
    .about(i18n.send_about)
    .arg(
      arg!(<file_or_dir>)
        .help(i18n.arg_file_or_dir)
        .value_parser(value_parser!(PathBuf)),
    )
    .arg(arg!(<peer_addr>).help(i18n.arg_peer_addr))
    .arg(
      arg!(--rate <bps>)
        .help(i18n.arg_rate)
        .value_parser(parse_rate_str)
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
      Arg::new("listen")
        .short('l')
        .long("listen")
        .value_name("addr")
        .help(i18n.arg_listen_addr)
        .value_parser(value_parser!(SocketAddr)),
    )
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
    bind_addr: m.get_one::<SocketAddr>("listen").copied(),
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
  } else if let Some(dir) = default_config_dir() {
    KeyMode::Persistent(dir)
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
