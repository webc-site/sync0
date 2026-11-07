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
use s0_const::Role;
use s0_peer::{KeyMode, NodeConfig, PeerError, PeerNode, ShareConfig, TransferOpts, encode_id_b64};
use s0_profile::{ProfileDraft, ProfileError};
use s0_region::Region;
use s0_udp::{UdpError, bench_recv_opt, bench_send_opt};

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
    eprintln!("sync0 error: {e}");
    return ExitCode::FAILURE;
  }
  ExitCode::SUCCESS
}

fn run() -> Result<(), CliError> {
  let m = clap_args::parse!(|cmd| {
    cmd
      .about("对等文件同步与 Dragonfly P2P 分发系统")
      .arg_required_else_help(true)
      .subcommand(bench_send_cmd("bench-send"))
      .subcommand(bench_send_cmd("send"))
      .subcommand(bench_recv_cmd("bench-recv"))
      .subcommand(bench_recv_cmd("recv"))
      .subcommand(node_args(Command::new("id").about("输出节点公钥 ID")))
      .subcommand(node_args(
        Command::new("profile").about("生成并签名节点名片"),
      ))
      .subcommand(node_args(
        Command::new("share")
          .about("发布共享目录")
          .arg(arg!(<share_name> "共享名称"))
          .arg(arg!(<dir> "本地共享目录").value_parser(value_parser!(PathBuf)))
          .arg(
            arg!(--rate <bps> "限速 (bytes/s)")
              .value_parser(value_parser!(u64))
              .default_value("0"),
          )
          .arg(arg!(--auth "启用鉴权").action(ArgAction::SetTrue)),
      ))
      .subcommand(node_args(
        Command::new("pull")
          .about("从对等节点拉取共享目录")
          .arg(arg!(<peer_addrs_csv> "对端地址列表 (逗号分隔)"))
          .arg(arg!(<share_name> "共享名称"))
          .arg(arg!(<dst_dir> "目标目录").value_parser(value_parser!(PathBuf)))
          .arg(arg!(--subpath <path> "子路径前缀").default_value(""))
          .arg(arg!(--delete "删除多余文件").action(ArgAction::SetTrue))
          .arg(
            arg!(--rate <bps> "限速 (bytes/s)")
              .value_parser(value_parser!(u64))
              .default_value("0"),
          ),
      ))
      .subcommand(node_args(
        Command::new("push")
          .about("向对等节点推送目录")
          .arg(arg!(<peer_addr> "对端地址").value_parser(value_parser!(SocketAddr)))
          .arg(arg!(<share_name> "共享名称"))
          .arg(arg!(<src_dir> "源目录").value_parser(value_parser!(PathBuf)))
          .arg(arg!(--subpath <path> "子路径前缀").default_value(""))
          .arg(arg!(--delete "删除多余文件").action(ArgAction::SetTrue))
          .arg(
            arg!(--rate <bps> "限速 (bytes/s)")
              .value_parser(value_parser!(u64))
              .default_value("0"),
          ),
      ))
      .subcommand(node_args(
        Command::new("sub")
          .about("订阅源节点并做种分发")
          .arg(arg!(<origin_addr> "源节点地址").value_parser(value_parser!(SocketAddr)))
          .arg(arg!(<share_name> "共享名称"))
          .arg(arg!(<local_dir> "本地目录").value_parser(value_parser!(PathBuf)))
          .arg(arg!(--prefix <subpath> "订阅子路径前缀").default_value(""))
          .arg(
            arg!(--rate <bps> "限速 (bytes/s)")
              .value_parser(value_parser!(u64))
              .default_value("0"),
          ),
      ))
      .subcommand(node_args(
        Command::new("group")
          .about("加入多写对等同步群组")
          .arg(arg!(<group_name> "群组名称"))
          .arg(arg!(<local_dir> "本地目录").value_parser(value_parser!(PathBuf)))
          .arg(arg!(<peer_addrs_csv> "种子节点地址列表 (逗号分隔)"))
          .arg(arg!(--prefix <subpath> "同步子路径前缀").default_value("")),
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

fn bench_send_cmd(name: &'static str) -> Command {
  Command::new(name)
    .about("UDP 直传发送文件或目录")
    .arg(arg!(<file_or_dir> "待发送文件或目录").value_parser(value_parser!(PathBuf)))
    .arg(arg!(<peer_addr> "接收端地址").value_parser(value_parser!(SocketAddr)))
    .arg(
      arg!(--rate <bps> "限速 (bytes/s)")
        .value_parser(value_parser!(u64))
        .default_value("0"),
    )
    .arg(arg!(--delete "删除接收端多余文件").action(ArgAction::SetTrue))
    .arg(arg!(-q --quiet "静默模式").action(ArgAction::SetTrue))
}

fn bench_recv_cmd(name: &'static str) -> Command {
  Command::new(name)
    .about("UDP 直传接收文件或目录")
    .arg(arg!(<listen_addr> "监听地址").value_parser(value_parser!(SocketAddr)))
    .arg(arg!(<out_path> "输出文件或目录").value_parser(value_parser!(PathBuf)))
    .arg(arg!(--delete "删除多余文件").action(ArgAction::SetTrue))
    .arg(arg!(-q --quiet "静默模式").action(ArgAction::SetTrue))
}

fn node_args(cmd: Command) -> Command {
  cmd
    .arg(
      Arg::new("config")
        .short('c')
        .long("config")
        .visible_alias("config-dir")
        .value_name("dir")
        .help("密钥与配置持久化目录")
        .value_parser(value_parser!(PathBuf)),
    )
    .arg(
      Arg::new("name")
        .short('u')
        .long("name")
        .visible_alias("user")
        .value_name("user")
        .default_value("peer")
        .help("节点昵称"),
    )
    .arg(arg!(--region <city> "地域名称或代码"))
    .arg(arg!(--email <email> "联系邮箱"))
    .arg(arg!(--url <url> "主页网址"))
    .arg(arg!(--bio <bio> "节点简介"))
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
