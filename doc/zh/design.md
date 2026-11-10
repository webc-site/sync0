# `sync0` 系统架构设计

`sync0` 采用**全对称对等网络（Symmetric P2P）**与**控制/数据双平面解耦**架构，单节点同时具备分享服务、实时订阅做种与多源并发拉取能力。

## 1. 核心架构分层

1. **免 IP 身份与多分享组托管（`s0_conf` + `s0_wire`）**
   - 外部配置与命令行统一使用 `base64url` 编码的 `iroh` 节点地址；内部路由表、配额表与种子表全量使用定长 `[u8; 32]` 二进制 `PeerId` 索引。
   - 单节点支持托管多个独立分享组，支持全量或逗号分隔多选子目录/单文件同步，权限模型见 [`crypto.md`](./crypto.md)。
2. **控制与实时变更推送面（`iroh` QUIC / TCP）**
   - 首字节发送 1 字节协议版本号 `PROTO_VERSION` 握手，支持平滑协议升级。
   - 服务端通过跨平台文件事件监听（`notify`：macOS `FSEvents` / Linux `inotify` / Windows `ReadDirectoryChangesW`）结合 `(size, mtime_ns, mode)` 增量扫描，提取 `FsOp::Upsert`（内容增改）、`FsOp::Remove`（删除）、`FsOp::Meta`（仅权限/时间变更）三类原语。
   - 变更帧 `SubPush` 自动经 `mzstd` 压缩并支持 `u64::MAX` 级海量变更流式分批推送（线格式见 [`proto.md`](./proto.md)）。
3. **Redis RDB + AOF 混合持久化（`subs_store`）**
   - 分享根目录下的 `.s0_subs.bin` 首部保存全量 RDB 基准快照（做种节点表 + `Manifest`），日常订阅变更与文件改动仅在文件尾部以带 `CRC16` 校验的 7B 帧头 AOF 记录 $O(\Delta)$ 追加。
   - 启动时回放 AOF 并自动截断半写损坏帧（`aof-load-truncated`）；累计达 `64` 次追加或 `64 KiB` 阈值时自动执行 `.s0tmp` + `sync_all()` + 原子 `rename` 全量重写整理。
4. **咆哮位图（`roaring::RoaringBitmap`）块归属路由与双层宏块调度（`s0_cdc` + `s0_peer`）**
   - **咆哮位图记录谁有什么块**：服务端做种表 `SwarmSeedRecord` 采用 `roaring::RoaringBitmap`（启用 `Run-Container` 游程优化）记录每个在线节点已通过校验的块集合，跨多次 `Op::Have` 原位位或（`|=`）累加并导出连续游程段；控制面 `Op::Pull` / `Op::Push` 的 `have_bits` 帧采用 `RoaringTag`（`Empty = 0`、`Full = 1`、`Roaring = 2`、`Raw = 3`）自适应选取最短字节表示（空/满位图仅占 `1 B`）。
   - **微块校验 + 8 MiB 自适应宏块（Macro-Piece）边下边传**：底层保持 `16~64 KiB`（均值 `32 KiB`）FastCDC 微块以契合单 `u64` 寄存器 UDP 分片重传掩码与 `gxhash128` 逐块防投毒校验；上层调度按 `macro_piece_blocks(total_blocks) = (total_blocks >> 3).clamp(4, 256)` 在 `128 KiB ~ 8 MiB` 间自适应缩放 `Op::Have` 汇报步长与 Work-Stealing 抢占粒度，并按 Most-Constrained-First（`blk_hi` 升序）优先消费半程 Leecher 已就绪前缀块。
5. **自适应 UDP 数据分发面与 `compio` 异步 I/O（`s0_udp` + `s0_cc` + `s0_fs`）**
   - 接收端收到 `SubPush` 或 `P2pResp` 清单后，对 `FsOp::Meta` 直接原位更新权限与 `mtime`；对 `FsOp::Upsert` 通过内联 `ChunkSig` 与本地同名文件执行 FastCDC 块比对（`match_basis_file`），未变块原位保留。
   - 缺失或差异块按城市/大区亲和度、实测控制流 RTT 与可用块区间切分为不相交条带，通过自定义 UDP 协议（时延梯度拥塞控制 + 64-bit 分片级 NACK 恢复 + Work-Stealing 慢节点折半抢占）多源并发拉取，经 `compio` 异步落盘。

---

## 2. Crate 拓扑

| Crate | 职责 |
| :--- | :--- |
| [`s0_const`](../../crate/s0_const/src/lib.rs) | 协议版本号 `PROTO_VERSION`、魔数、尺寸上限、枚举原语（`Op` / `FsOp` / `Perm`）与城市级地域编码 |
| [`s0_cc`](../../crate/s0_cc/src/lib.rs) | BBRv3 + Copa 混合拥塞控制、自适应 RTT/RTO/RACK 估计器、并发度控制器 |
| [`s0_codec`](../../crate/s0_codec/src/lib.rs) | 自适应 Zstd 编解码、首块魔数/香农熵不可压缩检测与动态压缩等级调节 |
| [`s0_cdc`](../../crate/s0_cdc/src/lib.rs) | 并行 FastCDC 分块、`gxhash128` 摘要、二进制清单 `Manifest`、原位增量匹配与零拷贝游标 |
| [`s0_wire`](../../crate/s0_wire/src/lib.rs) | UDP 数据包头、TACK 反馈帧、P2P 控制报文、`SubPush` 增量帧与 Ed25519 自签名名片编解码 |
| [`s0_conf`](../../crate/s0_conf/src/lib.rs) | NestedText（`.nt`）配置解析、带宽速率解析与多选 URI 解析 |
| [`s0_fs`](../../crate/s0_fs/src/lib.rs) | `compio` 并发异步文件 I/O、小文件内存聚合打包、元数据（`mode` + `mtime`）同步与共享接收池 |
| [`s0_udp`](../../crate/s0_udp/src/lib.rs) | 微秒级令牌桶 Pacer、公网/隧道双栈打洞、单点流水线引擎与多源区间拉取引擎 |
| [`s0_peer`](../../crate/s0_peer/src/lib.rs) | 对称 P2P 节点引擎：多分享组托管、RDB+AOF 持久化、实时目录监控、多源调度与群组同步 |
| [`s0_i18n`](../../crate/s0_i18n/src/lib.rs) | 75 种语言编译期静态字典 |
| [`s0_sim`](../../crate/s0_sim/src/lib.rs) | 离散事件网络仿真器（千兆内网、跨洋专线、浅队列长肥网、高丢包弱网与增量补丁仿真） |
| [`sync0`](../../crate/sync0/src/main.rs) | 命令行入口（`share`、`pull`、`push`、`sub`、`group`、`profile`、`id`、`send`、`recv`） |
