mod cli;

use std::{net::SocketAddr, path::PathBuf, process::ExitCode};

use cli::{
  CliError, build_cli, build_cli_share_cfg, park_forever, parse_key_mode, parse_peer_uri_path_args,
  parse_profile_draft, resolve_send_target, start_node,
};
use rusty_alloc_default as _;
use s0_conf::GroupUri;
use s0_i18n::t;
use s0_udp::{
  bench::{recv_opt as bench_recv_opt, send_opt as bench_send_opt},
  consts::Role,
  peer::{PeerError, TransferOpts, active_iroh_peers, encode_id_b64, parse_iroh_ids_csv},
};

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
  let m = clap_args::parse!(|cmd| build_cli(cmd, i18n));
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
