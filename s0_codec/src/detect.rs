use std::str::from_utf8;

use phf::phf_set;

use crate::{
  Codec, Result,
  block::{MAX_BLOCK, SLACK},
};

/// 开头试压块数与连续不可压触发跳过阈值
const PROBE_HEAD: u32 = 2;
/// 复探间隔（块）
const PROBE_EVERY: u32 = 64;
/// 试压前缀采样字节数（不可压块仅试压前 16 KiB 即快速短路，省去整块 128 KiB 压缩开销）
const PROBE_SAMPLE: usize = 16 * 1024;
/// 扩展名最大长度
const EXT_MAX: usize = 12;

/// 已压缩 / 不可压扩展名
static SKIP_EXT: phf::Set<&'static str> = phf_set! {
  "zip", "gz", "tgz", "zst", "xz", "7z", "rar", "bz2", "lz4", "br",
  "jpg", "jpeg", "png", "gif", "webp", "avif", "heic",
  "mp4", "mkv", "mov", "avi", "webm", "flv",
  "mp3", "aac", "flac", "ogg", "opus",
  "woff", "woff2", "ttf", "otf",
  "docx", "xlsx", "pptx", "pdf",
  "onnx", "safetensors", "gguf", "parquet",
};

/// 文件魔数（不可压格式）
static MAGICS: &[&[u8]] = &[
  b"\x89PNG\r\n\x1a\n",
  b"\xff\xd8\xff",
  b"PK\x03\x04",
  b"\x1f\x8b",
  b"\x28\xb5\x2f\xfd",
  b"\xfd7zXZ\x00",
  b"7z\xbc\xaf\x27\x1c",
  b"Rar!\x1a\x07",
  b"\x00\x00\x00\x18ftyp",
  b"\x00\x00\x00\x20ftyp",
  b"OggS",
  b"fLaC",
];

pub fn skip_name(path: &str) -> bool {
  let Some((_, ext)) = path.rsplit_once('.') else {
    return false;
  };
  let bytes = ext.as_bytes();
  if bytes.is_empty() || bytes.len() > EXT_MAX {
    return false;
  }
  let mut buf = [0u8; EXT_MAX];
  for (dst, &src) in buf.iter_mut().zip(bytes) {
    *dst = src.to_ascii_lowercase();
  }
  from_utf8(&buf[..bytes.len()]).is_ok_and(|s| SKIP_EXT.contains(s))
}

pub fn skip_magic(head: &[u8]) -> bool {
  MAGICS.iter().any(|m| head.starts_with(m))
}

/// 文件级压缩探测：扩展名 → 首块魔数 → 前 2 块试压 / 连续 2 块不可压跳过 → 每 64 块复探
pub struct Detector {
  skip_name: bool,
  magic_checked: bool,
  skip: bool,
  bad: u32,
  blocks: u32,
  scratch: Box<[u8]>,
}

impl Detector {
  pub fn new(path: &str) -> Self {
    Self::with_flags(skip_name(path), true)
  }

  /// 按指定扩展名跳过与首块魔数检查策略构造探测器
  pub fn with_flags(skip_name: bool, check_magic: bool) -> Self {
    Self {
      skip_name,
      magic_checked: !check_magic,
      skip: false,
      bad: PROBE_HEAD - 1,
      blocks: 0,
      scratch: if skip_name {
        Box::new([])
      } else {
        vec![0u8; MAX_BLOCK + SLACK].into_boxed_slice()
      },
    }
  }

  pub const fn skip(&self) -> bool {
    self.skip
  }

  /// 是否尝试压缩本块（复用 `compress` 状态机）
  pub fn want(&mut self, raw: &[u8], codec: &mut Codec) -> Result<bool> {
    Ok(self.compress(raw, codec)?.is_some())
  }

  /// 按探测策略尝试压缩本块：
  /// 若应跳过或压缩收益不足，返回 `Ok(None)`（零额外拷贝）；
  /// 若压缩有效，返回内部 `scratch` 中的压缩帧切片。
  pub fn compress(&mut self, raw: &[u8], codec: &mut Codec) -> Result<Option<&[u8]>> {
    if self.skip_name {
      return Ok(None);
    }
    self.blocks += 1;
    if !self.magic_checked {
      self.magic_checked = true;
      if skip_magic(raw) {
        self.skip = true;
        return Ok(None);
      }
    }
    if self.skip && self.blocks > PROBE_HEAD && !self.blocks.is_multiple_of(PROBE_EVERY) {
      return Ok(None);
    }
    if self.bad > 0
      && raw.len() > PROBE_SAMPLE
      && codec
        .try_compress_block(&raw[..PROBE_SAMPLE], &mut self.scratch)?
        .is_none()
    {
      return Ok(self.on_bad());
    }
    if let Some(len) = codec.try_compress_block(raw, &mut self.scratch)? {
      self.bad = 0;
      self.skip = false;
      Ok(Some(&self.scratch[..len]))
    } else {
      Ok(self.on_bad())
    }
  }

  const fn on_bad(&mut self) -> Option<&[u8]> {
    self.bad += 1;
    if self.bad >= PROBE_HEAD {
      self.skip = true;
    }
    None
  }
}
