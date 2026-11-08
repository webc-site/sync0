# `sync0` 跨洲广域网性能实测（`doc/zh/benchmark.md`）

本文记录 `sync0` 与 `rsync -az` 在真实跨洲链路上的吞吐对比，复现脚本为仓库根目录的 [`bench.sh`](../../bench.sh)。

## 1. 测试拓扑

| 节点 | 角色 | 平台 | 位置 | 地址 |
| :--- | :--- | :--- | :--- | :--- |
| `c1` | 数据源 / 单点发送端 / P2P 做种源 | x86_64 Linux，8 核 | 美国 | 公网 IPv6 + Tailscale `100.64.1.1` |
| `n1` | 接收端 / P2P 拉取端 | aarch64 Linux，6 核 | 欧洲 | 公网 IPv6 + Tailscale `100.105.13.82` |
| `s1` | P2P 第二做种源 | x86_64 Linux，4 核 | 欧洲 | Tailscale `100.107.76.41` |

方向固定为 `c1 → n1`。`s1` 只参与 P2P 多源做种，不改变数据方向。

### 网络延时

`c1 → n1` 公网 IPv6 ICMP（20 包）：

```text
rtt min/avg/max/mdev = 61.946/70.993/134.513/18.500 ms
```

Tailscale 直连（非 DERP 中继）：

```text
pong from n1 (100.105.13.82) via [2a0a:4cc0:2000:34aa::1]:41641 in 64ms
```

传输过程中因排队，ICMP 平均值会升到约 `88 ms`。下文吞吐均基于上述真实 RTT，不做本地回环测试。

### 路径说明

- 单点直传与 `rsync` 走公网 IPv6 直连，对应生产环境中 `rsync` 使用 22 端口、`sync0` 直传使用 UDP 的常规部署。
- P2P 控制面需要额外的 TCP 监听端口，机房安全组只放行 SSH 等少数端口，因此 P2P 走 Tailscale 直连地址。`tailscale ping` 显示为公网 IPv6 直连（`:41641`），不是 DERP 中继，物理链路与单点测试相同。
- Tailscale 会带来 WireGuard 封装与 1200 B 载荷上限，这部分开销已计入 P2P 结果。

## 2. 数据集

测试时在 `c1` 上直接下载 [`linux-7.3.3.tar.xz`](https://www.kernel.org/pub/linux/kernel/v7.x/linux-7.3.3.tar.xz)，并缓存到 `/tmp/s0bench/data`，后续复跑不再重复下载。

| 数据集 | 大小 | 文件数 | 对应特征 |
| :--- | ---: | ---: | :--- |
| `linux-7.3.3.tar.xz` | 162,426,868 B（154.9 MiB） | 1 | 不可压缩：xz 已压缩，熵接近上限 |
| `linux-7.3.3.tar` | 1,718,722,560 B（1.60 GiB） | 1 | 可压缩：源码文本与零填充，压缩收益高 |
| `linux-7.3.3/` | 1,641,555,619 B（1.53 GiB） | 95,979 | 海量小文件：完整内核源码树 |

`s1` 侧用同一份官方 xz 在本地解压出 tar 与源码树，作为第二做种副本，避免为了让 `s1` 做种而额外走一遍广域网。

## 3. 对比模式

| 模式 | 命令 | 说明 |
| :--- | :--- | :--- |
| `sync0` 单点 | `sync0 bench-send` / `bench-recv` | `c1` 单 UDP 会话直传 `n1` |
| `sync0` P2P | `sync0 share` + `sync0 sub` + `sync0 pull` | `c1`、`s1` 双源做种，`n1` 拉取，实测 `peers=2` |
| `rsync` | `rsync -az --delete` | `c1` 经 SSH 推到 `n1`，与生产用法一致 |

计时口径：

- 单点模式取 `sync0` 发送端输出的 `us`，覆盖扫描、清单、数据传输与收尾。
- P2P 模式在 `n1` 上用单调时钟记录 `sync0 pull` 全流程，包含控制面、清单与多源传输。
- `rsync` 在 `c1` 上用单调时钟记录完整命令耗时，包含 SSH 与压缩。

## 4. 结果

| 数据集 | `sync0` 单点 | `sync0` P2P 双源 | `rsync -az` | 单点 / rsync | P2P / rsync |
| :--- | ---: | ---: | ---: | ---: | ---: |
| `linux-7.3.3.tar.xz`（不可压缩） | **78.88 Mbps**<br>16.5 s | **278.19 Mbps**<br>4.7 s | 32.00 Mbps<br>40.6 s | 2.46× | **8.69×** |
| `linux-7.3.3.tar`（可压缩） | **315.65 Mbps**<br>43.6 s | **847.41 Mbps**<br>16.2 s | 122.46 Mbps<br>112.3 s | 2.58× | **6.92×** |
| `linux-7.3.3/`（海量小文件） | 140.82 Mbps<br>93.3 s | **535.58 Mbps**<br>24.5 s | 178.15 Mbps<br>73.7 s | 0.79× | **3.01×** |

原始数据：

- [`bench/kernel-results.tsv`](../../bench/kernel-results.tsv)
- [`bench/kernel-results.json`](../../bench/kernel-results.json)

每轮传输完成后，都在 `n1` 侧对文件或整棵树计算 SHA-256，并与 `c1` 基准值逐字节比对，全部通过。

## 5. 结果分析

### 不可压缩文件

`xz` 归档无法再压缩，吞吐只取决于拥塞控制、丢包恢复与并发能力。

- `rsync` 单条 TCP 流在 62~135 ms RTT 下受窗口与丢包恢复限制，只有 32 Mbps。
- `sync0` 单点用 UDP 块级重传，达到 78.9 Mbps。
- P2P 双源条带化后达到 278 Mbps，为 `rsync` 的 8.69 倍。

### 可压缩文件

tar 含大量源码文本与零填充，三个模式都获得压缩收益，但上限不同。

- `rsync -z` 受单流 TCP 限制，112.3 s，122 Mbps。
- `sync0` 单点内置 Zstd 检测与压缩，43.6 s，315.7 Mbps。
- P2P 双源同时承担压缩与传输，16.2 s，847 Mbps。

### 海量小文件

这是唯一需要单独说明的场景：

- 单点 `bench-send` 对 95,979 个文件逐一调度与确认，93.3 s，140.8 Mbps，**慢于 `rsync` 的 178.2 Mbps（0.79×）**。逐文件控制开销超过了协议本身的收益。
- P2P 模式把清单聚合后按块条带化，并在 `c1`、`s1` 两端并发取数，24.5 s、535.6 Mbps，为 `rsync` 的 3.01 倍。
- 结论：海量小文件应使用 P2P / 订阅模式，单点直传在小文件密度极高时并不占优。

## 6. 复现

```bash
# 全量：编译交叉产物、部署、准备数据、三种模式、SHA-256 校验
./bench.sh

# 复用已编译二进制与已缓存数据
SKIP_BUILD=1 ./bench.sh

# 只跑部分模式 / 数据集
MODES="sp p2p" DATASETS="xz tar" ./bench.sh

# 强制重新下载数据集
REFRESH=1 ./bench.sh
```

脚本会：

1. 交叉编译 `x86_64-unknown-linux-gnu` 与 `aarch64-unknown-linux-gnu` 的 `sync0`。
2. 按 SHA-256 差异部署到 `c1`、`s1`、`n1` 的 `/tmp/sync0`。
3. 在 `c1` 下载并解压内核数据，在 `s1` 准备做种副本。
4. 测量 `c1 → n1` 的公网与 Tailscale RTT。
5. 依次执行单点、P2P、`rsync`，每轮校验 SHA-256 并清理 `/tmp/s0bench/out`。
6. 退出时通过 `trap` 杀掉三端 `sync0` 进程。

## 7. 清理

测试结束后已确认：

```text
c1: NO_SYNC0
s1: NO_SYNC0
n1: NO_SYNC0
```

`/tmp/s0bench/data` 与 `/tmp/s0bench/s1` 保留为缓存，便于复跑；临时输出目录已删除。测试期间为排查直连端口临时添加的 `iptables` 规则也已移除。
