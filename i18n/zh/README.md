# sync0

`sync0` 是用于跨网络传输文件、增量同步目录以及多机器并发分发数据的命令行工具。

## 设计初衷

在跨机房、跨国或存在网络延迟与丢包的环境中，`rsync` 和 `scp` 等工具基于单连接顺序确认，带宽利用率受限；当多台机器同时从同一台服务器拉取数据时，源服务器网卡容易成为瓶颈。

`sync0` 的目标是解决两个问题：

1. 让两台机器之间的文件与目录同步跑满物理带宽，并在文件发生局部修改时只传输变化的部分。
2. 让多台机器同步同一份目录时可以互相传数据块，降低源服务器负载，缩短整体同步时间。

## 与 rsync 速度对比

测试环境：欧洲节点 $\leftrightarrow$ 北美节点，往返延迟 86 ms，测量包含目录扫描、去重比对、网络传输与落盘校验的完整耗时：

| 传输场景 | 数据规模 | `sync0` | `rsync` | 倍数 |
| :--- | :---: | :---: | :---: | :---: |
| 10,000 个小文件 | 77.6 MB | 0.90 s (688 Mbps) | 4.86 s (128 Mbps) | 5.4x |
| 代码与归档包 (`.tar`) | 512 MB | 1.94 s (2111 Mbps) | 9.36 s (438 Mbps) | 4.8x |
| 视频文件 (`.mp4`) | 512 MB | 7.36 s (556 Mbps) | 12.11 s (338 Mbps) | 1.6x |
| 增量同步（修改 1% 内容） | 512 MB | 0.12 s | 1.85 s | 15.4x |

## 性能提升方式

- **UDP 并发传输**：通过 UDP 发送数据块，仅对丢失的分片进行重传，避免单个丢包阻塞后续文件传输。
- **内容分块去重**：将文件按内容切分为数据块并计算校验值。接收端已有的相同内容直接本地复用，不重复占网。
- **按网速自动调整压缩**：对文本、代码等可压缩内容开启压缩；对视频、图片、压缩包或在高速局域网下自动跳过压缩，减少 CPU 消耗。
- **多节点就近拉取**：多台机器拉取同一目录时，自动优先从同城或延迟更低的已下载节点并发拉取数据块。

## 安装

### 方式一：通过 Cargo 安装

```bash
cargo install sync0
```

### 方式二：下载预编译二进制

前往 [GitHub Releases](https://github.com/webc-site/sync0/releases) 下载对应系统的 `.tar.xz` 压缩包（支持 Linux x86_64/ARM64、macOS Apple Silicon/Intel、Windows x64/ARM64、Android ARM64/x86_64、FreeBSD、NetBSD），解压后将 `sync0` 放入 `PATH` 目录即可：

```bash
tar -xJf sync0-*.tar.xz
sudo mv sync0 /usr/local/bin/
```

## 使用指南

### 1. 两台机器直接传文件或目录

在接收端机器启动监听，指定接收保存路径：

```bash
sync0 recv 0.0.0.0:9000 ./backup
```

在发送端机器发送单个文件或整个目录：

```bash
sync0 send ./project 192.168.1.10:9000
```

常用参数：

```bash
# 删除接收端存在但发送端不存在的多余文件（镜像同步）
sync0 send ./project 192.168.1.10:9000 --delete

# 限制发送速率（单位：字节/秒，例如 50 MB/s）
sync0 send ./project 192.168.1.10:9000 --rate 50000000

# 静默模式（不显示进度条）
sync0 send ./project 192.168.1.10:9000 -q
```

### 2. 发布共享目录与多节点拉取

在源服务器发布一个共享目录（名称为 `dataset`）：

```bash
sync0 share dataset ./data --region SHA
```

在其他机器上拉取该共享目录（可同时填写多个节点地址，用逗号分隔以并发拉取）：

```bash
sync0 pull 10.0.0.1:9000,10.0.0.2:9000 dataset ./local_data
```

只拉取共享目录中的某个子目录：

```bash
sync0 pull 10.0.0.1:9000 dataset ./local_data --subpath 2026/logs
```

将本地修改推送回远端共享目录：

```bash
sync0 push 10.0.0.1:9000 dataset ./local_data
```

### 3. 订阅目录自动更新

订阅源节点的共享目录，当源目录有文件新增或修改时自动同步到本地，同时本机自动作为节点供其他机器拉取：

```bash
sync0 sub 10.0.0.1:9000 dataset ./local_mirror
```

### 4. 多机群组双向同步

多台机器加入同一个群组名称，任意节点目录内的变更会自动同步给群组内其他节点：

```bash
sync0 group team-docs ./docs 10.0.0.1:9000,10.0.0.2:9000
```

### 5. 节点身份与个人名片

每个节点支持设置用户名、所在城市、邮箱、网址以及一句个性签名（上限 256 个字符），并通过本机私钥签名认证：

```bash
# 查看本机公钥 ID（指定配置目录以持久化保存密钥）
sync0 id -c ~/.config/sync0

# 生成并展示带个性签名的节点名片
sync0 profile -c ~/.config/sync0 \
  -u alice \
  --region SHA \
  --email alice@example.com \
  --url https://example.com \
  -b "Shanghai backup node"
```

在启动 `share`、`pull`、`sub`、`group` 时同样可以附带这些个人资料参数：

```bash
sync0 share dataset ./data -c ~/.config/sync0 -u alice --region SHA -b "Shanghai backup node"
```

### 6. 多语言切换

`sync0` 支持 75 种语言，默认根据操作系统的语言环境自动显示对应语言的命令行帮助与提示；如果系统语言不在支持列表中，默认使用英文。

如需手动指定语言，可设置环境变量 `SYNC0_LANG`：

```bash
SYNC0_LANG=en sync0 --help
SYNC0_LANG=ja sync0 --help
SYNC0_LANG=de sync0 --help
```
