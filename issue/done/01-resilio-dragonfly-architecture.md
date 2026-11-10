# 01. Resilio Sync (IDA 逆向) + Dragonfly (蜻蜓) + Aspera + Iroh 对等网络架构设计

## 目标
彻底摒弃传统 Client/Server 架构，构建全对称 P2P 文件同步与分发网络（`sync0`）：
1. **Resilio Sync 基因**：每个节点是对等节点（Peer），可分享本地文件夹（支持只读/读写密钥或公开分享）、按需订阅下载（全量或子路径前缀 Selective Sync）、加入群组（Group）相互自动备份（支持全量 `""` 或指定路径前缀 `"sub/path"`）。
2. **Alibaba Dragonfly (蜻蜓) 基因**：处于长连接订阅（`Sub`）或群组（`Group`）中的对等节点自动成为 P2P 分发源（Seed Peer）；需求方通过 `ParentSelector` + `PieceCollector` 从多个持有块的对等节点并发拉取不相交块区间 `[blk_lo, blk_hi)`，支持边下边传与慢节点 Work-Stealing 故障接管。
3. **IBM Aspera 基因**：`iroh` 仅负责节点身份认证（Ed25519 `EndpointId`）、控制面信令与内网/NAT 打洞地址交换；100% 文件块数据通过自研零队头阻塞原生 UDP 协议传输，速度第一。

## 任务清单
- [ ] 使用本机 IDA Professional 9.5 (`idat`) 与符号分析工具对 `/tmp/resilio/rslsync` 进行逆向分析，提炼其目录树同步状态机、Selective Sync 路径前缀过滤、Share Secret 权限模型与 Piece 交换原语
- [ ] 结合 `/tmp/dragonfly-client` 的 `ParentSelector` / `PieceCollector` 设计，确定全数值二进制协议原语与 7 个高内聚 Crate 拆分方案
- [ ] 输出架构与协议文档至 `doc/zh/`
