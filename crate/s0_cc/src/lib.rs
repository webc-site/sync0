#![cfg_attr(docsrs, feature(doc_cfg))]

mod amp;
mod cc;
mod math;
mod rtt;

pub use amp::{AntiAmp, DEFAULT_AMP_FACTOR};
pub use cc::{Cc, delay_cwnd_gain, delay_pacing_gain};
pub use rtt::{MAX_RTO_US, MAX_TACK_US, MIN_TACK_US, RttEstimator, ack_interval_us};
