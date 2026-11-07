# 03. SIMD 内容寻址分块清单与无锁原位存储引擎 (`s0_cdc` + `s0_fs`)

## 目标
保证多节点 P2P 并发分发的**强一致性（128-bit BLAKE3 快照锁 + 逐块哈希校验）**与**极低内存占用（$O(\text{窗口})$ 环形槽位 + 多线程共享 `.s0tmp` 无锁 `pwrite`）**。

## 任务清单
- [ ] `s0_cdc`：
  - FastCDC 内容定义分块 + SIMD BLAKE3 128-bit 块摘要
  - 紧凑二进制 `Manifest` 编解码（支持按路径前缀 `prefix` 零拷贝裁剪子集，支持计算 128-bit `root_hash`）
  - 原位流式基准文件匹配 `match_basis_file`（64KiB 栈缓冲，零整文件内存分配）与缺失块区间迭代器 `for_each_missing_span`
- [ ] `s0_fs`：
  - 跨平台无锁原位随机读写 `FileAt`（`read_exact_at` / `write_all_at`）
  - 并发预分配暂存文件 `SharedStagingFile`（支持多个对等节点并发写入同一文件的不同 `[blk_lo, blk_hi)` 区间，全部块校验通过后原子 `rename`）
  - 目录扫描与增量变更检测器
