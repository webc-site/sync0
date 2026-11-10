# `sync0` 二进制协议规范

所有控制面、身份名片、内容清单与 UDP 数据面报文均采用小端（Little-Endian）二进制、位打包与变长整数（`u15` / `uvar` ULEB128）编码，**零 JSON、零字符串键名、零填充字节**。全部协议常量统一定义于 [`s0_const`](../../crate/s0_const/src/lib.rs)。

---

## 1. 基础编码原语

- **`u15` 变长整数**（`0..=32767`）：`< 128` 占 1B（`v as u8`）；`>= 128` 占 2B（`[0x80 | (v >> 8), v & 0xFF]`）。
- **`uvar` 变长整数（ULEB128）**：每字节低 7 位为数值，最高位 `1` 表示后续仍有字节。
- **`strs_front` 前缀差分压缩字符串列表（Front-Coding）**：首项为 `slice_u15(s[0])`，后续项为 `shared_prefix_len: u15` + `suffix: slice_u15`。
- **客户端连接与会话握手 1 字节协议版本号 `ver: u8` (`PROTO_VERSION = 2`)**：客户端发起 TCP 控制连接、`iroh` QUIC 双向流、控制请求帧 `P2pReq`、变更推送帧 `SubPush` 及 UDP 打洞/探测握手时，均携带独立 1 字节协议版本号 `ver: u8`，便于未来平滑演进与版本协商。
- **协议魔数**：清单 `"S0M2"`（`0x324D_3053`）、控制面 `"S0P2"`（`0x3250_3053`）、订阅通知 `"S0B2"`（`0x3242_3053`）、做种持久化 `"S0S1"`（`0x3153_3053`）、名片 `"S0C1"`（`0x3143_3053`）。

### 1.1 操作码 `Op` (`u8`)

| 值 | 名称 | 语义 |
| :--- | :--- | :--- |
| `1` | `Pull` | 拉取分享组、多选路径集合、指定块区间 `[blk_lo, blk_hi)` 或字节区间 `[byte_lo, byte_hi)` |
| `2` | `Push` | 上传文件、目录或多选路径集合（校验权限与 `write_prefix`） |
| `3` | `Stat` | 查询清单、`root_hash`、节点名片及可用 Swarm 种子列表 |
| `4` | `Sub` | 长连接订阅变更通知并注册为做种源 |
| `5` | `Have` | 上报已完成校验的 `root_hash` 与块区间 `[blk_lo, blk_hi)` |
| `6` | `GroupJoin` | 加入双向同步群组，通告名片与同步路径 |
| `7` | `ListShares` | 查询节点公开或对当前访问者授权的全部分享组摘要 |

### 1.2 权限掩码 `Perm` (`u8`)、文件变更操作码 `FsOp` (`u8`) 与名片属性键 `AttrKey` (`u8`)

- **`Perm`（位掩码，默认只读 `1 = R`，配置中可省略）**：`1 = R`（只读）、`2 = W`（仅写投递）、`3 = Rw`（`R | W` 读写）。
- **`FsOp`（实时订阅路径变更数字操作码）**：`1 = Upsert`（新增或内容修改，含 `mode + mtime + ChunkSig`）、`2 = Remove`（删除文件并清理空父目录）、`3 = Meta`（仅元数据变更：仅同步 `mode` POSIX 权限或 `mtime` 毫秒修改时间，零 `ChunkSig` 签名、零 UDP 块传输）。
- **`AttrKey`**：`1 = Name`（`<= 64` 字符）、`2 = Email`（`<= 128 B`）、`3 = Url`（`<= 256 B`）、`4 = Region`（定长 `u16 LE` 城市编码）、`5 = Bio`（`<= 256` 字符）。
- **城市编码 `Region` (`u16`)**：按百位划分大区（`zone = id / 100`），同城亲和度 `300`、同大区 `200`、跨洲 `100`（如 `101 = BJS`、`102 = SHA`、`107 = HKG`、`109 = TYO`、`201 = SIN`、`301 = SFO`、`401 = FRA`）。

---

## 2. 自签名名片 `SignedProfile` (最小 102 B)

```text
magic: u32 ("S0C1") | pubkey: [u8; 32] | ts_sec: uvar | attr_cnt: u8
attrs: [key: u8 + (Region ? u16 LE : slice_u15); attr_cnt]
sig: [u8; 64] (Ed25519 签名)
```

---

## 3. 内容寻址清单 `Manifest`

```text
magic: "S0M2" | flags: u8 (1=DELETE, 2=PARTIAL) | entry_cnt: uvar (支持 u64::MAX，无硬编码条数上限)
entries:
  path: Front-Coding (首项 slice_u15，后续 shared: u15 + suffix: slice_u15)
  tag:  uvar (0 = 删除墓碑; ((n << 1) | has_meta) + 1 = 含 n 个数据块，has_meta 标识非默认元数据)
  [has_meta == 1]: mode: u15 (POSIX 权限低 9 位 0o777，默认 0o644) | mtime: uvar (毫秒级 Unix 时间戳)
  chunks: [len: uvar + hash: [u8; 16] (gxhash128); n]
```

---

## 4. TCP 控制面帧

客户端建立 TCP 连接时首先发送 1 字节协议版本号 `ver: u8 (PROTO_VERSION = 2)` 完成版本握手。后续帧头 `u32` 长度最高位 `1 << 31` 标识 Zstd 压缩（`>= 256 B` 自动压缩，首部带 `uvar` 原始长度）。

### 4.1 请求帧 `P2pReq`
```text
magic: u32 ("S0P2") | ver: u8 (PROTO_VERSION) | op: u8 | flags: u8
share_id: [u8; 16]  | req_rate: uvar | paths: strs_front
peer_addrs: slice_u15 | profile_card: slice_u15
[FLAG_RANGE]:      root_hash: [u8; 16] | blk_lo: uvar | blk_hi: uvar
[FLAG_RW]:         token: [u8; 16]
[FLAG_BYTE_RANGE]: byte_lo: uvar | byte_hi: uvar
```

### 4.2 响应帧 `P2pResp`、`SwarmPeer`、`ShareSummary` 与咆哮位图 `have_bits`
```text
P2pResp:
  magic: u32 ("S0P2") | status: u8 | flags: u8 (FLAG_AUTH = 1<<6 标识该分享组受私有白名单保护，做种节点同步继承可读成员 ACL) | eff_rate: uvar
  root_hash: [u8; 16] | peer_addrs: slice_u15 | profile_card: slice_u15
  peer_cnt: uvar | peers: [SwarmPeer; peer_cnt]
  mf_len: uvar   | manifest_bytes: [u8; mf_len]

SwarmPeer:
  endpoint_id: [u8; 32] | region: u16 LE | weight_rate: uvar
  blk_lo: uvar | blk_hi: uvar | peer_addrs: slice_u15 | profile_card: slice_u15

ShareSummary (ListShares 响应负载):
  share_id: [u8; 16] | name: slice_u15 | perm: u8 | eff_rate: uvar | write_prefixes: strs_front

Roaring have_bits (Op::Pull 请求附带帧 / Op::Push 响应附带帧):
  tag: u8 (RoaringTag: 0 = Empty 全 0 空位图 [1B], 1 = Full 全 1 满位图 [1B], 2 = Roaring 经 optimize() Run/Array/Bitmap 容器压缩流, 3 = Raw 原始位图字节流)
  body: (tag == 0 | 1 ? empty : tag == 2 ? roaring_portable_bytes : raw_bitset_bytes)

SocketAddr 二进制序列 (peer_addrs / origin_addrs 载荷，无冗余计数前缀，由外层 slice_u15 定界):
  repeat [tag: u8 (4 = IPv4 + 4B IP + 2B Port LE | 6 = IPv6 + 16B IP + 2B Port LE)]
```

### 4.3 `iroh` 实时增量变更推送帧 `SubPush` (`mzstd` 压缩) 与做种持久化 `.s0_subs.bin`

当被订阅分享目录发生文件创建、修改、权限/时间变更或删除时，服务端通过 `iroh` (`ALPN = "s0/delta/1"`) 向匹配的做种节点推送 `SubPush` 增量变更流（单帧 `>= 256 B` 自动经 `mzstd` 压缩并置位 `MANIFEST_ZSTD`，超大批量变更按 `65,536` 条/帧在单条 QUIC 流上多帧连续传输，支持 `u64::MAX` 级变更数）：
- 内联接收方最新权限 `perm` (`1=R, 2=W, 3=Rw`) 与允许写入路径前缀 `write_prefixes`；
- 对 `FsOp::Remove (2)` 仅传前缀差分压缩路径，客户端本地删除目标并清理空父目录；
- 对 `FsOp::Meta (3)` 仅传前缀差分压缩路径、POSIX 权限 `mode: u15` 与毫秒级最后修改时间 `mtime: uvar`（**零 `ChunkSig` 块签名、零 UDP 会话建立**），客户端直接原位更新本地文件权限与 `mtime` 并更新内存 `Manifest`；
- 对 `FsOp::Upsert (1)` 内联 `mode: u15`、`mtime: uvar` 与该变更文件的 `ChunkSig` 块签名列表，客户端**免除二次 `Op::Stat` 往返**，直接与本地同名文件执行 FastCDC 块比对（`match_basis_file` + `in_place`），仅通过自定义 UDP 协议拉取改动的差异块，并在内存中原位合并 `Manifest`（`Manifest::apply_changes`）：

```text
SubPush (承载于 iroh QUIC 流 ALPN "s0/delta/1"，流首字节发送 ver: u8 握手，支持多帧流式批次，>= 256B 自动 mzstd 压缩):
  magic: u32 ("S0B2") | ver: u8 (PROTO_VERSION) | flags: u8 | share_id: [u8; 16]
  perm: u8 (1=R, 2=W, 3=Rw) | eff_rate: uvar | write_prefixes: strs_front
  root_hash: [u8; 16] | origin_addrs: slice_u15
  peer_cnt: uvar      | peers: [SwarmPeer; peer_cnt]
  change_cnt: uvar    | changes: [
    op: u8 (1=Upsert, 2=Remove, 3=Meta) + path: Front-Coding
    + (op == 1 ? mode: u15 + mtime: uvar + chunk_cnt: uvar + [len: uvar + hash: [u8; 16]; chunk_cnt]
       : op == 3 ? mode: u15 + mtime: uvar
       : empty);
    change_cnt
  ]

.s0_subs.bin (Redis RDB + AOF 混合持久化：头部为全量 RDB 基准快照，日常改动仅在尾部追加 AOF 增量帧，达 64 次或 64KiB 阈值及重启加载时自动全量重写整理):
  [RDB Preamble]:
    magic: u32 ("S0S1") | ver: u8 (PROTO_VERSION) | base_mf_len: uvar | base_mf_bytes: [u8; base_mf_len]
    sub_cnt: uvar       | subs: [endpoint_id: [u8; 32] + eff_rate: uvar + ctrl_addrs: slice_u15 + paths: strs_front; sub_cnt]
  [AOF Tail Frames (0..N 帧，支持 CRC16 抗掉电半包截断)]:
    payload_len: u32 LE | crc16: u16 LE | tag: u8 (1=UpsertSub, 2=RemoveSub, 3=ManifestDelta) | payload: [u8; payload_len]
```

---

## 5. UDP 数据面帧

所有 UDP 报文首 4 字节为小端 `u32`：`pkt_type: u8 | ((session_id & 0xFF_FFFF) << 8)`。

| 报文类型 | 类型码 | 长度 | 字段布局 |
| :--- | :--- | :--- | :--- |
| `DataHeader` | `0x00` | 20 B | `hdr: u32`、`block_idx: u32`、`seg_idx: u16`、`seg_cnt: u16`（最高位 Zstd 标志）、`offset_in_block: u32`（高 8 位为 SuperBlock 附加块数）、`payload_len: u16`、`crc16: u16` |
| `TackFrame` | `0x01` | 18 B+ | `hdr: u32`、`cum_blk: u32`、`active_blk: u32`、`delivered_segs: u32`、`echo_delay_ms: u16`、`recv_rate_kbps: u32`、`partial_cnt: u8` + `(blk: u32, mask: u128)` |
| `NackPkt` | `0x81` | 变长 | `hdr: u32`、`span_cnt: uvar`（差分 `gap: uvar, len: uvar`）、`partial_cnt: uvar`（差分 `gap: uvar, mask: u64`） |
| `AckPkt` | `0x82` | 12 B | `hdr: u32`、`blk_lo: u32`、`blk_hi: u32` |
| `Punch` | `0x83` | 5 B | `hdr: u32`、`ver: u8 (PROTO_VERSION)` |
