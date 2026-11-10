mod manifest;
mod manifest_net;
mod prefetch;
mod probe;
mod rx;
mod sink;
mod sink_basis;
mod source;
mod stats;
mod tx;
mod tx_state;

pub use probe::{handle_probe_observed, make_probe, parse_probe_ack_observed};
pub use rx::{recv, recv_opt};
pub use stats::Stats;
pub use tx::{send, send_opt};

/// 空槽标记
pub(crate) const EMPTY: u32 = u32::MAX;
pub(crate) const IDLE_US: u64 = 15_000_000;
