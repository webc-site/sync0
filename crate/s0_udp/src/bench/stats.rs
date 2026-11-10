use itoa::Buffer;
use s0_const::{MAX_BLOCK, Role};

#[derive(Default, Debug, Clone, Copy)]
pub struct Stats {
  pub bytes: u64,
  pub us: u64,
  pub packets: u64,
  pub retx: u64,
  pub blocks: u32,
  pub srtt_us: u64,
  pub rtt_min_us: u64,
}

impl Stats {
  pub fn line(&self, role: Role) -> String {
    let mut out = String::with_capacity(128);
    let mut b = Buffer::new();
    out.push_str("{\"role\":\"");
    out.push_str(role.as_str());
    out.push_str("\",\"bytes\":");
    out.push_str(b.format(self.bytes));
    out.push_str(",\"us\":");
    out.push_str(b.format(self.us));
    out.push_str(",\"packets\":");
    out.push_str(b.format(self.packets));
    out.push_str(",\"retx\":");
    out.push_str(b.format(self.retx));
    out.push_str(",\"blocks\":");
    out.push_str(b.format(self.blocks));
    out.push_str(",\"srtt_us\":");
    out.push_str(b.format(self.srtt_us));
    out.push_str(",\"rtt_min_us\":");
    out.push_str(b.format(self.rtt_min_us));
    out.push('}');
    out
  }
}

/// 已完成字节数（按 `done_lo` 块粒度）
pub(crate) const fn done_bytes(done_lo: u32, total: u64) -> u64 {
  let done = done_lo as u64 * MAX_BLOCK as u64;
  if done < total { done } else { total }
}
