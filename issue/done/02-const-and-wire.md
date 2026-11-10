# 02. 全数值协议常量、城市级地域、自签名节点名片与零拷贝线协议 (`s0_const` + `s0_region` + `s0_wire` + `s0_profile`)

## 目标
所有协议原语、节点名片属性键与地域编码全部采用紧凑数字常量（禁止字符串键名与国家主权冲突编码），实现 `s0_const`、`s0_region`、`s0_wire` 与 `s0_profile` 四个基础 crate。

## 任务清单
- [ ] `s0_const` (`#![no_std]`)：
  - 定义数值操作码 `Op` (`Pull=1`, `Push=2`, `Stat=3`, `Sub=4`, `Have=5`, `GroupJoin=6`)
  - 定义数值状态码 `Status` (`Ok=0`, `Unauth=1`, `Forbidden=2`, `NotFound=3`, `Conflict=4`, `BadReq=5`)
  - 定义节点自签名名片属性键数字常量 `AttrKey` (`Name=1`, `Email=2`, `Url=3`, `Region=4`, `Bio=5`) 及 `MAX_NAME_CHARS = 64`
  - 定义请求标志位与 UDP 数据面常量
- [ ] `s0_region` (`#![no_std]`)：
  - 定义精确到城市级别的 `#[repr(u16)] pub enum Region` 数字常量（规避国家概念与主权争议）
  - 提供城市三字码、中文/英文显示名、大区号 `zone()` 及 P2P 就近调度亲和度函数 `proximity_score(a, b)`
- [ ] `s0_wire` (`#![no_std]`)：
  - 实现 `Rd` / `Wr` 零拷贝游标与 `u15` 变长整数
  - 实现 20B `DataHeader`、紧凑 `Nack` / `Ack`
  - 实现对等控制面帧 `P2pReq`、`P2pResp`、`SwarmPeer`、`SubPush`
- [ ] `s0_profile`：
  - 基于 `iroh` Ed25519 密钥对实现节点自签名名片 `SignedProfile`（支持用户名 $\le 64$ 字符、邮箱、网址、城市 `Region` 数字常量、简介，按 `AttrKey` 升序紧凑 TLV 编码与 Ed25519 签名/验签）
  - 编写完整测试
