pub use s0_const::DEFAULT_AMP_FACTOR;

/// 抗反射放大与路径源地址验证守卫（sans-IO，可独立复用于任意 UDP 服务端/接收端）
///
/// 不变式：`validated ∨ tx_bytes ≤ factor · rx_bytes`（计数只针对当前 `primary`）。
///
/// - 地址验证 = 往返证明：对端回送了只发往该地址的不可预测内容。
///   握手：OPEN 的 AEAD 密钥依赖 ACCEPT 中的 `e_s`，OPEN 校验通过即证明对端收到了发往该地址的 ACCEPT。
///   迁移：向新地址发 PROBE（路径挑战），新地址回送的 AEAD ACK 确认了该 PROBE 的 `pn`。
/// - 会话绑定首个对端地址；其他源地址的包 `on_recv` 返回 `false`，由调用方决定是否 `migrate_to`
///   （仅限通过 AEAD 校验的包，防止伪造源地址把流量反射到受害者）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AntiAmp<A: Copy + Eq> {
  primary: Option<A>,
  rx_bytes: u64,
  tx_bytes: u64,
  factor: u64,
  validated: bool,
}

impl<A: Copy + Eq> Default for AntiAmp<A> {
  fn default() -> Self {
    Self::new(DEFAULT_AMP_FACTOR)
  }
}

impl<A: Copy + Eq> AntiAmp<A> {
  /// `factor = 0` 按 1 处理（至少允许对称回应）
  pub const fn new(factor: u64) -> Self {
    Self {
      primary: None,
      rx_bytes: 0,
      tx_bytes: 0,
      factor: if factor == 0 { 1 } else { factor },
      validated: false,
    }
  }

  pub const fn primary(&self) -> Option<A> {
    self.primary
  }

  pub const fn is_validated(&self) -> bool {
    self.validated
  }

  /// 剩余发送预算（已验证返回 `u64::MAX`）
  pub const fn allowance(&self) -> u64 {
    if self.validated {
      return u64::MAX;
    }
    self
      .rx_bytes
      .saturating_mul(self.factor)
      .saturating_sub(self.tx_bytes)
  }

  /// 记录收到来自 `addr` 的 `bytes` 字节：未绑定时锁定 `addr`；
  /// 来自 `primary` 返回 `true` 并累加预算，其他地址返回 `false`（不计入、不切换）
  pub fn on_recv(&mut self, addr: A, bytes: usize) -> bool {
    match self.primary {
      Some(p) if p != addr => false,
      _ => {
        self.primary = Some(addr);
        self.rx_bytes = self.rx_bytes.saturating_add(bytes as u64);
        true
      }
    }
  }

  /// 是否允许向 `primary` 发送 `bytes` 字节
  pub const fn can_send(&self, bytes: usize) -> bool {
    (bytes as u64) <= self.allowance()
  }

  /// 登记已发送 `bytes` 字节
  pub fn on_send(&mut self, bytes: usize) {
    self.tx_bytes = self.tx_bytes.saturating_add(bytes as u64);
  }

  /// 当前 `primary` 完成地址验证（握手 OPEN 校验通过 / 迁移 PROBE 被新地址确认）
  pub fn mark_validated(&mut self) {
    self.validated = true;
  }

  /// 切换到新地址 `addr`（触发包已通过 AEAD 校验，`bytes` 为其长度）：
  /// 预算重置为 `factor · bytes`，待 PROBE 被确认后 `mark_validated`
  pub fn migrate_to(&mut self, addr: A, bytes: usize) {
    *self = Self {
      primary: Some(addr),
      rx_bytes: bytes as u64,
      tx_bytes: 0,
      validated: false,
      ..*self
    };
  }
}
