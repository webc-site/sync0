mod dict;

use std::{env::var, str::from_utf8, sync::OnceLock};

pub use dict::{EN, I18n, LANG_CODES, ZH, by_code};
use sys_locale::{get_locale, get_locales};

static CURRENT: OnceLock<(&'static str, &'static I18n)> = OnceLock::new();

/// 获取当前系统默认语言对应的静态多语言字典（匹配不上自动回退英文 `EN`）
#[inline]
pub fn t() -> &'static I18n {
  CURRENT.get_or_init(detect).1
}

/// 检测系统默认语言并返回 `(语言代码, &I18n)`，未匹配时回退至 `("en", &EN)`
pub fn detect() -> (&'static str, &'static I18n) {
  for key in ["SYNC0_LANG", "LANGUAGE", "LC_ALL", "LC_MESSAGES", "LANG"] {
    if let Ok(val) = var(key)
      && !val.trim().is_empty()
      && let Some(hit) = match_locale_list(&val)
    {
      return hit;
    }
  }
  for sys in get_locales() {
    if let Some(hit) = match_locale_list(&sys) {
      return hit;
    }
  }
  if let Some(sys) = get_locale()
    && let Some(hit) = match_locale_list(&sys)
  {
    return hit;
  }
  ("en", &EN)
}

fn match_locale_list(raw_list: &str) -> Option<(&'static str, &'static I18n)> {
  for item in raw_list.split([':', ';', ',']) {
    let item = item.trim();
    if item.is_empty() {
      continue;
    }
    if let Some(hit) = match_single_locale(item) {
      return Some(hit);
    }
  }
  None
}

fn match_single_locale(raw: &str) -> Option<(&'static str, &'static I18n)> {
  let base = raw.split(['.', '@']).next().unwrap_or("").trim();
  if base.is_empty()
    || base.len() > 32
    || base.eq_ignore_ascii_case("c")
    || base.eq_ignore_ascii_case("posix")
  {
    return None;
  }

  let mut buf = [0u8; 32];
  let len = base.len();
  for (dst, src) in buf[..len].iter_mut().zip(base.as_bytes()) {
    *dst = if *src == b'_' {
      b'-'
    } else {
      src.to_ascii_lowercase()
    };
  }
  let norm = from_utf8(&buf[..len]).ok()?;

  // 1. 繁体中文区域与脚本别名归一为 zh-TW
  if matches!(
    norm,
    "zh-tw"
      | "zh-hk"
      | "zh-mo"
      | "zh-hant"
      | "zh-hant-tw"
      | "zh-hant-hk"
      | "zh-hant-mo"
      | "zh-cht"
      | "yue"
      | "yue-hk"
      | "yue-hant"
  ) {
    return by_code("zh-TW");
  }

  // 2. 简体中文区域与脚本别名归一为 zh
  if matches!(
    norm,
    "zh-cn"
      | "zh-sg"
      | "zh-my"
      | "zh-hans"
      | "zh-hans-cn"
      | "zh-hans-sg"
      | "zh-chs"
      | "cmn"
      | "cmn-hans"
  ) {
    return by_code("zh");
  }

  // 3. 精确匹配完整标签
  if let Some(hit) = by_code(norm) {
    return Some(hit);
  }

  // 4. 提取主语言子标签并映射 ISO 639-1 / 639-2 / 639-3 及各平台历史代码别名
  let primary = norm.split('-').next().unwrap_or(norm);
  let mapped = map_lang_alias(primary);
  by_code(mapped)
}

fn map_lang_alias(code: &str) -> &str {
  match code {
    "he" | "heb" => "iw",
    "tl" | "tgl" => "fil",
    "jw" | "jav" => "jv",
    "in" | "ind" => "id",
    "ji" => "iw",
    "nb" | "nn" | "nob" | "nno" | "nor" => "no",
    "zho" | "chi" => "zh",
    "eng" => "en",
    "deu" | "ger" => "de",
    "fra" | "fre" => "fr",
    "spa" => "es",
    "ita" => "it",
    "rus" => "ru",
    "jpn" => "ja",
    "kor" => "ko",
    "por" => "pt",
    "ara" => "ar",
    "hin" => "hi",
    "tur" => "tr",
    "vie" => "vi",
    "tha" => "th",
    "msa" | "may" => "ms",
    "nld" | "dut" => "nl",
    "pol" => "pl",
    "ukr" => "uk",
    "ces" | "cze" => "cs",
    "ron" | "rum" | "mo" => "ro",
    "ell" | "gre" => "el",
    "hun" => "hu",
    "swe" => "sv",
    "dan" => "da",
    "fin" => "fi",
    "fas" | "per" => "fa",
    "urd" => "ur",
    "ben" => "bn",
    "tam" => "ta",
    "tel" => "te",
    "mar" => "mr",
    "guj" => "gu",
    "kan" => "kn",
    "ori" => "or",
    "asm" => "as",
    "sin" => "si",
    "nep" => "ne",
    "mya" | "bur" => "my",
    "khm" => "km",
    "lao" => "lo",
    "kat" | "geo" => "ka",
    "hye" | "arm" => "hy",
    "aze" => "az",
    "kaz" => "kk",
    "uzb" => "uz",
    "snd" => "sd",
    "afr" => "af",
    "swa" => "sw",
    "cat" => "ca",
    "eus" | "baq" => "eu",
    "glg" => "gl",
    "cym" | "wel" => "cy",
    "isl" | "ice" => "is",
    "ltz" => "lb",
    "mlt" => "mt",
    "sqi" | "alb" => "sq",
    "mkd" | "mac" => "mk",
    "bul" => "bg",
    "srp" | "scc" | "sh" => "sr",
    "hrv" | "scr" => "hr",
    "bos" => "bs",
    "slv" => "sl",
    "slk" | "slo" => "sk",
    "bel" => "be",
    "lit" => "lt",
    "lav" => "lv",
    "est" => "et",
    other => other,
  }
}
