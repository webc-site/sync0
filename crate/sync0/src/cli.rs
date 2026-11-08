use std::{
  net::{AddrParseError, SocketAddr},
  path::PathBuf,
  sync::Arc,
  thread,
  time::Duration,
};

use clap_args::clap::{Arg, ArgAction, ArgMatches, Command, arg, value_parser};
use s0_conf::{GroupUri, parse_rate_str};
use s0_i18n::I18n;
use s0_udp::{
  UdpError,
  consts::Perm,
  peer::{
    KeyMode, NodeConfig, PeerError, PeerId, PeerNode, ShareConfig, active_iroh_peers,
    decode_id_b64, default_config_dir, parse_iroh_ids_csv, resolve_iroh_endpoint,
  },
  profile::{ProfileDraft, ProfileError},
  region::Region,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum CliError {
  #[error(transparent)]
  Addr(#[from] AddrParseError),
  #[error(transparent)]
  Profile(#[from] ProfileError),
  #[error(transparent)]
  Peer(#[from] PeerError),
  #[error(transparent)]
  Udp(#[from] UdpError),
}

pub(crate) fn build_cli(cmd: Command, i18n: &'static I18n) -> Command {
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
        )
        .arg(
          Arg::new("offset")
            .long("offset")
            .value_name("bytes")
            .value_parser(value_parser!(u64)),
        )
        .arg(
          Arg::new("len")
            .long("len")
            .value_name("bytes")
            .value_parser(value_parser!(u64)),
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
}

pub(crate) fn build_cli_share_cfg(
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

pub(crate) fn parse_peer_uri_path_args(
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

pub(crate) fn resolve_send_target(s: &str) -> Result<SocketAddr, CliError> {
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

pub(crate) fn start_node(m: &ArgMatches) -> Result<Arc<PeerNode>, CliError> {
  Ok(PeerNode::start(NodeConfig {
    key_mode: parse_key_mode(m),
    profile: parse_profile_draft(m),
    bind_addr: m.get_one::<SocketAddr>("listen").copied(),
    enable_iroh: false,
  })?)
}

pub(crate) fn park_forever() -> ! {
  loop {
    thread::sleep(Duration::from_secs(3600));
  }
}

pub(crate) fn parse_key_mode(m: &ArgMatches) -> KeyMode {
  if let Some(dir) = m.get_one::<PathBuf>("config") {
    KeyMode::Persistent(dir.clone())
  } else if let Some(dir) = default_config_dir() {
    KeyMode::Persistent(dir)
  } else {
    KeyMode::Ephemeral
  }
}

pub(crate) fn parse_profile_draft(m: &ArgMatches) -> ProfileDraft {
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
