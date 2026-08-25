---
task_manifest: task.yaml
status: approved
---

# Refresh PN Observation On Re-online Proposal

Risk profile: ./risk-profile.yaml

## Workflow Tier Judgment
- Proposed tier: high-risk
- Final tier: high-risk
- Tier rationale / triggered boundaries: 该修复改变 cmd158 的处理上下文、SN 对 PN 上线恢复的地址观察时机，以及跨 `vpn-frame` 与 `bucky-vpn-server` 的运行时接口。它直接影响生产控制面生命周期和客户端可选 PN endpoint，属于已确认的跨模块运行时集成风险，因此建议 `high-risk`。
- Proposal and tier confirmation: confirmed by the user on 2026-08-17 with “确认，自动完成”

## Background and Goal
独立 PN 配置 `report_local_address: false` 时，cmd158 不携带公网 IP；SN 必须从承载 PN 控制命令的实际连接中观察公网地址。PN 心跳超时后可能仍沿用同一条控制连接继续发送 cmd158，也可能重连后发送首次 cmd158。连接接受回调只在建连时执行，单靠它无法表达“本次重新上线由哪条连接承载”。

目标是在 SN 判定 PN 由心跳离线恢复为在线时，根据承载该次 cmd158 的精确 `peer_id + tunnel_id` 重新读取连接 `remote_ep`，更新观察地址，再与 PN 报告的协议和映射端口合并。控制连接断开本身不立即废弃观察地址，PN 在线/离线仍由原有心跳 TTL 逻辑决定。

## Scope
### In scope
- cmd158 handler 保留并使用当前命令的 `tunnel_id`，通过注入的观察能力解析该 tunnel 的实际远端 endpoint。
- 每次有效 cmd158 均可刷新其控制连接观察地址；至少保证心跳离线到在线转换时使用最新观察值完成 endpoint 合并。
- 移除任务 026 新增的 peer disconnect 立即清理观察地址和 observation generation 逻辑，恢复为仅由心跳 TTL 判断 PN 离线。
- 保留任务 026 中“心跳超时不丢失可用于恢复的观察信息”的必要状态分离，并使重上线时的刷新覆盖旧观察值。
- 覆盖同一连接恢复、断线重连后恢复、remote IP 变化、无法解析当前 tunnel，以及多 tunnel 精确匹配的定向测试。

### Out of scope
- 不修改 cmd158 的 wire 编码、字段或命令号。
- 不用 peer connect/disconnect 事件直接决定 PN 在线状态，也不在连接断开时立即清空 PN。
- 不改变 PN 的 5 秒上报周期、心跳 TTL、审批、流量统计、端口映射配置或客户端 PN 版本/增量协议。
- 不从 cmd158 猜测公网 IP，也不把固定 `advertised_ip` 作为恢复前提。

### Boundary with neighboring modules
`vpn-frame` 负责把 cmd158 的精确 tunnel 上下文交给可注入的 endpoint 观察能力，并保持协议兼容；`bucky-vpn-server` 负责从具体 PN 控制命令服务中按 `peer_id + tunnel_id` 读取 `SnTunnelWrite::remote()`，构造观察到的 PN 地址并更新 `PnServerManager`。底层 `sfo-cmd-server` 和 `p2p-frame` 依赖不修改。

## Requirement Review
该要求合理，且比“连接一断就立即废弃观察地址”更符合现有模型：连接事件和 PN 心跳在线状态不是同一层语义，在线性应继续由 cmd158 TTL 决定。但重新上线时不能只复用历史 IP；应利用 handler 已有的精确 `tunnel_id` 重新读取承载连接的远端地址。

建议将地址观察作为注入能力，而不是扩展通用 `sfo-cmd-server::CmdServer` 或修改底层依赖。这样共享层只传递必要上下文，具体服务保留对 `SnTunnelRead/Write` 的认识。若 tunnel 已在 handler 执行期间消失或无法取得 endpoint，本次心跳不得用空地址覆盖已有观察值；应记录可诊断日志并按既有心跳/可用 endpoint 规则处理。

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-observe-pn-heartbeat-tunnel | 让 cmd158 使用当前 `peer_id + tunnel_id` 调用可注入的控制连接 endpoint 观察器，并把观察结果与心跳一起交给 PN selector。 | 只增加进程内接口，不改变 cmd158 wire contract 或底层命令服务。 | 增加一个异步观察接口和失败分支，但能够精确关联多 tunnel。 | 单元测试证明 handler 传递正确 tunnel，观察失败不伪造地址且响应语义稳定。 | 不修改其他 PN 控制命令或通用命令服务器协议。 |
| P-002 | CHG-refresh-pn-observation-on-reonline | 从匹配 tunnel 读取实际 `remote_ep`，在 cmd158 恢复路径刷新观察地址；删除断线立即废弃机制，继续由心跳 TTL 决定在线。 | 修改 PN 控制服务接入和远端 PN 状态合并，不改变审批及客户端协议。 | 心跳离线期间可短暂保留历史观察值，但 PN 不可选；下一次有效 cmd158 会用实际连接地址校正。 | 测试证明同连接恢复、重连换 IP、多 tunnel 精确匹配均得到正确 endpoint，连接断开不直接改变在线状态。 | 不把连接存活等同于 PN 在线，也不无限延长心跳在线时间。 |

## Success Criteria
- Concrete user-visible or system-visible result: PN 出现一次心跳上下线后，下一次 cmd158 会重新观察其承载连接公网 IP，`pn_proxy_nodes` 和后续客户端 PN 信息恢复为包含正确 `47.113.93.155:3625`（或实际新地址）的可连接 endpoint，无需重启 PN/SN。
- Required evidence: handler tunnel 关联测试；同连接离线恢复、断线重连换 IP、多 tunnel、观察失败的状态测试；受影响 crate 的格式、定向测试和编译检查；确认不再注册断线立即清理 listener。
- Explicit non-goals: 不改变心跳 TTL、wire contract、SN 增量返回规则、审批或流量统计。

## Risks
- 若观察器按 peer 取“任意/最新连接”而非精确 tunnel，多连接时可能把错误公网 IP 合并到 PN endpoint。
- 若 handler 获取 writer 元数据时与响应写锁互相等待，可能阻塞 cmd158；设计需保证只短暂读取且不跨后续 await 持锁。
- 若观察失败时清空旧值，短暂的连接移除竞态会再次造成 PN 上线但无 endpoint；失败分支必须保留最后有效观察并输出诊断。
- 任务 026 已有未提交修改；实现必须只撤销其中被本次需求明确替代的断线立即废弃部分，保留与心跳恢复有关的状态分离，并保护工作树中其他既有修改。
