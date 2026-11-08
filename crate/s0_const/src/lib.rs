pub mod region;

pub use region::{PROX_BASE, PROX_SAME_CITY, PROX_SAME_ZONE, REGION_ZONE_DIV, Region};
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
/// Ed25519 签名字节长度（64 B）
pub const SIG_LEN: usize = 64;
/// 无填充 `base64url` 编码的 32B `PeerId` 字符长度（43 字符）
pub const PEER_ID_B64_LEN: usize = 43;

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

/// 控制帧首字节协议版本号左移位数（高 4 位版本号，低 4 位 `Op`）
pub const PROTO_VER_SHIFT: u8 = 4;
/// 控制帧首字节操作码掩码（低 4 位）
pub const PROTO_OP_MASK: u8 = 0x0F;

/// 节点自签名名片最小合法字节长度：`MAGIC_LEN(4) + PEER_ID_LEN(32) + min_uvar_ts(1) + attr_cnt(1) + SIG_LEN(64) = 102 B`
pub const MIN_PROFILE_WIRE_LEN: usize = MAGIC_LEN + PEER_ID_LEN + 1 + 1 + SIG_LEN;

/// 二进制 `SocketAddr` 列表编码：IPv4 标签与长度
pub const ADDR_TAG_V4: u8 = 4;
pub const IPV4_OCTETS: usize = 4;
/// 二进制 `SocketAddr` 列表编码：IPv6 标签与长度
pub const ADDR_TAG_V6: u8 = 6;
pub const IPV6_OCTETS: usize = 16;
/// 端口号字节长度（2 B）
pub const PORT_LEN: usize = 2;
/// 单个节点通告的最大候选 `SocketAddr` 数量
pub const MAX_SOCKET_ADDRS: usize = 16;
/// 单条 IPv6 地址编码最大字节数（`1 + 16 + 2 = 19 B`）
pub const MAX_ENCODED_ADDR_LEN: usize = 1 + IPV6_OCTETS + PORT_LEN;

// ============================================================================
// 2. 协议版本、魔数与标志位常量
// ============================================================================

/// 协议版本号
pub const PROTO_VERSION: u8 = 2;

/// 内容寻址清单魔数 `"S0M2"`
pub const MANIFEST_MAGIC: [u8; MAGIC_LEN] = *b"S0M2";
/// 对等控制面请求/响应魔数 `"S0P2"` (LE)
pub const P2P_MAGIC: u32 = 0x3250_3053;
/// 订阅/群组实时推送帧魔数 `"S0B2"` (LE)
pub const SUB_MAGIC: u32 = 0x3242_3053;
/// 节点自签名名片魔数 `"S0C1"` (LE)
pub const PROFILE_MAGIC: u32 = 0x3143_3053;
/// 节点自签名名片 Ed25519 域分隔前缀
pub const PROFILE_SIG_DOMAIN: &[u8] = b"s0/profile/v1";

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

/// 缺失块位图全 1 标记（表示接收方已拥有全部块，0 字节位图负载）
pub const HAVE_ALL_ONES: u32 = 0xFFFF_FFFF;

/// TCP 控制帧长度前缀字节数（4 B）
pub const CTRL_LEN_HDR: usize = 4;
/// 单帧控制面消息最大字节数上限（64 MiB，防止恶意帧长度触发 OOM）
pub const MAX_CTRL_FRAME_LEN: usize = 64 * 1024 * 1024;
/// 控制面路径列表最大条目数
pub const MAX_CTRL_PATHS: usize = 1024;
/// 分享组允许上传路径前缀最大条目数
pub const MAX_WRITE_PREFIXES: usize = 64;
/// 启用 Zstd 压缩的最小字节阈值（256 B）
pub const MIN_ZSTD_BYTES: usize = 256;

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
/// 原生 UDP 保底打洞探测包 `encode_punch` 固定长度（4 字节 = 1 × `u32`）
pub const PUNCH_PKT_LEN: usize = 4;
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

/// 清单分片包 32 位小端魔数（`0x4D44_3053` = `"S0DM"`）
pub const DIR_MAGIC: u32 = u32::from_le_bytes(*b"S0DM");
/// 清单确认包 32 位小端魔数（`0x4144_3053` = `"S0DA"`）
pub const DIR_ACK_MAGIC: u32 = u32::from_le_bytes(*b"S0DA");
/// 传输完成包 32 位小端魔数（`0x4642_3053` = `"S0BF"`）
pub const FIN_MAGIC: u32 = u32::from_le_bytes(*b"S0BF");
/// 双栈竞速探测包 32 位小端魔数（`0x4250_3053` = `"S0PB"`）
pub const PROBE_MAGIC: u32 = u32::from_le_bytes(*b"S0PB");
/// 双栈竞速探测回包 32 位小端魔数（`0x4150_3053` = `"S0PA"`）
pub const PROBE_ACK_MAGIC: u32 = u32::from_le_bytes(*b"S0PA");
/// 双栈竞速 IPv6 优先裕度（5 ms = 5000 µs）
pub const V6_PREFER_MARGIN_US: u64 = 5_000;

/// 会话包头长度（12 B）
pub const SESSION_HLEN: usize = 12;
/// 高速流数据包头长度（20 B）
pub const DATA_HLEN: usize = 20;
/// 双栈竞速探测包长度（12 B：`magic u32 ‖ ts_us u64`）
pub const PROBE_LEN: usize = 12;
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
/// 数据包标志位：Reed-Solomon 校验包
pub const PAR: u8 = 1 << 3;
/// 数据包标志位：校验包数偏移（bit 4..7）
pub const R_SHIFT: u8 = 4;

/// 清单单包分片载荷长度（1200 B）
pub const MANIFEST_CHUNK: usize = 1200;
/// 清单块 / 控制帧 / `have_bits` 压缩标记位（bit 31）
pub const MANIFEST_ZSTD: u32 = 1 << 31;
/// `DIR_ACK` `have_bits` 全 1 游程标记位（bit 30：100% 命中本地基准块，0 字节载荷）
pub const HAVE_ALL_ONES_BIT: u32 = 1 << 30;
/// 清单头部标记：请求接收端清理不在清单中的多余文件（bit 31）
pub const MANIFEST_DELETE: u32 = 1 << 31;
/// 清单头部标记：当前清单仅为增量文件子集（bit 30）
pub const MANIFEST_PARTIAL: u32 = 1 << 30;
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
  pub const fn code(self) -> u8 {
    self as u8
  }

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

/// 对等分享组员访问权限
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, FromRepr)]
pub enum Perm {
  /// 只读成员（允许 `Stat`、`Pull`、`Sub`、`Have`、`ListShares` 及 P2P 做种）
  Ro = 1,
  /// 读写/上传成员（额外允许 `Push` 上传、`GroupJoin` 双向写入与 `FLAG_DELETE`）
  Rw = 2,
  /// 管理员/创建者（拥有全部读写上传与群组管理权限）
  Admin = 3,
}

impl Perm {
  /// 是否具备上传/修改权限 (`Rw` 或 `Admin`)
  #[inline]
  pub const fn can_write(self) -> bool {
    matches!(self, Self::Rw | Self::Admin)
  }

  /// 权限短标识字符串
  #[inline]
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Ro => "ro",
      Self::Rw => "rw",
      Self::Admin => "admin",
    }
  }

  /// 从配置字符串解析权限
  pub fn parse_str(s: &str) -> Option<Self> {
    let t = s.trim();
    if ["ro", "r", "read", "readonly", "1"]
      .iter()
      .any(|k| t.eq_ignore_ascii_case(k))
    {
      Some(Self::Ro)
    } else if ["rw", "w", "write", "readwrite", "upload", "2"]
      .iter()
      .any(|k| t.eq_ignore_ascii_case(k))
    {
      Some(Self::Rw)
    } else if ["admin", "owner", "all", "3"]
      .iter()
      .any(|k| t.eq_ignore_ascii_case(k))
    {
      Some(Self::Admin)
    } else {
      None
    }
  }
}
