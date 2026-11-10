# 04. 原生 UDP 极速数据面传输引擎 (`s0_udp`)

## 目标
实现媲美 IBM Aspera FASP 的零队头阻塞原生 UDP 传输层，`iroh` 打洞完成后由 `s0_udp` 直接接管 100% 文件块收发。

## 任务清单
- [ ] 微秒级步进器 `Pacer` 与自适应拥塞控制（根据带宽限额 `eff_rate` 与 RTT/丢包动态调节发送间隔 `init_gap_us`）
- [ ] 区间发送器 `send_range_udp`：仅发送 `[blk_lo, blk_hi)` 内接收方缺失的块，响应 `Nack` 位图重传直至 `Ack`
- [ ] 区间接收器 `recv_range_udp`：接收指定 `[blk_lo, blk_hi)` 块区间，逐块校验 128-bit BLAKE3 摘要并直接通过 `FileAt::write_all_at` 写入共享 `.s0tmp`，内存严格限制在滑动窗口大小
- [ ] 双栈 UDP Socket 绑定与基于 `PeerAddrs` 的并发双向打洞（LAN 优先直连 + NAT 穿透）
