use s0_const::ZSTD_DEFAULT_LEVEL;

/// 候选压缩级别
const LEVELS: [i32; 6] = [-7, -3, 1, 3, 6, 9];

/// 级别决策结果
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LevelChoice {
  Off,
  Level(i32),
}

/// EWMA 平滑系数
const A: f64 = 0.2;
/// 启用压缩所需的最小等效吞吐增益（相对于线路速率 `W`）
const MIN_GAIN: f64 = 1.05;
/// 初始单线程压缩速度（B/s）
const DEFAULT_S: [f64; LEVELS.len()] = [4.0e8, 2.5e8, 1.2e8, 7.0e7, 3.0e7, 1.0e7];
/// 初始压缩比估算（越小越好）
const DEFAULT_Q: [f64; LEVELS.len()] = [0.80, 0.70, 0.60, 0.50, 0.44, 0.40];

/// 级别选择 + 实测 EWMA（design §10）
///
/// `T_L = min(k·s_L, W/q_L, D)`，选最大；不足 `1.05·W` 则关闭。
pub struct Policy {
  threads: f64,
  peer_bps: f64,
  s: [f64; LEVELS.len()],
  q: [f64; LEVELS.len()],
}

impl Policy {
  pub fn new(threads: u32, peer_bps: u64) -> Self {
    Self {
      threads: threads.max(1) as f64,
      peer_bps: peer_bps as f64,
      s: DEFAULT_S,
      q: DEFAULT_Q,
    }
  }

  pub fn set_peer_bps(&mut self, peer_bps: u64) {
    self.peer_bps = peer_bps as f64;
  }

  /// 估算级别 `L` 的等效吞吐（B/s）
  fn estimate(&self, idx: usize, line_bps: u64) -> f64 {
    let mut t = self.s[idx] * self.threads;
    if line_bps > 0 {
      t = t.min(line_bps as f64 / self.q[idx]);
    }
    if self.peer_bps > 0.0 {
      t = t.min(self.peer_bps);
    }
    t
  }

  pub fn choose(&self, line_bps: u64) -> LevelChoice {
    if line_bps == 0 {
      return LevelChoice::Level(ZSTD_DEFAULT_LEVEL);
    }
    let mut best = (0.0f64, None);
    for (i, &level) in LEVELS.iter().enumerate() {
      let t = self.estimate(i, line_bps);
      if t > best.0 {
        best = (t, Some(level));
      }
    }
    match best.1 {
      Some(level) if best.0 >= line_bps as f64 * MIN_GAIN => LevelChoice::Level(level),
      _ => LevelChoice::Off,
    }
  }

  /// 实测后更新对应级别的速度与压缩比，并维持跨级别单调性不变量（`s` 与 `q` 随级别单调不增）
  pub fn observe(&mut self, level: i32, speed_bps: f64, ratio: f64) {
    if speed_bps <= 0.0 || !(0.0..=1.0).contains(&ratio) {
      return;
    }
    if let Some(i) = LEVELS.iter().position(|&l| l == level) {
      self.s[i] = self.s[i] * (1.0 - A) + speed_bps * A;
      self.q[i] = self.q[i] * (1.0 - A) + ratio * A;
      for j in (0..i).rev() {
        self.s[j] = self.s[j].max(self.s[j + 1]);
        self.q[j] = self.q[j].max(self.q[j + 1]);
      }
      for j in (i + 1)..LEVELS.len() {
        self.s[j] = self.s[j].min(self.s[j - 1]);
        self.q[j] = self.q[j].min(self.q[j - 1]);
      }
    }
  }

  pub fn stat(&self, level: i32) -> Option<(f64, f64)> {
    LEVELS
      .iter()
      .position(|&l| l == level)
      .map(|i| (self.s[i], self.q[i]))
  }
}
