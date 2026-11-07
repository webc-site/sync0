mod block;
mod detect;
mod error;
mod level;

pub use block::{Codec, Encoded, FALLBACK_PCT, HEADER, MAX_BLOCK, SLACK};
pub use detect::{Detector, skip_magic, skip_name};
pub use error::{Error, FrameError, Result};
pub use level::{DEFAULT_LEVEL, LEVELS, LevelChoice, Policy};
