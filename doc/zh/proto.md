# `sync0` 权威二进制线协议规范 (`proto.md`)

所有控制面、身份名片与数据面帧均采用紧凑小端（Little-Endian）二进制与 `u15` 变长整数编码，**零 JSON、零字符串键名**。
节点身份在外部配置与 CLI 中统一使用无填充 `base64url` 编码的 `iroh` 地址，在线路层与内存结构体中固定为 **32 字节二进制 `PeerId = [u8; 32]`**。

---

## 1. 基础数值常量 (`s0_const` & `s0_region`)

- **`u15` 变长无符号整数**：
  - `0..=127`：1 字节 `v as u8`
  - `128..=32767`：2 字节 `[0x80 | ((v >> 8) as u8), (v & 0xFF) as u8]`
- **魔数**：
  - `P2P_MAGIC: u32 = 0x3250_3053` (`"S0P2"`)
  - `SUB_MAGIC: u32 = 0x3242_3053` (`"S0B2"`)
  - `PROFILE_MAGIC: u32 = 0x3143_3053` (`"S0C1"`)

### 1.1 数值操作码 `Op` (`#[repr(u8)]`)

| 数值 | 枚举名 | 语义 |
| :--- | :--- | :--- |
| `1` | `Op::Pull` | 拉取整个分享组、多选文件/目录集合或指定块区间 `[blk_lo, blk_hi)` |
| `2` | `Op::Push` | 向拥有写权限的对等节点上传文件、目录或多选路径集合（服务端校验 `Perm` 与该组员允许上传的路径前缀 `write_prefixes`） |
| `3` | `Op::Stat` | 查询目标分享组 + 多选路径集合的 `Manifest`、`root_hash`、节点名片及可用 P2P Swarm 做种节点列表 |
| `4` | `Op::Sub` | 长连接订阅分享组更新（全量 `paths = []` 或多选文件/目录列表 `paths`），同时注册为 P2P 分发做种节点 |
| `5` | `Op::Have` | 订阅/群组节点上报已完成校验的 `root_hash` 与块区间 `[blk_lo, blk_hi)` |
| `6` | `Op::GroupJoin` | 加入群组相互备份与做种，通告本节点名片及多选同步路径列表 `paths` |
| `7` | `Op::ListShares` | 查询目标节点托管的全部文件分享组摘要（`Vec<ShareSummary>`，含当前访问者在各组的有效权限与允许上传路径前缀） |

### 1.2 组员权限级别 `Perm` (`#[repr(u8)]`)

| 数值 | 枚举名 | 配置名 | 语义 |
| :--- | :--- | :--- | :--- |
| `1` | `Perm::Ro` | `ro` | 只读：允许拉取 (`Pull`)、查询 (`Stat`/`ListShares`)、订阅做种 (`Sub`) |
| `2` | `Perm::Rw` | `rw` | 读写：除只读权限外，允许在授权的上传路径前缀 `write_prefixes`（默认分享根目录）内上传 (`Push`) |
| `3` | `Perm::Admin` | `admin` | 管理员：完整读写、镜像清理 (`FLAG_DELETE`) 与群组管理权限 |

### 1.3 节点自签名属性键 `AttrKey` (`#[repr(u8)]`)

| 数值 | 枚举名 | 值类型与约束 |
| :--- | :--- | :--- |
| `1` | `AttrKey::Name` | UTF-8 字符串，最长 64 个字符 (`MAX_NAME_CHARS = 64`) |
| `2` | `AttrKey::Email` | UTF-8 字符串，最长 128 字节 |
| `3` | `AttrKey::Url` | UTF-8 字符串，最长 256 字节 |
| `4` | `AttrKey::Region` | 2 字节小端 `u16` 城市级地域常量（`s0_region::Region`） |
| `5` | `AttrKey::Bio` | UTF-8 字符串，最长 256 个字符 (`MAX_BIO_CHARS = 256`) |

### 1.4 城市级地域数字常量 `Region` (`#[repr(u16)]`, `s0_region`)

按大区（高百位 `zone = id / 100`）编码全球核心城市，支持 O(1) 地域亲和度计算：
- `0`: `Unknown` (`UNK`)
- `1xx` 东亚城市：`101 = Beijing (BJS)`, `102 = Shanghai (SHA)`, `103 = Shenzhen (SZX)`, `104 = Guangzhou (CAN)`, `105 = Hangzhou (HGH)`, `106 = Chengdu (CTU)`, `107 = HongKong (HKG)`, `108 = Taipei (TPE)`, `109 = Tokyo (TYO)`, `110 = Osaka (OSA)`, `111 = Seoul (SEL)`
- `2xx` 东南亚城市：`201 = Singapore (SIN)`, `202 = Bangkok (BKK)`, `203 = Jakarta (CGK)`, `204 = KualaLumpur (KUL)`
- `3xx` 北美城市：`301 = SanFrancisco (SFO)`, `302 = Seattle (SEA)`, `303 = LosAngeles (LAX)`, `304 = NewYork (NYC)`, `305 = Chicago (ORD)`, `306 = Dallas (DFW)`, `307 = Toronto (YYZ)`
- `4xx` 欧洲城市：`401 = Frankfurt (FRA)`, `402 = Amsterdam (AMS)`, `403 = London (LON)`, `404 = Paris (PAR)`, `405 = Zurich (ZRH)`, `406 = Stockholm (STO)`
- `5xx` 大洋洲与其他核心城市：`501 = Sydney (SYD)`, `502 = Melbourne (MEL)`, `601 = SaoPaulo (GRU)`, `701 = Dubai (DXB)`, `801 = Johannesburg (JNB)`

---

## 2. 节点自签名名片帧 `SignedProfile` (`s0_profile`)

```text
+---------------------------------------------------------------+
| magic: u32 = 0x31433053 ("S0C1")                              |
| pubkey: [u8; 32] (32B 二进制 iroh Ed25519 公钥)               |
| ts_sec: u64 (签名时间戳，单调递增防重放回滚)                  |
| attr_cnt: u8 (按 AttrKey 升序排列的属性条目数)                |
| attrs: [AttrEntry; attr_cnt]                                  |
|   +- key: u8 (AttrKey 数值常量)                               |
|   +- val_len: u15                                             |
|   +- val_bytes: [u8; val_len] (Region 固定为 2B u16 LE)       |
| sig: [u8; 64] (Ed25519 签名，覆盖域分隔符 + 上述全部字节)     |
+---------------------------------------------------------------+
```

---

## 3. 对等控制面帧 (`ALPN = b"s0/p2p/1"`)

### 3.1 对等请求帧 `P2pReq`
```text
+---------------------------------------------------------------+
| magic: u32 = 0x32503053 ("S0P2")                              |
| version: u8 = 2   | op: u8 (Op)   | flags: u8  | rsv: u8 = 0  |
| share_id: [u8; 16] (目标分享组 ID = BLAKE3_128(group_name))   |
| req_rate: u64 (请求方期望带宽上限 B/s，0 表示不限)            |
| path_cnt: u15     | paths: [slice_u15; path_cnt]              |
|                   | (0 个表示全量分享组；>=1 表示多选文件/目录)|
| addrs_len: u15    | peer_addrs: [u8; addrs_len] (二进制端点)  |
| card_len: u15     | profile_card: [u8; card_len] (自签名名片) |
| [当 flags & FLAG_RANGE != 0 时附加]:                          |
|   root_hash: [u8; 16]                                         |
|   blk_lo: u32     | blk_hi: u32                               |
| [当 flags & FLAG_RW != 0 时附加]:                             |
|   token: [u8; 16]                                             |
+---------------------------------------------------------------+
```

### 3.2 对等响应帧 `P2pResp`
```text
+---------------------------------------------------------------+
| magic: u32 = 0x32503053 ("S0P2")                              |
| status: u8 (Status) | flags: u8   | rsv: u16 = 0              |
| eff_rate: u64 (协商生效的带宽上限 B/s)                        |
| root_hash: [u8; 16] (当前 Manifest 的 128-bit BLAKE3 根摘要)  |
| addrs_len: u15      | peer_addrs: [u8; addrs_len]             |
| card_len: u15       | profile_card: [u8; card_len]            |
| peer_cnt: u15       | peers: [SwarmPeer; peer_cnt]            |
| mf_len: u32         | manifest_bytes: [u8; mf_len]            |
+---------------------------------------------------------------+
```
其中每个 `SwarmPeer` 内部采用定长 32 字节二进制 `endpoint_id` 编码：
```text
+---------------------------------------------------------------+
| endpoint_id: [u8; 32] (32B 二进制 iroh PeerId)                |
| region: u16 (城市级地域常量) | rsv: u16 = 0                   |
| weight_rate: u64 (可用带宽权重 B/s)                           |
| blk_lo: u32         | blk_hi: u32                             |
| addrs_len: u15      | peer_addrs: [u8; addrs_len]             |
| card_len: u15       | profile_card: [u8; card_len]            |
+---------------------------------------------------------------+
```

### 3.3 多分享组列表摘要 `ShareSummary`（用于 `Op::ListShares = 7`）
当请求 `Op::ListShares` 时，`P2pResp.manifest` 承载 `u15` 条目数 + 连续编码的 `ShareSummary`：
```text
+---------------------------------------------------------------+
| share_id: [u8; 16]                                            |
| name_len: u15       | name_utf8: [u8; name_len]               |
| perm: u8 (Perm: 1=Ro, 2=Rw, 3=Admin)                          |
| eff_rate: u64                                                 |
| prefix_cnt: u15     | write_prefixes: [slice_u15; prefix_cnt] |
|                     | (0 个表示允许上传至该分享组根目录)      |
+---------------------------------------------------------------+
```

---

## 4. 原生 UDP 数据面帧格式 (`s0_udp`)

- **缺失块位图帧 `HAVE`**：`0xFFFF_FFFF` 表示全 1 位图（0 字节位图负载），`0` 表示全缺，否则后跟位图字节。
- **数据包头 `DataHeader` (20B)**：`flags_and_sid: u32`、`block_idx: u32`、`seg_idx: u16`、`seg_cnt: u16`、`offset_in_block: u32`、`payload_len: u16`、`header_crc16: u16`。
- **反馈包 `Nack` (`0x81`) / `Ack` (`0x82`)**：首字节最高位为 `1`，与数据包零歧义复用同一 UDP 端口。
