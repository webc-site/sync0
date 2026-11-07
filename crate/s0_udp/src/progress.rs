use core::fmt::Write as _;
use std::io::{IsTerminal, Write, stderr};

use coarsetime::{Duration, Instant};
use itoa::Buffer as ItoaBuffer;
use jiff::SignedDuration;
use zmij::Buffer as ZmijBuffer;

use crate::s0_const::Role;

/// 刷新间隔
const TICK: Duration = Duration::from_secs(1);
/// 进度条宽度（字符）
const BAR: usize = 24;
/// 编译期预置进度条填充字符（每个字符占 3 字节 UTF-8）
const BAR_FILL: &str = "████████████████████████";
/// 编译期预置进度条空白字符（每个字符占 3 字节 UTF-8）
const BAR_EMPTY: &str = "░░░░░░░░░░░░░░░░░░░░░░░░";
/// 速率平滑系数（新样本权重）
const EWMA: f64 = 0.5;
const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];

/// 命令行进度条：每秒刷新一行到 stderr
pub struct Progress {
  role: Role,
  total: u64,
  tty: bool,
  start: Instant,
  next: Instant,
  last: (Instant, u64),
  done_at: Option<Instant>,
  rate: f64,
  drawn: bool,
  buf: String,
}

impl Progress {
  pub fn new(role: Role, total: u64) -> Self {
    let now = Instant::now();
    Self {
      role,
      total,
      tty: stderr().is_terminal(),
      start: now,
      next: now + TICK,
      last: (now, 0),
      done_at: None,
      rate: 0.0,
      drawn: false,
      buf: String::with_capacity(128),
    }
  }

  pub const fn set_total(&mut self, total: u64) {
    self.total = total;
  }

  pub fn mark_done(&mut self) {
    self.done_at.get_or_insert_with(Instant::now);
  }

  pub fn tick(&mut self, done: u64, retx: u64, pkts: u64) {
    let now = Instant::now();
    if now < self.next {
      return;
    }
    self.next = now + TICK;
    let (t0, b0) = self.last;
    let dt = now.duration_since(t0).as_f64();
    if dt > 0.0 {
      let inst = done.saturating_sub(b0) as f64 / dt;
      self.rate = if self.drawn {
        self.rate + EWMA * (inst - self.rate)
      } else {
        inst
      };
    }
    self.last = (now, done);
    self.draw(done, retx, pkts, false);
  }

  pub fn finish(&mut self, done: u64, retx: u64, pkts: u64) {
    let elapsed = self.elapsed();
    let secs = elapsed.as_f64();
    if secs > 0.0 {
      self.rate = done as f64 / secs;
    }
    self.draw(done, retx, pkts, true);
  }

  fn elapsed(&self) -> Duration {
    self
      .done_at
      .unwrap_or_else(Instant::now)
      .duration_since(self.start)
  }

  fn draw(&mut self, done: u64, retx: u64, pkts: u64, end: bool) {
    self.fmt_line(done, retx, pkts, end);
    let mut e = stderr().lock();
    let _ = e.write_all(self.buf.as_bytes());
    let _ = e.flush();
    self.drawn = true;
  }

  fn fmt_line(&mut self, done: u64, retx: u64, pkts: u64, end: bool) {
    let elapsed_secs = self.elapsed().as_secs();
    self.buf.clear();
    if self.tty {
      self.buf.push('\r');
    }
    let done = if self.total > 0 {
      done.min(self.total)
    } else {
      done
    };
    let pct = if self.total > 0 {
      done as f64 * 100.0 / self.total as f64
    } else {
      0.0
    };
    let fill = (((pct / 100.0) * BAR as f64) as usize).min(BAR);
    let retx = if pkts > 0 {
      retx as f64 * 100.0 / pkts as f64
    } else {
      0.0
    };
    self.buf.push_str(self.role.as_str());
    self.buf.push_str(" [");
    self.buf.push_str(&BAR_FILL[..fill * 3]);
    self.buf.push_str(&BAR_EMPTY[..(BAR - fill) * 3]);
    let _ = write!(self.buf, "] {pct:5.1}% ");
    push_size(&mut self.buf, done as f64);
    self.buf.push('/');
    push_size(&mut self.buf, self.total as f64);
    self.buf.push(' ');
    push_size(&mut self.buf, self.rate);
    let _ = write!(self.buf, "/s 重传 {retx:.1}% ");
    if end {
      self.buf.push_str("用时 ");
      push_dur(&mut self.buf, elapsed_secs);
    } else if self.rate >= 1.0 && self.total > done {
      self.buf.push_str("剩余 ");
      push_dur(
        &mut self.buf,
        ((self.total - done) as f64 / self.rate) as u64,
      );
    } else {
      self.buf.push_str("剩余 --");
    }
    if self.tty {
      self.buf.push_str("\x1b[K");
      if end {
        self.buf.push('\n');
      }
    } else {
      self.buf.push('\n');
    }
  }
}

fn push_size(out: &mut String, mut v: f64) {
  let mut u = 0;
  while v >= 1024.0 && u + 1 < UNITS.len() {
    v /= 1024.0;
    u += 1;
  }
  if u == 0 {
    let mut ibuf = ItoaBuffer::new();
    out.push_str(ibuf.format(v as u64));
  } else {
    let rounded = (v * 100.0).round() / 100.0;
    let mut fbuf = ZmijBuffer::new();
    out.push_str(fbuf.format_finite(rounded));
  }
  out.push(' ');
  out.push_str(UNITS[u]);
}

fn push_dur(out: &mut String, secs: u64) {
  let d = SignedDuration::from_secs(secs.min(i64::MAX as u64) as i64);
  let _ = write!(out, "{d:#}");
}
