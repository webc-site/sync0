# sync0

`sync0` 是一款用于替代 `rsync` 的去中心化 P2P 文件分享、实时同步与多源分发工具——跨洲广域网实测速度最高达 `rsync` 的 **5.89×**（单点几何平均快 **2.67×**，P2P 双源几何平均快 **3.16×**），跨洋高延迟链路 P2P 加速高达 **7.68×**（全链路 24 场景综合几何平均快 **2.56×**）。

每台机器既是服务端也是客户端：无需中心服务器与公网地址，基于分布式网络公钥地址自动打洞，支持两机直传、多分享组权限托管、按需多选子目录、毫秒级实时增量同步与 P2P 多源并发加速。

## 安装

### 方式一：Cargo 安装

```bash
cargo install sync0
```

### 方式二：下载预编译二进制

前往 [GitHub Releases](https://github.com/webc-site/sync0/releases) 下载对应系统的 `.tar.xz` 压缩包（支持 Linux、macOS、Windows、Android、iOS、FreeBSD），解压放入 `PATH`：

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## 快速上手

### 1. 两机零配置直传（`send` / `recv`）

无需配置分享组，直接在两台机器间高速传输文件或目录：

```bash
# 接收端监听端口并写入 ./backup
sync0 recv 0.0.0.0:9000 ./backup

# 发送端直传（可选 --delete 镜像清理多余文件，--rate 50MB 限速）
sync0 send ./project 192.168.1.10:9000
```

### 2. 发布分享与按需拉取 / 上传（`share` / `pull` / `push`）

```bash
# 服务端：一行命令发布分享组（启动后输出免 IP 的 iroh 节点地址）
sync0 share dataset:./data

# 客户端：全量拉取 dataset 分享组到本地 ./local_data
sync0 pull dataset:// ./local_data -p <节点地址>

# 客户端：按需多选拉取（仅同步指定的单文件与子目录）
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <节点地址>

# 客户端：上传文件或目录到授权分享组
sync0 push dataset://releases/v1 ./build_output -p <节点地址>
```

### 3. 实时订阅做种与多机双向同步（`sub` / `group`）

建立长连接后，远端发生任何文件**新增、修改、删除、权限或修改时间变更**，本地目录都会毫秒级实时同步更新（新增/修改大文件仅拉取差异块原位更新，删除文件自动清理本地文件与空目录，仅改权限或时间戳零数据块直达）：

- **`sync0 sub`（单向实时镜像 + 自动做种）**：本地实时跟随分享端增删改，同时自动作为 P2P 种子源为其他拉取节点提供并发加速。
- **`sync0 group`（多机对等双向实时同步）**：群组内任意一台机器发生文件增删改，其余所有在线节点实时同步更新并互为种子。

```bash
# 订阅整个分享组：远端增删改本地实时同步，并自动做种
sync0 sub dataset:// ./local_mirror -p <节点地址>

# 按需多选订阅：仅实时同步并做种指定的单文件与子目录
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <节点地址>

# 多机对等群组双向实时同步（同样支持全量或逗号多选路径）
sync0 group team-docs:// ./docs -p <节点地址>
```

### 4. 团队多分享组与成员权限配置（`.nt` 配置）

默认配置目录为 `~/.config/sync0`（可通过 `-c <目录>` 切换），采用极简 [NestedText](https://nestedtext.org)（`.nt`）格式：
- **支持限制公网读取（仅授权用户可读）**：只要分享组配置了 `users:` 成员列表（或命令行指定 `--allow-r` / `--allow-rw` / `--allow-w` / `--auth`），系统**自动开启私有鉴权（`auth: true`）**，禁止公网匿名节点枚举或读取，且 P2P 订阅做种节点也会同步继承白名单，仅允许授权用户读取；未配置 `users` 的分享组默认公开只读（如需公开只读且仅对指定成员开放上传，可显式设置 `auth: false`）。
- **细粒度读写与子目录投递权限**：成员默认只读（`r` 无需显式编写），支持 `r`（只读）、`w`（仅写投递箱，不可读取）、`rw`（读写）及 `write_prefix`（限制允许上传的子目录前缀）。

**成员配置示例（`~/.config/sync0/users/`）：**

```yaml
# ~/.config/sync0/users/alice.nt（默认只读，限速 80MB）
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
rate: 80MB
```

```yaml
# ~/.config/sync0/users/bob.nt（读写权限，仅允许上传到指定子目录）
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

```yaml
# ~/.config/sync0/users/charlie.nt（仅写投递权限，禁止读取）
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: w
```

**分享组配置示例（`~/.config/sync0/config.nt`）：**

```yaml
users_dir: ./users

groups:
  # 未配置 users：默认公网只读
  dataset:
    path: ./data
    rate: 50MB

  # 配置了 users：自动限制公网读取（仅 alice、bob 可读，charlie 仅可投递上传）
  team-vault:
    path: ./vault
    users:
      - alice
      - bob
      - charlie
```

启动服务（加载配置文件或直接用命令行参数限制公网读取并授权成员）：

```bash
# 加载 ~/.config/sync0/config.nt 启动
sync0 share --region SHA

# 或纯命令行发布私有分享组（指定 --allow-* 自动限制公网读取，仅授权用户可访问）
sync0 share team-vault:./vault \
  --allow-r 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-w 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 5. 节点地址、名片与多语言

```bash
# 查看本机免 IP 节点地址（首次运行自动生成密钥）
sync0 id

# 生成 Ed25519 自签名节点名片（昵称、城市代码、邮箱、网址、简介）
sync0 profile -u alice --region SHA --email alice@example.com -b "Shanghai node"

# 内置 75 种语言，默认跟随系统语言，可通过环境变量切换
SYNC0_LANG=en sync0 --help
```

## 性能与核心特性

### 跨洲广域网实测（对比 `rsync`）

实测环境：美国 `c1` → 欧洲 `n1`（RTT 65 ms），数据集为 Linux 内核 `linux-7.3.3`：

#### 1) 单点模式 vs `rsync`

| 数据集 | 大小 | `sync0` 单点<br>(Mbps) | `rsync`<br>(Mbps) | 加速比 (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`（不可压缩） | 155 MB | **236.48** | 108.74 | **2.17×** |
| `linux-7.3.3.tar`（可压缩） | 1.6 GB | **843.83** | 385.06 | **2.19×** |
| `linux-7.3.3.tar`（局部改动增量） | 1.6 GB | **8,014.15** | 1,361.72 | **5.89×** |
| `linux-7.3.3/`（95,979 个小文件） | 1.5 GB | **627.58** | 347.80 | **1.80×** |
| **几何平均** | — | **1,000.91** | 375.26 | **2.67×** |

#### 2) P2P 双源模式 vs `rsync`

| 数据集 | 大小 | `sync0` P2P 双源<br>(Mbps) | `rsync`<br>(Mbps) | 加速比 (`sync0` / `rsync`) |
| :--- | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`（不可压缩） | 155 MB | **330.09** | 108.74 | **3.04×** |
| `linux-7.3.3.tar`（可压缩） | 1.6 GB | **1,551.86** | 385.06 | **4.03×** |
| `linux-7.3.3.tar`（局部改动增量） | 1.6 GB | **5,474.17** | 1,361.72 | **4.02×** |
| `linux-7.3.3/`（95,979 个小文件） | 1.5 GB | **706.56** | 347.80 | **2.03×** |
| **几何平均** | — | **1,186.42** | 375.26 | **3.16×** |

> 注：P2P 双源模式下由欧洲节点 `s1` 协同做种，跨洲主干 RTT 65 ms。

### 核心产品特性

- **公钥寻址自动打洞**：无需中心服务器与公网地址，基于分布式网络公钥地址自动 NAT 打洞穿透。
- **毫秒级实时同步与元数据感知**：基于原生文件事件监听（`FSEvents` / `inotify` / `ReadDirectoryChangesW`），实时推送文件增删改、POSIX 权限（`mode`）与毫秒级修改时间（`mtime`）；仅修改权限或时间时零数据块传输。
- **BT 式边下载边上传与地域亲和调度**：下载节点无需等全量落盘，凡已通过 128-bit 哈希校验的分块区间（`Op::Have`）即刻在内存/暂存池无锁供其他节点并发拉取，配合动态 PEX 节点交换与慢节点折半抢占（Work-Stealing）最大化集群吞吐。
- **防恶意节点篡改与集群投毒免疫**：权威源清单 `root_hash` 自洽锁定 + 每块落盘与对外做种前强制执行硬件加速 `gxhash128` 摘要校验；发现篡改块毫秒级熔断拉黑恶意节点并自动从诚实源重拉恢复。
- **FastCDC 原位增量与小文件聚合**：大文件局部修改仅传输差异块并原位落盘；海量小文件内存打包与前缀差分压缩，消除逐文件往返开销。
- **断电自愈与重启续推**：采用类 Redis RDB + AOF 混合格式（`.s0_subs.bin`）持久化做种状态与文件清单，日常改动微秒级追加、定期自动重写整理，服务端重启后自动向做种节点补推离线改动。

## 技术架构与文档

- **双平面解耦协议**：
  - **控制与实时变更面（`iroh` QUIC / TCP，首字节 `PROTO_VERSION` 版本握手）**：负责 Ed25519 名片验签、权限校验、Swarm 种子发现，以及支持 `u64::MAX` 级海量变更流式分批与 `mzstd` 压缩的 `SubPush` 实时推送（`FsOp::Upsert` / `FsOp::Remove` / `FsOp::Meta` 三原语）。
  - **数据分发面（原生 UDP + `compio` 异步 I/O）**：20B 紧凑包头、18B TACK 周期反馈、BBRv3/Copa 混合拥塞控制、RACK-TLP 丢包恢复、首块香农熵自适应 Zstd 压缩旁路，以及 `gxhash128` 分块端到端校验。
- **深入了解**：
  - [性能实测报告与复现脚本](https://github.com/webc-site/sync0/blob/main/doc/zh/benchmark.md)
  - [系统架构设计](https://github.com/webc-site/sync0/blob/main/doc/zh/design_v4.md)
  - [二进制线协议与持久化规范](https://github.com/webc-site/sync0/blob/main/doc/zh/proto.md)
  - [身份认证、权限控制与完整性校验](https://github.com/webc-site/sync0/blob/main/doc/zh/crypto.md)
