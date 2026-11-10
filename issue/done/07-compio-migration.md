# 07. 彻底移除 `tokio` 依赖，全栈基于 `compio` 异步运行时重构 (`s0_profile` + `s0_udp` + `s0_peer`)

## 目标
清理依赖树中的 `tokio`（含间接依赖），将网络控制面与原生 UDP 数据面全面迁移至基于完成式 I/O（io_uring / kqueue / IOCP）的 `compio` 异步运行时。

## 任务清单
- [ ] `s0_profile`：使用 `ed25519-dalek` 原生实现 32B `iroh` 兼容 `SecretKey` / `PublicKey` / `Signature`，移除间接引入 `tokio` 的重型依赖
- [ ] `s0_udp`：基于 `compio::net::UdpSocket`、`compio::time` 与 `socket2`（32 MiB 内核收发缓冲）实现异步原生 UDP 块区间收发 (`send_range_udp` / `recv_range_udp`)
- [ ] `s0_peer`：基于 `compio::runtime::Runtime` 与 `compio::net::{TcpListener, TcpStream}` 实现对等控制面帧收发、长连接订阅推送与多源并发 `swarm_pull`
- [ ] 验证 `Cargo.lock` 中零 `tokio` 依赖，通过 `./sh/clippy.sh` 与 `./test.sh`，并由子代理完成代码审查后移入 `issue/done/`
