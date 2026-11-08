/// 分享组与多选文件/目录资源定位符（支持 `组名://` 全量、`组名://文件或目录`、`组名://路径1,路径2` 多选）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupUri {
  pub group: String,
  /// 选中的文件或目录相对路径集合（空列表表示同步整个分享组全部内容）
  pub paths: Vec<String>,
}

impl GroupUri {
  /// 解析 `组名://路径1,路径2` 或纯组名
  pub fn parse(input: &str) -> Option<Self> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
      return None;
    }
    let (group_raw, paths_raw) = if let Some((g, p)) = trimmed.split_once("://") {
      (g.trim(), p.trim())
    } else if let Some((g, p)) = trimmed.split_once(':') {
      (g.trim(), p.trim())
    } else {
      (trimmed, "")
    };
    if group_raw.is_empty() {
      return None;
    }
    let mut uri = Self {
      group: group_raw.to_owned(),
      paths: Vec::new(),
    };
    uri.extend_paths_csv(paths_raw);
    Some(uri)
  }

  /// 追加逗号分隔的一个或多个文件/目录路径
  pub fn extend_paths_csv(&mut self, csv: &str) {
    extend_clean_paths_csv(&mut self.paths, csv);
  }
}

/// 将逗号分隔的路径字符串归一化去重追加到目标集合
pub fn extend_clean_paths_csv(dst: &mut Vec<String>, csv: &str) {
  for part in csv.split(',') {
    let clean = part.trim().trim_matches(['/', '\\']);
    if !clean.is_empty() && !dst.iter().any(|p| p == clean) {
      dst.push(clean.to_owned());
    }
  }
}

/// 归一化路径前缀集合（遇到空前缀自动表示允许整个分享根目录，返回空列表）
pub fn normalize_prefixes<I, S>(prefixes: I) -> Vec<String>
where
  I: IntoIterator<Item = S>,
  S: AsRef<str>,
{
  let mut out = Vec::new();
  for p in prefixes {
    let clean = p.as_ref().trim().trim_matches(['/', '\\']);
    if clean.is_empty() {
      return Vec::new();
    }
    if !out.iter().any(|x| x == clean) {
      out.push(clean.to_owned());
    }
  }
  out.sort_unstable();
  out
}
