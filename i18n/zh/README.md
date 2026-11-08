# sync0

`sync0` 是一款对标 Resilio Sync 的点对点（P2P）文件分享与多机同步工具。

每台机器既是服务器也是客户端：任何节点都可以发布多个文件分享组，其他节点加入后可从对等服务器自动获取可用的种子节点并多源并发拉取，同步完成后自动做种供更多节点加速下载。

## 核心亮点

- **每个人既是服务器也是客户端**：对标 Resilio Sync，无中心服务器依赖。任意节点均可创建多个文件分享组，也可加入他人的分享组；节点之间自动交换并获取可用的种子节点，实现多机并发拉取与实时同步。
- **免 IP 节点地址**：默认在 `~/.config/sync0` 自动生成节点密钥与 `base64url` 编码的 `iroh` 地址，无需记忆或暴露公网 IP。
- **按需多选同步**：支持同步整个分享组，也支持同时勾选多个文件或子目录按需同步。
- **组员分级权限与上传目录控制**：支持只读（`ro`）、读写上传（`rw`）、管理（`admin`）权限，并可单独指定每个成员允许上传的路径前缀（默认允许上传到该分享的根目录）。
- **跨网满速与增量同步**：大文件并发传输、海量小文件内存聚合落盘，文件局部修改时仅同步变化的数据块。

## 与 rsync 速度对比

### 跨洋网络单源实测（中国 → 美国，延迟 260 ms）

| 传输场景 | 数据规模 | 模式 | `sync0` | `rsync -az` | 提升倍数 |
| :--- | :---: | :---: | :---: | :---: | :---: |
| 500 个小文件 | 1.91 MB | 全量同步 | 18.84 Mbps | 5.32 Mbps | 3.5x |
| 500 个小文件 | 1.91 MB | 增量同步 | 27.15 Mbps | 3.83 Mbps | 7.1x |
| 代码与归档包 (`.tar`) | 10.0 MB | 全量同步 | 139.70 Mbps | 24.97 Mbps | 5.6x |
| 代码与归档包 (`.tar`) | 10.0 MB | 增量同步 | 83.38 Mbps | 26.02 Mbps | 3.2x |
| 视频文件 (`.mp4`) | 15.0 MB | 全量同步 | 54.06 Mbps | 27.04 Mbps | 2.0x |
| 视频文件 (`.mp4`) | 15.0 MB | 增量同步 | 216.83 Mbps | 21.51 Mbps | 10.1x |

### 跨洋网络 P2P 多节点拉取实测（中国 → 美国双节点，延迟 260 ms）

多源种子节点条带化并发拉取实测对比：

| 传输场景 | 数据规模 | 单节点做种 | 双节点 P2P 并发做种 | 提升倍数 |
| :--- | :---: | :---: | :---: | :---: |
| 500 个小文件 | 1.97 MB | 5.28 Mbps | 7.87 Mbps | 1.5x |
| 代码与归档包 (`.tar`) | 30.0 MB | 56.35 Mbps | 83.17 Mbps | 1.5x |
| 视频文件 (`.mp4`) | 30.0 MB | 27.00 Mbps | 39.46 Mbps | 1.5x |

### 跨洲高速专线实测（欧洲 → 北美，延迟 86 ms）

| 传输场景 | 数据规模 | `sync0` | `rsync` | 提升倍数 |
| :--- | :---: | :---: | :---: | :---: |
| 10,000 个小文件 | 77.6 MB | 688 Mbps | 128 Mbps | 5.4x |
| 代码与归档包 (`.tar`) | 512 MB | 2111 Mbps | 438 Mbps | 4.8x |
| 视频文件 (`.mp4`) | 512 MB | 556 Mbps | 338 Mbps | 1.6x |
| 增量同步（修改 1% 内容） | 512 MB | 34133 Mbps | 2214 Mbps | 15.4x |

## 安装

### 方式一：通过 Cargo 安装

```bash
cargo install sync0
```

### 方式二：下载预编译二进制

前往 [GitHub Releases](https://github.com/webc-site/sync0/releases) 下载对应系统的 `.tar.xz` 压缩包（支持 Linux、macOS、Windows、Android、FreeBSD、NetBSD），解压后放入 `PATH` 目录：

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## 快速上手

`sync0` 默认使用 `~/.config/sync0` 作为配置目录（如需切换其他目录，可添加 `-c <目录>`）。

### 1. 查看本机节点地址与设置名片

```bash
# 查看本机节点地址（首次运行自动在 ~/.config/sync0 生成密钥）
sync0 id

# 生成个人节点名片（支持用户名、所在城市、邮箱、网址与 256 字以内签名）
sync0 profile -u alice --region SHA --email alice@example.com --url https://example.com -b "Shanghai node"
```

### 2. 发布多个文件分享组与设置组员权限（`.nt` 配置）

`sync0` 采用 [NestedText](https://nestedtext.org)（`.nt`）格式配置，每个用户单独一个 `.nt` 配置文件，可在 `config.nt` 中通过 `users_dir` 指定用户配置文件目录（自动包含该目录下的全部 `.nt` 文件），用户与允许上传的路径前缀均使用无序列表（`-`）。**未填写上传路径前缀时，默认允许上传到该分享的根目录**。

1. 在 `~/.config/sync0/users/` 目录下为每个成员创建独立的 `.nt` 配置文件：

`~/.config/sync0/users/alice.nt`（只读成员，限速 `80MB`，`rate` 支持 `GB`、`MB`、`KB` 单位）：

```yaml
id: 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c
perm: ro
rate: 80MB
```

`~/.config/sync0/users/bob.nt`（受限上传成员：仅允许上传到 `uploads/bob` 和 `shared/common` 目录下）：

```yaml
id: 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f
perm: rw
write_prefix:
  - uploads/bob
  - shared/common
```

`~/.config/sync0/users/charlie.nt`（默认上传成员：省略 `write_prefix` 时，默认允许上传到该分享的根目录）：

```yaml
id: 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
perm: rw
```

2. 在 `~/.config/sync0/config.nt` 中配置用户目录与多个文件分享组：

```yaml
users_dir: ./users

groups:
  dataset:
    path: ./data
    auth: false
    perm: ro
    rate: 50MB

  models:
    path: ./models
    auth: false
    perm: ro
    rate: 1GB

  team-vault:
    path: ./vault
    auth: true
    perm: ro
    rate: 512KB
    users:
      - alice
      - bob
      - charlie
```

启动分享服务：

```bash
# 直接启动（自动加载 ~/.config/sync0/config.nt 及 users_dir 目录下的全部 .nt 用户配置）
sync0 share --region SHA

# 或直接通过命令行快速发布多个分享组并授权组员
sync0 share dataset:./data team-vault:./vault \
  --auth \
  --allow-ro 8J9aBcDeFgHiJkLmNoPqRsTuVwXyZ0123456789ab_c \
  --allow-rw 7K8bCdEfGhIjKlMnOpQrStUvWxYz0123456789de_f:uploads/bob,shared/common \
  --allow-rw 9M0cDeFgHiJkLmNoPqRsTuVwXyZ0123456789gh_i
```

### 3. 拉取全部内容或多选文件与目录 (`sync0 pull`)

连接任意一个对等节点即可自动获取该分享组内所有可用的种子节点，并从多个种子节点并发拉取：

```bash
# 拉取整个 dataset 分享组到本地 ./local_data
sync0 pull dataset:// ./local_data -p <节点地址>

# 多选同步：同时拉取单文件 docs/readme.md 与子目录 assets/icons、src/core
sync0 pull dataset://docs/readme.md,assets/icons,src/core ./local_data -p <节点地址>
```

### 4. 向分享组上传文件或目录 (`sync0 push`)

拥有上传权限（`rw` 或 `admin`）的成员可将本地文件或目录上传至远端分享组：

```bash
# 拥有根目录上传权限的成员：直接上传到分享根目录
sync0 push team-vault:// ./new_release -p <节点地址>

# 仅被授权特定前缀（如 uploads/bob）的成员：上传到允许的路径前缀下
sync0 push team-vault://uploads/bob/report.pdf ./report.pdf -p <节点地址>
```

### 5. 订阅自动同步与做种 (`sync0 sub`)

订阅某台节点的分享组后，本地既是接收客户端，也是新的做种服务器；当源节点文件更新时自动同步，同时为其他节点提供分块下载：

```bash
# 订阅整个分享组并自动做种
sync0 sub dataset:// ./local_mirror -p <节点地址>

# 仅多选订阅指定文件与子目录并做种
sync0 sub dataset://models/llama.onnx,configs ./local_mirror -p <节点地址>
```

### 6. 多机群组对等同步 (`sync0 group`)

多台机器加入同一分享组，彼此自动发现可用种子节点并保持双向同步（支持全量同步或多选文件与目录）：

```bash
# 全量加入群组双向同步
sync0 group team-docs:// ./docs -p <节点地址>

# 仅多选同步群组中的 wal 目录与 meta.json 文件
sync0 group team-docs://wal,meta.json ./docs -p <节点地址>
```

### 7. 两台机器点对点直传 (`send` / `recv`)

无需预先配置群组，临时在两台机器间直传文件或目录：

```bash
# 接收端监听
sync0 recv 0.0.0.0:9000 ./backup

# 发送端直传文件或目录（可选 --delete 镜像清理多余文件，--rate 限速）
sync0 send ./project 192.168.1.10:9000
```

### 8. 多语言切换

`sync0` 支持 75 种语言，默认根据系统语言环境自动切换；也可通过 `SYNC0_LANG` 手动指定：

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```
