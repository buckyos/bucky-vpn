---
task_manifest: task.yaml
status: approved
---

# Proxy Node Traffic Report Logging Proposal

Risk profile: not-created (replace with ./risk-profile.yaml only after high-risk confirmation)

## Workflow Tier Judgment
- Proposed tier: trivial
- Final tier: trivial
- Tier rationale / triggered boundaries: 该改动仅在 `bucky-vpn-server` 的既有流量上报调用前增加结构化运行日志，不改变上报数据、协议、持久化、重试、并发、生命周期或配置；修改点局限于一个模块和一个函数，并可通过定向 Rust 检查验证，因此建议 `trivial`。
- Proposal and tier confirmation: confirmed by the user on 2026-07-21 with “确认”

## Background and Goal
代理节点当前会收集每个节点的流量增量，并分块调用远端 reporter 上报，但正常上报路径不会打印实际流量数据，运行时难以直接确认某次上报包含哪些节点及流量值。

目标是在代理节点发起每个流量上报分块时打印可检索日志，使运维人员能够从日志确认上报记录及其关键数据。

## Scope
### In scope
- 在 `PnTrafficService::drain_upload_once` 发起每个远端上报请求前记录 `info` 级日志。
- 每条上报记录打印 report ID、节点 ID、采集时间窗口、TX/RX 字节数以及 TX/RX 速率。
- 日志对应实际 RPC 尝试；若同一记录因重试再次上报，允许再次打印，以保留尝试级可观测性。

### Out of scope
- 不修改流量采集、分块、并发、重试、响应对账或持久化逻辑。
- 不修改 VPN 协议结构、服务端接收逻辑、配置项或日志初始化。
- 不新增日志采样、脱敏开关或专用日志级别配置。

### Boundary with neighboring modules
改动仅位于 `vpn-server/src/pn_traffic_service.rs` 的代理节点发送侧；`vpn-frame` 的共享协议及中心服务端接收路径保持不变。

## Requirement Review
需求合理，发送前记录日志最直接地反映代理节点实际尝试上报的数据，并能覆盖正常发送和重试。采用每条记录一行的 `info` 日志便于检索，但节点数较多或上报间隔较短时会增加日志量；本提案优先满足“打印上报流量数据”的明确可见性诉求，不额外引入配置复杂度。

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-log-proxy-node-traffic-report | 在代理节点每次发送流量上报分块前，以 `info` 日志逐条打印 report ID、节点 ID、时间窗口、TX/RX 字节数和速率。 | 仅修改 `PnTrafficService::drain_upload_once` 的发送侧可观测性。 | 每个发送尝试逐条记录，重试会重复且会增加日志量。 | 代码检查确认日志覆盖所有关键字段且位于 reporter 调用之前；运行定向格式检查和 `vpn-server` 编译/测试检查。 | 不改变协议、接收端、流量统计语义或重试行为。 |

## Success Criteria
- Concrete user-visible or system-visible result: 代理节点日志中可看到每次实际发起上报的各条流量记录及关键数值。
- Required evidence: 日志包含 report ID、节点 ID、开始/结束时间、TX/RX 字节数和 TX/RX 速率；定向 Rust 格式与编译/测试检查通过，或明确记录环境限制。
- Explicit non-goals: 不调整上报周期、批大小、并发数、失败处理、协议或存储。

## Risks
- `info` 级逐记录日志会随活跃节点数和重试次数增长，属于可接受但需明确的运行日志量权衡。
- 节点 ID 与流量计数是运行标识和遥测数据；日志不包含用户凭证、密钥或报文内容。
- 当前工作树已有大量与本任务无关的修改和未跟踪文件，执行时必须保留并排除这些既有内容。
