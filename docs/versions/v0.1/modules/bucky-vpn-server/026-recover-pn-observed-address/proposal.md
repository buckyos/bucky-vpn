---
task_manifest: task.yaml
status: approved
---

# Recover PN Observed Address After Re-online Proposal

Risk profile: ./risk-profile.yaml

## Workflow Tier Judgment
- Proposed tier: high-risk
- Final tier: high-risk
- Tier rationale / triggered boundaries: 该修复会改变 SN 对远端 PN 的在线状态、控制连接生命周期和客户端可选 PN 集合，直接影响生产控制面故障恢复；还必须正确处理心跳超时、同一控制连接恢复、连接断开、多次上下线以及不可连接 endpoint，属于已确认的运行时集成和生命周期风险，因此建议 `high-risk`。
- Proposal and tier confirmation: confirmed by the user on 2026-08-17 with “确认，自动完成”

## Background and Goal
独立 PN 使用 `report_local_address: false` 时，cmd158 只报告监听协议和映射端口，实际公网 IP 由 SN 在接受 PN 控制连接时从 `remote_ep` 观察得到。当前 SN 在 PN 心跳超过 TTL 后会删除整个运行态，其中也包括仍然有效的控制连接观察地址。如果同一条控制连接随后恢复发送 cmd158，接受连接的回调不会再次执行；SN 只能得到不含公网 IP 的 PN 报告，于是会打印 PN 再次在线，却无法生成可连接 endpoint，`pn_proxy_nodes` 和下发给 VPN 客户端的 PN 列表都可能为空。重启 PN 会建立新控制连接并重新触发地址观察，所以地址暂时恢复。

目标是让 PN 在一次心跳离线后通过现有或新控制连接恢复时，SN 仍能使用该控制会话实际观察到的公网地址，重新构造可连接的 PN endpoint，并将 PN 恢复到客户端可选择状态，无需重启 PN 或 SN。

## Scope
### In scope
- 将“控制连接观察到的远端地址”与“cmd158 心跳是否在线”分开管理，心跳超时不得删除仍有控制连接支撑的观察地址。
- 在 PN 从心跳离线恢复为在线时，使用当前控制会话的观察地址与 cmd158 报告的协议、端口映射重新合并 endpoint。
- 只有合并结果包含可连接 endpoint 时，远端 PN 才能进入客户端可选择集合；日志应能区分离线、重新上线和地址变化。
- 在该 PN 的最后一条控制命令连接真正断开时清理会话观察地址；新连接接受时重新观察其当前 `remote_ep`。
- 覆盖 `report_local_address: false`、心跳 TTL 超时、同一控制连接恢复 cmd158、控制连接断开/重连以及地址变化的定向回归测试。

### Out of scope
- 不修改 cmd158、SN call 或 PN 信息的线协议和编码结构。
- 不要求 PN 配置固定 `advertised_ip`，也不把重启 PN/SN 作为恢复手段。
- 不改变 PN 审批、流量统计、端口映射配置语义或 VPN 客户端的 PN 选择协议。
- 不承诺在控制连接已经断开且尚未重新建立时继续信任旧公网地址或把 PN 标记为可用。

### Boundary with neighboring modules
实现限定在 `bucky-vpn-server`：`pn_control_server` 已可在连接接受时取得 `remote_ep`，并可通过命令服务的 peer connect/disconnect 事件维护控制会话观察生命周期；`pn_server_manager` 负责把该观察值与 cmd158 报告合并并决定心跳在线和客户端可选状态。`vpn-frame` 的 `PnServerSelector` 合约、cmd158 handler 和协议结构保持不变。

## Requirement Review
“重新上线时也应该观察外网地址”的要求合理，但这里的观察源必须是实际 PN 控制连接，不能从 cmd158 中猜测公网 IP。对于同一条仍存活的控制连接，连接接受时记录的 `remote_ep` 就是该会话的观察地址；心跳短暂中断不应销毁它。若控制连接真正断开，旧观察值不再具有会话依据，应清理并等待新连接重新观察。

该方向比无限保留过期 PN 状态更安全，也比仅在日志中打印 online 更符合可用性语义。主要权衡是需要维护控制连接与 PN 运行态之间的生命周期关联，并处理同一 PN 多连接时“最后一条连接断开”才清理的边界。

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-recover-pn-observed-address | 将控制连接观察地址与心跳 TTL 分离；PN 心跳恢复时基于仍存活控制会话的观察 IP 和报告的映射端口重建可连接 endpoint；最后一条控制连接断开时清理观察值。 | 仅修改 `bucky-vpn-server` 的 PN 控制连接和远端 PN 状态管理，不改变共享协议。 | 增加会话生命周期状态以及多连接断开边界处理，但避免无限信任无连接支撑的旧地址。 | 定向测试证明：未报告本地地址的 PN 在超时后由同一控制连接恢复 cmd158 时重新可选且 endpoint 正确；连接断开后不再使用旧观察值；新连接/新地址会重新观察并生效。 | 不修改 PN 审批、客户端协议、固定地址配置或流量统计。 |

## Success Criteria
- Concrete user-visible or system-visible result: PN 出现一次心跳下线再上线后，`pn_proxy_nodes` 重新包含由控制连接公网 IP 与配置端口 `3625` 合成的地址，VPN 客户端后续能再次收到并连接该 PN，无需重启进程。
- Required evidence: 覆盖同连接恢复、断连接清理、新连接地址变化、不可连接 endpoint 不下发的定向测试；受影响 crate 的格式、测试和编译检查；对 offline/online/address-changed 日志顺序及客户端选择条件的实现审查。
- Explicit non-goals: 不消除控制连接本身断开期间的短暂不可用，不改变 SN 返回 PN 列表的增量/版本规则，不修改任何 wire contract。

## Risks
- 若心跳超时处理仍误删观察值，会复现当前“cmd158 持续但 PN 无地址”的故障。
- 若控制连接断开后仍保留观察值，公网地址变化或 NAT 重绑定时可能下发过期 endpoint。
- 同一 PN 可能存在多条控制连接；不能在任意一条连接结束时提前清理仍由其他连接支撑的观察状态。
- online 日志和审批状态不能替代可连接性判断；恢复路径必须以合并后存在非 unspecified endpoint 为客户端可选前提。
- 当前工作树包含与本任务无关的既有修改和未跟踪文件，后续实现与证据必须保留并排除这些内容。
