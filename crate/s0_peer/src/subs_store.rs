use std::{
  fs::{self, OpenOptions},
  io::Write,
  net::SocketAddr,
  path::Path,
};

use s0_cdc::{Manifest, PathChange, Rd, WireError, Wr};
use s0_const::{
  PEER_ID_LEN, PROTO_VERSION, SUBS_AOF_HDR_LEN, SUBS_AOF_MIN_REWRITE_BYTES, SUBS_AOF_REWRITE_OPS,
  SUBS_MAGIC, SubsAofTag, ZERO_PEER_ID,
};
use s0_fs::{SUBS_FILE, stage_path};
use s0_wire::{crc16_slice, decode_socket_addrs, encode_socket_addrs};

use crate::{
  PeerId,
  iroh_net::register_iroh_quic_addrs,
  register_iroh_endpoints, resolve_iroh_endpoints,
  watch::{ShareState, SubscriberRecord},
};

fn encode_sub_record(sub: &SubscriberRecord, w: &mut Wr<'_>) -> Result<(), WireError> {
  w.bytes(&sub.endpoint_id);
  w.uvar(sub.eff_rate);
  let addrs_raw = encode_socket_addrs(&sub.ctrl_addrs);
  w.slice_u15(&addrs_raw)?;
  w.strs_front(&sub.paths)
}

fn decode_sub_record(r: &mut Rd<'_>) -> Option<SubscriberRecord> {
  let endpoint_id = r.array::<PEER_ID_LEN>().ok()?;
  let eff_rate = r.uvar().ok()?;
  let ctrl_addrs = decode_socket_addrs(r.vec_u15().ok()?).ok()?;
  let paths = r.strs_front().ok()?;
  Some(SubscriberRecord {
    endpoint_id,
    ctrl_addrs,
    paths,
    eff_rate,
  })
}

pub(crate) fn upsert_sub_in_vec(subs: &mut Vec<SubscriberRecord>, sub: SubscriberRecord) {
  subs.retain(|s| {
    s.ctrl_addrs != sub.ctrl_addrs
      && (sub.endpoint_id == ZERO_PEER_ID || s.endpoint_id != sub.endpoint_id)
  });
  subs.push(sub);
}

pub(crate) fn prune_subs_in_vec(
  subs: &mut Vec<SubscriberRecord>,
  dead_subs: &[(PeerId, Vec<SocketAddr>)],
) {
  if dead_subs.is_empty() {
    return;
  }
  subs.retain(|s| {
    !dead_subs
      .iter()
      .any(|(id, addrs)| (*id != ZERO_PEER_ID && s.endpoint_id == *id) || s.ctrl_addrs == *addrs)
  });
}

fn append_aof_frame(
  out: &mut Vec<u8>,
  tag: SubsAofTag,
  build: impl FnOnce(&mut Wr<'_>) -> Result<(), WireError>,
) -> Option<()> {
  let start = out.len();
  out.extend_from_slice(&[0u8; SUBS_AOF_HDR_LEN]);
  out[start + 6] = tag as u8;
  let body_start = out.len();
  let mut w = Wr::new(out);
  if build(&mut w).is_err() {
    out.truncate(start);
    return None;
  }
  let payload_len = u32::try_from(out.len() - body_start).ok()?;
  let crc = crc16_slice(&out[start + 6..]);
  out[start..start + 4].copy_from_slice(&payload_len.to_le_bytes());
  out[start + 4..start + 6].copy_from_slice(&crc.to_le_bytes());
  Some(())
}

fn write_aof_bytes(root: &Path, buf: &[u8]) -> Option<u64> {
  if buf.is_empty() {
    return Some(0);
  }
  let dst = root.join(SUBS_FILE);
  let mut f = OpenOptions::new()
    .create(false)
    .append(true)
    .open(dst)
    .ok()?;
  f.write_all(buf).ok()?;
  Some(buf.len() as u64)
}

/// 全量 RDB 快照重写（原子写入 `<share_root>/.s0_subs.bin` 基础镜像并清空 AOF 尾部，对应 Redis `BGREWRITEAOF` / RDB Preamble）
pub fn save_subs_file(root: &Path, baseline: Option<&Manifest>, subs: &[SubscriberRecord]) -> u64 {
  let dst = root.join(SUBS_FILE);
  if subs.is_empty() {
    let _ = fs::remove_file(dst);
    return 0;
  }
  if !root.exists() {
    return 0;
  }
  let base_bytes = baseline.and_then(|mf| mf.encode().ok()).unwrap_or_default();
  let mut buf = Vec::with_capacity(64 + base_bytes.len() + subs.len() * 64);
  let mut w = Wr::new(&mut buf);
  w.u32(SUBS_MAGIC);
  w.u8(PROTO_VERSION);
  w.uvar(base_bytes.len() as u64);
  w.bytes(&base_bytes);
  w.uvar(subs.len() as u64);
  for sub in subs {
    if encode_sub_record(sub, &mut w).is_err() {
      return 0;
    }
  }
  let tmp = stage_path(&dst);
  if fs::write(&tmp, &buf).is_ok() && fs::rename(&tmp, &dst).is_ok() {
    buf.len() as u64
  } else {
    0
  }
}

/// 增量 AOF 追加单条订阅节点变更 (`SubsAofTag::UpsertSub`)，避免重写完整 `Manifest`
pub(crate) fn append_subs_aof_sub(root: &Path, sub: &SubscriberRecord) -> Option<u64> {
  let mut buf = Vec::with_capacity(96);
  append_aof_frame(&mut buf, SubsAofTag::UpsertSub, |w| {
    encode_sub_record(sub, w)
  })?;
  write_aof_bytes(root, &buf)
}

/// 增量 AOF 追加清单变更差集 (`SubsAofTag::ManifestDelta`) 与失效节点剔除记录 (`SubsAofTag::RemoveSub`)，免除全量序列化 `Manifest`
pub fn append_subs_aof_delta(
  root: &Path,
  changes: &[PathChange],
  dead_subs: &[(PeerId, Vec<SocketAddr>)],
) -> Option<(u64, u32)> {
  let mut buf = Vec::with_capacity(32 + changes.len() * 32 + dead_subs.len() * 48);
  let mut ops = 0u32;
  if !changes.is_empty() {
    append_aof_frame(&mut buf, SubsAofTag::ManifestDelta, |w| {
      PathChange::encode_slice(changes, w)
    })?;
    ops += 1;
  }
  if !dead_subs.is_empty() {
    append_aof_frame(&mut buf, SubsAofTag::RemoveSub, |w| {
      w.uvar(dead_subs.len() as u64);
      for (id, addrs) in dead_subs {
        w.bytes(id);
        let addrs_raw = encode_socket_addrs(addrs);
        w.slice_u15(&addrs_raw)?;
      }
      Ok(())
    })?;
    ops += 1;
  }
  let written = write_aof_bytes(root, &buf)?;
  Some((written, ops))
}

/// 从 `<share_root>/.s0_subs.bin` 加载 RDB 基准快照并回放尾部 AOF 增量日志（自动容忍掉电半包截断并在回放后执行紧缩整理）
pub fn load_subs_file(root: &Path) -> Option<(Option<Manifest>, Vec<SubscriberRecord>, u64)> {
  let raw = fs::read(root.join(SUBS_FILE)).ok()?;
  let mut r = Rd::new(&raw);
  if r.u32().ok()? != SUBS_MAGIC || r.u8().ok()? != PROTO_VERSION {
    return None;
  }
  let base_len = r.uvar_usize().ok()?;
  let base_bytes = r.bytes(base_len).ok()?;
  let mut baseline = (!base_bytes.is_empty())
    .then(|| Manifest::decode(base_bytes).ok())
    .flatten();
  let cnt = r.uvar_usize().ok()?;
  if cnt > r.rem() / PEER_ID_LEN {
    return None;
  }
  let mut subs = Vec::with_capacity(cnt.min(256));
  for _ in 0..cnt {
    subs.push(decode_sub_record(&mut r)?);
  }
  let rdb_end = raw.len() - r.rem();
  let mut aof_changes: Vec<PathChange> = Vec::new();
  let mut had_aof = false;

  while r.rem() >= SUBS_AOF_HDR_LEN {
    let Ok(payload_len) = r.u32().map(|v| v as usize) else {
      break;
    };
    let Ok(expected_crc) = r.u16() else {
      break;
    };
    let tag_pos = raw.len() - r.rem();
    let Ok(raw_tag) = r.u8() else {
      break;
    };
    let Ok(payload) = r.bytes(payload_len) else {
      break;
    };
    if crc16_slice(&raw[tag_pos..tag_pos + 1 + payload_len]) != expected_crc {
      break;
    }
    let Some(tag) = SubsAofTag::from_repr(raw_tag) else {
      break;
    };
    had_aof = true;
    let mut pr = Rd::new(payload);
    match tag {
      SubsAofTag::UpsertSub => {
        if let Some(sub) = decode_sub_record(&mut pr) {
          upsert_sub_in_vec(&mut subs, sub);
        } else {
          break;
        }
      }
      SubsAofTag::RemoveSub => {
        let Ok(dcnt) = pr.uvar_usize() else {
          break;
        };
        if dcnt > pr.rem() / PEER_ID_LEN {
          break;
        }
        let mut dead = Vec::with_capacity(dcnt.min(64));
        let mut ok = true;
        for _ in 0..dcnt {
          if let (Ok(id), Ok(raw_addrs)) = (pr.array::<PEER_ID_LEN>(), pr.vec_u15())
            && let Ok(addrs) = decode_socket_addrs(raw_addrs)
          {
            dead.push((id, addrs));
          } else {
            ok = false;
            break;
          }
        }
        if !ok {
          break;
        }
        prune_subs_in_vec(&mut subs, &dead);
      }
      SubsAofTag::ManifestDelta => {
        if let Ok(mut delta) = PathChange::decode_vec(&mut pr) {
          aof_changes.append(&mut delta);
        } else {
          break;
        }
      }
    }
  }

  if !aof_changes.is_empty() {
    let mf = baseline.get_or_insert_with(Manifest::default);
    mf.apply_changes(&aof_changes);
  }

  for sub in &subs {
    if sub.endpoint_id != ZERO_PEER_ID && !sub.ctrl_addrs.is_empty() {
      if resolve_iroh_endpoints(&sub.endpoint_id).is_empty() {
        register_iroh_endpoints(sub.endpoint_id, &sub.ctrl_addrs);
      }
      register_iroh_quic_addrs(sub.endpoint_id, &sub.ctrl_addrs);
    }
  }

  let rdb_bytes = if had_aof || r.rem() > 0 {
    save_subs_file(root, baseline.as_ref(), &subs)
  } else {
    rdb_end as u64
  };

  Some((baseline, subs, rdb_bytes))
}

impl ShareState {
  pub(crate) fn upsert_subscriber(&mut self, sub: SubscriberRecord) {
    upsert_sub_in_vec(&mut self.subscribers, sub);
  }

  pub(crate) fn prune_dead_subs(&mut self, dead_subs: &[(PeerId, Vec<SocketAddr>)]) {
    if dead_subs.is_empty() {
      return;
    }
    prune_subs_in_vec(&mut self.subscribers, dead_subs);
    self.seeds.retain(|s| {
      !dead_subs.iter().any(|(id, addrs)| {
        (*id != ZERO_PEER_ID && s.endpoint_id == *id) || addrs.contains(&s.ctrl_addr)
      })
    });
  }

  #[inline]
  pub(crate) const fn needs_subs_rdb_rewrite(&self) -> bool {
    self.subs_rdb_bytes == 0
      || self.subs_aof_ops >= SUBS_AOF_REWRITE_OPS
      || (self.subs_aof_bytes >= SUBS_AOF_MIN_REWRITE_BYTES
        && self.subs_aof_bytes >= self.subs_rdb_bytes)
  }

  #[inline]
  fn rewrite_subs_rdb(&mut self) {
    self.subs_rdb_bytes = save_subs_file(
      &self.cfg.root,
      self.baseline_manifest.as_ref(),
      &self.subscribers,
    );
    self.subs_aof_bytes = 0;
    self.subs_aof_ops = 0;
  }

  /// 持久化单个新增/更新订阅节点：已有 RDB 基准且未达紧缩阈值时仅追加 AOF 记录，否则执行全量 RDB 重写
  pub(crate) fn persist_sub_upsert(&mut self, sub: &SubscriberRecord) {
    if !self.needs_subs_rdb_rewrite()
      && let Some(written) = append_subs_aof_sub(&self.cfg.root, sub)
    {
      self.subs_aof_bytes += written;
      self.subs_aof_ops += 1;
      return;
    }
    self.rewrite_subs_rdb();
  }

  /// 持久化增量清单差集与失效订阅者剔除：未达紧缩阈值时仅追加 `PathChange` AOF 帧（免全量序列化 `Manifest`），达阈值时自动触发 RDB 全量整理
  pub(crate) fn persist_subs_delta(
    &mut self,
    changes: &[PathChange],
    dead_subs: &[(PeerId, Vec<SocketAddr>)],
  ) {
    if !self.subscribers.is_empty()
      && !self.needs_subs_rdb_rewrite()
      && let Some((written, ops)) = append_subs_aof_delta(&self.cfg.root, changes, dead_subs)
    {
      self.subs_aof_bytes += written;
      self.subs_aof_ops += ops;
      return;
    }
    self.rewrite_subs_rdb();
  }
}
