# 08. 极致网络传输原语优化与 `c1` 广域网基准对标 (`s0_cc` + `s0_codec` + `s0_udp` + `bench.sh`)

## 目标
结合 FASP/Vegas 时延梯度拥塞控制、非对称 RTT 与自适应 RACK 乱序时间窗、带宽×CPU 自适应 Zstd 块压缩、16 MiB 小文件内存池及 `HAVE_ALL_ONES` 零载荷增量快路径，实现极致广域网传输性能并在 `c1` 节点实测对标。

## 任务清单
- [ ] `s0_cc`：引入时延梯度拥塞控制 (`Cc`)、非对称平滑 RTT 与自适应 RACK 乱序时间窗估计器 (`RttEstimator`) 及 3× 抗反射放大 (`AntiAmp`)
- [ ] `s0_codec`：引入 `mzstd` 块编解码器 (`Codec`)、扩展名/魔数/16KiB 前缀试压探测器 (`Detector`) 与带宽×CPU×对端解压速率 EWMA 级别策略 (`Policy`)
- [ ] `s0_udp`：集成 RACK 发送在途环 (`Sender`)、TACK 8192 位包号位图 + 8 块未完成分片位图 (`AckGen`)、零分配接收块池 (`Receiver`)、16 MiB 小文件内存池与 P2P 区间自适应 Zstd 压缩
- [ ] `sync0` + `bench.sh`：支持 `bench-send` / `bench-recv` 并对标实测本机到 `c1` 广域网全量、修改增量、零改动复传吞吐与加速比
