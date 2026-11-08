//! `sync0` 配置与格式解析库 (`s0_conf`)
//!
//! 职责：
//! 1. **带宽单位解析 (`rate`)**：单次遍历零分配状态机支持 `GB`、`MB`、`KB`、`TB`、`B`（含小数与可选 `/s` 后缀）及纯数字字节/秒；
//! 2. **资源定位符与路径前缀 (`uri`)**：解析 `组名://路径1,路径2` 多选语法与路径前缀归一化；
//! 3. **NestedText (`.nt`) 配置解析 (`nt`)**：支持独立用户配置文件目录 `users_dir` 自动包含与无序列表语法。

mod error;
mod nt;
mod rate;
mod uri;

pub use error::ConfError;
pub use nt::{NtGroupSpec, NtPeerGrant, parse_nt_groups};
pub use rate::{parse_rate, parse_rate_str};
pub use uri::{GroupUri, extend_clean_paths_csv, normalize_prefixes};
