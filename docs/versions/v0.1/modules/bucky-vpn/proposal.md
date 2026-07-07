---
module: bucky-vpn
version: v0.1
status: approved
approved_by: user-request
approved_at: 2026-07-06T23:48:34+08:00
approved_content_sha256: 96b1401c6fde7f84ece1ff389540e45898097601461262492c9915fbeb32f324
---

# bucky-vpn Proposal

## Background and Goal
`bucky-vpn` 是客户端二进制，负责把本地配置、P2P stack、VPN client manager 和本地控制面装配成可运行进程。

当前 pntunnel 创建路径已经能从 VPN 信息中得到 `PnServerInfo`，但客户端装配层还缺少一个明确的代理路由决策点。目标是在 vpn client 中引入可替换的 `PnProxyRouteResolver` trait，使创建 pntunnel 时能够根据 network、target node 和候选 PN server 选择正确的代理服务器或直连路径。

进程级 PN proxy 集成测试还需要在同一台机器上同时拉起多个 `vpn-client daemon`。当前客户端本地 API 固定监听 `127.0.0.1:4536`，`join` CLI 也固定访问该地址，导致第二个客户端 daemon 无法并发运行。本次补充要求将客户端本地 API 地址配置化，同时保持默认地址 `127.0.0.1:4536` 不变。

新增要求：客户端所有对外 `NodeId` 字符串操作统一使用 base36，包括 join/state 输出、本地 API 参数、P2P id 到 VPN `NodeId` 字符串的转换，以及与服务端 HTTP API 交互时发送的 node id。内部 bytes 和非 NodeId 的 base58 用法不在范围内。

新增要求：vpn client 连接 SN 时不能只注册单一 QUIC endpoint，还需要为同一 SN 地址设置 QUIC 和 TCP 两种 endpoint，并把 QUIC endpoint 放在 TCP endpoint 前面；客户端本地 P2P 环境也必须同时启用 QUIC 和 TCP transport listener，具体使用哪个 endpoint 建立连接由 p2p-frame 处理。

新增要求：`join` 命令和本地 `/join` API 允许传入 `server_name`，用于设置连接 SN 时传给 `P2pSn::new` 的 name。未显式设置时，如果 `server` 是域名则默认使用该域名作为 `server_name`；如果 `server` 是 IP 地址则默认使用证书 id / `server_id`。

新增要求：当控制节点返回的代理节点信息中包含代理节点上报的名字时，客户端连接该代理节点必须使用该名字。该名字来自服务端返回的 `PnServerInfo.name` 或 design 选定的等价字段；它只影响代理节点连接 name，不替代代理节点 id、IP/port endpoint 或客户端选择策略。

新增要求：控制节点下发给客户端的代理节点地址应使用 `Endpoint` 类型或 design 选定的等价共享 Endpoint 合同。客户端连接 PN 代理节点时应消费该 Endpoint，不应再从拆散的 `ip`/`port` 字段重建传输地址。

## Scope
### In scope
- `vpn-client` 客户端装配层的 pntunnel 代理选择需求。
- 新增 `PnProxyRouteResolver` trait 作为客户端侧代理路由决策边界。
- 创建 pntunnel 前使用 resolver 选择正确的 PN 代理服务器。
- 保持现有客户端启动、join、本地 API 和 settings 布局不被重做。
- 客户端 daemon 本地 API 监听地址可通过配置设置，默认 `127.0.0.1:4536`。
- 客户端 `join` / `state` CLI 使用同一套本地 API 地址解析，允许进程集成测试为每个客户端指定不同端口。
- 客户端输出和请求中的 NodeId 字符串使用 base36。
- 客户端连接 SN 时支持 QUIC 和 TCP endpoint，并保持 QUIC 优先。
- 客户端 `join` 支持可选 `server_name`，用于 SN 连接 name，且按域名/IP 输入自动选择兼容默认值。
- 客户端连接 PN 代理节点时使用服务端返回的代理节点上报名字。
- 客户端连接 PN 代理节点时使用服务端返回的 Endpoint 地址。

### Out of scope
- 重做服务端 PN server 选择策略。
- 改变网络成员、VPN 协议包或持久化 schema。
- 新增 UI 行为。
- 改变非 NodeId 字符串编码。
- 改变本地 API 路由、请求/响应 schema 或鉴权语义。
- 绕过 `vpn-frame` 当前 tunnel manager / factory 边界直接散落代理选择逻辑。
- 改变 SN 服务端发布格式、服务端监听策略或服务端协议实现。
- 把现有 `join --name` 的网络成员名语义改成 SN 连接 name。
- 客户端自行决定或覆盖代理节点上报名字；名字来源由控制节点返回的代理节点信息决定。

### Boundary with neighboring modules
- `bucky-vpn` 负责客户端装配和 P2P tunnel 创建策略注入。
- `vpn-frame` 负责共享 tunnel manager、VPN 协议类型和跨客户端/服务端复用逻辑；若实现需要调整 `vpn-frame` 的 trait 合同或 tunnel manager 参数传递，必须在 `vpn-frame` packet 中补充独立 proposal/design/admission。
- `bucky-vpn-server` 负责返回包含代理节点上报名字的 PN server 信息；客户端只消费该字段。

## Assumptions and Ambiguities
- 假设 resolver 的输入至少包含 `NetworkGroupId`、`NetworkId`、目标 `NodeId`，以及来自路由表或 VPN 信息的候选 `PnServerInfo`。
- 假设默认 resolver 必须保持现有行为：已有 PN server 时继续使用该候选，没有 PN server 时直连。
- 假设客户端本地 API 地址使用现有 `setting.toml` 和 `VPN_*` 环境配置机制，配置键为 `api.ip` / `api.port`，环境变量为 `VPN_API_IP` / `VPN_API_PORT`。
- 仍需在 design 阶段确认 resolver trait 位于 `vpn-client` 还是共享到 `vpn-frame`。若共享 manager 必须感知 PN server，设计阶段需要拆分出 `vpn-frame` 的配套变更。
- “正确的代理服务器”目前定义为由服务端返回的网络 PN server 或 resolver 根据客户端策略选出的 PN server；暂不包含延迟探测、负载均衡或健康评分。
- NodeId canonical string 改为 base36；如果服务端 design 允许旧 base58 输入，客户端仍应优先发送 base36。
- 假设现有 SN 配置 key 仍然只提供 `sn_id` 与 `host:port`，TCP endpoint 复用该地址端口；客户端不新增显式协议/端口输入格式。
- 假设 p2p-frame 负责从 `P2pSn` endpoint 列表中选择可用地址并建立连接；客户端负责提供 QUIC 在前、TCP 在后的远端 SN endpoint 列表，并在本地 P2P 环境启用 QUIC/TCP 两种 transport。
- 假设 `server_name` 只影响连接 SN 时的 remote name，不参与 identity 目录命名，也不改变服务端证书 id / `server_id` 的解析。
- 假设空字符串或全空白 `server_name` 等价于未设置；域名默认值来自用户输入的 `server` 字段，IP 默认值来自 `server_id`。
- 假设服务端返回的代理节点名字字段是可选的；未提供、为空或全空白时，客户端按 design 定义 fallback，不能因此破坏旧服务端或旧代理节点。
- 假设代理节点名字用于连接代理节点的 remote/proxy name，不改变 resolver 选择哪一个 `PnServerInfo`，也不改变 Endpoint 地址选择。
- 假设代理节点地址由 `vpn-frame` shared protocol 以 Endpoint 形状提供；客户端不负责从 split ip/port 重组协议端点。

## Constraints
- 允许使用的库/组件：现有 `vpn-client`、`vpn-frame`、`p2p_frame`、异步 trait 模式和现有 VPN 类型。
- 禁止采用的方案：把 PN 代理选择硬编码到 packet dispatch、绕过现有 tunnel factory、复制 `vpn-frame` 路由表逻辑到多个文件。
- 系统约束：保持异步创建流程不阻塞；缺省行为必须向后兼容；错误处理继续使用 `VpnResult<T>` 和 `VpnErrorCode`。
- 客户端新展示/请求的 NodeId 字符串必须使用 base36，与 `vpn-frame` 合同一致。
- SN 多传输支持不得改变现有 key 解析、identity 存储目录和默认 QUIC 连接行为；TCP 只作为补充 endpoint 提供给 p2p-frame。
- `server_name` 支持必须保持已有 `join --name` 的网络成员名含义不变，并保持旧 `joined_networks` 记录可读。
- 连接代理节点时，客户端不得把代理节点名字当作 `PnServerInfo.id`、Endpoint 或持久化 key；名字只作为连接 name 输入。
- 客户端连接代理节点时必须使用控制节点返回的 Endpoint 地址形状，保留协议、地址和端口整体语义。

## Requirement Challenge
| question | evaluation | risk_or_tradeoff | decision |
| --- | --- | --- | --- |
| 是否需要一个独立 `PnProxyRouteResolver` trait，而不是直接在 `create_tunnel` 中读取 PN server？ | 独立 trait 能把“如何选代理”从“如何创建 P2P stream”中分离，便于未来按网络、节点或配置扩展策略。 | 新 trait 会增加装配复杂度，且如果放错模块会迫使 `vpn-frame` 依赖客户端策略。 | 接受 trait 方案，但 design 必须保持 trait 边界最小，并优先放在客户端装配层。 |
| 是否要把健康检查、负载均衡和多 PN 优选一起实现？ | 当前请求只要求创建 pntunnel 时选正确代理服务器，现有协议也只暴露单个候选 PN server。 | 一次性引入复杂策略会扩大协议和测试面。 | 本 change 只保留可替换 resolver 边界和默认候选选择，不实现评分或多候选调度。 |
| 是否允许修改 `vpn-frame`？ | 当前创建 worker 的代码在共享 tunnel manager 内部，如果 PN server 参数没有传递到 factory，实现可能需要共享合同配套改动。 | 跨模块改动需要额外 admission，不能由 `bucky-vpn` proposal 单独授权。 | 本 proposal 只授权客户端需求；若 design 证明必须改 `vpn-frame`，先补 `vpn-frame` packet。 |
| 是否要改成本地 API 随机端口以服务测试？ | 随机端口会改变用户和脚本依赖的默认行为，也会让 CLI 难以找到 daemon。 | 自动随机化降低兼容性；只支持配置化更可控。 | 采用显式配置，默认仍为 `127.0.0.1:4536`。 |
| 客户端是否应继续向服务端发送 base58 node id？ | 不应。用户要求所有 NodeId 操作改为 base36，且服务端/API 合同也要迁移。 | 如果服务端未同步，base36 请求会被旧服务端拒绝。 | 客户端 change 依赖 `vpn-frame`/`bucky-vpn-server` base36 合同同步后实施。 |
| SN 连接是否应同时支持 QUIC 和 TCP？ | 合理。当前客户端只构造 QUIC SN endpoint 且本地 P2P 环境只启用 QUIC listener，一旦运行环境屏蔽 UDP/QUIC，p2p-frame 没有 TCP transport 可用于 SN 连接。 | 客户端不能重复实现 p2p-frame 的连接选择逻辑；过度接管 fallback 会扩大运行时行为面。 | 接受需求，但客户端只设置 QUIC 在前、TCP 在后的 SN endpoint 列表，并启用本地 QUIC/TCP transport；实际连接选择由 p2p-frame 处理。 |
| `server_name` 是否应该复用现有 `join --name`？ | 不应复用。`--name` 已表示加入网络时服务端看到的节点名，而 `server_name` 是连接 SN 时用于 p2p-frame remote name / SNI 的技术参数。 | 复用会让网络成员显示名和连接证书名混淆，且无法在同一次 join 中同时表达两个值。 | 新增独立 `--server_name` 和 API 字段；未设置时按域名/IP 规则生成默认 SN name。 |
| 客户端是否应使用服务端返回的代理节点名字连接 PN 代理？ | 应该。用户要求控制节点返回代理节点上报名字后，客户端连接时使用该名字。 | 需要确认具体底层调用的 remote name 参数；如果仍用 id，命名证书场景会失败或不匹配。 | 客户端在连接代理节点时优先使用 `PnServerInfo.name`，缺失时按 design fallback。 |
| 代理节点名字是否应参与代理选择或持久化 key？ | 不应。代理选择仍应由服务端返回的 `PnServerInfo` 和 resolver 策略决定；名字可能重复或变更。 | 把名字当 key 会导致名字变化破坏连接缓存或选错代理。 | 名字只作为连接 name 传给底层 P2P 连接，不替代 id/ip/port。 |
| 客户端是否应继续从 `ip`/`port` 重建 PN Endpoint？ | 不应。控制节点下发 Endpoint 时已经携带协议、地址和端口，客户端再重组会丢失或误判协议语义。 | 需要依赖 `vpn-frame` Endpoint-shaped PN server 合同，并同步连接代码和测试。 | 客户端消费服务端返回的 Endpoint 地址，不再把 split ip/port 当作 PN 地址合同。 |

## Large Module Submodule Decision
| submodule | new_or_existing | responsibility | proposal_packet | reason |
| --- | --- | --- | --- | --- |
| `p2p_vpn.rs` | existing | 客户端 P2P tunnel factory、listener 和 manager glue | `docs/versions/v0.1/modules/bucky-vpn/proposal.md` | 该需求是现有 p2p tunnel 装配的策略注入，不是独立业务子模块。 |
| `sn-client-transport` | existing | 客户端创建 P2P stack 时的 SN endpoint 和本地 P2P transport 装配策略 | `docs/versions/v0.1/modules/bucky-vpn/proposal.md` | 该需求只改变现有 `p2p_vpn.rs` SN endpoint 列表和 `main.rs` P2P local endpoints，不需要独立业务子模块。 |
| `sn-server-name` | existing | 客户端 join 参数到 `P2pSn::new` name 的解析和持久化 | `docs/versions/v0.1/modules/bucky-vpn/proposal.md` | 该需求附着在现有 join/API/P2P stack 装配路径上，不需要独立业务子模块 packet。 |
| `pn-proxy-name` | existing | 客户端从 `PnServerInfo` 读取代理节点上报名字，并在连接代理节点时使用该名字。 | `docs/versions/v0.1/modules/bucky-vpn/proposal.md` | 该需求附着在现有代理连接和 pntunnel 创建路径上，不需要独立业务子模块 packet。 |
| `pn-proxy-endpoint` | existing | 客户端从 `PnServerInfo` 读取代理节点 Endpoint 地址，并按 Endpoint 协议连接代理节点。 | `docs/versions/v0.1/modules/bucky-vpn/proposal.md` | 该需求附着在现有代理连接路径上，不需要独立业务子模块 packet。 |

## Trigger Matrix
| trigger_category | applies | evidence | required_checks | deferred_checks_and_reason |
| --- | --- | --- | --- | --- |
| contract/protocol | yes | 可能需要让 tunnel 创建路径携带候选 PN server 或 resolver 结果，影响 trait 合同。 | design 阶段必须列明 trait 输入/输出和兼容性；implementation 前 admission 必须绑定 scope paths。 | 若涉及 `vpn-frame` 合同，owner: design follow-up，risk: 跨模块调用方迁移。 |
| contract/protocol | yes | SN 连接 endpoint 列表和本地 P2P transport endpoints 都将包含 QUIC 和 TCP 两种 p2p-frame endpoint，客户端只声明列表顺序和可用 transport。 | design 阶段必须列明 endpoint 构造顺序、本地 listener 构造，以及连接选择由 `P2pSn` / p2p-frame 处理。 | 若 p2p-frame 行为不符合预期，owner: p2p-frame follow-up；本 change 不在客户端重写连接选择。 |
| data/schema | yes | 本地 `/join` 请求和 `joined_networks` 需要新增可选 `server_name` 字段；VPN 协议和服务端持久化不变。 | design 必须列明 API 字段、旧持久化记录兼容和默认规则。 |  |
| security/privacy/permission | yes | PN proxy 选择会影响流量经由哪个代理服务器转发。 | design 必须说明默认行为、失败回退和日志中不得泄漏额外敏感信息。 |  |
| runtime/integration | yes | pntunnel 创建是运行时集成路径，错误选择会导致连接失败或绕过代理。 | implementation 后应至少运行客户端相关 build/check 或记录不可运行原因。 |  |
| runtime/integration | yes | SN 连接路径是客户端上线前置条件，远端 SN endpoint 列表或本地 transport listener 缺 TCP 都会导致 TCP SN 地址不可用。 | implementation 后应运行 `cargo check -p bucky-vpn` 或 harness 中 bucky-vpn 相关入口，并在 testing 阶段补 endpoint/listener 列表验证。 |  |
| runtime/integration | yes | `server_name` 会参与 p2p-frame SN command tunnel 建连，错误默认值会导致域名证书场景连接失败。 | implementation 后应覆盖显式 `server_name`、域名默认和 IP 默认，并运行 bucky-vpn unit/DV。 |  |
| runtime/integration | yes | 代理节点上报名字会参与客户端连接 PN proxy；错误 fallback 或继续使用 id 会导致命名证书场景连接失败。 | implementation 后应覆盖有 `PnServerInfo.name`、无 name fallback、空白 name fallback 和不改变 proxy selection key。 | owner: bucky-vpn/vpn-frame/bucky-vpn-server; risk: shared protocol 或服务端返回字段未同步时无法完整验证。 |
| runtime/integration | yes | 代理节点地址改为 Endpoint-shaped 下发；客户端错误重建 ip/port 或忽略协议会导致连接错误。 | implementation 后应覆盖 QUIC/TCP Endpoint 消费、Endpoint 顺序和无 split ip/port fallback。 | owner: bucky-vpn/vpn-frame/bucky-vpn-server; risk: shared protocol 未同步时无法完整验证。 |
| build/dependency/config/deployment | yes | 客户端本地 API 地址需要通过配置或环境变量设置。 | design 必须列明配置键、默认值和兼容性；implementation 后运行客户端构建或集成脚本。 |  |
| ui/datamodel/workflow | yes | `join` / `state` CLI 需要使用配置化本地 API 地址连接 daemon，并以 base36 展示/传递 NodeId；`join` 还需要可选 `--server_name`。 | design 必须保证默认 CLI 工作流不变，只有显式配置时切换地址；NodeId 输出与服务端 API 合同一致；`server_name` 不改变 `--name` 语义。 |  |
| harness/process | yes | 现有 approved docs 无 `change_id`，本需求必须先补 proposal/design 才能 implementation。 | `doc-structure-check.py --docs proposal` 和 proposal stage scope check。 |  |

## High-Level Outcomes
- vpn client 有明确的 `PnProxyRouteResolver` 策略边界。
- 创建 pntunnel 时不再只能隐式依赖固定 PN server，而是由 resolver 决定使用哪个代理服务器。
- 默认策略保持现有候选 PN server 行为，避免破坏既有网络连接。
- 客户端本地 API 地址可配置，允许同机多客户端进程集成测试，同时不破坏默认 CLI 使用方式。
- 客户端 NodeId 字符串展示和请求使用 base36。
- 客户端连接 SN 时可同时使用 QUIC 和 TCP，并保持 QUIC 优先。
- 客户端 join 可为 SN 连接指定 `server_name`，未指定时域名 server 使用域名、IP server 使用 `server_id`。
- 客户端连接代理节点时使用控制节点返回的代理节点上报名字。
- 客户端连接代理节点时使用控制节点返回的 Endpoint 地址，而不是从拆散 ip/port 字段重组。

## Proposal Items
| proposal_id | change_id | outcome | success_evidence |
| --- | --- | --- | --- |
| PROP-client-pn-proxy-route-resolver | CHG-client-pn-proxy-route-resolver | 客户端装配层支持通过 `PnProxyRouteResolver` 在创建 pntunnel 前选择 PN 代理服务器。 | Design 中出现同名 `change_id`、明确 trait 合同和 scope paths；implementation admission 通过后代码路径使用 resolver 结果创建 pntunnel。 |
| PROP-client-configurable-local-api-address | CHG-client-configurable-local-api-address | 客户端 daemon 和 CLI 支持配置本地 API 地址，默认仍为 `127.0.0.1:4536`。 | Design 中出现同名 `change_id`、明确配置键和 scope paths；implementation admission 通过后进程集成脚本可以为多个 client 指定不同 API 端口。 |
| PROP-client-node-id-base36 | CHG-client-node-id-base36 | 客户端对外 NodeId 字符串、CLI 显示和本地/API 请求使用 base36。 | Design 映射 `vpn-client/src/p2p_vpn.rs` 和相关 CLI/API 调用；implementation 不再用 base58 表示 NodeId。 |
| PROP-client-sn-quic-tcp-priority | CHG-client-sn-quic-tcp-priority | 客户端连接 SN 时同时注册 QUIC 和 TCP endpoint，并把 QUIC endpoint 放在 TCP endpoint 前面；本地 P2P 环境同时启用 QUIC/TCP listener。 | Design 中出现同名 `change_id`、明确 endpoint/listener 构造顺序；implementation admission 通过后 `vpn-client/src/p2p_vpn.rs` 不再只为 SN 构造单一 QUIC endpoint，`vpn-client/src/main.rs` 不再只启用单一 QUIC P2P endpoint。 |
| PROP-client-join-server-name-for-sn | CHG-client-join-server-name-for-sn | `join` 命令和本地 `/join` API 支持可选 `server_name`，并在创建 SN client stack 时传给 `P2pSn::new`；未设置时域名 server 默认用域名，IP server 默认用 `server_id`。 | Design 中出现同名 `change_id`、明确 CLI/API/持久化字段、默认规则和 scope paths；implementation admission 通过后 `vpn-client/src/p2p_vpn.rs` 第 544 行不再无条件使用 `sn_id.to_string()` 作为 SN name。 |
| PROP-client-pn-proxy-reported-name | CHG-client-pn-proxy-reported-name | 客户端连接 PN 代理节点时优先使用服务端返回的代理节点上报名字；未提供名字时按 approved design fallback，且名字不替代 PN server id。 | Design 中出现同名 `change_id`、明确 `PnServerInfo.name` 消费位置、底层连接参数、fallback 和 scope paths；implementation admission 通过后代理连接不再只使用 id/name fallback。 |
| PROP-client-pn-proxy-endpoint-address | CHG-client-pn-proxy-endpoint-address | 客户端连接 PN 代理节点时使用服务端返回的 Endpoint 地址，保留协议、地址和端口整体语义。 | Design 中出现同名 `change_id`、明确 `PnServerInfo` Endpoint 字段消费位置、QUIC/TCP 协议处理和 scope paths；implementation admission 通过后代理连接不再从 split ip/port 重建地址。 |

## Success Criteria
- `PnProxyRouteResolver` 的职责、输入、输出和默认行为在 design 中明确。
- 创建 pntunnel 的路径能够拿到 resolver 选择结果，并在连接 PN proxy 或直连时使用该结果。
- 没有 PN server 或 resolver 返回不使用代理时，客户端行为保持向后兼容。
- 如果实现需要修改 `vpn-frame`，对应模块拥有自己的 approved proposal/design 和 admission evidence。
- 未配置本地 API 地址时，daemon 监听和 CLI 访问仍使用 `127.0.0.1:4536`。
- 显式配置本地 API 地址时，daemon 监听地址与 CLI 访问目标一致，支持同机多客户端进程测试。
- 客户端发送到服务端和展示给用户的 NodeId 字符串是 base36。
- SN 连接 endpoint 同时包含 QUIC 和 TCP，且同一 SN 地址下 QUIC endpoint 排在 TCP endpoint 前面；客户端本地 P2P env 同时启用 QUIC/TCP listener。
- 客户端不改变默认 join key 或 identity 存储目录，也不重复实现 p2p-frame 的连接选择逻辑。
- 显式传入 `server_name` 时，SN client stack 使用该值作为 `P2pSn::new` 的 name；未传时，域名 server 使用域名，IP server 使用 `server_id`；旧 `joined_networks` 记录可继续复连。
- 当 `PnServerInfo.name` 非空时，客户端连接代理节点使用该名字作为 remote/proxy connection name。
- 当 `PnServerInfo.name` 缺失或空白时，客户端按 design fallback，并保持旧服务端兼容。
- 代理节点名字不改变 resolver 选择结果、PN server id、endpoint 派生或持久化 key。
- 当 `PnServerInfo` 返回 Endpoint 地址时，客户端按 Endpoint 协议、地址和端口连接 PN 代理节点。
- 客户端不再依赖 split `ip`/`port` 字段重建 PN 代理 Endpoint。

## Risks
- resolver 边界若放到共享层，可能把客户端策略泄漏到 `vpn-frame`。
- pntunnel 创建参数如果改动不完整，worker pool 复用可能按旧 key 复用错误 tunnel。
- 默认失败回退若定义不清，可能在代理不可用时错误直连或反复重连。
- 客户端先于服务端切换 base36 会导致旧服务端拒绝 node id 参数。
- p2p-frame 对 endpoint 列表的选择策略属于外部 crate 行为；客户端只能保证提供 QUIC 在前、TCP 在后的列表，并启用对应本地 transport。
- 同端口 TCP endpoint 可能与实际 SN 服务端监听配置不一致，design 必须确认这是当前部署前提。
- 如果 `server_name` 没有随 `joined_networks` 持久化，daemon 重启后可能回退到不同 SN name；如果把 `server_name` 加入 identity 目录，又会破坏既有身份复用。
- 如果客户端继续用 PN server id 作为代理连接 name，配置了名字的代理节点可能无法通过证书/name 校验。
- 如果客户端把代理节点名字放进 resolver key 或 worker pool key，名字变化可能导致错误复用或重复连接。
- 如果没有无 name fallback，旧服务端返回的 `PnServerInfo` 会变成不可连接。
- 如果客户端继续从 split ip/port 重建 Endpoint，可能丢失 QUIC/TCP 协议信息，或在服务端已合成 Endpoint 后连接错误地址。
- 如果 Endpoint 地址参与 worker/cache key 的方式未定义，地址更新后可能错误复用旧 tunnel。

## Downstream Follow-Up
- Design stage: 为 `CHG-client-pn-proxy-route-resolver` 补充 `design.md`，明确 trait 位置、调用流、错误处理、兼容性和 scope paths。
- Implementation stage: 只有 proposal/design 均 approved 且 admission 通过后，才能修改 `vpn-client` 生产代码。
- Cross-module route: 如果设计要求修改 `vpn-frame/src/client/tunnel_manager.rs` 或 `vpn-frame/src/vpn_protocol.rs` 的共享合同，先在 `docs/versions/v0.1/modules/vpn-frame/` 增加对应 proposal/design 覆盖并独立 admission。
- Testing stage: implementation 后补充验证设计，覆盖默认 PN 候选、无 PN 候选、resolver 返回替代 PN 和失败路径。
- Design stage: 为 `CHG-client-configurable-local-api-address` 补充 `design.md`，明确配置键、默认值、CLI/daemon 共享解析方式和 scope paths。
- Implementation stage: 只有新增 proposal/design 均 approved 且 admission 通过后，才能修改 `vpn-client/src/main.rs` 和 `vpn-client/src/cli.rs`。
- Testing stage: implementation 后 rerun process-level PN proxy integration，验证多客户端可使用不同本地 API 端口。
- Design stage: 为 `CHG-client-node-id-base36` 补充 `design.md`，明确客户端 NodeId base36 输出、请求参数和跨模块依赖。
- Implementation stage: 只有 `vpn-frame` / `bucky-vpn-server` base36 合同和本模块 admission 均通过后，才能替换客户端 NodeId base58 调用。
- Design stage: 为 `CHG-client-sn-quic-tcp-priority` 补充 `design.md`，明确 SN endpoint 列表构造顺序、本地 P2P listener 构造、p2p-frame 连接选择边界、配置兼容性和 scope paths。
- Implementation stage: 只有 proposal/design 重新 approved 且 admission 通过后，才能修改 `vpn-client/src/p2p_vpn.rs` 中 SN endpoint 装配和 `vpn-client/src/main.rs` 中 P2P local endpoints。
- Testing stage: implementation 后补充验证，覆盖 SN endpoint 列表包含 QUIC/TCP、QUIC 排在 TCP 前面、本地 P2P endpoints 包含 QUIC/TCP、无行为破坏的默认 join key/identity 路径。
- Design stage: 为 `CHG-client-join-server-name-for-sn` 补充 `design.md`，明确 `--server_name`、`Join.server_name`、`JoinRecord.server_name`、默认规则和 `P2pSn::new` name 传递。
- Implementation stage: 只有 proposal/design 重新 approved 且 admission 通过后，才能修改 `vpn-client/src/main.rs`、`vpn-client/src/api.rs`、`vpn-client/src/cli.rs` 和 `vpn-client/src/p2p_vpn.rs`。
- Testing stage: implementation 后补充验证，覆盖显式 `server_name`、域名默认、IP 默认和旧记录兼容。
- Design stage: 为 `CHG-client-pn-proxy-reported-name` 补充 `design.md`，明确 `PnServerInfo.name` 字段消费、连接 PN proxy 时的 remote name 参数、无名字 fallback、worker/cache key 是否受影响和 scope paths。
- Implementation stage: 只有 `vpn-frame` / `bucky-vpn-server` 代理名字合同和本模块 admission 均通过后，才能修改客户端代理连接路径。
- Testing stage: implementation 后补充验证，覆盖有上报名字、无名字、空白名字、名字变化不影响 resolver id/endpoint 和旧服务端兼容。
- Design stage: 为 `CHG-client-pn-proxy-endpoint-address` 补充 `design.md`，明确 `PnServerInfo` Endpoint 字段消费、连接 PN proxy 时的 Endpoint 协议处理、地址更新对 worker/cache key 的影响和 scope paths。
- Implementation stage: 只有 `vpn-frame` / `bucky-vpn-server` Endpoint 地址合同和本模块 admission 均通过后，才能修改客户端代理连接路径。
- Testing stage: implementation 后补充验证，覆盖 QUIC Endpoint、TCP Endpoint、Endpoint 顺序、地址更新和不再依赖 split ip/port。

## Approval Record
- approver: user-request
- approval_date: 2026-07-06T23:48:34+08:00
- user_statement: "确认，自动处理后续步骤"
