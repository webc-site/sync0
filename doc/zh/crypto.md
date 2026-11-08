# `sync0` 身份认证、组员权限与上传路径前缀规范 (`crypto.md`)

## 1. 对等节点身份（外部 `base64url` `iroh` 地址，内部 32B 二进制 `PeerId`）

- 每个对等节点拥有一个 Ed25519 密钥对，其 32 字节公钥即为全局唯一的二进制 `PeerId = [u8; 32]`。
- **外部配置与展示**：统一采用无填充 `base64url` 编码（43 字符纯公钥 `encode_id_b64`，或附带紧凑二进制路由提示的 `encode_iroh_b64`），命令行与配置文件中不暴露明文 IP。
- **内部内存表示**：解码函数 `decode_id_b64` 在入口处将 `base64url` 字符串一次性还原为栈上 32 字节数组 `[u8; 32]`，所有路由表、配额表（`HashMap<PeerId, PeerQuota>`）与在线节点表均以 `[u8; 32]` 寻址。

## 2. 基于 `iroh` 公钥的自签名节点名片 (`SignedProfile`)

- 节点使用自身 `SecretKey` 对域分隔前缀 `b"s0/profile/v1"` + `pubkey (32B)` + `ts_sec (8B LE)` + 按数值 `AttrKey` 升序排列的紧凑 TLV 属性字节流生成 64 字节 Ed25519 签名。
- 校验规则：
  1. 校验 `AttrKey::Name` 字符数 `chars().count() <= 64`（`MAX_NAME_CHARS = 64`），`AttrKey::Bio` 字符数 `<= 256`（`MAX_BIO_CHARS = 256`）。
  2. 校验 `AttrKey::Region` 为合法的 `u16` 城市级地域常量（`s0_region::Region`）。
  3. 使用名片内的 `pubkey` 独立验证 64 字节 Ed25519 签名，防止中间节点在 P2P PEX 转发中篡改他人用户名、邮箱、网址或城市地域。

## 3. 多分享组、组员权限与可上传路径前缀控制 (`ShareConfig` & `groups.conf`)

单个用户节点可通过 `groups.conf` 或 CLI 同时发布多个文件分享组，并为不同组员配置分级权限与允许上传的路径前缀：

```ini
[group.pub-site]
path = ./site
auth = false
default_perm = ro
default_rate = 50000000

[group.priv-vault]
path = ./vault
auth = true
default_perm = ro
default_rate = 10000000
# 格式: member = <base64url_iroh_id> <ro|rw|admin> [max_rate_bps] [允许上传的路径前缀列表,逗号分隔]
# 1) 只读组员（限速 80 MB/s）
member = 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c ro 80000000
# 2) 受限上传组员：仅允许上传到 uploads/bob 与 shared/common 子路径下
member = 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f rw 0 uploads/bob,shared/common
# 3) 默认上传组员：省略第 4 列时，默认允许上传到该分享组的根目录
member = 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i rw 0
```

- **上传路径前缀校验规则 (`ResolvedAccess::can_write_manifest`)**：
  1. 若组员权限为 `Perm::Ro`，任何 `Op::Push` 请求直接拒绝并返回 `Status::Forbidden`。
  2. 若组员拥有 `Perm::Rw` 或 `Perm::Admin`：
     - 当 `write_prefixes` 为空列表（默认配置）时，允许在该分享组的**根目录**下任意相对路径上传或更新文件。
     - 当 `write_prefixes` 非空（如 `["uploads/bob", "shared/common"]`）时，服务端严格校验 `P2pReq.paths` 与上传 `Manifest` 中的每一个条目路径 `entry.path` 是否命中允许的前缀（精确匹配或位于该子目录下，且经 `sanitize_rel_path` 阻断 `..` 路径穿越）；只要存在任意一条越权路径或尝试全目录 `--delete`，立即拒绝并返回 `Status::Forbidden`。

## 4. 多源 P2P 数据面三层 BLAKE3 一致性保证

1. **清单根摘要 (`root_hash: [u8; 16]`)**：锁定同一快照版本（全量或多选子集各自拥有确定性的 `root_hash`）。
2. **分块级 128-bit BLAKE3 校验**：从任意对等节点接收的每个 FastCDC 块均在内存中实时验证 128-bit BLAKE3，损坏或伪造块立即丢弃并从其他健康节点重拉。
3. **共享 `.s0tmp` 原子提交**：全部块写入 `.s0tmp` 并校验通过后原子 `rename` 为最终文件。
