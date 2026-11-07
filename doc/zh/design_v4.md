# `sync0` 对等同步与 P2P 分发架构 (`design_v4.md`)

## 1. 逆向与竞品对标总结（Resilio Sync + Dragonfly + IBM Aspera + Iroh）

通过对 `/tmp/resilio/rslsync`（IDA Professional 9.5 Hex-Rays 反编译 `SyncFolderMergeController`、`SelectiveStrategyBase`、`PartialPiecesStorage`）、`/tmp/dragonfly-client`（阿里蜻蜓 P2P `ParentSelector` 与 `PieceCollector`）及 `/tmp/aspera-cli` 的深度剖析，`sync0` 采用**全对称对等网络（Symmetric P2P）**架构：

1. **超越 Resilio Sync**：
   - Resilio Sync 在 `SyncFolderMergeController::ProcessRemoteMessage` 中使用字符串字典 `BencodedDict` 与互斥锁合并目录树，传输层受限于 uTP。
   - `sync0` 将全部原语数值化（`Op`、`Status`、`AttrKey`、`Region` 全部为整数常量），分块采用 FastCDC 内容定义分块 + 128-bit SIMD BLAKE3，支持**单文件拉取**、**单目录拉取**、**路径前缀选择性同步 (`Selective Sync`)** 以及**群组相互备份（全量 `""` 或子路径前缀 `"sub/path"`）**。
2. **融合 Dragonfly (蜻蜓) P2P 并发分发**：
   - 处于长连接订阅（`Op::Sub`）或群组（`Op::GroupJoin`）中的对等节点自动成为 P2P 分发源（Seed Peer）。
   - 下载节点通过 `ParentSelector`（综合**同城/同大区地域亲和度 `s0_region`** 与**节点可用带宽权重**）+ `PieceCollector`，将缺失块 `[0, total_blocks)` 切分为不相交区间 `[blk_lo, blk_hi)`，同时从多个对等节点并发拉取，慢节点或断连节点区间自动 Work-Stealing 接管。
3. **融合 IBM Aspera 极速原生 UDP 数据面**：
   - `iroh` 仅用于 Ed25519 身份认证、控制面原语协商、P2P 节点发现与内网/NAT 打洞。
   - 100% 文件块数据通过自研 `s0_udp` 原生 UDP 协议传输（20B 包头 + 微秒级 Pacer + 紧凑位图/游程 NACK + 无锁 `pwrite` 原位落盘），内存占用严格恒定为 $O(\text{窗口})$。
4. **基于 `iroh` 公钥的自签名节点名片 (`s0_profile`) 与城市级无主权冲突地域 (`s0_region`)**：
   - 每个对等节点可用自己的 `iroh` Ed25519 私钥签名自身属性（全部 Key 采用 `u8` 数字常量：`1=用户名` 最长 64 字符、`2=邮箱`、`3=网址`、`4=城市级地域 u16`、`5=简介`）。
   - 地域精确到**城市级别**（如上海、深圳、香港、台北、东京、新加坡、旧金山、法兰克福等 `u16` 数字常量），彻底规避国家主权争议，同时为 P2P 调度提供就近节点优选依据。

---

## 2. Crate 拆分架构（8 个高内聚低耦合 Crate）

| Crate | 职责 |
| :--- | :--- |
| `s0_const` | `#![no_std]` 编译期常量、全数值操作码 `Op`、状态码 `Status`、属性键 `AttrKey`、位标志与协议参数 |
| `s0_region` | `#![no_std]` 城市级地域数字常量（`#[repr(u16)] enum Region`）、IATA 城市码、中英文名与 P2P 地域亲和度计算 |
| `s0_wire` | `#![no_std]` 零拷贝二进制编解码器（`Rd`/`Wr`、`u15` 变长整数、`DataHeader`、`Nack`、`Ack`、P2P 控制帧） |
| `s0_profile` | 基于 `iroh` Ed25519 公钥的节点自签名名片（`SignedProfile`：用户名 $\le 64$ 字符、邮箱、网址、城市 `Region`） |
| `s0_cdc` | FastCDC 内容定义分块、SIMD BLAKE3 摘要、紧凑二进制 `Manifest`、流式原位基准匹配 `match_basis_file` |
| `s0_fs` | 无锁原位文件读写 `FileAt`、多源并发预分配暂存文件 `SharedStagingFile`、目录扫描与变更检测 |
| `s0_udp` | 原生 UDP 极速传输引擎（`Pacer`、拥塞控制、`send_range_udp`、`recv_range_udp`、双栈 NAT 打洞） |
| `s0_peer` | 对称 P2P 节点引擎（分享与权限限速、长连接订阅 P2P 分发、Dragonfly 多源并发拉取、群组全量/前缀相互备份） |
| `sync0` | 统一命令行工具（`share`、`pull`、`push`、`sub`、`group`、`profile`、`stat`、`id`、`bench`） |
