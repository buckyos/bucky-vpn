---
task_manifest: task.yaml
status: approved
---

# PN Transport Modes Proposal

Risk profile: ./risk-profile.yaml

## Workflow Tier Judgment
- Proposed tier: high-risk
- Final tier: high-risk
- Tier rationale / triggered boundaries: 该功能让独立 PN 可选择 TCP、QUIC 或两者同时启用，会改变生产监听协议、控制连接协议、identity endpoint 和客户端可见 PN endpoint。虽然实现预计集中在 `bucky-vpn-server`，但它直接影响运行时连接可达性、配置合同和现有部署兼容性，因此建议 `high-risk`。
- Proposal and tier confirmation: confirmed by the user on 2026-08-25 with “确认，自动完成”; automatic downstream execution authorized from design.

## Background and Goal
当前服务端把根 `ip`/`port` 固定构造成 QUIC 主 endpoint，再自动追加同地址 TCP endpoint；独立 PN 的 `pn.control_server.endpoint` 也固定按 QUIC 解析。协议类型和客户端连接代码已经能表达 TCP endpoint，但配置无法让独立 PN 完全不启用 QUIC/UDP。

目标是让 `sn.enabled: false`、`pn.enabled: true` 的独立 PN 可以显式选择三种 transport 模式：`tcp`、`quic`、`dual`。本地 listener、对控制节点使用的 endpoint 以及向控制面/客户端发布的 PN endpoint 必须与所选模式一致。未配置新选项时使用 `dual`，继续保持当前 QUIC+TCP 行为。

## Scope
### In scope
- 为独立 PN 增加枚举式 `pn.transport` 配置，支持 `tcp`、`quic`、`dual` 三种值；缺省为 `dual`，保留当前 QUIC+TCP 行为。
- `tcp` 模式下，本地 P2P identity、监听、控制连接和上报 endpoint 只使用 TCP，不创建 QUIC/UDP listener。
- `quic` 模式下，本地 P2P identity、监听、控制连接和上报 endpoint 只使用 QUIC，不创建 TCP listener。
- `dual` 模式下，本地同时监听并发布 QUIC 与 TCP endpoint；到控制节点的连接按 QUIC 优先、TCP fallback 的有序候选执行，并且同一时刻只保留一条有效控制/心跳链路。
- 三种模式均继续使用现有 `advertised_ip`、对应协议的 `port_mapping.quic` / `port_mapping.tcp` 和观测地址合并语义；不发布未启用协议的映射端口。
- 对未知 transport 值以及同进程 SN+PN 使用非 `dual` 模式的情况返回清晰配置错误；组合部署当前共享同一 P2P endpoint 集合，不能诚实承诺只限制 PN。
- 更新服务端配置示例，并增加配置、endpoint 构造和默认兼容性的定向验证。

### Out of scope
- 不为同一进程内的 SN 与 PN 拆分两套 listener/P2P identity；组合部署仍保持现有 QUIC+TCP 共享监听。
- 不改变默认 transport、默认端口或已有配置的行为。
- 不改变 PN 心跳、审批、流量统计、地址观察、端口映射数据结构或 wire 编码。
- 不修改 `vpn-frame` 或 `vpn-client` 的 endpoint 协议结构和选择逻辑；当前代码已能消费 TCP PN endpoint。

### Boundary with neighboring modules
`bucky-vpn-server` 负责解析 transport、选择本地/控制端 endpoint 并发布实际监听协议。`vpn-frame` 继续承载已有的 `quic`/`tcp` endpoint 表达，`bucky-vpn` 客户端继续按现有逻辑连接服务端下发的 TCP/QUIC endpoint，不在本任务中改变共享协议或客户端策略。

## Requirement Review
该要求对不同网络条件下的独立 PN 部署是合理的：TCP-only 可适配只允许 TCP 的网络，QUIC-only 可避免不需要的 TCP listener，dual 则保留双协议兼容性。使用单一枚举式 transport 配置比多个 enable/disable 布尔值更清晰，也能避免无效组合。为了避免配置看似生效但组合部署仍由 SN 共享另一协议 listener，本任务将非 dual 模式限定为独立 PN，并在不支持的组合上失败而不是静默降级。

兼容性策略是缺省保持 `dual`；只有显式设置 `pn.transport: tcp` 或 `pn.transport: quic` 才收窄运行时协议。单协议模式同时约束入站 PN listener、PN 到控制节点的连接以及对外发布的 endpoint，确保配置改变的是实际传输面，而不只是隐藏某类上报地址。

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-enable-standalone-pn-transport-modes | 独立 PN 支持 `pn.transport: tcp`、`quic`、`dual`；本地监听、控制连接和发布 endpoint 与模式一致，dual 控制连接按 QUIC 优先、TCP fallback，缺省为 dual。 | 仅修改 `bucky-vpn-server` 的配置与运行时装配，复用现有共享 endpoint 合同和客户端 TCP/QUIC 支持。 | 单协议模式会放弃另一协议的连通路径；组合 SN+PN 暂不支持协议拆分并会对非 dual 配置明确报错。 | 定向测试分别证明三种模式生成正确的本地、控制和上报 endpoint，dual 能从 QUIC 失败回退 TCP 且不产生重复心跳；默认兼容、无效值及组合配置失败路径通过；相关 server 测试与编译检查通过。 | 不拆分组合部署 listener，不改 wire contract、客户端选择或 PN 生命周期。 |

## Success Criteria
- Concrete user-visible or system-visible result: 独立 PN 可按 `pn.transport` 选择只启用 TCP、只启用 QUIC，或同时启用 TCP/QUIC；实际 listener、控制连接和对客户端公布的 PN 地址与模式一致。
- Required evidence: 三种配置模式及无效值的解析测试；每种模式的本地 listener endpoint、控制 endpoint 与上报 endpoint 定向自动化测试；dual 的 QUIC 优先、TCP fallback 和单一有效控制链路；默认 dual 兼容性与组合部署拒绝路径；`bucky-vpn-server` 相关编译/测试；配置示例与实际键名一致。
- Explicit non-goals: 不支持组合 SN+PN 的独立协议拆分，不调整客户端优先级，不改变端口映射、心跳或地址观察。

## Risks
- P2P endpoint 列表同时驱动 identity、listener 和 TTP runtime；若只过滤上报而未过滤实际监听，单协议模式仍会占用另一协议端口，属于必须由运行时测试/检查覆盖的失败模式。
- 若本地 endpoint 已按模式收窄但 `control_server.endpoint` 仍固定按 QUIC 解析，TCP-only PN 仍会产生 UDP 流量；反向固定为 TCP 也会破坏 QUIC-only 模式。
- dual 模式需要在底层单 target 接口之上形成有序候选连接；实现必须避免 QUIC/TCP 并发成功后形成重复心跳、重复流量上报或不一致的重连状态。
- 组合部署共享 SN/PN P2P 环境；静默接受任一单协议配置会让运维误判实际监听面，因此本任务选择显式拒绝非 `dual` 模式。
- 工作树已有与本任务无关的 `vpn-frame` 修改和大量未跟踪文件；后续实现必须只触及批准范围并保留这些既有内容。
