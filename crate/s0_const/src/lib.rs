pub mod region;

use strum::FromRepr;

// ============================================================================
// 1. 基础尺寸、掩码与变长整数常量（全局唯一定义，消除魔法数字）
// ============================================================================

/// 4 字节协议魔数长度
pub const MAGIC_LEN: usize = 4;
/// 128-bit `gxhash128` 摘要 / `root_hash` / `share_id` / `token` 字节长度（16 B）
pub const HASH_LEN: usize = 16;
/// 分享组 ID 字节长度（16 B）
pub const SHARE_ID_LEN: usize = HASH_LEN;
/// 能力令牌字节长度（16 B）
pub const TOKEN_LEN: usize = HASH_LEN;
/// `iroh` Ed25519 节点公钥 (`PeerId` / `EndpointId`) 字节长度（32 B）
pub const PEER_ID_LEN: usize = 32;
/// `iroh` Ed25519 节点私钥字节长度（32 B）
pub const SECRET_KEY_LEN: usize = 32;
/// 全零 32B `PeerId` 占位常量
pub const ZERO_PEER_ID: [u8; PEER_ID_LEN] = [0u8; PEER_ID_LEN];
/// 全零 16B `Hash128` / `root_hash` / `share_id` / `token` 占位常量
pub const ZERO_HASH: [u8; HASH_LEN] = [0u8; HASH_LEN];
/// Ed25519 签名字节长度（64 B）
pub const SIG_LEN: usize = 64;

/// `u15` 变长整数最大值 (`32767 = 0x7FFF`)
pub const U15_MAX: usize = 0x7FFF;
/// `u15` / `uvar` 变长整数延续高位标记 (`0x80`)
pub const VARINT_CONT_BIT: u8 = 0x80;
/// `u15` / `uvar` 变长整数单字节有效低 7 位掩码 (`0x7F`)
pub const VARINT_VAL_MASK: u8 = 0x7F;
/// `uvar` (ULEB128) 每步位移位数
pub const UVAR_STEP_BITS: u32 = 7;
/// `uvar` (ULEB128) 64 位整数最大位移上界
pub const UVAR_MAX_SHIFT: u32 = 63;

/// 24-bit UDP 会话标识掩码 (`0x00FF_FFFF`)
pub const SID24_MASK: u32 = 0x00FF_FFFF;
/// 24-bit UDP 会话标识在首 32 位字中的左移位数（低 8 位存放包类型或 `0x00`）
pub const SID24_SHIFT: u32 = 8;

/// 节点自签名名片最小合法字节长度：`MAGIC_LEN(4) + PEER_ID_LEN(32) + min_uvar_ts(1) + attr_cnt(1) + SIG_LEN(64) = 102 B`
pub const MIN_PROFILE_WIRE_LEN: usize = MAGIC_LEN + PEER_ID_LEN + 1 + 1 + SIG_LEN;

/// 二进制 `SocketAddr` 列表编码：IPv4 标签与长度
pub const ADDR_TAG_V4: u8 = 4;
pub const IPV4_OCTETS: usize = 4;
/// 二进制 `SocketAddr` 列表编码：IPv6 标签与长度
pub const ADDR_TAG_V6: u8 = 6;
pub const IPV6_OCTETS: usize = 16;
/// 端口号字节长度（2 B）
const PORT_LEN: usize = 2;
/// 单个节点通告的最大候选 `SocketAddr` 数量
pub const MAX_SOCKET_ADDRS: usize = 16;
/// 单条 IPv6 地址编码最大字节数（`1 + 16 + 2 = 19 B`）
pub const MAX_ENCODED_ADDR_LEN: usize = 1 + IPV6_OCTETS + PORT_LEN;

// ============================================================================
// 2. 协议版本、魔数与标志位常量
// ============================================================================

/// 客户端握手与帧头 1 字节协议版本号
pub const PROTO_VERSION: u8 = 2;

/// 内容寻址清单魔数 `"S0M2"` (`0x324D_3053`, LE)
pub const MANIFEST_MAGIC: u32 = 0x324D_3053;
/// 对等控制面请求/响应魔数 `"S0P2"` (LE)
pub const P2P_MAGIC: u32 = 0x3250_3053;
/// 订阅/群组实时推送帧魔数 `"S0B2"` (LE)
pub const SUB_MAGIC: u32 = 0x3242_3053;
/// `iroh` 控制与增量变更推送 ALPN 协议标识 (`"s0/delta/1"`)
pub const S0_IROH_ALPN: &[u8] = b"s0/delta/1";
/// 订阅者与基准快照持久化文件魔数 `"S0S1"` (`0x3153_3053`, LE)
pub const SUBS_MAGIC: u32 = 0x3153_3053;
/// `.s0_subs.bin` AOF 增量日志帧头长度（7 B：`len: u32 LE ‖ crc16: u16 LE ‖ tag: u8`）
pub const SUBS_AOF_HDR_LEN: usize = 7;
/// `.s0_subs.bin` 触发全量 RDB 重写整理的增量 AOF 追加次数阈值（64 次）
pub const SUBS_AOF_REWRITE_OPS: u32 = 64;
/// `.s0_subs.bin` 触发全量 RDB 重写整理的最小 AOF 尾部字节阈值（64 KiB）
pub const SUBS_AOF_MIN_REWRITE_BYTES: u64 = 64 * 1024;
/// 节点自签名名片魔数 `"S0C1"` (LE)
pub const PROFILE_MAGIC: u32 = 0x3143_3053;

/// 用户名最大字符数（Unicode 标量值个数）
pub const MAX_NAME_CHARS: usize = 64;
/// 邮箱最大字节长度
pub const MAX_EMAIL_LEN: usize = 128;
/// 网址最大字节长度
pub const MAX_URL_LEN: usize = 256;
/// 个性签名最大字符数（Unicode 标量值个数）
pub const MAX_BIO_CHARS: usize = 256;

/// 请求/推送标志位：镜像清理多余文件
pub const FLAG_DELETE: u8 = 1 << 0;
/// 请求/推送标志位：增量路径子集更新
pub const FLAG_PARTIAL: u8 = 1 << 1;
/// 请求标志位：指定块区间 `[blk_lo, blk_hi)` 与快照 `root_hash`
pub const FLAG_RANGE: u8 = 1 << 2;
/// 请求标志位：声明请求方开启本地 UDP P2P 供块服务
pub const FLAG_SEED: u8 = 1 << 3;
/// 请求标志位：携带读写能力令牌或请求读写权限
pub const FLAG_RW: u8 = 1 << 4;
/// 请求标志位：指定点播/随机读取字节区间 `[byte_lo, byte_hi)`（`byte_hi == u64::MAX` 表示 `byte_lo..EOF`）
pub const FLAG_BYTE_RANGE: u8 = 1 << 5;
/// 响应/推送标志位：标识分享组开启了私有成员认证（限制公网匿名读取，做种节点同步继承白名单）
pub const FLAG_AUTH: u8 = 1 << 6;

/// TCP 控制帧长度前缀字节数（4 B）
pub const CTRL_LEN_HDR: usize = 4;
/// 单帧控制面消息最大字节数上限（64 MiB，防止恶意帧长度触发 OOM）
pub const MAX_CTRL_FRAME_LEN: usize = 64 * 1024 * 1024;
/// 启用 Zstd 压缩的最小字节阈值（256 B）
pub const MIN_ZSTD_BYTES: usize = 256;
/// 默认 POSIX 文件权限位（`0o644 = rw-r--r--`，清单中等于默认值时零字节编码）
pub const DEFAULT_FILE_MODE: u16 = 0o644;
/// POSIX 文件权限位有效低 9 位掩码（`0o777 = rwxrwxrwx`）
pub const FILE_MODE_MASK: u16 = 0o777;

/// 归一化 POSIX 文件权限位（保留低 9 位 `0o777`，`0` 自动回退为默认 `0o644`）
#[inline]
pub const fn norm_mode(mode: u16) -> u16 {
  let m = mode & FILE_MODE_MASK;
  if m == 0 { DEFAULT_FILE_MODE } else { m }
}

/// 取两个速率/配额中的最小非零值（`0` 表示不限制）
#[inline]
pub const fn min_nonzero(a: u64, b: u64) -> u64 {
  match (a, b) {
    (0, x) | (x, 0) => x,
    (x, y) => {
      if x < y {
        x
      } else {
        y
      }
    }
  }
}

/// 从 `root_hash` 前 3 字节与区间起始块号/客户端端口盐值派生 24-bit UDP 会话标识（`salt = 0` 用于单流 Push）
#[inline]
pub const fn derive_session_id(root_hash: &[u8; HASH_LEN], salt: u32) -> u32 {
  ((((root_hash[0] as u32) << 16) | ((root_hash[1] as u32) << 8) | (root_hash[2] as u32)) ^ salt)
    & SID24_MASK
}

// ============================================================================
// 3. 原生 UDP 数据面与 TACK 协议头常量
// ============================================================================

/// 原生 UDP 数据包头固定长度（20 字节）
pub const DATA_HDR_LEN: usize = 20;
/// `DataHeader` 参与 CRC16 校验的前置字节数（18 字节）
pub const DATA_CRC_INPUT_LEN: usize = 18;
/// `DataHeader` CRC16 初始种子
pub const DATA_CRC_SEED: u32 = 0x5A3C;
/// `DataHeader` CRC16 黄金比例乘数
pub const DATA_CRC_MUL: u32 = 0x9E37;

/// 原生 UDP 完成确认包 `AckPkt` 固定长度（12 字节 = 3 × `u32`）
pub const ACK_PKT_LEN: usize = 12;
/// 原生 UDP 保底打洞握手探测包 `encode_punch` 固定长度（5 字节 = `u32` 会话头 + 1B `PROTO_VERSION`）
pub const PUNCH_PKT_LEN: usize = 5;
/// 单包 `NackPkt` 最大缺失块区间数
pub const NACK_MAX_SPANS: usize = 256;
/// 单包 `NackPkt` 最大部分到达块分片掩码数
pub const NACK_MAX_PARTIALS: usize = 64;

/// 默认单包 UDP 分片有效载荷字节数
pub const DEFAULT_SEG_PAYLOAD: usize = 1380;
/// 隧道/Overlay 网卡（Tailscale / WireGuard / IPv6 最小 MTU 1280）单包有效载荷上限（`1200 + 20 = 1220 <= 1280`）
pub const TUNNEL_SEG_PAYLOAD: usize = 1200;
/// 单包 UDP 接收缓冲区大小
pub const MAX_UDP_PKT: usize = 1500;

/// UDP 反馈包类型：缺失块 NACK
pub const PKT_NACK: u8 = 0x81;
/// UDP 反馈包类型：区间完成 ACK
pub const PKT_ACK: u8 = 0x82;
/// UDP 打洞探测包类型
pub const PKT_PUNCH: u8 = 0x83;

/// 原生 UDP 块载荷是否经 Zstd 压缩标志位（复用 `DataHeader.seg_cnt` bit 15）
pub const DATA_FLAG_ZSTD: u16 = 0x8000;
/// `SuperBlock` 聚合帧在 `DataHeader.offset_in_block` 高 8 位编码 `extra_blocks` 的位移位数（24）
pub const SUPER_EXTRA_SHIFT: u32 = 24;
/// `SuperBlock` 聚合帧在 `DataHeader.offset_in_block` 低 24 位编码块内偏移的掩码 (`0x00FF_FFFF`)
pub const SUPER_OFFSET_MASK: u32 = 0x00FF_FFFF;

/// 清单分片包 32 位小端魔数（`0x4D44_3053` = `"S0DM"`）
pub const DIR_MAGIC: u32 = 0x4D44_3053;
/// 清单确认包 32 位小端魔数（`0x4144_3053` = `"S0DA"`）
pub const DIR_ACK_MAGIC: u32 = 0x4144_3053;
/// 传输完成包 32 位小端魔数（`0x4642_3053` = `"S0BF"`）
pub const FIN_MAGIC: u32 = 0x4642_3053;
/// 双栈竞速探测包 32 位小端魔数（`0x4250_3053` = `"S0PB"`）
pub const PROBE_MAGIC: u32 = 0x4250_3053;
/// 双栈竞速探测回包 32 位小端魔数（`0x4150_3053` = `"S0PA"`）
pub const PROBE_ACK_MAGIC: u32 = 0x4150_3053;
/// 双栈竞速 IPv6 优先裕度（5 ms = 5000 µs）
pub const V6_PREFER_MARGIN_US: u64 = 5_000;

/// 会话包头长度（12 B）
pub const SESSION_HLEN: usize = 12;
/// 高速流数据包头长度（20 B）
pub const DATA_HLEN: usize = 20;
/// 双栈竞速握手探测包长度（13 B：`magic u32 ‖ ver u8 ‖ ts_us u64`）
pub const PROBE_LEN: usize = 13;
/// 清单分片包头长度（12 B：`magic u32 ‖ total_len u32 ‖ idx u32`）
pub const DIR_HDR_LEN: usize = 12;
/// 清单确认包头长度（8 B：`magic u32 ‖ chunks u32`）
pub const DIR_ACK_HDR_LEN: usize = 8;

/// UDP 最大报文缓冲长度（2048 B）
pub const MAX_DATAGRAM: usize = 2048;
/// 默认单包载荷长度（1400 B，标准以太网最大吞吐）
pub const DEFAULT_PKT: usize = 1400;
/// 隧道/Overlay 网卡与 IPv6 最小 MTU (1280 B) 零分片安全单包载荷长度（1200 B）
pub const TUNNEL_PKT: usize = 1200;
/// 默认数据块字节上限（128 KiB）
pub const MAX_BLOCK: usize = 128 * 1024;
/// 单块最大分片位图槽位数（128，对应 `u128` 块位图）
pub const MAX_CNT: usize = 128;
/// 发送/接收滑动窗口块数（512 块 = 64 MiB）
pub const WINDOW_BLOCKS: u32 = 512;

/// 会话包标志位：密钥相位
pub const PHASE: u8 = 1;
/// 数据包标志位：重传包
pub const RETX: u8 = 1 << 1;
/// 数据包标志位：本块经 Zstd 压缩
pub const ZSTD: u8 = 1 << 2;

/// 清单单包分片载荷长度（1200 B）
pub const MANIFEST_CHUNK: usize = 1200;
/// 清单块 / 控制帧 / `have_bits` 压缩标记位（bit 31）
pub const MANIFEST_ZSTD: u32 = 1 << 31;
/// `DIR_ACK` `have_bits` 全 1 游程标记位（bit 30：100% 命中本地基准块，0 字节载荷）
pub const HAVE_ALL_ONES_BIT: u32 = 1 << 30;
/// 清单头部标记：请求接收端清理不在清单中的多余文件（bit 31）
pub const MANIFEST_DELETE: u32 = 1 << 31;
/// 默认 Zstd 压缩级别
pub const ZSTD_DEFAULT_LEVEL: i32 = 3;

/// `ACK.delay` 时间单位（8 µs）
pub const ACK_DELAY_US: u64 = 8;
/// 单包 `ACK` 携带的最大连续确认区间数
pub const MAX_RANGES: usize = 255;
/// 单包 `ACK` 携带的最大未完成块位图数
pub const ACK_BLOCKS: usize = 8;
/// 接收端扫描未完成块位图的窗口上界
pub const ACK_SCAN_BLOCKS: u32 = 64;
/// 缺失块最小有效权重（1 KiB），防止极小块被算术截断为 0
pub const MIN_MISSING_BLOCK_WEIGHT: u64 = 1024;
/// 边下边传（Leecher Seeding）最小触发与最小汇报步长块数（4 块 ≈ 128 KiB，兼顾小文件快速做种）
pub const LEECH_MIN_BLOCKS: u32 = 4;
/// P2P 宏块（Macro-Piece）最大汇报与调度步长块数（256 块 ≈ 8 MiB，大幅收敛大文件控制面开销）
pub const MACRO_PIECE_MAX_BLOCKS: u32 = 256;
/// 单个对等节点在 Swarm 路由表中最多通告的不相交连续块游程段数（Roaring Run-Container 稀疏段上限）
pub const MAX_PEER_RUNS: usize = 8;

/// 计算自适应 P2P 宏块（Macro-Piece）步长块数：按 `total_blocks / 8` 在 `[4 (~128 KiB), 256 (~8 MiB)]` 间自适应缩放
#[inline]
pub const fn macro_piece_blocks(total_blocks: u32) -> u32 {
  let step = total_blocks >> 3;
  if step < LEECH_MIN_BLOCKS {
    LEECH_MIN_BLOCKS
  } else if step > MACRO_PIECE_MAX_BLOCKS {
    MACRO_PIECE_MAX_BLOCKS
  } else {
    step
  }
}
/// 接收端/发送端完成突发 ACK 重传包数
pub const ACK_BURST_COUNT: usize = 3;
/// 原生 UDP 套接字单次等待超时（6 ms）
pub const SOCK_POLL_TIMEOUT_MS: u64 = 6;
/// 主打洞首选端点切换时间窗（350 ms）
pub const PUNCH_PRIMARY_WINDOW_MS: u64 = 350;

// ============================================================================
// 4. 数值枚举原语（零字符串线协议）
// ============================================================================

/// 传输端点角色（数值枚举）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Role {
  Send = 0,
  Recv = 1,
}

impl Role {
  #[inline]
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Send => "send",
      Self::Recv => "recv",
    }
  }
}

/// 对等网络控制面数值操作码
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromRepr)]
pub enum Op {
  /// 拉取单文件、单目录、多选路径集合或指定块区间 `[blk_lo, blk_hi)`
  Pull = 1,
  /// 向拥有写权限的对等节点推送单文件、目录或多选路径变更
  Push = 2,
  /// 查询目标分享组 + 多选路径的 `Manifest`、`root_hash`、节点名片及可用 P2P 节点列表
  Stat = 3,
  /// 长连接订阅分享组更新（全量或多选文件/目录集合），同时注册为 P2P 分发节点
  Sub = 4,
  /// 订阅/群组节点上报已完成校验的 `root_hash` 与块区间 `[blk_lo, blk_hi)`
  Have = 5,
  /// 加入群组相互备份，通告本节点名片、导出的分享组及多选备份路径策略
  GroupJoin = 6,
  /// 查询对等节点名下托管的全部文件分享组与当前访问权限
  ListShares = 7,
}

/// 对等网络控制面数值响应状态码
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromRepr)]
pub enum Status {
  /// 成功
  Ok = 0,
  /// 未授权（开启认证且 `EndpointId` / 令牌不匹配）
  Unauth = 1,
  /// 权限不足（只读权限尝试 `Push` 或 `FLAG_DELETE`）
  Forbidden = 2,
  /// 请求的 Share 或子路径不存在
  NotFound = 3,
  /// 请求的 `root_hash` 快照已过期或不一致
  Conflict = 4,
  /// 协议格式或路径参数非法
  BadReq = 5,
}

/// 节点自签名名片属性键数字常量
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, FromRepr)]
pub enum AttrKey {
  /// 用户名（UTF-8，最长 64 字符）
  Name = 1,
  /// 邮箱（UTF-8，最长 128 字节）
  Email = 2,
  /// 网址（UTF-8，最长 256 字节）
  Url = 3,
  /// 城市级地域数字常量（2 字节小端 `u16`，对应 `s0_const::Region`）
  Region = 4,
  /// 个性签名（UTF-8，最长 256 字符）
  Bio = 5,
}

/// 实时同步文件变更操作码（数值枚举，1B）
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, FromRepr)]
pub enum FsOp {
  /// 文件新增或内容修改（携带 `mode`、`mtime` 与 `ChunkSig` 块签名列表，客户端按需增量拉取差异块）
  Upsert = 1,
  /// 文件或目录删除（仅携带路径，客户端本地删除并清理空父目录）
  Remove = 2,
  /// 仅文件元数据（权限 `mode` 或最后修改时间 `mtime`）修改，内容未变（仅携带路径、`mode: u15` 与 `mtime: uvar`，零块签名、零 UDP 传输，客户端直接原地更新元数据）
  Meta = 3,
}

/// `.s0_subs.bin` RDB+AOF 混合持久化增量日志记录标签（数值枚举，1B）
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromRepr)]
pub enum SubsAofTag {
  /// 新增或更新单个订阅/做种节点记录（`endpoint_id + eff_rate + ctrl_addrs + paths`）
  UpsertSub = 1,
  /// 批量移除失效订阅/做种节点记录（`dead_cnt + [endpoint_id + ctrl_addrs]`）
  RemoveSub = 2,
  /// 基准清单增量变更记录（`change_cnt + [PathChange]`，免全量序列化 `Manifest`）
  ManifestDelta = 3,
}

/// 咆哮位图（`roaring::RoaringBitmap`）帧标签（1B 数值枚举）
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, FromRepr)]
pub enum RoaringTag {
  /// 全 0 空位图（无任何已就绪块，仅占 1B）
  Empty = 0,
  /// 全 1 满位图（全部块均已就绪，仅占 1B）
  Full = 1,
  /// 咆哮位图（`roaring::RoaringBitmap` 经 `optimize()` Run/Array/Bitmap 容器自适应优化后的紧凑序列化流）
  Roaring = 2,
  /// 原始定长位图（当小文件原始位图比 Roaring 容器头更短时自动退化为 `1B + raw_bits`）
  Raw = 3,
}

/// 对等分享组员访问权限（位掩码数值枚举：`1 = R`, `2 = W`, `3 = Rw`，默认 `R` 只读）
#[repr(u8)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, FromRepr)]
pub enum Perm {
  /// 只读（默认权限，配置中可省略：允许 `ListShares`、`Stat`、`Pull`、`Sub`、`Have` 及 P2P 做种）
  #[default]
  R = 1,
  /// 只写（允许 `ListShares` 与在授权路径前缀内 `Push` 上传）
  W = 2,
  /// 读写（具备完整读写、`Push` 上传、`GroupJoin` 双向同步与 `--delete` 权限）
  Rw = 3,
}

impl Perm {
  /// 是否具备读取/拉取/订阅权限 (`R` 或 `Rw`)
  #[inline]
  pub const fn can_read(self) -> bool {
    (self as u8) & (Self::R as u8) != 0
  }

  /// 是否具备上传/修改权限 (`W` 或 `Rw`)
  #[inline]
  pub const fn can_write(self) -> bool {
    (self as u8) & (Self::W as u8) != 0
  }

  /// 从配置字符串解析权限（支持 `r`、`w`、`rw` 及对应数值 `1`、`2`、`3`）
  pub fn parse_str(s: &str) -> Option<Self> {
    match s.trim() {
      "r" | "R" | "1" => Some(Self::R),
      "w" | "W" | "2" => Some(Self::W),
      "rw" | "RW" | "wr" | "WR" | "3" => Some(Self::Rw),
      _ => None,
    }
  }
}
