# `sync0` 身份认证、自签名名片与一致性规范 (`crypto.md`)

## 1. 对等节点身份 (`iroh` Ed25519 `EndpointId`)

- 每个对等节点拥有一个 Ed25519 密钥对，其 32 字节公钥即为全局唯一的 `EndpointId`。
- 支持内存随机临时身份（`KeyMode::Ephemeral`，默认零配置）与固定持久化身份（`KeyMode::Persistent`）。

## 2. 基于 `iroh` 公钥的自签名节点名片 (`SignedProfile`)

- 节点使用自身 `iroh::SecretKey` 对域分隔前缀 `b"s0/profile/v1"` + `pubkey (32B)` + `ts_sec (8B LE)` + 按数值 `AttrKey` 升序排列的紧凑 TLV 属性字节流生成 64 字节 Ed25519 签名。
- 校验规则：
  1. 校验 `AttrKey::Name` 字符数 `chars().count() <= 64`（`MAX_NAME_CHARS = 64`）。
  2. 校验 `AttrKey::Region` 为合法的 `u16` 城市级地域常量（`s0_region::Region`）。
  3. 使用名片内的 `pubkey` 验证 64 字节 Ed25519 签名；若在直连控制流中收到，还需校验 `pubkey == conn.remote_id()`；若通过 P2P 节点列表（`SwarmPeer`）间接转发收到，仅凭 `pubkey` 即可独立完成密码学验签，防止中间节点篡改他人用户名、邮箱、网址或城市地域。

## 3. 访问控制与带宽限额

- 每个分享 (`Share`) 默认无需认证（`auth = false`），任何节点均可拉取/订阅并适用默认带宽限额 `default_max_rate`。
- 开启认证（`auth = true`）时，仅允许白名单中的 `EndpointId` 或持有有效 128-bit 能力密钥的节点访问，并支持按 `EndpointId` 绑定专属带宽限额 `peer_max_rate`。

## 4. 多源 P2P 数据面三层 BLAKE3 一致性保证

1. **清单根摘要 (`root_hash: [u8; 16]`)**：锁定同一快照版本。
2. **分块级 128-bit BLAKE3 校验**：从任意对等节点（包括长连接订阅节点）接收的每个 FastCDC 块均在内存中实时验证 128-bit BLAKE3，损坏或伪造块立即丢弃并从其他健康节点重拉。
3. **共享 `.s0tmp` 原子提交**：全部块写入 `.s0tmp` 并校验通过后原子 `rename` 为最终文件。
