---
task_manifest: task.yaml
status: approved
---

# Restore Minimal PN Control Client Proposal

Risk profile: not-created (replace with ./risk-profile.yaml only after high-risk confirmation)

## Workflow Tier Judgment
- Proposed tier: standard
- Final tier: standard
- Tier rationale / triggered boundaries: 该任务不是重新设计控制链，而是把 `pn_control_client.rs` 恢复到修改前的单 endpoint factory，只保留三模式所需的 endpoint 选择。生产改动局限于一个内部模块，不改变公开 API、wire、`p2p-frame`、默认 dual listener/report 行为或持久状态；配套只清理不再适用的测试并修正一条配置注释，因此建议 standard。
- Proposal and tier confirmation: confirmed by the user on 2026-08-25 with “确认”.

## Background and Goal
当前未提交实现对 `vpn-server/src/pn_control_client.rs` 改动过大：除了 transport 选择，还增加了多候选 fallback、active target、旧 target 清理、并发锁和 TTP 测试注入抽象。用户明确本任务目标是恢复该文件的修改，只保留 transport 支持真正需要的部分。

目标是把 `ControlCmdTunnelFactory` 恢复到修改前的单 target 结构和直接创建流程。唯一必要的生产差异是从已经按 transport 排序的 control endpoint 集合中选择第一项：`tcp` 得到 TCP，`quic` 得到 QUIC，`dual` 得到优先的 QUIC。

## Scope
### In scope
- 在 `vpn-server/src/pn_control_client.rs` 中恢复原有的具体 `TtpClientRef`、单 `control_endpoint`、单 `open_cmd_tunnel` 和直接 `create_tunnel` 流程。
- 创建 control client 时只取 `control_server.endpoints` 第一项；现有配置顺序保证 `tcp` 为 TCP、`quic` 为 QUIC、`dual` 为 QUIC。
- 删除该文件新增的 `ControlTtpClientOps`、默认 adapter、test constructor、`active_target`、`create_lock`、多候选 fallback、classified endpoint 搜索和 target 转移逻辑。
- 删除仅验证上述被移除机制的 fake/async 测试，保留三模式配置、endpoint 顺序及其他原有 PN control 测试接线。
- 把配置示例中的“dual 失败后回退 TCP”改为准确的“dual control 使用 QUIC”。

### Out of scope
- 不修改 `server_config.rs`、`main.rs` 或 `p2p-frame` 的实现。
- 不改变三种模式的配置解析、本地 listener、identity endpoint、PN 上报 endpoint、primary 或 port mapping 过滤行为。
- 不实现 dual 控制连接 TCP fallback，不新增 control target 状态管理。
- 不改变心跳、认证、审批、流量统计、wire 编码或客户端选择逻辑。

### Boundary with neighboring modules
`server_config.rs` 继续生成按 transport 排序的 control endpoint 集合；`pn_control_client.rs` 只消费第一项并构造单个 `TtpTarget`；`p2p-frame` 根据该 endpoint 的明确 protocol 创建 tunnel。`dual` 的 TCP endpoint 仍用于 PN 入站监听和客户端可见地址，不作为 PN 到 SN 控制连接的备用候选。

## Requirement Review
该方向与修改前的控制连接行为一致：旧 factory 只保存一个 endpoint，并直接调用 `TtpClient::connect_server` 和 `open_control_stream`。恢复原结构能避免为了当前需求引入额外状态机和测试抽象。

明确 tradeoff 是：`dual` 并不意味着 PN 控制连接在 QUIC 失败后自动回退 TCP；若 QUIC 控制链不可用，PN 会继续按原有客户端机制重试 QUIC。这个行为与修改前一致，并符合“只改 transport 必要部分”的范围。

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-simplify-pn-control-transport-selection | 恢复 `pn_control_client.rs` 的单 endpoint factory，仅按 transport 选择 control endpoint 第一项；tcp、quic、dual 分别使用 TCP、QUIC、QUIC。 | 生产代码只修改 `pn_control_client.rs`；测试和示例只做与删除逻辑直接相关的必要同步。 | dual 不提供控制连接 TCP fallback，但保持修改前行为并显著缩小改动。 | diff 证明 factory 恢复原结构且只保留 endpoint 选择差异；三模式顺序测试、现有 PN control 测试和受影响编译通过。 | 不修改配置解析、listener/report 装配、`p2p-frame` 或共享协议。 |

## Success Criteria
- Concrete user-visible or system-visible result: 独立 PN 三种 transport 模式保持可用；`pn_control_client.rs` 恢复为接近修改前的单 endpoint 实现，dual 选择 QUIC。
- Required evidence: 生产 diff 只保留必要的单 endpoint 选择；tcp/quic/dual endpoint 顺序测试通过；不再存在 fallback/active-target/test adapter 符号；现有相关测试和 `bucky-vpn-server` 编译通过。
- Explicit non-goals: 不修改 `server_config.rs`、`main.rs`、`p2p-frame`，不实现 dual 控制连接 TCP fallback，不改变本地双协议 listener 和发布行为。

## Risks
- 在只允许 TCP 出站的环境中，`dual` 控制链不会回退 TCP；运维需要显式使用 `pn.transport: tcp`。
- 必须避免整文件回退覆盖 `pn_control_client_tests.rs` 中其他既有或任务无关测试；只删除本次 fallback/testability seam 对应内容。
- 当前工作树包含 031 任务的未提交实现；执行时必须只恢复 `pn_control_client.rs` 中非必要的控制状态机改动，保留三模式配置、listener、上报和 mapping 行为。
