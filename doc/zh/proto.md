# `sync0` 权威二进制线协议规范 (`proto.md`)

所有控制面、身份名片与数据面帧均采用紧凑小端（Little-Endian）二进制与 `u15` 变长整数编码，**零 JSON、零字符串键名**。

---

## 1. 基础数值常量 (`s0_const` & `s0_region`)

- **`u15` 变长无符号整数**：
  - $v \in [0, 127]$：1 字节 `v as u8`
  - $v \in [128, 32767]$：2 字节 `[0x80 | ((v >> 8) as u8), (v & 0xFF) as u8]`
- **魔数**：
  - `P2P_MAGIC: u32 = 0x3250_3053` (`"S0P2"`)
  - `SUB_MAGIC: u32 = 0x3242_3053` (`"S0B2"`)
  - `PROFILE_MAGIC: u32 = 0x3143_3053` (`"S0C1"`)

### 1.1 数值操作码 `Op` (`#[repr(u8)]`)

| 数值 | 枚举名 | 语义 |
| :--- | :--- | :--- |
| `1` | `Op::Pull` | 拉取单文件、单目录、路径前缀或指定块区间 `[blk_lo, blk_hi)` |
| `2` | `Op::Push` | 向拥有写权限的对等节点推送单文件或目录变更 |
| `3` | `Op::Stat` | 查询目标 Share + 路径的 `Manifest`、`root_hash`、节点名片及可用 P2P 节点列表 |
| `4` | `Op::Sub` | 长连接订阅 Share 更新（全量 `""` 或路径前缀 `"sub/dir"`），同时注册为 P2P 分发节点 |
| `5` | `Op::Have` | 订阅/群组节点上报已完成校验的 `root_hash` 与块区间 `[blk_lo, blk_hi)` |
| `6` | `Op::GroupJoin` | 加入群组相互备份，通告本节点名片、导出的 Share 及备份前缀策略 |

### 1.2 节点自签名属性键 `AttrKey` (`#[repr(u8)]`)

| 数值 | 枚举名 | 值类型与约束 |
| :--- | :--- | :--- |
| `1` | `AttrKey::Name` | UTF-8 字符串，最长 **64 个字符** (`MAX_NAME_CHARS = 64`) |
| `2` | `AttrKey::Email` | UTF-8 字符串，最长 128 字节 |
| `3` | `AttrKey::Url` | UTF-8 字符串，最长 256 字节 |
| `4` | `AttrKey::Region` | 2 字节小端 `u16` 城市级地域常量（`s0_region::Region`） |
| `5` | `AttrKey::Bio` | UTF-8 字符串，最长 128 个字符 |

### 1.3 城市级地域数字常量 `Region` (`#[repr(u16)]`, `s0_region`)

按大区（高百位 `zone = id / 100`）编码全球核心城市，规避国家主权争议并支持 $O(1)$ 地域亲和度计算：
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
| pubkey: [u8; 32] (iroh Ed25519 公钥)                          |
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
| share_id: [u8; 16] (目标分享或群组 ID)                        |
| req_rate: u64 (请求方期望带宽上限 B/s，0 表示不限)            |
| path_len: u15     | path_utf8: [u8; path_len] ("" 表示根)     |
| addrs_len: u15    | peer_addrs: [u8; addrs_len] (UDP 打洞地址)|
| card_len: u15     | profile_card: [u8; card_len] (可选自签名) |
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
其中每个 `SwarmPeer` 编码为：
```text
+---------------------------------------------------------------+
| endpoint_id: [u8; 32]                                         |
| region: u16 (城市级地域常量) | rsv: u16 = 0                   |
| weight_rate: u64 (可用带宽权重 B/s)                           |
| blk_lo: u32         | blk_hi: u32                             |
| addrs_len: u15      | peer_addrs: [u8; addrs_len]             |
| card_len: u15       | profile_card: [u8; card_len]            |
+---------------------------------------------------------------+
```

---

## 4. 原生 UDP 数据面帧格式 (`s0_udp`)

- **缺失块位图帧 `HAVE`**：`0xFFFF_FFFF` 表示全 1 位图（0 字节位图负载），`0` 表示全缺，否则后跟位图字节。
- **数据包头 `DataHeader` (20B)**：`flags_and_sid: u32`、`block_idx: u32`、`seg_idx: u16`、`seg_cnt: u16`、`offset_in_block: u32`、`payload_len: u16`、`header_crc16: u16`。
- **反馈包 `Nack` (`0x81`) / `Ack` (`0x82`)**：首字节最高位为 `1`，与数据包零歧义复用同一 UDP 端口。
