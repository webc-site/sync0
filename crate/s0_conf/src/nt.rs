use std::{
  fs,
  path::{Path, PathBuf},
};

use gxhash::HashMap;
use nested_text::{Top, Value, loads};
use s0_const::Perm;

use crate::{ConfError, normalize_prefixes, parse_rate};

/// 单用户在特定分享组下的覆盖规则
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct NtGroupRule {
  perm: Option<Perm>,
  rate: Option<u64>,
  write_prefixes: Option<Vec<String>>,
}

/// 单用户 `.nt` 配置文件解析结果
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct NtUserSpec {
  ids: Vec<String>,
  perm: Option<Perm>,
  rate: Option<u64>,
  write_prefixes: Vec<String>,
  groups: HashMap<String, NtGroupRule>,
}

/// 分享组内解析展开后的单节点授权项
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NtPeerGrant {
  pub id: String,
  pub perm: Option<Perm>,
  pub rate: u64,
  pub write_prefixes: Vec<String>,
}

/// 分享组 `.nt` 配置解析结果
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NtGroupSpec {
  pub name: String,
  pub root: PathBuf,
  pub auth_required: bool,
  pub default_perm: Option<Perm>,
  pub default_max_rate: u64,
  pub default_write_prefixes: Vec<String>,
  pub r_secret: Option<String>,
  pub rw_secret: Option<String>,
  pub grants: Vec<NtPeerGrant>,
}

/// 解析 NestedText (`.nt`) 格式的分享组与用户配置文件
///
/// - 支持通过 `users_dir` 指定用户配置目录，自动包含该目录下的全部 `*.nt` 用户文件；
/// - 每个用户单独一个 `.nt` 配置文件，用户列表与上传路径前缀均采用无序列表（`- ...`）；
/// - `rate` 支持 `GB`、`MB`、`KB`、`B` 等单位或纯数字字节/秒。
pub fn parse_nt_groups(text: &str, base_dir: Option<&Path>) -> Result<Vec<NtGroupSpec>, ConfError> {
  let Some(root_val) = loads(text, Top::Dict)? else {
    return Ok(Vec::new());
  };
  let Some(root_pairs) = root_val.as_dict() else {
    return Ok(Vec::new());
  };

  let users_dir_path = root_pairs
    .iter()
    .find(|(k, _)| matches!(k.as_str(), "users_dir" | "user_dir"))
    .and_then(|(_, v)| v.as_str())
    .map(|s| resolve_users_dir(s, base_dir))
    .or_else(|| base_dir.map(|b| b.join("users")).filter(|p| p.is_dir()));
  let dir_users = match users_dir_path {
    Some(ref udir) => load_users_nt_dir(udir)?,
    None => Vec::new(),
  };

  let group_pairs: &[(String, Value)] = if let Some(gval) = root_val.get("groups")
    && let Some(gp) = gval.as_dict()
  {
    gp
  } else {
    root_pairs
  };

  let mut out = Vec::new();
  for (gname, gval) in group_pairs {
    if matches!(gname.as_str(), "users_dir" | "user_dir") {
      continue;
    }
    let Some(gpairs) = gval.as_dict() else {
      continue;
    };
    let name = gname
      .strip_prefix("group.")
      .or_else(|| gname.strip_prefix("share."))
      .unwrap_or(gname)
      .trim();

    let mut spec = NtGroupSpec {
      name: name.to_owned(),
      ..Default::default()
    };
    let mut explicit_users = false;
    let mut explicit_auth = None;

    for (k, v) in gpairs {
      let s = v.as_str().map(str::trim);
      match k.as_str() {
        "path" | "root" | "dir" => {
          if let Some(s) = s {
            spec.root = PathBuf::from(s);
          }
        }
        "auth" => explicit_auth = Some(s.is_some_and(|s| matches!(s, "true" | "1" | "yes"))),
        "perm" | "default_perm" => spec.default_perm = s.and_then(Perm::parse_str),
        "rate" | "default_rate" => {
          spec.default_max_rate = s.and_then(parse_rate).unwrap_or(spec.default_max_rate)
        }
        "write_prefix" | "default_write_prefix" => {
          spec.default_write_prefixes = parse_nt_prefixes(v)
        }
        "r_secret" => spec.r_secret = s.map(str::to_owned),
        "rw_secret" => spec.rw_secret = s.map(str::to_owned),
        "users_dir" | "user_dir" => {
          if let Some(dir_str) = s {
            for (_, uspec) in load_users_nt_dir(&resolve_users_dir(dir_str, base_dir))? {
              apply_user_spec_to_group(&mut spec, &uspec, name, true);
            }
          }
        }
        "users" => {
          explicit_users = true;
          apply_nt_users_list(&mut spec, v, &dir_users, name);
        }
        _ => {}
      }
    }

    for (_, uspec) in &dir_users {
      apply_user_spec_to_group(&mut spec, uspec, name, !explicit_users);
    }
    spec.auth_required = explicit_auth.unwrap_or(explicit_users || spec.r_secret.is_some());

    out.push(spec);
  }

  Ok(out)
}

/// 加载用户配置目录下的全部 `*.nt` 文件（每个用户单独一个 `.nt` 配置文件）
fn load_users_nt_dir(dir: &Path) -> Result<Vec<(String, NtUserSpec)>, ConfError> {
  if !dir.is_dir() {
    return Ok(Vec::new());
  }
  let mut paths = Vec::new();
  for entry in fs::read_dir(dir)? {
    let entry = entry?;
    let p = entry.path();
    if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("nt") {
      paths.push(p);
    }
  }
  paths.sort_unstable();
  let mut users = Vec::with_capacity(paths.len());
  for p in paths {
    let stem = p
      .file_stem()
      .and_then(|s| s.to_str())
      .unwrap_or("")
      .trim()
      .to_owned();
    let text = fs::read_to_string(&p)?;
    if let Some(val) = loads(&text, Top::Any)? {
      let spec = parse_user_nt_value(&stem, &val);
      if !spec.ids.is_empty() {
        users.push((stem, spec));
      }
    }
  }
  Ok(users)
}

fn resolve_users_dir(raw: &str, base_dir: Option<&Path>) -> PathBuf {
  let p = PathBuf::from(raw.trim());
  match base_dir {
    Some(base) if p.is_relative() => base.join(p),
    _ => p,
  }
}

fn parse_nt_prefixes(v: &Value) -> Vec<String> {
  match v {
    Value::String(s) => normalize_prefixes(s.split(',')),
    Value::List(items) => normalize_prefixes(items.iter().filter_map(Value::as_str)),
    Value::Dict(_) => Vec::new(),
  }
}

fn parse_nt_ids(v: &Value, out: &mut Vec<String>) {
  match v {
    Value::String(s) => {
      let clean = s.trim();
      if !clean.is_empty() && !out.iter().any(|x| x == clean) {
        out.push(clean.to_owned());
      }
    }
    Value::List(items) => {
      for item in items {
        parse_nt_ids(item, out);
      }
    }
    Value::Dict(_) => {}
  }
}

fn parse_group_rule(v: &Value) -> NtGroupRule {
  let mut rule = NtGroupRule::default();
  if let Some(s) = v.as_str() {
    rule.perm = Perm::parse_str(s);
    return rule;
  }
  let Some(pairs) = v.as_dict() else {
    return rule;
  };
  for (k, val) in pairs {
    match k.as_str() {
      "perm" => rule.perm = val.as_str().and_then(Perm::parse_str),
      "rate" | "max_rate" => {
        rule.rate = val.as_str().and_then(parse_rate);
      }
      "write_prefix" | "write_prefixes" => {
        rule.write_prefixes = Some(parse_nt_prefixes(val));
      }
      _ => {}
    }
  }
  rule
}

fn parse_user_nt_value(stem: &str, val: &Value) -> NtUserSpec {
  let mut spec = NtUserSpec::default();
  if val.as_str().is_some() {
    parse_nt_ids(val, &mut spec.ids);
    return spec;
  }
  if let Some(items) = val.as_list() {
    for item in items {
      let sub = parse_user_nt_value(stem, item);
      for id in sub.ids {
        if !spec.ids.contains(&id) {
          spec.ids.push(id);
        }
      }
      if sub.perm.is_some() {
        spec.perm = sub.perm;
      }
      if sub.rate.is_some() {
        spec.rate = sub.rate;
      }
      if !sub.write_prefixes.is_empty() {
        spec.write_prefixes = sub.write_prefixes;
      }
      for (g, r) in sub.groups {
        spec.groups.insert(g, r);
      }
    }
    return spec;
  }
  if let Some(pairs) = val.as_dict() {
    for (k, v) in pairs {
      match k.as_str() {
        "id" | "ids" | "iroh" | "pub" | "pubkey" => parse_nt_ids(v, &mut spec.ids),
        "perm" | "default_perm" => spec.perm = v.as_str().and_then(Perm::parse_str),
        "rate" | "max_rate" | "default_rate" => spec.rate = v.as_str().and_then(parse_rate),
        "write_prefix" | "write_prefixes" => spec.write_prefixes = parse_nt_prefixes(v),
        "groups" => match v {
          Value::Dict(gpairs) => {
            for (gname, gval) in gpairs {
              spec
                .groups
                .insert(gname.trim().to_owned(), parse_group_rule(gval));
            }
          }
          Value::List(gitems) => {
            for item in gitems {
              if let Some(gname) = item.as_str() {
                spec
                  .groups
                  .insert(gname.trim().to_owned(), NtGroupRule::default());
              } else if let Some(dpairs) = item.as_dict() {
                for (gname, gval) in dpairs {
                  spec
                    .groups
                    .insert(gname.trim().to_owned(), parse_group_rule(gval));
                }
              }
            }
          }
          Value::String(gname) => {
            spec
              .groups
              .insert(gname.trim().to_owned(), NtGroupRule::default());
          }
        },
        _ => {}
      }
    }
  }
  if spec.ids.is_empty() && !stem.is_empty() {
    spec.ids.push(stem.to_owned());
  }
  spec
}

fn upsert_grant(spec: &mut NtGroupSpec, grant: NtPeerGrant) {
  if let Some(existing) = spec.grants.iter_mut().find(|g| g.id == grant.id) {
    *existing = grant;
  } else {
    spec.grants.push(grant);
  }
}

fn grant_user_with_rule(
  spec: &mut NtGroupSpec,
  user: &NtUserSpec,
  rule_perm: Option<Perm>,
  rule_rate: Option<u64>,
  rule_prefixes: Option<&[String]>,
) {
  let perm = rule_perm.or(user.perm).or(spec.default_perm);
  let rate = rule_rate.or(user.rate).unwrap_or(0);
  let prefixes = rule_prefixes.unwrap_or(&user.write_prefixes);
  for id in &user.ids {
    upsert_grant(
      spec,
      NtPeerGrant {
        id: id.clone(),
        perm,
        rate,
        write_prefixes: prefixes.to_vec(),
      },
    );
  }
}

fn apply_user_spec_to_group(
  spec: &mut NtGroupSpec,
  user: &NtUserSpec,
  group_name: &str,
  include_unscoped: bool,
) {
  if let Some(rule) = user.groups.get(group_name) {
    grant_user_with_rule(
      spec,
      user,
      rule.perm,
      rule.rate,
      rule.write_prefixes.as_deref(),
    );
  } else if user.groups.is_empty() && include_unscoped {
    grant_user_with_rule(spec, user, None, None, None);
  }
}

fn apply_nt_users_list(
  spec: &mut NtGroupSpec,
  v: &Value,
  dir_users: &[(String, NtUserSpec)],
  group_name: &str,
) {
  let Some(items) = v.as_list() else {
    return;
  };
  for item in items {
    match item {
      Value::String(s) => {
        let name = s.trim();
        if let Some((_, uspec)) = dir_users.iter().find(|(u, _)| u == name) {
          apply_user_spec_to_group(spec, uspec, group_name, true);
        } else if !name.is_empty() {
          upsert_grant(
            spec,
            NtPeerGrant {
              id: name.to_owned(),
              perm: spec.default_perm,
              rate: 0,
              write_prefixes: spec.default_write_prefixes.clone(),
            },
          );
        }
      }
      Value::Dict(pairs) => {
        if let [(k, v)] = pairs.as_slice()
          && !matches!(
            k.as_str(),
            "id"
              | "ids"
              | "iroh"
              | "pub"
              | "pubkey"
              | "user"
              | "name"
              | "perm"
              | "rate"
              | "write_prefix"
          )
        {
          let uname = k.trim();
          let r = parse_group_rule(v);
          if let Some((_, u)) = dir_users.iter().find(|(u, _)| u == uname) {
            grant_user_with_rule(spec, u, r.perm, r.rate, r.write_prefixes.as_deref());
          } else if !uname.is_empty() {
            upsert_grant(
              spec,
              NtPeerGrant {
                id: uname.to_owned(),
                perm: r.perm.or(spec.default_perm),
                rate: r.rate.unwrap_or(0),
                write_prefixes: r.write_prefixes.unwrap_or_default(),
              },
            );
          }
          continue;
        }

        let ref_user = pairs
          .iter()
          .find(|(k, _)| matches!(k.as_str(), "user" | "name"))
          .and_then(|(_, v)| v.as_str().map(str::trim));
        let inline_spec = parse_user_nt_value("", item);
        if let Some(uname) = ref_user
          && let Some((_, uspec)) = dir_users.iter().find(|(u, _)| u == uname)
        {
          let ov_prefixes = (!inline_spec.write_prefixes.is_empty())
            .then_some(inline_spec.write_prefixes.as_slice());
          grant_user_with_rule(spec, uspec, inline_spec.perm, inline_spec.rate, ov_prefixes);
        } else {
          apply_user_spec_to_group(spec, &inline_spec, group_name, true);
        }
      }
      Value::List(_) => {}
    }
  }
}
