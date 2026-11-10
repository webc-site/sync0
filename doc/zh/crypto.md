# `sync0` 认证与权限控制

## 1. 节点身份 (`PeerId`)

- 每个节点拥有独立的 Ed25519 密钥对，32 字节公钥即为全局唯一 `PeerId = [u8; 32]`。
- **外部表示**：CLI 与配置文件统一使用 43 字符无填充 `base64url` 编码（或附带二进制路由提示的 `iroh` 地址），无需暴露明文 IP。
- **内部表示**：入口处一次性解码为栈上 `[u8; 32]`，路由表、配额表与在线节点表全量以二进制数组寻址。

## 2. 自签名节点名片 (`SignedProfile`)

- 节点使用私钥对域分隔符 `b"s0/profile/v1"` + `pubkey (32B)` + `ts_sec (uvar)` + 升序 TLV 属性流生成 64 字节 Ed25519 签名。
- 验签规则：
  1. 校验属性长度上限（昵称 `<= 64` 字符、邮箱 `<= 128 B`、网址 `<= 256 B`、简介 `<= 256` 字符）及城市地域码合法性。
  2. 校验 `ts_sec` 单调递增，拒绝旧名片回滚重放。
  3. 使用名片自带 `pubkey` 独立验证 Ed25519 签名，防止 P2P 节点交换（PEX）过程中中间节点篡改名片。

## 3. 公网读取限制、位掩码权限与上传路径前缀控制

- **限制公网匿名读取（仅授权用户可读）**：
  - 在 `config.nt` 中为分享组配置 `users:` 列表（或 `r_secret`），或命令行指定 `--allow-r` / `--allow-rw` / `--allow-w` / `--auth` 时，自动开启私有访问保护（`auth_required = true`）。
  - 开启后，未列于 `users` 白名单的公网匿名节点在 `Op::ListShares` 中看不到该分享组，调用 `Op::Stat` / `Op::Pull` / `Op::Sub` / `Op::Push` 均直接拒绝并返回 `Status::Unauth`。
  - 订阅私有分享组的做种节点（`Op::Sub` / `SubPush`）通过 `FLAG_AUTH (1 << 6)` 自动继承源节点的可读成员白名单，防止公网匿名节点绕过源站从做种节点窃取数据。
- **成员位掩码权限**：每个分享组默认权限为只读（`Perm::R = 1`，配置文件中无需编写 `perm: r`），并可在 `users/*.nt` 中按成员 `PeerId` 单独授权 `r`、`w`、`rw`：

| 权限级别 | 位掩码数值 | 配置值 | 允许操作 |
| :--- | :--- | :--- | :--- |
| 只读 (`Perm::R`) | `1` (`0b01`) | 默认省略（或 `r`） | 枚举分享组 (`ListShares`)、查询 (`Stat`)、拉取 (`Pull`)、订阅做种 (`Sub`) |
| 仅写 (`Perm::W`) | `2` (`0b10`) | `w` | 枚举分享组 (`ListShares`)、在授权路径前缀 `write_prefix` 内单向投递上传 (`Push`)，禁止读取/拉取 |
| 读写 (`Perm::Rw`) | `3` (`0b11`) | `rw` | 只读全部权限 + 授权路径前缀内上传 (`Push`)、远端镜像清理 (`--delete`) 与对等群组互备 (`GroupJoin`) |

- **上传路径前缀校验**：
  - 未配置 `write_prefix` 时，允许 `w` / `rw` 成员向该分享组根目录下任意相对路径上传。
  - 配置 `write_prefix`（如 `uploads/bob`、`shared/common`）时，服务端对请求路径与上传 `Manifest` 中的所有文件条目逐一归一化（阻断 `..` 路径穿越）并校验前缀命中；任意条目越权或尝试全目录 `--delete` 均立即拒绝并返回 `Status::Forbidden`。

## 4. P2P 多源防恶意节点篡改与投毒设计（Anti-Tampering & Anti-Poisoning）

1. **权威源清单锚定与根哈希自洽校验 (`root_hash: [u8; 16]`)**：
   - `swarm_pull` 固定优先向主源（`seed_ids[0]`）请求 `Op::Stat` 获取权威 `Manifest`，并在解码后立即重算校验 `mf.root_hash() == resp.root_hash`，阻断辅助做种节点伪造或篡改文件清单。
   - 节点交换（PEX）名片验签强制核对 `SwarmPeer.endpoint_id == card.pubkey` 及 `card.ts_sec >= prev.ts_sec`，防止路由劫持与旧名片回滚重放。
2. **分块级 128-bit 落盘前强校验 (`gxhash128`)**：
   - 无论是常规大文件分块（`SharedSink::write_verified_block`）还是小文件聚合超块（`SharedSink::write_superblock`），接收端在写入内存池或暂存文件及置位 `have_bits` 之前，均强制对比 `gxhash128(data) == loc.hash`。
   - 任何被恶意节点篡改的字节块在落盘和 `have_bits` 发布前即被拦截，彻底杜绝边下边传（Leecher Seeding）将脏块扩散给其他节点的 **Swarm 集群投毒（Swarm Poisoning）**。
3. **恶意节点快速熔断剔除与诚实源自动重拉**：
   - `recv_range_udp` 对连续发送校验失败块的节点触发 2 次计数快速熔断（`corrupt_strikes >= 2`），毫秒级中止该恶意会话而无需等待超时。
   - `execute_striped_pull` 自动将该 `PeerId` 加入本轮黑名单（`banned_peers`），并由剩余健康节点接管补齐缺失块区间。
4. **原子落盘与原位差分提交**：新建文件写入临时文件 `.s0_stage.<filename>` 并完成全块校验后原子 `rename` 替换；已存在文件通过 `match_basis_file` 校验复用未变块，仅对差异块校验后原位 `pwrite` 更新。
