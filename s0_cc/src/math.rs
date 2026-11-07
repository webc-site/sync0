/// `a × b / d`，u128 中间值防溢出，结果饱和到 u64（`d == 0` 时返回 0）
pub(crate) const fn mul_div(a: u64, b: u64, d: u64) -> u64 {
  if d == 0 {
    return 0;
  }
  let v = (a as u128) * (b as u128) / (d as u128);
  if v > u64::MAX as u128 {
    u64::MAX
  } else {
    v as u64
  }
}

/// const 版 `clamp`（`Ord::clamp` 非 const）；要求 `lo ≤ hi`
pub(crate) const fn clamp(v: u64, lo: u64, hi: u64) -> u64 {
  if v < lo {
    lo
  } else if v > hi {
    hi
  } else {
    v
  }
}
