use std::{
  net::SocketAddr,
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
  },
  time::Duration,
};

use gxhash::HashSet;
use notify::{Event, RecursiveMode, Watcher, recommended_watcher};
use roaring::RoaringBitmap;
use s0_cdc::{Manifest, ManifestEntry, PathChange, roaring_runs};
use s0_const::{FLAG_PARTIAL, HASH_LEN, MAX_PEER_RUNS, ZERO_PEER_ID, region::Region};
use s0_fs::{ScannedFile, SharedSink, build_manifest_incremental, is_staging_path};
use s0_udp::now_us;
use s0_wire::{SubPush, SwarmPeer, encode_socket_addrs};

use crate::{
  PeerError, PeerId, ShareConfig, min_nonzero, resolve_iroh_endpoints, server::ServerCtx,
  sub::path_matches_sel,
};

/// 实时目录监控扫描周期（50ms）
const WATCH_SCAN_INTERVAL_US: u64 = 50_000;
/// 实时目录变更防抖合并窗口（50ms）
const WATCH_DEBOUNCE_US: u64 = 50_000;

/// 做种/订阅节点持久化与运行时记录
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscriberRecord {
  pub endpoint_id: PeerId,
  pub ctrl_addrs: Vec<SocketAddr>,
  pub paths: Vec<String>,
  pub eff_rate: u64,
}

#[derive(Clone)]
pub(crate) struct SwarmSeedRecord {
  pub(crate) endpoint_id: PeerId,
  pub(crate) region: Region,
  pub(crate) ctrl_addr: SocketAddr,
  pub(crate) profile_card: Vec<u8>,
  pub(crate) root_hash: [u8; HASH_LEN],
  pub(crate) have_blocks: RoaringBitmap,
  pub(crate) weight_rate: u64,
}

impl SwarmSeedRecord {
  pub(crate) fn new(
    endpoint_id: PeerId,
    region: Region,
    ctrl_addr: SocketAddr,
    profile_card: Vec<u8>,
    root_hash: [u8; HASH_LEN],
    (blk_lo, blk_hi): (u32, u32),
    weight_rate: u64,
  ) -> Self {
    let mut have_blocks = RoaringBitmap::new();
    if blk_lo < blk_hi {
      have_blocks.insert_range(blk_lo..blk_hi);
      have_blocks.optimize();
    }
    Self {
      endpoint_id,
      region,
      ctrl_addr,
      profile_card,
      root_hash,
      have_blocks,
      weight_rate,
    }
  }
}

pub(crate) struct ShareState {
  pub(crate) cfg: ShareConfig,
  pub(crate) sub_paths: Vec<String>,
  pub(crate) subscribers: Vec<SubscriberRecord>,
  pub(crate) seeds: Vec<SwarmSeedRecord>,
  /// 正在下载中的暂存池（实现边下载边上传，无需等待整个分享下载完成即可向其他节点供应已校验块）
  pub(crate) active_sink: Option<([u8; HASH_LEN], Arc<SharedSink>)>,
  /// 最近一次扫描缓存的 `(paths, root_hash, Manifest)`，使紧随 `Stat` 的 `Pull` 免除重复全盘扫描与分块
  pub(crate) cached_manifest: Option<(Vec<String>, [u8; HASH_LEN], Manifest)>,
  /// 最近一次向订阅者广播完成时的全量基准 `Manifest`（用于计算增量 `PathChange`）
  pub(crate) baseline_manifest: Option<Manifest>,
  /// 最近一次目录扫描的文件元数据快照（按路径升序排列，与 `baseline_manifest` 对齐，用于 $O(N)$ 免重扫增量构建）
  pub(crate) last_scanned: Vec<ScannedFile>,
  /// 最近一次文件系统扫描微秒时间戳
  pub(crate) last_scan_us: u64,
  /// 待防抖处理的首次变更检测微秒时间戳（`0` 表示当前稳定）
  pub(crate) pending_change_us: u64,
  /// `.s0_subs.bin` 最近一次全量 RDB 基准快照字节数（`0` 表示尚未建立 RDB 基准文件）
  pub(crate) subs_rdb_bytes: u64,
  /// `.s0_subs.bin` 自上次 RDB 快照以来累计追加的 AOF 尾部字节数
  pub(crate) subs_aof_bytes: u64,
  /// `.s0_subs.bin` 自上次 RDB 快照以来累计追加的 AOF 帧数
  pub(crate) subs_aof_ops: u32,
}

impl ShareState {
  #[inline]
  pub(crate) fn new(cfg: ShareConfig, sub_paths: Vec<String>) -> Self {
    Self {
      cfg,
      sub_paths,
      subscribers: Vec::new(),
      seeds: Vec::new(),
      active_sink: None,
      cached_manifest: None,
      baseline_manifest: None,
      last_scanned: Vec::new(),
      last_scan_us: 0,
      pending_change_us: 0,
      subs_rdb_bytes: 0,
      subs_aof_bytes: 0,
      subs_aof_ops: 0,
    }
  }

  pub(crate) fn upsert_seed(&mut self, seed: SwarmSeedRecord) {
    if let Some(ex) = self.seeds.iter_mut().find(|s| {
      s.ctrl_addr == seed.ctrl_addr
        || (seed.endpoint_id != ZERO_PEER_ID && s.endpoint_id == seed.endpoint_id)
    }) {
      ex.endpoint_id = seed.endpoint_id;
      ex.region = seed.region;
      ex.ctrl_addr = seed.ctrl_addr;
      if !seed.profile_card.is_empty() {
        ex.profile_card = seed.profile_card;
      }
      ex.weight_rate = seed.weight_rate;
      if ex.root_hash != seed.root_hash {
        ex.root_hash = seed.root_hash;
        ex.have_blocks = seed.have_blocks;
      } else {
        ex.have_blocks |= &seed.have_blocks;
        ex.have_blocks.optimize();
      }
      return;
    }
    self.seeds.push(seed);
  }

  #[inline]
  pub(crate) fn set_scan_baseline(
    &mut self,
    baseline: Option<Manifest>,
    scanned: Vec<ScannedFile>,
    now: u64,
  ) {
    if let Some(bmf) = baseline {
      self.baseline_manifest = Some(bmf);
    }
    self.last_scanned = scanned;
    self.last_scan_us = now;
    self.pending_change_us = 0;
  }
}

/// 构建本节点 + 已匹配 `root_hash` 的在线做种节点列表（`Stat` 与 `SubPush` 一处定义复用）
pub(crate) fn build_swarm_peers(
  origin: (PeerId, Region, &[u8], &[u8]),
  snap: (u64, u32, [u8; HASH_LEN]),
  seeds: &[SwarmSeedRecord],
  exclude_id: PeerId,
) -> Vec<SwarmPeer> {
  let (my_id, my_region, my_ctrl_addrs, my_card) = origin;
  let (eff_rate, total_blks, root_hash) = snap;
  let mut peers = Vec::with_capacity(1 + seeds.len());
  peers.push(SwarmPeer {
    endpoint_id: my_id,
    region: my_region,
    weight_rate: eff_rate,
    blk_lo: 0,
    blk_hi: total_blks,
    peer_addrs: my_ctrl_addrs.to_vec(),
    profile_card: my_card.to_vec(),
  });
  for s in seeds {
    if s.root_hash == root_hash && s.endpoint_id != exclude_id {
      let runs = roaring_runs(&s.have_blocks, MAX_PEER_RUNS);
      if runs.is_empty() {
        continue;
      }
      let seed_addrs = resolve_iroh_endpoints(&s.endpoint_id);
      let encoded_addrs = if seed_addrs.is_empty() {
        encode_socket_addrs(&[s.ctrl_addr])
      } else {
        encode_socket_addrs(&seed_addrs)
      };
      let weight_rate = min_nonzero(s.weight_rate, eff_rate);
      for (blk_lo, blk_hi) in runs {
        peers.push(SwarmPeer {
          endpoint_id: s.endpoint_id,
          region: s.region,
          weight_rate,
          blk_lo,
          blk_hi,
          peer_addrs: encoded_addrs.clone(),
          profile_card: s.profile_card.clone(),
        });
      }
    }
  }
  peers
}

impl ServerCtx {
  /// 增量扫描并计算指定 `share_id` 的路径与块签名差分 (`PathChange`)，通过 `iroh` (`mzstd` 压缩) 向全部匹配订阅者推送 `SubPush`
  pub(crate) fn publish_share_id(&self, share_id: [u8; HASH_LEN]) -> Result<usize, PeerError> {
    let _guard = self.publish_lock.lock();
    let (share_cfg, subs, seeds, prev_base, prev_scanned) = {
      let guard = self.shares.read();
      let Some(st) = guard.get(&share_id) else {
        return Ok(0);
      };
      (
        st.cfg.clone(),
        st.subscribers.clone(),
        st.seeds.clone(),
        st.baseline_manifest.clone(),
        st.last_scanned.clone(),
      )
    };

    if subs.is_empty() {
      return Ok(0);
    }

    let prev_pair = prev_base.as_ref().map(|mf| (mf, prev_scanned.as_slice()));
    let (curr_mf, curr_scanned) = build_manifest_incremental(&share_cfg.root, prev_pair)?;
    let curr_rh = curr_mf.root_hash()?;
    let curr_total_blks = curr_mf.total_blocks();
    let full_changes = match prev_base.as_ref() {
      Some(prev) => prev.diff(&curr_mf),
      None => curr_mf
        .entries
        .iter()
        .map(ManifestEntry::to_change)
        .collect(),
    };

    let now = now_us();
    if let Some(st) = self.shares.write().get_mut(&share_id) {
      st.cached_manifest = Some((Vec::new(), curr_rh, curr_mf.clone()));
      st.set_scan_baseline(None, curr_scanned, now);
      if full_changes.is_empty() {
        if st.baseline_manifest.is_none() {
          st.baseline_manifest = Some(curr_mf);
        }
        return Ok(0);
      }
    }

    let mut pushed = 0usize;
    let mut dead_subs: Vec<(PeerId, Vec<SocketAddr>)> = Vec::new();

    for sub in &subs {
      let Some(access) = share_cfg
        .resolve_access(&sub.endpoint_id, sub.eff_rate, None)
        .filter(|a| a.perm.can_read())
      else {
        dead_subs.push((sub.endpoint_id, sub.ctrl_addrs.clone()));
        continue;
      };

      let sub_changes: Vec<PathChange> = if sub.paths.is_empty() {
        full_changes.clone()
      } else {
        full_changes
          .iter()
          .filter(|c| path_matches_sel(&c.path, &sub.paths))
          .cloned()
          .collect()
      };
      if sub_changes.is_empty() {
        continue;
      }

      let (total_blks, rh) = if sub.paths.is_empty() {
        (curr_total_blks, curr_rh)
      } else {
        let filtered = curr_mf.filter_paths(&sub.paths);
        (filtered.total_blocks(), filtered.root_hash()?)
      };
      let swarm_peers = self.swarm_peers(
        &share_cfg,
        access.eff_rate,
        total_blks,
        rh,
        &seeds,
        sub.endpoint_id,
      );

      let mut push_flags = if sub.paths.is_empty() {
        0
      } else {
        FLAG_PARTIAL
      };
      if share_cfg.auth_required {
        push_flags |= s0_const::FLAG_AUTH;
      }

      let push_ev = SubPush {
        flags: push_flags,
        share_id,
        perm: access.perm,
        eff_rate: access.eff_rate,
        write_prefixes: access.write_prefixes,
        root_hash: rh,
        origin_addrs: self.my_ctrl_addrs.clone(),
        peers: swarm_peers,
        changes: sub_changes,
      };

      if self
        .iroh_ctrl
        .send_sub_push(sub.endpoint_id, &sub.ctrl_addrs, &push_ev)
        .is_ok()
      {
        pushed += 1;
      } else {
        dead_subs.push((sub.endpoint_id, sub.ctrl_addrs.clone()));
      }
    }

    if let Some(st) = self.shares.write().get_mut(&share_id) {
      st.prune_dead_subs(&dead_subs);
      st.baseline_manifest = Some(curr_mf);
      st.persist_subs_delta(&full_changes, &dead_subs);
    }

    Ok(pushed)
  }

  /// 处理 `notify` 内核文件监听上报的路径事件，立即激活对应分享组的防抖巡检
  pub(crate) fn on_notify_paths(&self, paths: &[PathBuf]) {
    let now = now_us();
    let mut guard = self.shares.write();
    for st in guard.values_mut() {
      if st.subscribers.is_empty() || st.active_sink.is_some() {
        continue;
      }
      if paths.iter().any(|p| p.starts_with(&st.cfg.root)) {
        if st.pending_change_us == 0 {
          st.pending_change_us = now;
        }
        st.last_scan_us = 0;
      }
    }
  }

  /// 后台周期性增量巡检全部含订阅者的分享目录，防抖合并后通过 `iroh` 自动推送实时改动
  pub(crate) fn tick_watch(&self) {
    let now = now_us();
    let candidates: Vec<_> = self
      .shares
      .read()
      .iter()
      .filter(|(_, st)| {
        !st.subscribers.is_empty()
          && st.active_sink.is_none()
          && now.saturating_sub(st.last_scan_us) >= WATCH_SCAN_INTERVAL_US
      })
      .map(|(&sid, st)| {
        (
          sid,
          st.cfg.root.clone(),
          st.baseline_manifest.clone(),
          st.last_scanned.clone(),
          st.pending_change_us,
        )
      })
      .collect();

    for (sid, root_dir, prev_base, prev_scanned, pending_us) in candidates {
      let prev_pair = prev_base.as_ref().map(|mf| (mf, prev_scanned.as_slice()));
      let Ok((curr_mf, curr_scanned)) = build_manifest_incremental(&root_dir, prev_pair) else {
        continue;
      };
      let scan_ts = now_us();
      let Some(base) = prev_base else {
        if let Some(st) = self.shares.write().get_mut(&sid) {
          st.set_scan_baseline(Some(curr_mf), curr_scanned, scan_ts);
        }
        continue;
      };

      if base.diff(&curr_mf).is_empty() {
        if let Some(st) = self.shares.write().get_mut(&sid) {
          st.set_scan_baseline(None, curr_scanned, scan_ts);
        }
        continue;
      }

      if pending_us == 0 {
        if let Some(st) = self.shares.write().get_mut(&sid) {
          st.last_scan_us = scan_ts;
          st.pending_change_us = scan_ts;
        }
      } else if scan_ts.saturating_sub(pending_us) >= WATCH_DEBOUNCE_US {
        let _ = self.publish_share_id(sid);
      }
    }
  }
}

/// 启动基于 `notify` 跨平台原生内核事件（macOS FSEvents / Linux inotify / Windows ReadDirectoryChangesW）+ 增量扫描防抖的目录监控循环
pub(crate) fn run_watch_loop(stop: Arc<AtomicBool>, ctx: ServerCtx, poll_sleep: Duration) {
  let (tx, rx) = mpsc::channel::<PathBuf>();
  let mut watcher = recommended_watcher(move |res: notify::Result<Event>| {
    if let Ok(ev) = res {
      for p in ev.paths {
        if !is_staging_path(&p) {
          let _ = tx.send(p);
        }
      }
    }
  })
  .ok();
  let mut watched_roots: HashSet<PathBuf> = HashSet::default();

  while !stop.load(Ordering::Acquire) {
    if let Some(ref mut w) = watcher {
      let active_roots: Vec<PathBuf> = ctx
        .shares
        .read()
        .values()
        .filter(|st| !st.subscribers.is_empty())
        .map(|st| st.cfg.root.clone())
        .collect();
      for root in active_roots {
        if !watched_roots.contains(&root) && w.watch(&root, RecursiveMode::Recursive).is_ok() {
          watched_roots.insert(root);
        }
      }
    }

    if let Ok(first_path) = rx.recv_timeout(poll_sleep) {
      let mut dirty_paths = vec![first_path];
      while let Ok(p) = rx.try_recv() {
        dirty_paths.push(p);
      }
      ctx.on_notify_paths(&dirty_paths);
    }
    if stop.load(Ordering::Acquire) {
      break;
    }
    ctx.tick_watch();
  }
}
