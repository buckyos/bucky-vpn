---
module: bucky-vpn-server
version: v0.1
status: approved
approved_by: auto-pipeline
approved_at: 2026-06-25
approved_content_sha256: 43e41655acd6a99d5e5d82b92f48d32996c77b9f88bbb18502a5618c8b0a4c2a
---

# bucky-vpn-server Design

> 本文件按已批准 proposal 设计控制节点与代理节点配置、注册、心跳、选择和统计持久化边界。测试细节留给 post-implementation testing。

## Design Scope
### Goals
- 将服务端术语统一为控制节点和代理节点。
- 从启动配置合同中移除 `pn.server_addresses`。
- 为纯代理节点设计控制节点地址配置。
- 设计外部代理节点主动连接/注册、控制节点接受、心跳 liveness 和选择边界。
- 将外部代理节点批准状态持久化到 SQLite，并与心跳 liveness 分开建模。
- 在 HTTP 控制面导出外部代理节点列表、批准和拒绝接口。
- 保持控制节点内置代理节点默认允许，并继续由 `pn.enabled` 控制本地内置代理节点启动。
- 复用现有 SQLite-backed 存储接口持久化代理流量统计。

### Non-goals
- 不在 design 阶段修改代码、测试或 acceptance artifact。
- 不新增客户端或 Flutter Web 行为。
- 不引入账单、限额、清零或报表系统。
- 不用静态 YAML 地址列表实现外部代理节点 federation。
- 不修改外部 `p2p-frame` 内部协议，除非后续 implementation 证明现有公开接口不足并回流 design。

## Overall Approach
`bucky-vpn-server` 继续作为装配型 crate。`main.rs` 是进程 assembly root，读取配置、初始化 identity/SQLite/HTTP、创建 SN service、按配置启动控制节点内置代理节点，并把共享运行时和控制面 API 组装起来。

配置合同分两类：
- 控制节点本地开关：`sn.enabled` 和 `pn.enabled`。默认均为 `true`，保持无配置兼容。
- 纯代理节点 bootstrap：新增设计项为“控制节点地址”配置。最终字段名由 implementation 前的精确 schema 决定，但不能复用 `pn.server_addresses`。该地址只用于纯代理节点主动连接控制节点。

代理节点分两类：
- 内置代理节点：与控制节点同进程，由本地 `pn.enabled` 控制，默认允许参与选择，不需要外部接受流程。
- 外部代理节点：独立进程，必须主动连接/注册到控制节点。控制节点基于身份、注册信息和运行时策略接受或拒绝。被接受后必须保持心跳；心跳超时后不得被用于新的代理节点选择，直到重连或恢复条件满足。

外部代理节点的批准状态由 SQLite 拥有，建议表名为 `pn_proxy_node`，主键使用 `pn_server` 字符串，字段至少包含 `status`、`updated_at` 和可选 `comment`。状态枚举为 `pending`、`approved`、`rejected`。外部代理节点首次心跳时，如果不存在持久记录，控制节点创建 `pending` 记录并刷新运行时 liveness；只有 `approved + live` 的外部代理节点能参与新的选择。`rejected` 节点继续记录心跳但不得进入选择集。内置代理节点不写入该批准表，仍由本地配置 `pn.enabled` 默认允许。

HTTP 控制面新增管理员接口：
- `GET /pn_proxy_nodes`：列出外部代理节点及其批准状态和当前是否 live。
- `POST /approve_pn_proxy_node`：将指定 `pn_server` 置为 `approved`。
- `POST /reject_pn_proxy_node`：将指定 `pn_server` 置为 `rejected`。

这些接口复用现有 Bearer session 解码；当前账号体系只有登录用户与 network group 绑定，没有更细的角色模型，因此 design 将“能访问现有管理 API 的已认证用户”作为管理员边界。若后续引入多角色权限，应在账号/HTTP 控制面 packet 中单独设计。

流量统计继续沿用“runtime snapshot + SQLite 累计真相”模型。内置代理节点和后续外部代理节点的统计写入都必须走同一个 database-backed 存储接口，不新增平行 store。

## Simplicity Check
| topic | decision | reused_component | new_abstraction_reason |
| --- | --- | --- | --- |
| 配置解析 | 移除静态外部代理地址列表，保留本地开关和纯代理节点控制节点地址。 | `config` crate、`server_config.rs` | 不新增通用配置框架，只扩展现有解析模块。 |
| 外部代理节点控制 | 使用 SQLite 批准状态和心跳 liveness 共同作为选择依据。 | 现有 SN/TTP/P2P identity 能力、`ConfigPnServerSelector` | 需要一个明确的代理节点运行时状态模型，否则选择逻辑会继续依赖配置地址或纯内存状态。 |
| HTTP 审批接口 | 在现有 `Api::register_api` 中注册三条控制面接口。 | 现有 Bearer session 解码和 `Resp::from_result` 模式 | 不新增 HTTP 框架或权限系统，只暴露当前 proposal 要求的最小审批能力。 |
| 内置代理节点 | 与控制节点同进程时默认允许。 | `pn.enabled`、现有本地 PN server 装配 | 不新增注册流程，避免破坏无配置默认行为。 |
| 流量持久化 | 复用现有 SQLite-backed 统计存储接口。 | `pn_traffic_service.rs`、`sqlite_store_factory.rs` | 不新增第二套 store，避免累计统计分叉。 |

## Current Structure
- `main.rs` 当前负责配置、数据目录、identity、SN service、PN server、`VpnServer`、账户服务和 HTTP API 的启动编排。
- `server_config.rs` 当前选择 `config.yaml`/`config.toml`，读取 `sn.enabled`、`pn.enabled`、`pn.server_addresses`、`pn.control_server` 和 `pn.report_interval_secs`。
- `vpn_control_client.rs` 当前为 SN disabled + PN enabled 场景提供连接控制 server 的客户端能力。
- `pn_connection_validator.rs` 当前基于本地 `VpnServer` 做代理连接校验。
- `pn_traffic_service.rs` 当前合并运行时快照和 SQLite 累计统计。
- `sqlite_store_factory.rs` 当前拥有服务端 SQLite schema 和 store 实现。
- `api.rs` 当前集中注册 HTTP 控制面接口，已有接口通过 Authorization Bearer session 解析认证用户。

## Invariants to Preserve
- 无配置文件时，控制节点默认启动 SN service 和内置代理节点。
- `sn.enabled=false` 不得跳过 HTTP、SQLite、identity、账户或共享 `VpnServer` 初始化。
- `pn.enabled=false` 只关闭本地内置代理节点，不关闭控制面和历史统计读取。
- 代理转发 source-target 授权必须继续校验同组和 joined-node 审批状态。
- 外部代理节点选择必须同时满足 SQLite 批准状态和当前 heartbeat liveness。
- 控制节点重启后可以保留批准/拒绝决定，但不能仅凭持久批准状态认为代理节点 live。
- HTTP 审批接口必须复用现有认证边界，不提供匿名批准路径。
- 用户统计只能聚合当前登录用户所属 `network_id/group_id` 的数据。
- `tx/rx bytes` 是 SQLite-backed 跨重启累计值；`tx/rx speed` 是当前进程运行时值。
- 旧 `config.toml` 仅作为兼容入口，不作为新增配置推荐格式。

## Submodules
| submodule | type | responsibility | depends_on |
| --- | --- | --- | --- |
| `process-assembly` | assembly | 启动编排，组装配置、identity、SN service、代理节点、统计服务和 HTTP API。 | `server-config`, `control-node-control`, `relay-authorization`, `traffic-statistics`, `sqlite-persistence`, `http-api` |
| `server-config` | technical | 解析 YAML/环境变量配置，移除静态外部代理地址合同，提供本地开关和纯代理节点控制节点地址。 | none |
| `control-node-control` | business | 管理内置代理节点默认允许、外部代理节点批准状态、心跳 liveness 和选择状态。 | `server-config`, `sqlite-persistence` |
| `relay-authorization` | business | 基于 joined-node/group 真相校验代理转发 source-target 对。 | `sqlite-persistence` |
| `traffic-statistics` | business | 合并 runtime snapshot 与 SQLite 累计统计，并复用数据库存储接口。 | `sqlite-persistence` |
| `http-api` | business | 注册账户和 VPN 控制面 API，暴露节点/用户统计视图以及外部代理节点审批接口。 | `traffic-statistics`, `relay-authorization`, `control-node-control`, `sqlite-persistence` |
| `sqlite-persistence` | technical | 拥有 SQLite schema、store factory、节点/用户累计统计、joined-node/group 真相和外部代理节点批准状态。 | none |

## Boundary Rationale
控制节点配置、外部代理节点批准状态、外部代理节点运行时 liveness、代理转发授权和流量统计是不同责任。配置只描述启动输入；SQLite 批准状态负责管理员策略；运行时 liveness 负责当前在线性；relay authorization 负责单次 source-target 是否允许；traffic statistics 负责统计视图和持久化。把这些责任拆开可以避免把“节点可用性”“连接权限”和“流量账本”混成一个状态源。

## Boundary Decision Matrix
| boundary | classification | business_responsibility | shared_logic_or_technical_area | decision |
| --- | --- | --- | --- | --- |
| 配置解析与运行时选择 | technical/business split | 控制节点控制决定代理节点可用性。 | `server-config` 只解析配置输入。 | 移除 `pn.server_addresses` 后，配置不再直接驱动外部代理选择。 |
| 内置代理节点与外部代理节点 | business split | 控制节点控制区分同进程可信代理和外部注册代理。 | 共享底层 PN/TTP 能力。 | 内置代理默认允许；外部代理必须主动连接、被 SQLite 批准并保持心跳。 |
| 批准状态与心跳状态 | business/technical split | 批准是管理员策略，心跳是运行时 liveness。 | SQLite store 持久化批准；selector 维护 live map。 | 持久批准不等于在线；选择必须要求 approved + live。 |
| HTTP API 与控制状态 | business split | HTTP API 只暴露管理动作，控制状态 owner 仍是 `control-node-control`/`sqlite-persistence`。 | `api.rs` 复用认证和响应模式。 | API 不直接维护第二套状态，避免审批真相分叉。 |
| 代理授权与代理可用性 | business split | `relay-authorization` 校验 source-target；`control-node-control` 管理代理节点 liveness。 | SQLite joined-node/group 真相。 | 两者独立，避免可用代理绕过 source-target 授权。 |
| 统计写入与统计展示 | business/technical split | `traffic-statistics` 定义合并语义。 | `sqlite-persistence` 拥有累计存储。 | 所有代理流量统计通过同一 SQLite-backed 接口写入。 |

## Dependency Graph
| source | depends_on | reason | cycle_check |
| --- | --- | --- | --- |
| `process-assembly` | `server-config`, `control-node-control`, `relay-authorization`, `traffic-statistics`, `sqlite-persistence`, `http-api` | 进程启动组合这些子模块。 | no cycle: assembly root only |
| `control-node-control` | `server-config`, `sqlite-persistence` | 需要配置和可选持久状态判断代理节点可用性。 | no cycle |
| `relay-authorization` | `sqlite-persistence` | 需要 joined-node/group 真相。 | no cycle |
| `traffic-statistics` | `sqlite-persistence` | 需要累计统计存储接口。 | no cycle |
| `http-api` | `traffic-statistics`, `relay-authorization`, `control-node-control`, `sqlite-persistence` | API 读取统计、执行资源权限校验，并调用控制状态接口完成代理节点审批。 | no cycle |
| `server-config` | none | 配置解析不依赖业务子模块。 | no cycle |
| `sqlite-persistence` | none | 持久化是技术边界，不依赖业务子模块。 | no cycle |

## Key Call Flows
| flow | caller | callee_submodule_path | purpose | failure_handling |
| --- | --- | --- | --- | --- |
| 控制节点启动 | `process-assembly` | `server-config` -> `control-node-control` -> `traffic-statistics` | 读取配置并启动控制节点、内置代理节点和统计服务。 | 配置解析失败沿用启动失败；`pn.enabled=false` 使用零值统计 provider 保持 API 可用。 |
| 纯代理节点连接 | 外部代理节点进程 | `control-node-control` | 使用控制节点地址主动连接/注册。 | 连接失败进入重试/未注册状态；未接受节点不得进入选择集。 |
| 心跳维护 | 外部代理节点进程 | `control-node-control` -> `sqlite-persistence` | 维持 liveness；首次出现时创建 pending 批准记录。 | 写入 pending 失败时该代理不进入选择；心跳超时将代理节点标记不可用于新选择；重连成功后仍需 approved 状态。 |
| 代理节点审批 | 已认证管理员 HTTP 请求 | `http-api` -> `control-node-control` -> `sqlite-persistence` | 查询、批准或拒绝外部代理节点。 | 未认证请求拒绝；不存在的 `pn_server` 可创建目标状态以支持预批准；重复批准/拒绝幂等。 |
| 代理转发授权 | 代理连接入口 | `relay-authorization` -> `sqlite-persistence` | 校验 source-target 是否同组且已审批。 | 授权失败在 open 阶段拒绝，不进入 bridge。 |
| 统计持久化 | `traffic-statistics` | `sqlite-persistence` | 写入节点/用户累计流量并服务 API 查询。 | 写库失败不得更新进程内 flush 基线；下次 flush 重试，避免丢量。 |

## Large Module Submodule Decision
| submodule | source_proposal | decision | design_packet | reason |
| --- | --- | --- | --- | --- |
| server-config-and-pn-control | PROP-pn-config-no-static-addresses | 保持在 `bucky-vpn-server` module-level design。 | `docs/versions/v0.1/modules/bucky-vpn-server/design.md` | 当前变更横跨启动装配、配置、控制状态和统计接口，但仍是服务端装配职责，不新增独立 packet。 |

## Trigger Matrix
| trigger_category | applies | evidence | design_coverage | required_checks | deferred_checks_and_reason |
| --- | --- | --- | --- | --- | --- |
| contract/protocol | yes | 外部代理节点由主动连接/注册替代静态地址。 | `Overall Approach` 和 `Key Call Flows` 定义注册、心跳和选择边界。 | Implementation 后需要覆盖注册成功、拒绝、心跳超时。 | owner: testing; risk: wire-level 覆盖等实现接口确定后生成。 |
| data/schema | yes | 外部代理节点批准状态必须 SQLite 持久化，统计复用数据库接口。 | `Overall Approach`、`Data and State` 和 `Interfaces and Dependencies` 定义 `pn_proxy_node` 状态 owner、迁移和选择组合规则。 | Implementation 后检查新增 schema、迁移和状态读写。 | owner: testing/acceptance; acceptance impact: schema 细节需以代码为准审计。 |
| security/privacy/permission | yes | 控制节点决定外部代理节点是否可用，HTTP 批准接口会改变可用代理集合。 | `Boundary Decision Matrix` 和 `Key Call Flows` 分离可用性、授权和审批权限。 | 测试未认证 HTTP 请求、未批准/拒绝代理不选择、source-target 未授权。 | owner: testing; risk: 安全回归必须在实现后验证。 |
| runtime/integration | yes | 选择依赖内置代理、外部代理和 heartbeat liveness。 | `Overall Approach`、`Data and State`、`Implementation Order` 覆盖运行时状态。 | 运行时测试覆盖启动、注册、超时、恢复。 | owner: testing; acceptance impact: 需要真实或替身 P2P runtime。 |
| build/dependency/config/deployment | yes | YAML 配置移除 `pn.server_addresses`，增加纯代理节点控制节点地址。 | `Interfaces and Dependencies` 定义配置接口兼容性。 | 配置解析单元测试和启动配置 DV。 | owner: implementation/testing; risk: 字段名和迁移行为必须与实现一致。 |
| ui/datamodel/workflow | yes | HTTP 控制面需要外部代理节点列表、批准和拒绝工作流；Flutter Web UI 不在当前范围内。 | `Overall Approach` 和 `Interfaces and Dependencies` 定义 HTTP API 数据模型和工作流。 | HTTP API DV 或 handler 级测试覆盖列表、批准、拒绝和未认证。 | owner: testing; risk: API 可用但 Web UI 不展示，需要后续 UI packet 承接。 |
| harness/process | yes | proposal 已批准，design 需映射 change_id。 | `Directly Mapped Change Items` 覆盖全部 proposal items。 | 运行 design doc structure 和 stage scope checks。 | owner: design stage; acceptance impact: 未批准 design 不能进入 implementation admission。 |

## Directly Mapped Change Items
| change_id | proposal_id | design_coverage | scope_paths |
| --- | --- | --- | --- |
| CHG-pn-config-no-static-addresses | PROP-pn-config-no-static-addresses | `server-config` 移除静态外部代理地址合同，`process-assembly` 不再注入额外 endpoint。 | `vpn-server/src/server_config.rs`, `vpn-server/src/main.rs` |
| CHG-external-pn-active-control | PROP-external-pn-active-control | `control-node-control` 设计外部代理节点主动连接/注册和接受状态。 | `vpn-server/src/main.rs`, `vpn-server/src/vpn_control_client.rs`, `vpn-server/src/pn_connection_validator.rs` |
| CHG-external-pn-approval-persistence | PROP-external-pn-approval-persistence | `sqlite-persistence` 持久化外部代理节点批准状态，`control-node-control` 只选择 approved + live 的外部代理节点。 | `vpn-server/src/sqlite_store_factory.rs`, `vpn-server/src/server_config.rs`, `vpn-server/src/main.rs` |
| CHG-external-pn-approval-http-api | PROP-external-pn-approval-http-api | `http-api` 导出外部代理节点列表、批准和拒绝接口，并复用现有认证边界。 | `vpn-server/src/api.rs`, `vpn-server/src/server_config.rs`, `vpn-server/src/main.rs`, `vpn-server/src/sqlite_store_factory.rs` |
| CHG-pure-pn-sn-address | PROP-pure-pn-sn-address | `server-config` 提供纯代理节点控制节点地址配置，区别于 removed static proxy address list。 | `vpn-server/src/server_config.rs`, `vpn-server/src/vpn_control_client.rs`, `vpn-server/src/main.rs` |
| CHG-pn-sn-heartbeat | PROP-pn-sn-heartbeat | `control-node-control` 定义 heartbeat liveness、timeout 和恢复对选择的影响。 | `vpn-server/src/vpn_control_client.rs`, `vpn-server/src/main.rs` |
| CHG-colocated-pn-default-allowed | PROP-colocated-pn-default-allowed | `process-assembly` 对内置代理节点默认允许，仍由 `pn.enabled` 控制。 | `vpn-server/src/main.rs`, `vpn-server/src/server_config.rs` |
| CHG-pn-traffic-db-interface | PROP-pn-traffic-db-interface | `traffic-statistics` 通过 `sqlite-persistence` 统一写入累计统计。 | `vpn-server/src/pn_traffic_service.rs`, `vpn-server/src/sqlite_store_factory.rs` |
| CHG-local-pn-toggle-preserved | PROP-local-pn-toggle-preserved | `server-config` 和 `process-assembly` 保持 `pn.enabled` 的本地内置代理节点开关语义。 | `vpn-server/src/server_config.rs`, `vpn-server/src/main.rs` |

## Implementation Order
| step | goal | prerequisite | output | can_parallel |
| --- | --- | --- | --- | --- |
| 1 | 更新配置合同，移除 `pn.server_addresses` 并增加纯代理节点控制节点地址解析。 | approved design | `server_config.rs` 配置解析合同 | no |
| 2 | 调整 `main.rs` 装配：内置代理节点默认允许，不再从静态地址注入 endpoint。 | step 1 | 控制节点启动路径对齐 | no |
| 3 | 增加 SQLite 外部代理节点批准状态 schema 和 store 方法。 | step 1 | `pending`/`approved`/`rejected` 持久状态 | no |
| 4 | 实现 selector 对 approved + live 的组合选择，首次心跳创建 pending。 | step 3 | 外部代理节点可用性模型 | no |
| 5 | 增加 HTTP 代理节点列表、批准和拒绝接口。 | step 3 | 管理员审批控制面 | no |
| 6 | 确认代理流量统计统一走 SQLite-backed 接口。 | step 2 | 统计持久化边界对齐 | yes |
| 7 | 编译和最小范围验证。 | steps 1-6 | 实现证据 | no |

## Key Decisions
| decision | chosen | alternatives_considered | rejection_reason |
| --- | --- | --- | --- |
| 外部代理节点发现方式 | 外部代理节点主动连接/注册到控制节点。 | 静态 `pn.server_addresses` 地址列表。 | 静态列表无法表达接受状态和 liveness，会形成双真相源。 |
| 外部代理节点批准持久化 | SQLite 持久化批准状态，selector 保留运行时 liveness。 | 只存在内存；把批准和心跳写成同一状态。 | 内存会在重启后丢失管理员批准；单一状态会混淆策略和在线性。 |
| 审批接口位置 | 复用现有 HTTP 控制面。 | 增加 CLI 或配置文件审批。 | 用户明确要求 HTTP 接口；配置文件审批会重新引入静态控制输入。 |
| HTTP 权限边界 | 复用现有 Bearer session 认证用户作为管理员。 | 本次新增角色权限模型。 | 账号模型没有现成角色字段，新增角色系统超出 proposal 范围。 |
| 纯代理节点 bootstrap 地址 | 使用单独控制节点地址配置。 | 复用 `pn.server_addresses`。 | 复用旧字段会让“代理地址列表”和“控制节点地址”语义混淆。 |
| 内置代理节点授权 | 控制节点同进程内置代理节点默认允许。 | 也要求内置代理节点走外部注册流程。 | 会破坏无配置默认行为，并给受信本地装配增加不必要复杂度。 |
| 心跳语义 | 心跳 liveness 控制外部代理节点是否可被新选择。 | 只在注册时验证一次。 | 一次性验证无法发现代理节点死亡或网络分区。 |
| 统计存储 | 复用现有 SQLite-backed 统计接口。 | 为代理节点心跳/统计新增平行 store。 | 平行 store 会造成 API 视图和持久化累计值分叉。 |

## Data and State
| data_or_state | owner_submodule | access_for_others | state_transitions |
| --- | --- | --- | --- |
| 本地服务开关 `sn.enabled`/`pn.enabled` | `server-config` | `process-assembly` 只读配置结果。 | missing -> default true；invalid -> startup error；explicit false -> 对应本地服务不启动。 |
| 纯代理节点控制节点地址 | `server-config` | `control-node-control` 读取解析后的地址用于连接。 | missing on pure proxy mode -> startup/config error；valid -> connect；invalid -> startup/config error。 |
| 外部代理节点批准状态 | `sqlite-persistence` | `control-node-control` 和 `http-api` 通过 store/selector 接口读取和写入。 | absent -> pending on heartbeat；pending -> approved/rejected by HTTP；approved/rejected -> pending only by explicit future reset not in current scope；write failure -> no selection change。 |
| 心跳 liveness | `control-node-control` | 选择逻辑读取 live/unavailable 状态，并与批准状态组合。 | no heartbeat -> unavailable；heartbeat -> live until ttl；ttl expired -> unavailable；heartbeat restored -> live, but selectable only if approved。 |
| joined-node/group 授权真相 | `sqlite-persistence` | `relay-authorization` 和 `http-api` 通过 store 接口读取。 | absent -> denied；pending -> denied；allow_join true -> eligible；delete/revoke -> denied。 |
| 代理流量累计统计 | `sqlite-persistence` | `traffic-statistics` 通过统一接口读写。 | no row -> zero；flush success -> accumulated；write failure -> retry without baseline advance。 |

## Testability
- `server-config` 可以通过纯解析单元测试验证缺省值、removed field、纯代理节点控制节点地址和错误地址。
- `control-node-control` 需要可替换 TTL/selector store seam，以测试 pending 创建、approved + live 选择、rejected 不选择、heartbeat timeout 和恢复。
- `http-api` 可以通过请求 handler 或 DV 覆盖未认证、列表、批准、拒绝和幂等更新。
- `relay-authorization` 可通过 SQLite fixture 验证同组/跨组/未审批 source-target。
- `traffic-statistics` 可通过 snapshot provider fake 和 SQLite test store 验证累计写入、写库失败不推进基线。
- `process-assembly` 通过较高层 DV 验证 `sn.enabled`/`pn.enabled` 组合和内置代理节点默认允许。

## Interfaces and Dependencies
| interface | consumer | compatibility | notes |
| --- | --- | --- | --- |
| YAML `sn.enabled` | CHG-local-pn-toggle-preserved | backward-compatible | 默认 `true`，继续控制本地 SN service。 |
| YAML `pn.enabled` | CHG-local-pn-toggle-preserved, CHG-colocated-pn-default-allowed | backward-compatible | 默认 `true`，继续控制内置代理节点。 |
| removed YAML `pn.server_addresses` | CHG-pn-config-no-static-addresses | migration-required | 旧字段不再是支持合同；implementation 必须选择 reject 或 warning ignore，并记录迁移。 |
| pure proxy control-node address config | CHG-pure-pn-sn-address | new | 字段名和格式由 implementation 前最终 schema 固化，不能复用 `pn.server_addresses`。 |
| external proxy registration/heartbeat control | CHG-external-pn-active-control, CHG-pn-sn-heartbeat | new | 消费者是外部代理节点和控制节点选择逻辑。 |
| SQLite `pn_proxy_node` approval state | CHG-external-pn-approval-persistence | new | 消费者是 `control-node-control` 和 `http-api`；迁移通过 `CREATE TABLE IF NOT EXISTS`。 |
| HTTP `GET /pn_proxy_nodes` | CHG-external-pn-approval-http-api | new | 消费者是管理员/API clients；返回 `pn_server`、`status`、`live`、`updated_at`、`comment`。 |
| HTTP `POST /approve_pn_proxy_node` / `POST /reject_pn_proxy_node` | CHG-external-pn-approval-http-api | new | 消费者是管理员/API clients；请求体包含 `pn_server` 和可选 `comment`。 |
| SQLite-backed traffic stat storage | CHG-pn-traffic-db-interface | backward-compatible | 复用现有累计统计接口，不新增平行 store。 |

## Document Index
| document | topic | scope |
| --- | --- | --- |
| `proposal.md` | 已批准需求和范围 | 控制节点/代理节点配置、注册、心跳、默认允许和统计持久化 |
| `design.md` | 本设计 | 服务端模块设计和 implementation admission 映射 |
| `testing.md` | 后续测试设计 | post-implementation testing 阶段更新 |

## Risks and Rollback
- 如果旧配置字段处理不清晰，部署方可能误以为静态外部代理地址仍生效；回滚策略是恢复上一版配置解析或明确 fail-fast。
- 如果 heartbeat timeout 过短，会误判健康代理节点；过长会保留死亡代理节点。Implementation 需要可配置或保守默认。
- 如果批准状态持久化缺少迁移保护，旧数据库启动会失败；回滚策略是删除新增表访问并保留已有 network/traffic schema。
- 如果 HTTP 审批接口未复用认证，恶意用户可能批准代理节点；回滚优先关闭接口或要求现有 session。
- 如果 selector 在数据库错误时继续选择远端代理节点，会绕过批准要求；数据库错误时必须 fail closed，不选择外部代理节点。
- 如果内置代理节点被错误放入外部接受流程，无配置部署会回归。回滚优先恢复 `pn.enabled=true` 时本地默认允许。
- 如果统计写入绕过 `sqlite-persistence`，累计值会分叉。回滚应移除平行写入路径并统一回 SQLite-backed 接口。
- 如果外部代理节点接受状态持久化策略不当，重启后可能保留过期可用性。Design 保留 runtime-first 默认，持久化必须另行证明必要性。

## Approval Record
| approver | approval_date | user_statement |
| --- | --- | --- |
| | | |
