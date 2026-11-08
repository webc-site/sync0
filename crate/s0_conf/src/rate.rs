use crate::ConfError;

const KB: u128 = 1 << 10;
const MB: u128 = 1 << 20;
const GB: u128 = 1 << 30;
const TB: u128 = 1 << 40;
const MAX_FRAC_SCALE: u128 = 1_000_000_000;

/// 单次扫描零堆分配解析带宽速率字符串（字节/秒）
///
/// 支持：
/// - 纯数字字节/秒：`"0"`、`"10485760"`
/// - 整数或小数配合单位（不区分大小写，支持可选 `/s` 或 `ps` 后缀）：
///   - `KB` / `K` / `KiB`（`1024`）
///   - `MB` / `M` / `MiB`（`1024^2`）
///   - `GB` / `G` / `GiB`（`1024^3`）
///   - `TB` / `T` / `TiB`（`1024^4`）
///   - `B`（`1`）
pub fn parse_rate(input: &str) -> Option<u64> {
  let bytes = trim_ascii(input.as_bytes());
  if bytes.is_empty() {
    return None;
  }

  let mut int_part: u128 = 0;
  let mut frac_part: u128 = 0;
  let mut frac_scale: u128 = 1;
  let mut has_digit = false;
  let mut in_frac = false;
  let mut unit_start = bytes.len();

  for (idx, &b) in bytes.iter().enumerate() {
    match b {
      b'0'..=b'9' => {
        has_digit = true;
        let d = u128::from(b - b'0');
        if in_frac {
          if frac_scale < MAX_FRAC_SCALE {
            frac_part = frac_part.checked_mul(10)?.checked_add(d)?;
            frac_scale *= 10;
          }
        } else {
          int_part = int_part.checked_mul(10)?.checked_add(d)?;
        }
      }
      b'.' if !in_frac => {
        in_frac = true;
      }
      b'_' => {}
      _ => {
        unit_start = idx;
        break;
      }
    }
  }

  if !has_digit {
    return None;
  }

  let unit_raw = trim_ascii(&bytes[unit_start..]);
  let unit = strip_per_sec(unit_raw);
  let mult = match_unit_mult(unit)?;

  let whole = int_part.checked_mul(mult)?;
  let frac = if frac_part > 0 {
    (frac_part.checked_mul(mult)? + (frac_scale >> 1)) / frac_scale
  } else {
    0
  };
  u64::try_from(whole.checked_add(frac)?).ok()
}

/// 供 CLI 参数解析器直接复用的 `Result` 包装
#[inline]
pub fn parse_rate_str(s: &str) -> Result<u64, ConfError> {
  parse_rate(s).ok_or(ConfError::BadRate)
}

#[inline]
const fn trim_ascii(mut s: &[u8]) -> &[u8] {
  while let [first, rest @ ..] = s {
    if first.is_ascii_whitespace() {
      s = rest;
    } else {
      break;
    }
  }
  while let [rest @ .., last] = s {
    if last.is_ascii_whitespace() {
      s = rest;
    } else {
      break;
    }
  }
  s
}

#[inline]
fn strip_per_sec(u: &[u8]) -> &[u8] {
  if let [rest @ .., b'/', s] = u
    && s.eq_ignore_ascii_case(&b's')
  {
    return trim_ascii(rest);
  }
  if let [rest @ .., p, s] = u
    && p.eq_ignore_ascii_case(&b'p')
    && s.eq_ignore_ascii_case(&b's')
  {
    return trim_ascii(rest);
  }
  u
}

#[inline]
fn match_unit_mult(u: &[u8]) -> Option<u128> {
  match u.len() {
    0 => Some(1),
    1 => match u[0].to_ascii_lowercase() {
      b'b' => Some(1),
      b'k' => Some(KB),
      b'm' => Some(MB),
      b'g' => Some(GB),
      b't' => Some(TB),
      _ => None,
    },
    2 if u[1].eq_ignore_ascii_case(&b'b') => match u[0].to_ascii_lowercase() {
      b'k' => Some(KB),
      b'm' => Some(MB),
      b'g' => Some(GB),
      b't' => Some(TB),
      _ => None,
    },
    3 if u[1].eq_ignore_ascii_case(&b'i') && u[2].eq_ignore_ascii_case(&b'b') => {
      match u[0].to_ascii_lowercase() {
        b'k' => Some(KB),
        b'm' => Some(MB),
        b'g' => Some(GB),
        b't' => Some(TB),
        _ => None,
      }
    }
    _ => None,
  }
}
