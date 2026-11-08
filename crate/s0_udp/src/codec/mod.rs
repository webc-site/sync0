mod block;
mod detect;
mod error;
mod level;

pub use block::{Codec, Encoded, FALLBACK_PCT, HEADER, SLACK};
pub use detect::{Detector, skip_magic, skip_name};
pub use error::{Error, FrameError, Result};
pub use level::{LEVELS, LevelChoice, Policy};
