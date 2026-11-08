# `sync0` 对等同步与 P2P 分发架构 (`design_v4.md`)

## 1. 架构设计核心

`sync0` 采用**全对称对等网络（Symmetric P2P）**架构，融合多分享组按需多选同步、就近多源条带化分发与原生 UDP 极速数据面：

1. **单用户多文件分享组与按需多选同步 (`Multi-Group & Multi-Select`)**：
   - **单用户多文件分享组**：单个 `PeerNode` 可同时托管任意多个独立文件分享组（`shares: HashMap<[u8; 16], ShareState>`），支持通过 `Op::ListShares = 7` 枚举节点对当前访问者可见的全部分享组摘要（`ShareSummary`），并支持通过 `groups.conf` 声明式配置。
   - **全量或多选文件/目录同步**：协议层 `P2pReq.paths` 采用 `u15` 路径数量 + `slice_u15` UTF-8 路径切片数组；`Manifest::filter_paths` 与 `build_manifest_from_paths` 支持同步整个分享组（`group://` 或 `paths = []`），或同时多选任意组合的单文件与子目录（如 `pub-site://docs/readme.md,assets/icons`），零本地占位文件污染。
   - **组员分级权限与按用户配置上传路径前缀**：每个分享组支持按组员 32 字节二进制 `PeerId` 配置权限（`Perm::Ro = 1` 只读、`Perm::Rw = 2` 读写上传、`Perm::Admin = 3` 管理）及允许上传的路径前缀列表 `write_prefixes`（未配置时默认允许上传到该分享组的根目录；配置前缀如 `uploads/bob,shared/common` 时，越权上传立即返回 `Status::Forbidden`）。
2. **纯 `iroh` 地址寻址（外部 `base64url` 配置，内部 32 字节二进制 `[u8; 32]`）**：
   - 外部 CLI 和 `groups.conf` 中统一采用无填充 `base64url` 编码的 `iroh` 地址（`encode_id_b64` / `encode_iroh_b64`）。
   - 内部路由表、Swarm 节点列表（`SwarmPeer`）、订阅表（`SubscriberRecord`）、做种表（`SwarmSeedRecord`）与权限配额表（`ShareConfig::peer_quotas`）全部使用定长 32 字节二进制 `PeerId = [u8; 32]` 存储与索引，零字符串堆分配。
3. **就近多源 P2P 并发分发**：
   - 处于长连接订阅（`Op::Sub`）或群组（`Op::GroupJoin`）中的对等节点自动成为 P2P 分发做种源（Seed Peer）。
   - 下载节点通过 `rank_swarm_peers`（综合同城/同大区地域亲和度 `s0_region` 与节点可用带宽权重）+ `plan_stripes`，将缺失块 `[0, total_blocks)` 切分为不相交条带 `[blk_lo, blk_hi)`，同时从多个做种节点并发拉取，慢节点或断连节点区间自动 Work-Stealing 接管。
4. **原生 UDP 极速数据面**：
   - 控制面负责 Ed25519 身份验签、多选 `Manifest` 交换、权限与上传路径前缀校验及 P2P 节点发现（PEX）。
   - 100% 文件块数据通过 `s0_udp` 原生 UDP 协议传输（20B 包头 + 微秒级 Pacer + 拥塞控制 + 紧凑位图/游程 NACK + 无锁 `pwrite` 原位落盘），内存占用恒定为 O(窗口)。

---

## 2. Crate 架构

| Crate | 职责 |
| :--- | :--- |
| `s0_cdc` | FastCDC 内容定义分块、SIMD BLAKE3 128-bit 摘要、紧凑二进制 `Manifest`（支持 `filter_paths` 多选文件/目录过滤）、流式原位基准匹配 `match_basis_file`、跨平台无锁定位读写 `FileAt` |
| `s0_udp` | 协议常量 `s0_const`、城市级地域 `s0_region`、二进制线协议 `s0_wire`、`iroh` Ed25519 自签名名片 `s0_profile`、无锁接收池 `s0_fs::SharedSink`、原生 UDP 传输引擎及全对称 P2P 节点引擎 `s0_peer` |
| `s0_i18n` | 75 种语言编译期零分配静态字典（根据系统 `LC_ALL`/`LANG` 或 `SYNC0_LANG` 自动匹配） |
| `sync0` | 统一命令行工具（`share`、`pull`、`push`、`sub`、`group`、`profile`、`id`、`send`、`recv`） |
| `s0_testkit` | 全链路集成测试（多分享组、多选文件/目录、上传路径前缀鉴权、多源条带化分发、高丢包恢复） |
