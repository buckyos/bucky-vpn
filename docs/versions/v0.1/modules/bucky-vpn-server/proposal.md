---
module: bucky-vpn-server
version: v0.1
status: approved
approved_by: user-request
approved_at: 2026-06-25
approved_content_sha256: ce27fa7c2b7630262cdcfd0714b67e2af4c57473d019bc84b77998870b51934f
---

# bucky-vpn-server Proposal

## Background and Goal
`bucky-vpn-server` 是服务端二进制，负责把账户管理、SQLite 持久化、P2P 基础设施、共享 VPN server runtime、代理转发行为、流量统计和 HTTP 控制面装配成一个可部署进程。

本文统一使用以下术语：
- 控制节点：同时承担 SNServer 和控制面职责的节点。
- 代理节点：承担 PNServer 职责的节点。
- 内置代理节点：与控制节点运行在同一受信进程内的代理节点角色。
- 外部代理节点：独立运行并主动连接控制节点的代理节点。

当前模块 packet 已经覆盖服务端基线职责和现有代理转发加固要求：代理连接必须基于服务端账号与 joined-node 真相授权，节点和用户流量统计必须通过控制面暴露，累计流量必须持久化到本地 SQLite。

本次 proposal 更新调整代理节点启动配置合同。代理节点配置不再需要 `pn.server_addresses`。外部代理节点应主动发起连接或注册流程接入控制节点。控制节点决定该外部代理节点是否被接受以及是否可用，而不是要求中心 `vpn-server` 配置列出外部代理监听地址。

对于只运行代理节点角色的节点，启动配置仍然需要知道要连接哪个控制节点。这个控制节点地址不是外部代理地址列表，而是纯代理节点用来连接控制面并接受审批或拒绝的 bootstrap endpoint。

代理节点连接到控制节点后，代理节点与控制节点之间必须保持心跳。心跳是运行时 liveness 信号，控制节点据此判断该代理节点是否仍可参与选择。

由于外部代理节点必须先被批准才能真正被使用，批准状态不能只保存在进程内存中。控制节点必须把外部代理节点的批准状态持久化到本地 SQLite，使控制节点重启后仍能区分已批准、未批准和被拒绝的代理节点；心跳仍然只表示当前 liveness，不能替代批准状态。

控制节点还需要导出 HTTP 批准接口，使管理员可以查看待批准代理节点并批准或拒绝外部代理节点。该接口属于服务端 HTTP 控制面，不引入 Flutter Web UI 要求；后续 UI 若要消费该能力，应另行进入 `vpn_web` packet。

当控制节点进程同时支持内置代理节点角色时，内置代理节点默认被允许。它不需要走外部代理节点的接受流程，因为它是在受信服务端进程内装配的。它的流量统计可以直接使用现有 database-backed 持久化接口。

最终目标是形成更小、更偏控制面的代理节点配置：
- 保留 YAML 启动配置，用于启用或关闭控制节点内置 SN service 和内置代理节点；
- 从代理节点配置合同中移除静态外部代理地址输入；
- 通过受控制节点策略控制的主动连接或注册路径表达外部代理节点可用性；
- 将外部代理节点批准状态持久化到 SQLite，并把批准状态与心跳 liveness 分开建模；
- 在 HTTP 控制面导出代理节点批准接口，供管理员批准或拒绝外部代理节点；
- 允许纯代理节点配置它需要连接的控制节点地址；
- 要求代理节点与控制节点在连接/注册后保持心跳；
- 当控制节点同时支持代理节点角色时，默认允许内置代理节点；
- 代理流量统计继续走现有数据库接口持久化路径；
- 未提供配置时保持现有默认行为。

## Scope
### In scope
- `bucky-vpn-server` 启动配置中 `sn.enabled` 和 `pn.enabled` 的合同。
- 从目标代理节点配置合同中移除 `pn.server_addresses`。
- 要求外部代理节点不再通过 `vpn-server` YAML 中的静态地址列表配置。
- 要求外部代理节点主动连接、注册或通过其他 server-controlled 路径建立自身可用性。
- 要求控制节点策略决定外部代理节点是否被接受和是否可被选择。
- 要求外部代理节点批准状态持久化到本地 SQLite。
- 要求 HTTP 控制面提供外部代理节点列表、批准和拒绝接口。
- 要求纯代理节点具备要连接的控制节点地址配置。
- 要求代理节点与控制节点保持心跳，使 liveness 影响外部代理节点可用性。
- 要求与控制节点同进程的内置代理节点默认被允许。
- 要求内置代理节点流量统计可以直接调用现有数据库存储接口。
- 保持本地默认行为：没有配置文件或省略代理节点配置时，内置代理节点行为仍默认启用，除非显式关闭。
- 保留现有基于账号组和 joined-node 审批状态的代理转发授权。
- 保留现有代理流量视图和 SQLite 累计持久化要求。
- 记录 design、implementation、testing 和 acceptance 的下游回流事项。

### Out of scope
- 客户端二进制行为和 Flutter Web 行为。
- 平台打包脚本。
- 账单、额度清零、报表或结算系统。
- 替换账号模型、JWT/session 模型或 SQLite 本地持久化真相。
- 修改外部 `p2p-frame` 协议内部实现，除非后续 design 阶段证明主动外部代理注册流程必须新增上游接口。
- 通过 YAML 地址列表增加静态代理 federation。
- 长期维护 `pn.server_addresses` 作为受支持配置字段。

### Boundary with neighboring modules
- `bucky-vpn-server` 拥有进程装配、本地持久化、控制节点策略和 HTTP 控制面集成职责。
- `vpn-frame` 拥有共享 VPN 领域类型和 server/client runtime 合同。
- `p2p-frame` 拥有 identity、SN service、PN server、TTP 和协议原语。
- 外部代理节点接受与否是控制节点决策；不能信任客户端自行决定哪个外部 relay 有效。
- 代理转发 source-target 授权仍属于服务端职责，必须继续使用本地持久化 joined-node 和 group 真相。

## Assumptions and Ambiguities
| item | assumption_or_ambiguity | decision_for_this_proposal | downstream_resolution |
| --- | --- | --- | --- |
| 外部代理节点主动连接形状 | 用户指定外部代理节点应主动连接，但尚未指定具体协议或命令形状。 | 在 proposal 中只记录需求基线，不发明具体 wire protocol。 | Design 必须选择主动注册/控制流程，并命名所需 `p2p-frame` 或 `vpn-frame` 接口。 |
| 控制节点接受语义 | “控制节点控制是否可以被使用”解释为控制节点对外部代理节点的接受与选择策略。 | Proposal 记录策略边界，不承诺具体实现细节。 | Design 必须定义已接受外部代理节点状态存放位置及其如何参与代理选择。 |
| 批准状态与 liveness 的关系 | 用户补充“需要批准才能真正使用”，说明批准是长期策略状态，心跳只是短期在线状态。 | 批准状态必须 SQLite 持久化；心跳不持久化为可用性真相。 | Design 必须定义表结构、状态枚举、重启恢复、心跳与批准状态组合后的选择规则。 |
| HTTP 批准接口形状 | 用户要求导出 HTTP 批准接口，但尚未指定 URL、方法、请求体和权限细节。 | Proposal 要求控制面能力，不锁死具体 API 路径。 | Design 必须命名 API 路径、请求/响应模型、管理员权限要求和错误语义。 |
| 纯代理节点的控制节点地址字段 | 纯代理节点需要一个或多个控制节点 bootstrap/control 地址，但最终 key 名称和基数尚未设计。 | 记录需求，不在 proposal 中命名最终 YAML 字段。 | Design 必须选择字段名、地址格式、必填/可选行为和迁移策略。 |
| 心跳间隔与超时 | 代理节点与控制节点需要心跳语义，但间隔、超时、重试和重连行为尚未指定。 | 将心跳记录为必需 liveness 合同，而不是具体计时算法。 | Design 必须定义默认间隔、超时行为、状态迁移、重连规则和选择影响。 |
| 内置代理节点授权 | 与控制节点在同一受信进程内装配的代理节点，与外部代理节点具有不同信任姿态。 | 除非 `pn.enabled=false`，内置代理节点默认允许。 | Design 必须区分内置代理节点权限和外部代理节点接受流程，并说明选择行为。 |
| 流量持久化接口复用 | 用户说明流量统计可以调用已经由数据库支持的存储接口。 | 将需求保持在接口边界层，不引入第二套统计 store。 | Design 必须命名现有存储接口以及 PN 统计如何使用它们。 |
| 现有 `pn.server_addresses` 用户兼容性 | 当前代码仍解析 `pn.server_addresses`，但请求的合同移除了该字段。 | 长期支持的配置移除该字段；兼容处理交给 design 决策。 | Design 必须决定旧字段是拒绝、带 warning 忽略，还是临时作为 deprecated 字段容忍。 |
| 内置代理 listener | 内置代理节点启用时仍使用本地 SN/TTP listener。 | 保留 `pn.enabled` 作为内置代理节点开关。 | Implementation 必须避免把内置 listener endpoint 和外部代理节点注册状态混在一起。 |

## Constraints
- 推荐启动配置格式必须使用 YAML。
- 缺少配置文件和缺少可选配置字段不得破坏现有默认启动。
- `sn.enabled` 默认值为 `true`。
- `pn.enabled` 默认值为 `true`。
- `pn.server_addresses` 不得被要求或文档化为外部代理节点机制。
- 静态外部代理节点地址不得作为代理节点是否可选择的真相源。
- 纯代理节点配置必须包含连接 bootstrap/control-plane 所需的控制节点地址。
- 纯代理节点的控制节点地址配置必须与已移除的静态外部代理地址列表区分开。
- 外部代理节点接受必须由控制节点策略控制，不能由不受信客户端选择。
- 外部代理节点批准状态必须写入 SQLite；控制节点重启后不得把所有曾心跳过的代理节点自动视为已批准。
- 只有已批准且当前 liveness 有效的外部代理节点才能参与新的代理选择。
- HTTP 批准接口必须走现有控制面认证/授权边界，不得提供匿名批准能力。
- 代理节点与控制节点必须在连接/注册后维持心跳。
- 心跳丢失后，受影响代理节点必须在满足 approved design 的恢复条件前不再参与新的选择。
- 当 `pn.enabled=true` 时，与控制节点同进程的内置代理节点默认允许。
- 内置代理节点流量统计必须复用现有 database-backed 存储接口，不得引入平行 store。
- 代理转发授权仍必须验证 source 和 target 的 joined-node 状态。
- 代理流量统计必须继续使用现有 runtime snapshot 加 SQLite 累计模型。
- Design 和 implementation 不得静默依赖聊天上下文；代码变更前必须有 proposal 和 design 的直接覆盖。

## Requirement Challenge
| question | evaluation | risk_or_tradeoff | decision |
| --- | --- | --- | --- |
| `pn.server_addresses` 是否应继续作为静态外部 relay 列表留在配置中？ | 静态列表看起来部署简单，但会把外部代理节点可用性耦合到中心配置，且无法表达 relay 是否实际在线或是否已被控制节点策略接受。 | 保留它会形成两套真相源：配置地址和运行时已接受外部代理节点。 | 从目标代理节点配置合同中移除 `pn.server_addresses`。 |
| 外部代理节点主动连接模型是否合理？ | 合理。它更符合动态 relay 可用性，并允许控制节点策略决定外部代理节点是否可用。 | 实现前需要具体注册/控制设计，可能需要新增状态和验证路径。 | 接受外部代理节点主动连接/注册作为需求基线。 |
| 批准状态是否应只保存在内存中？ | 不应。批准是管理员策略决定，不是瞬时在线信号；只存在内存会导致控制节点重启后丢失批准结果。 | 需要新增 SQLite schema 和迁移逻辑；持久批准若与 liveness 混淆，也可能误选离线代理。 | 批准状态持久化到 SQLite，但选择仍要求当前心跳有效。 |
| 是否应通过 HTTP 暴露批准能力？ | 应暴露。没有控制面接口时，批准状态虽然可以持久化，但管理员无法完成审批工作流。 | 新接口扩大控制面权限面，必须复用现有认证并定义拒绝/未找到/重复批准语义。 | 增加 HTTP 代理节点审批接口，具体路径和模型由 design 固化。 |
| 本任务是否应直接改代码？ | 不应。该请求收窄受支持配置行为并改变范围/需求。 | 在 approved proposal/design 更新前编辑代码会绕过 Harness admission，并削弱实现可追踪性。 | 本任务只进入 proposal，并记录 design/implementation 下游回流。 |
| 内置代理节点是否仍应可配置？ | 应保留。用户移除的是外部静态地址，不是启用或关闭本地内置代理转发的能力。 | 如果移除 `pn.enabled`，部署方会失去关闭本地 relay 行为的简单开关。 | 保留 `pn.enabled`；只移除静态外部地址配置。 |
| 纯代理节点是否仍需要配置地址？ | 需要，但这是用于连接 control/bootstrap server 的控制节点地址，不是客户端选择代理节点的静态列表。 | 没有该地址时，纯代理节点无法确定如何找到控制节点；若继续建模为 `pn.server_addresses`，旧歧义会复现。 | 增加纯代理节点控制节点地址配置的独立 design 要求。 |
| 代理节点与控制节点之间是否必须有心跳？ | 必须。主动注册只能证明初始可达性；心跳用于保持运行时 liveness 最新。 | 没有心跳时，控制节点选择可能继续把客户端路由到已经死亡或网络分区的代理节点。 | 要求代理节点与控制节点保持心跳，并让 liveness 影响代理节点选择。 |
| 控制节点进程内置的代理节点是否需要显式外部接受？ | 不需要。它已经属于受信服务端装配，并受本地 `pn.enabled` 控制。 | 把它当成外部代理节点会增加不必要的注册复杂度，并可能破坏无配置默认行为。 | 控制节点内置代理节点在启用时默认允许。 |
| 代理流量统计是否应写入新的存储路径？ | 不应。现有 database-backed 存储接口是累计流量数据的正确 ownership 边界。 | 第二套写入路径会导致 runtime 统计、API 视图和持久化真相分叉。 | 代理流量持久化复用现有数据库存储接口。 |

## Large Module Submodule Decision
| submodule | new_or_existing | responsibility | proposal_packet | reason |
| --- | --- | --- | --- | --- |
| server-config-and-pn-control | existing module-level scope | 启动配置和 PN 控制装配仍属于 `bucky-vpn-server` 进程装配职责。 | `docs/versions/v0.1/modules/bucky-vpn-server/proposal.md` | 本次变更收窄既有 config/PN assembly 职责，尚未定义独立业务 submodule packet。 |

## Trigger Matrix
| trigger_category | applies | evidence | required_checks | deferred_checks_and_reason |
| --- | --- | --- | --- | --- |
| contract/protocol | yes | 外部代理节点行为从静态配置地址改为 server-controlled 主动连接/注册。 | Implementation 前 design 必须定义外部代理节点接受合同。 | owner: design stage; risk: 具体流程未命名前无法推导协议测试。 |
| data/schema | yes | 外部代理节点批准状态必须 SQLite 持久化，且代理流量统计也应复用 database-backed 存储接口。 | Design 必须定义外部代理节点批准表或等价 schema、迁移行为、状态枚举、重启恢复语义，并命名现有统计存储接口复用方式。 | owner: design stage; acceptance impact: schema validation 延后到 design/implementation。 |
| security/privacy/permission | yes | 控制节点策略决定外部代理节点是否可用，HTTP 批准接口会改变可用代理集合。 | Design 必须定义外部代理节点接受的认证/授权方式，以及 HTTP 批准接口的管理员权限要求。 | owner: implementation/testing stages after design; risk: 授权测试需要 approved policy 和接口。 |
| runtime/integration | yes | 代理节点选择将基于内置允许代理节点、已连接/接受的外部代理节点和心跳 liveness，而不是配置地址列表。 | Design 必须覆盖启动、内置默认允许、注册、心跳、选择、断连、超时、重连和失败行为。 | owner: implementation/testing stages after design; acceptance impact: integration evidence 延后到实现存在后。 |
| ui/datamodel/workflow | yes | HTTP 控制面需要导出外部代理节点列表、批准和拒绝工作流；本次不要求 Flutter Web UI。 | Design 必须定义 HTTP API 数据模型和工作流，并明确 `vpn_web` UI 不在当前范围内。 | owner: design stage; risk: API 可用但 Web UI 不展示，需要后续 UI packet 承接。 |
| build/dependency/config/deployment | yes | YAML 配置合同移除 `pn.server_addresses` 作为外部代理节点输入，同时纯代理节点仍需配置控制节点地址。 | Proposal 和 design 必须记录配置迁移行为和纯代理节点控制节点地址字段。 | owner: design stage; risk: 部署验证依赖 approved compatibility behavior。 |
| harness/process | yes | 需求/范围变更必须先落在 proposal，再进入下游阶段。 | 运行 proposal doc structure 和 proposal stage scope 检查。 | owner: downstream stages; acceptance impact: proposal/design 更新并批准前 admission 继续延后。 |

## High-Level Outcomes
- 受支持的代理节点启动配置不再要求 operator 列出外部代理节点地址。
- 外部代理节点成为运行时参与者，主动连接/注册，并由控制节点策略接受或拒绝。
- 外部代理节点批准状态持久化到 SQLite，控制节点重启后保留管理员批准/拒绝决定。
- HTTP 控制面提供外部代理节点查询、批准和拒绝能力，并复用现有认证边界。
- 纯代理节点拥有明确控制节点地址配置，用于 bootstrap/control-plane 连接。
- 已接受代理节点与控制节点保持心跳，心跳丢失后不再参与新的选择。
- 当本地代理节点启用时，与控制节点同进程的内置代理节点默认允许。
- 内置代理节点流量统计通过现有 database-backed 存储接口持久化。
- `sn.enabled` 和 `pn.enabled` 仍是受支持的本地服务开关。
- 无配置默认行为保持兼容：除非配置关闭，否则本地 SN/PN 行为仍启用。
- 下游 design 必须用主动外部代理节点注册/控制模型替换所有 `pn.server_addresses` 设计。
- 下游 implementation 必须在 design 批准后移除或 deprecate 静态外部代理地址解析和选择。
- 现有代理转发授权和流量持久化要求保持有效。

## Proposal Items
| proposal_id | change_id | outcome | success_evidence |
| --- | --- | --- | --- |
| PROP-pn-config-no-static-addresses | CHG-pn-config-no-static-addresses | 从目标代理节点配置合同中移除 `pn.server_addresses`。 | Proposal/design/code 不再记录或要求静态外部代理节点地址列表。 |
| PROP-external-pn-active-control | CHG-external-pn-active-control | 外部代理节点主动连接/注册，并在使用前由控制节点策略接受。 | Design 命名主动流程、接受策略、状态 owner、失败行为和验证策略。 |
| PROP-external-pn-approval-persistence | CHG-external-pn-approval-persistence | 外部代理节点批准状态持久化到 SQLite，并与 heartbeat liveness 分开参与选择。 | Design 命名持久化 schema、状态枚举、迁移、重启恢复和选择组合规则。 |
| PROP-external-pn-approval-http-api | CHG-external-pn-approval-http-api | HTTP 控制面导出外部代理节点列表、批准和拒绝接口。 | Design 命名 API 路径、请求/响应模型、权限要求和错误语义。 |
| PROP-pure-pn-sn-address | CHG-pure-pn-sn-address | 纯代理节点启动配置包含连接 control/bootstrap server 所需的控制节点地址。 | Design 命名 YAML 字段、地址格式、验证行为以及纯代理节点如何使用它。 |
| PROP-pn-sn-heartbeat | CHG-pn-sn-heartbeat | 代理节点与控制节点在连接/注册后保持心跳，heartbeat liveness 控制代理节点可用性。 | Design 命名心跳间隔、超时、状态迁移、重连行为和选择影响。 |
| PROP-colocated-pn-default-allowed | CHG-colocated-pn-default-allowed | 与控制节点同进程的内置代理节点在本地代理启用时默认允许。 | Design 区分同进程内置代理节点权限和外部代理节点接受流程，并保持无配置默认行为。 |
| PROP-pn-traffic-db-interface | CHG-pn-traffic-db-interface | 内置代理节点流量统计直接使用现有 database-backed 存储接口。 | Design 命名存储接口，implementation 通过该接口写入流量统计且不新增平行 store。 |
| PROP-local-pn-toggle-preserved | CHG-local-pn-toggle-preserved | `pn.enabled` 继续控制内置本地代理节点。 | 缺少配置时仍默认启动本地代理节点，显式关闭时阻止本地代理节点启动。 |

## Success Criteria
- `proposal.md` 记录 `pn.server_addresses` 不再是受支持的外部代理节点机制。
- `proposal.md` 记录外部代理节点通过受控制节点策略控制的主动连接/注册路径接受。
- `proposal.md` 记录外部代理节点批准状态必须持久化到 SQLite。
- `proposal.md` 记录 HTTP 控制面必须导出代理节点列表、批准和拒绝接口。
- `proposal.md` 记录纯代理节点配置需要控制节点地址。
- `proposal.md` 记录代理节点与控制节点必须保持心跳，且 liveness 影响代理节点可用性。
- `proposal.md` 记录控制节点内置代理节点在本地代理启用时默认允许代理角色。
- `proposal.md` 记录代理流量统计通过现有 database-backed 存储接口持久化。
- Proposal 检查在 `version=v0.1`、`module=bucky-vpn-server` 下通过。
- 下游 follow-up 明确把 stale design、implementation、testing 和 acceptance artifacts 回流到对应阶段。
- 本 proposal-stage 任务不编辑代码、design、testing 或 acceptance artifacts。

## Risks
- 如果 design 继续保留 `pn.server_addresses`，系统会保留静态配置和运行时代理节点两套真相源。
- 如果外部代理节点主动注册缺少认证，恶意 relay 可能把自己宣告为可用。
- 如果已接受外部代理节点状态只存在内存中，重启行为可能让 operator 意外，除非明确文档化。
- 如果已接受外部代理节点状态持久化过宽，过期或断连 relay 可能仍被选择。
- 如果批准状态和心跳 liveness 混为一个字段，控制节点可能把离线但已批准的代理节点选给新连接，或把在线但未批准的代理节点错误放行。
- 如果 HTTP 批准接口缺少管理员权限校验，未授权用户可能批准恶意代理节点。
- 如果 HTTP 批准接口缺少幂等和错误语义，管理员重复操作或代理节点重连时可能得到不可预测状态。
- 如果心跳超时过于激进，短暂网络抖动可能把健康代理节点从选择中移除。
- 如果心跳超时过于宽松，死亡代理节点可能继续被选择并导致客户端连接失败。
- 如果内置代理节点被错误强制走外部接受流程，默认部署可能失败或需要不必要的注册配置。
- 如果内置代理节点绕过与其他代理流量相同的统计持久化接口，API 视图和存储累计值可能分叉。
- 如果旧 `pn.server_addresses` 配置兼容行为不明确，部署可能静默失败或继续使用不受支持语义。
- 如果纯代理节点的控制节点地址没有与被移除的代理地址列表分开建模，配置语义会继续含糊。
- 如果纯代理节点控制节点地址支持多种格式但缺少严格验证，启动可能延迟失败或注册到错误控制面。
- 如果代理节点选择更新时没有保留 source-target relay 授权，现有安全边界可能回退。
- 如果 `pn.enabled` 语义与外部代理节点可用性混淆，关闭本地代理节点可能意外关闭外部代理节点发现或上报。

## Downstream Follow-Up
| stage | required_follow_up | reason |
| --- | --- | --- |
| Design | 更新 `design.md`：移除 `pn.server_addresses`，加入外部代理节点主动连接/注册模型，定义纯代理节点控制节点地址字段、控制节点接受策略、SQLite 批准状态 schema、HTTP 批准接口、心跳行为、内置代理节点默认允许、统计存储接口复用、状态 owner、选择行为和兼容处理。 | 当前 design 仍描述静态额外代理地址解析和 endpoint 注入，且尚未单独建模纯代理节点 bootstrap 控制节点地址、持久批准状态、HTTP 审批接口、heartbeat liveness、内置代理节点 allowance 或存储接口复用。 |
| Implementation | 在 approved design 和 admission 后，更新 `vpn-server/src/server_config.rs`、`vpn-server/src/main.rs`、`vpn-server/src/api.rs`、`vpn-server/src/sqlite_store_factory.rs`、代理节点 selection/control/heartbeat code，以及代理 traffic persistence wiring 以匹配 approved design。 | 当前代码解析 `pn.server_addresses` 并注入额外 endpoint；纯代理节点控制节点地址解析、SQLite 批准状态、HTTP 审批接口、heartbeat 行为和内置代理节点 allowance 语义尚未设计。 |
| Testing | 在 post-implementation testing 阶段新增或更新测试，覆盖配置解析、deprecated/removed 字段行为、纯代理节点控制节点地址验证、内置代理节点默认允许、批准状态持久化、HTTP 批准/拒绝接口权限与错误语义、统计持久化接口使用、外部代理节点接受、heartbeat timeout/recovery、选择和失败路径。 | 该行为变更影响配置、schema、HTTP 控制面、权限、liveness、统计持久化和 runtime integration。 |
| Acceptance | 重新审计 proposal、design、implementation 和 testing 的一致性后再接受该变更。 | 当前 approved 下游 artifacts 在本 proposal 更新后已经 stale。 |

## Approval Record
- approver: user-request
- approval_date: 2026-06-25
- user_statement: 确定，自动处理后续步骤
