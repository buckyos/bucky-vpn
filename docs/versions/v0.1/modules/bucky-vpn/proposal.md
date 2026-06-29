---
module: bucky-vpn
version: v0.1
status: approved
approved_by: auto-pipeline
approved_at: 2026-06-26T15:51:05+08:00
approved_content_sha256: 90b93538e1ade3360970f08cb26d8f6f23363d63ad8f987d149392e69c8825e4
---

# bucky-vpn Proposal

## Background and Goal
`bucky-vpn` 是客户端二进制，负责把本地配置、P2P stack、VPN client manager 和本地控制面装配成可运行进程。

当前 pntunnel 创建路径已经能从 VPN 信息中得到 `PnServerInfo`，但客户端装配层还缺少一个明确的代理路由决策点。目标是在 vpn client 中引入可替换的 `PnProxyRouteResolver` trait，使创建 pntunnel 时能够根据 network、target node 和候选 PN server 选择正确的代理服务器或直连路径。

进程级 PN proxy 集成测试还需要在同一台机器上同时拉起多个 `vpn-client daemon`。当前客户端本地 API 固定监听 `127.0.0.1:4536`，`join` CLI 也固定访问该地址，导致第二个客户端 daemon 无法并发运行。本次补充要求将客户端本地 API 地址配置化，同时保持默认地址 `127.0.0.1:4536` 不变。

## Scope
### In scope
- `vpn-client` 客户端装配层的 pntunnel 代理选择需求。
- 新增 `PnProxyRouteResolver` trait 作为客户端侧代理路由决策边界。
- 创建 pntunnel 前使用 resolver 选择正确的 PN 代理服务器。
- 保持现有客户端启动、join、本地 API 和 settings 布局不被重做。
- 客户端 daemon 本地 API 监听地址可通过配置设置，默认 `127.0.0.1:4536`。
- 客户端 `join` / `state` CLI 使用同一套本地 API 地址解析，允许进程集成测试为每个客户端指定不同端口。

### Out of scope
- 重做服务端 PN server 选择策略。
- 改变网络成员、VPN 协议包或持久化 schema。
- 新增 UI 行为。
- 改变本地 API 路由、请求/响应 schema 或鉴权语义。
- 绕过 `vpn-frame` 当前 tunnel manager / factory 边界直接散落代理选择逻辑。

### Boundary with neighboring modules
- `bucky-vpn` 负责客户端装配和 P2P tunnel 创建策略注入。
- `vpn-frame` 负责共享 tunnel manager、VPN 协议类型和跨客户端/服务端复用逻辑；若实现需要调整 `vpn-frame` 的 trait 合同或 tunnel manager 参数传递，必须在 `vpn-frame` packet 中补充独立 proposal/design/admission。
- `bucky-vpn-server` 继续负责服务端侧 PN server 管理和校验，不在本 change 中修改。

## Assumptions and Ambiguities
- 假设 resolver 的输入至少包含 `NetworkGroupId`、`NetworkId`、目标 `NodeId`，以及来自路由表或 VPN 信息的候选 `PnServerInfo`。
- 假设默认 resolver 必须保持现有行为：已有 PN server 时继续使用该候选，没有 PN server 时直连。
- 假设客户端本地 API 地址使用现有 `setting.toml` 和 `VPN_*` 环境配置机制，配置键为 `api.ip` / `api.port`，环境变量为 `VPN_API_IP` / `VPN_API_PORT`。
- 仍需在 design 阶段确认 resolver trait 位于 `vpn-client` 还是共享到 `vpn-frame`。若共享 manager 必须感知 PN server，设计阶段需要拆分出 `vpn-frame` 的配套变更。
- “正确的代理服务器”目前定义为由服务端返回的网络 PN server 或 resolver 根据客户端策略选出的 PN server；暂不包含延迟探测、负载均衡或健康评分。

## Constraints
- 允许使用的库/组件：现有 `vpn-client`、`vpn-frame`、`p2p_frame`、异步 trait 模式和现有 VPN 类型。
- 禁止采用的方案：把 PN 代理选择硬编码到 packet dispatch、绕过现有 tunnel factory、复制 `vpn-frame` 路由表逻辑到多个文件。
- 系统约束：保持异步创建流程不阻塞；缺省行为必须向后兼容；错误处理继续使用 `VpnResult<T>` 和 `VpnErrorCode`。

## Requirement Challenge
| question | evaluation | risk_or_tradeoff | decision |
| --- | --- | --- | --- |
| 是否需要一个独立 `PnProxyRouteResolver` trait，而不是直接在 `create_tunnel` 中读取 PN server？ | 独立 trait 能把“如何选代理”从“如何创建 P2P stream”中分离，便于未来按网络、节点或配置扩展策略。 | 新 trait 会增加装配复杂度，且如果放错模块会迫使 `vpn-frame` 依赖客户端策略。 | 接受 trait 方案，但 design 必须保持 trait 边界最小，并优先放在客户端装配层。 |
| 是否要把健康检查、负载均衡和多 PN 优选一起实现？ | 当前请求只要求创建 pntunnel 时选正确代理服务器，现有协议也只暴露单个候选 PN server。 | 一次性引入复杂策略会扩大协议和测试面。 | 本 change 只保留可替换 resolver 边界和默认候选选择，不实现评分或多候选调度。 |
| 是否允许修改 `vpn-frame`？ | 当前创建 worker 的代码在共享 tunnel manager 内部，如果 PN server 参数没有传递到 factory，实现可能需要共享合同配套改动。 | 跨模块改动需要额外 admission，不能由 `bucky-vpn` proposal 单独授权。 | 本 proposal 只授权客户端需求；若 design 证明必须改 `vpn-frame`，先补 `vpn-frame` packet。 |
| 是否要改成本地 API 随机端口以服务测试？ | 随机端口会改变用户和脚本依赖的默认行为，也会让 CLI 难以找到 daemon。 | 自动随机化降低兼容性；只支持配置化更可控。 | 采用显式配置，默认仍为 `127.0.0.1:4536`。 |

## Large Module Submodule Decision
| submodule | new_or_existing | responsibility | proposal_packet | reason |
| --- | --- | --- | --- | --- |
| `p2p_vpn.rs` | existing | 客户端 P2P tunnel factory、listener 和 manager glue | `docs/versions/v0.1/modules/bucky-vpn/proposal.md` | 该需求是现有 p2p tunnel 装配的策略注入，不是独立业务子模块。 |

## Trigger Matrix
| trigger_category | applies | evidence | required_checks | deferred_checks_and_reason |
| --- | --- | --- | --- | --- |
| contract/protocol | yes | 可能需要让 tunnel 创建路径携带候选 PN server 或 resolver 结果，影响 trait 合同。 | design 阶段必须列明 trait 输入/输出和兼容性；implementation 前 admission 必须绑定 scope paths。 | 若涉及 `vpn-frame` 合同，owner: design follow-up，risk: 跨模块调用方迁移。 |
| data/schema | no | 不新增持久化字段，不改变 VPN 协议序列化结构。 |  |  |
| security/privacy/permission | yes | PN proxy 选择会影响流量经由哪个代理服务器转发。 | design 必须说明默认行为、失败回退和日志中不得泄漏额外敏感信息。 |  |
| runtime/integration | yes | pntunnel 创建是运行时集成路径，错误选择会导致连接失败或绕过代理。 | implementation 后应至少运行客户端相关 build/check 或记录不可运行原因。 |  |
| build/dependency/config/deployment | yes | 客户端本地 API 地址需要通过配置或环境变量设置。 | design 必须列明配置键、默认值和兼容性；implementation 后运行客户端构建或集成脚本。 |  |
| ui/datamodel/workflow | yes | `join` / `state` CLI 需要使用配置化本地 API 地址连接 daemon。 | design 必须保证默认 CLI 工作流不变，只有显式配置时切换地址。 |  |
| harness/process | yes | 现有 approved docs 无 `change_id`，本需求必须先补 proposal/design 才能 implementation。 | `doc-structure-check.py --docs proposal` 和 proposal stage scope check。 |  |

## High-Level Outcomes
- vpn client 有明确的 `PnProxyRouteResolver` 策略边界。
- 创建 pntunnel 时不再只能隐式依赖固定 PN server，而是由 resolver 决定使用哪个代理服务器。
- 默认策略保持现有候选 PN server 行为，避免破坏既有网络连接。
- 客户端本地 API 地址可配置，允许同机多客户端进程集成测试，同时不破坏默认 CLI 使用方式。

## Proposal Items
| proposal_id | change_id | outcome | success_evidence |
| --- | --- | --- | --- |
| PROP-client-pn-proxy-route-resolver | CHG-client-pn-proxy-route-resolver | 客户端装配层支持通过 `PnProxyRouteResolver` 在创建 pntunnel 前选择 PN 代理服务器。 | Design 中出现同名 `change_id`、明确 trait 合同和 scope paths；implementation admission 通过后代码路径使用 resolver 结果创建 pntunnel。 |
| PROP-client-configurable-local-api-address | CHG-client-configurable-local-api-address | 客户端 daemon 和 CLI 支持配置本地 API 地址，默认仍为 `127.0.0.1:4536`。 | Design 中出现同名 `change_id`、明确配置键和 scope paths；implementation admission 通过后进程集成脚本可以为多个 client 指定不同 API 端口。 |

## Success Criteria
- `PnProxyRouteResolver` 的职责、输入、输出和默认行为在 design 中明确。
- 创建 pntunnel 的路径能够拿到 resolver 选择结果，并在连接 PN proxy 或直连时使用该结果。
- 没有 PN server 或 resolver 返回不使用代理时，客户端行为保持向后兼容。
- 如果实现需要修改 `vpn-frame`，对应模块拥有自己的 approved proposal/design 和 admission evidence。
- 未配置本地 API 地址时，daemon 监听和 CLI 访问仍使用 `127.0.0.1:4536`。
- 显式配置本地 API 地址时，daemon 监听地址与 CLI 访问目标一致，支持同机多客户端进程测试。

## Risks
- resolver 边界若放到共享层，可能把客户端策略泄漏到 `vpn-frame`。
- pntunnel 创建参数如果改动不完整，worker pool 复用可能按旧 key 复用错误 tunnel。
- 默认失败回退若定义不清，可能在代理不可用时错误直连或反复重连。

## Downstream Follow-Up
- Design stage: 为 `CHG-client-pn-proxy-route-resolver` 补充 `design.md`，明确 trait 位置、调用流、错误处理、兼容性和 scope paths。
- Implementation stage: 只有 proposal/design 均 approved 且 admission 通过后，才能修改 `vpn-client` 生产代码。
- Cross-module route: 如果设计要求修改 `vpn-frame/src/client/tunnel_manager.rs` 或 `vpn-frame/src/vpn_protocol.rs` 的共享合同，先在 `docs/versions/v0.1/modules/vpn-frame/` 增加对应 proposal/design 覆盖并独立 admission。
- Testing stage: implementation 后补充验证设计，覆盖默认 PN 候选、无 PN 候选、resolver 返回替代 PN 和失败路径。
- Design stage: 为 `CHG-client-configurable-local-api-address` 补充 `design.md`，明确配置键、默认值、CLI/daemon 共享解析方式和 scope paths。
- Implementation stage: 只有新增 proposal/design 均 approved 且 admission 通过后，才能修改 `vpn-client/src/main.rs` 和 `vpn-client/src/cli.rs`。
- Testing stage: implementation 后 rerun process-level PN proxy integration，验证多客户端可使用不同本地 API 端口。

## Approval Record
- approver: auto-pipeline
- approval_date: 2026-06-26T15:51:05+08:00
- user_statement: "确认，自动处理后续步骤"
