---
module: bucky-vpn-server
version: v0.1
status: approved
approved_by: auto-pipeline
approved_at: 2026-07-07T00:41:31+08:00
approved_content_sha256: 2c1f270b9acb4137d4386e18a95e6b6f15c7c1fbd15ea56749a4b1c5b2edd605
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

新增要求：服务端所有 `NodeId` 对外字符串、HTTP 请求/响应字段、SQLite key 列和用于识别节点的日志输出应统一改成 base36，不再以 base58 作为新写入或新展示的标准格式。`NodeId` 内部 bytes、raw protocol 编码和非 `NodeId` 的 base58 用法不在该要求范围内。

本次 proposal 更新调整代理节点启动配置合同。代理节点配置不再需要 `pn.server_addresses`。外部代理节点应主动发起连接或注册流程接入控制节点。控制节点决定该外部代理节点是否被接受以及是否可用，而不是要求中心 `vpn-server` 配置列出外部代理监听地址。

对于只运行代理节点角色的节点，启动配置仍然需要知道要连接哪个控制节点。这个控制节点地址不是外部代理地址列表，而是纯代理节点用来连接控制面并接受审批或拒绝的 bootstrap endpoint。

控制节点地址属于 SN/control-node 配置域，而不属于本机 PN 代理角色配置域。纯代理节点应通过 `sn` 节点下的控制节点配置找到控制面；`pn` 节点只表达本机是否启用代理角色以及代理角色自身的心跳/上报配置。

HTTP 管理面监听配置、管理员账号 bootstrap 配置和 HTTP 登录会话 JWT 签名配置也属于 SN/control-node 配置域。HTTP 管理面用于控制节点账号登录、网络管理和代理节点审批；管理员账号用于登录该管理面；JWT 签名密钥用于该管理面的登录会话。三者都不属于 PN 代理角色自身能力，因此应从顶层 `http` / `admin` / `jwt` 迁移到 `sn.http` / `sn.admin` / `sn.jwt` 或 design 选定的等价 SN 子节点。

服务端配置还需要提供用于生成本机 P2P identity 证书的名字字段。该字段描述本服务端身份名称，和监听 `ip`/`port` 一样属于本机 identity/listener 启动输入，不属于 HTTP 登录 JWT。推荐配置模板应在现有监听地址附近增加 `name` 字段或 design 选定的等价字段，使 operator 能配置新生成证书的 subject/name。

如果 operator 修改了证书名字，服务端不得因此轮换私钥或改变节点身份密钥。启动时应在保留已有私钥的前提下生成带新名字的证书；只有 identity 文件缺失或私钥不可读取时，才允许走新 identity 生成路径。具体 identity 文件格式、旧证书名字检测和重签失败行为由 design 固化。

如果代理节点配置了名字，代理节点向控制节点注册、心跳或上报代理信息时也必须带上该名字。控制节点在保存外部代理节点运行时状态、HTTP 管理面列表和返回给客户端的代理节点信息时，应保留该上报名字；客户端后续连接该代理节点时应使用控制节点返回的名字。该名字是连接/display 元数据，不得替代 `pn_server` 稳定身份、批准状态主键或 source-target 授权 key。

代理节点连接到控制节点后，代理节点与控制节点之间必须保持心跳。心跳是运行时 liveness 信号，控制节点据此判断该代理节点是否仍可参与选择。

纯代理节点与控制节点之间已经有控制面通信通道时，纯代理节点不得为了接入控制节点而启动 `p2p-frame` 的 SN client 在线上报流程。纯代理节点不应向控制节点 SN service 发送 `ReportSn`，控制节点也不应因为纯代理节点启动而出现 `report sn from ... map_port: []` 这类 SN client 上报日志。控制面命令、注册、校验、心跳和流量上报应复用或补充独立控制通道，而不是把控制节点建模成纯代理节点的 SN bootstrap 节点。

由于外部代理节点必须先被批准才能真正被使用，批准状态不能只保存在进程内存中。控制节点必须把外部代理节点的批准状态持久化到本地 SQLite，使控制节点重启后仍能区分已批准、未批准和被拒绝的代理节点；心跳仍然只表示当前 liveness，不能替代批准状态。

控制节点还需要导出 HTTP 批准接口，使管理员可以查看待批准代理节点并批准或拒绝外部代理节点。该接口属于服务端 HTTP 控制面，不引入 Flutter Web UI 要求；后续 UI 若要消费该能力，应另行进入 `vpn_web` packet。

外部代理节点列表中的地址展示必须区分“代理节点本地配置/上报地址”和“代理节点实际通过连接进入控制节点时观察到的真实远端地址”。管理员界面需要看到真实连接来源地址，不能把代理节点配置的本地监听地址误当作其控制面连接来源地址。

当代理节点配置了 `port_mapping` 时，控制节点不应把代理节点本地上报 IP 或控制/SN bootstrap IP 当作最终可代理地址。代理节点仍应上报本地监听 `Endpoint` 端口，并把 `port_mapping` 作为独立元数据上报；控制节点使用代理节点连接进入控制面时观测到的远端 IP，并结合上报 `Endpoint` 的协议和对应 `port_mapping` 外网端口，合成新的可代理节点地址。该合成地址用于代理节点选择和返回给客户端的 `PnServerInfo`，但不作为代理节点地址持久化真相写入控制节点 SQLite。

控制/SN 连接 IP 和代理节点地址都不是需要持久化的配置真相。SQLite 只持久化外部代理节点审批状态、身份 key、统计等控制面真相；代理节点地址应由当前 live 控制连接观测值和代理节点上报的 `Endpoint` 列表/端口映射在运行时产生。控制节点重启后，已批准但尚未重新上线的代理节点不能仅凭旧地址参与选择或下发给客户端。

代理节点上报给控制节点的可监听地址应使用 `Endpoint` 类型或 design 选定的等价共享 Endpoint 合同，而不是拆散的 ip/port 字段。控制节点下发给客户端的代理节点地址也应保持 `Endpoint` 形状，使协议、地址族和端口作为一个传输端点整体流转。

当控制节点进程同时支持内置代理节点角色时，内置代理节点默认被允许。它不需要走外部代理节点的接受流程，因为它是在受信服务端进程内装配的。它的流量统计可以直接使用现有 database-backed 持久化接口。

最终目标是形成更小、更偏控制面的代理节点配置：
- 保留 YAML 启动配置，用于启用或关闭控制节点内置 SN service 和内置代理节点；
- 将控制节点地址、HTTP 管理面监听配置、管理员账号 bootstrap 配置和 JWT 会话签名配置归入 `sn` 配置域；
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
- 要求 HTTP 控制面的外部代理节点列表提供控制节点观察到的真实连接来源地址，供 UI 展示。
- 要求配置了 `port_mapping` 的代理节点上线到控制节点后，代理节点上报本地监听 `Endpoint` 端口和独立 `port_mapping` 元数据，控制节点用观测到的连接 IP、`Endpoint` 协议和对应映射外网端口合成可代理地址。
- 要求合成后的可代理地址进入代理节点选择和客户端下发的 `PnServerInfo`，但不作为代理节点地址持久化到控制节点 SQLite。
- 要求控制/SN 连接 IP 和代理节点地址不作为单独的 SQLite 地址真相持久化。
- 要求代理节点上报地址和控制节点下发给客户端的代理节点地址使用 `Endpoint` 形状。
- 要求纯代理节点具备要连接的控制节点地址配置。
- 要求纯代理节点的控制节点地址配置归入 `sn` 配置域，不能放在 `pn` 下。
- 要求 HTTP 管理面监听配置归入 `sn` 配置域，不再作为推荐顶层配置。
- 要求管理员账号 bootstrap 配置归入 `sn` 配置域，不能作为 PN 代理角色配置，也不再作为推荐顶层配置。
- 要求 HTTP 登录会话 JWT 签名配置归入 `sn` 配置域，不能作为 PN 代理角色配置，也不再作为推荐顶层配置。
- 要求服务端启动配置提供本机 P2P identity 证书名字字段，推荐模板在监听 `ip`/`port` 附近展示该字段或 design 选定的等价位置。
- 要求修改证书名字时保留已有私钥，只重新生成带新名字的证书。
- 要求代理节点向控制节点上报配置的名字，控制节点在代理节点信息中保留并返回该名字。
- 要求返回给客户端的代理节点信息带上控制节点收到的代理节点名字，供客户端连接代理节点时使用。
- 要求纯代理节点控制面通信不启动 `p2p-frame` SN client，不向控制节点执行 SN `ReportSn` 上报。
- 要求代理节点与控制节点保持心跳，使 liveness 影响外部代理节点可用性。
- 要求与控制节点同进程的内置代理节点默认被允许。
- 要求内置代理节点流量统计可以直接调用现有数据库存储接口。
- 保持本地默认行为：没有配置文件或省略代理节点配置时，内置代理节点行为仍默认启用，除非显式关闭。
- 保留现有基于账号组和 joined-node 审批状态的代理转发授权。
- 保留现有代理流量视图和 SQLite 累计持久化要求。
- 要求服务端 NodeId 字符串输入/输出和 SQLite NodeId key 写入使用 base36。
- 记录 design、implementation、testing 和 acceptance 的下游回流事项。

### Out of scope
- 客户端二进制行为和 Flutter Web 行为；客户端如何把返回的代理节点名字传给底层 P2P 连接由 `bucky-vpn` packet 负责。
- 平台打包脚本。
- 账单、额度清零、报表或结算系统。
- 替换账号模型、JWT/session 模型或 SQLite 本地持久化真相。
- 修改非 `NodeId` base58 编码，例如密码 hash 的 base58 表示。
- 修改外部 `p2p-frame` 协议内部实现，除非后续 design 阶段证明主动外部代理注册流程必须新增上游接口。
- 通过 YAML 地址列表增加静态代理 federation。
- 长期维护 `pn.server_addresses` 作为受支持配置字段。

### Boundary with neighboring modules
- `bucky-vpn-server` 拥有进程装配、本地持久化、控制节点策略和 HTTP 控制面集成职责。
- `vpn-frame` 拥有共享 VPN 领域类型和 server/client runtime 合同。
- `p2p-frame` 拥有 identity、SN service、PN server、TTP 和协议原语。
- `bucky-vpn` 拥有客户端连接代理节点时如何使用返回名字的实现职责。
- 外部代理节点接受与否是控制节点决策；不能信任客户端自行决定哪个外部 relay 有效。
- 代理转发 source-target 授权仍属于服务端职责，必须继续使用本地持久化 joined-node 和 group 真相。

## Assumptions and Ambiguities
| item | assumption_or_ambiguity | decision_for_this_proposal | downstream_resolution |
| --- | --- | --- | --- |
| 外部代理节点主动连接形状 | 用户指定外部代理节点应主动连接，但尚未指定具体协议或命令形状。 | 在 proposal 中只记录需求基线，不发明具体 wire protocol。 | Design 必须选择主动注册/控制流程，并命名所需 `p2p-frame` 或 `vpn-frame` 接口。 |
| 控制节点接受语义 | “控制节点控制是否可以被使用”解释为控制节点对外部代理节点的接受与选择策略。 | Proposal 记录策略边界，不承诺具体实现细节。 | Design 必须定义已接受外部代理节点状态存放位置及其如何参与代理选择。 |
| 批准状态与 liveness 的关系 | 用户补充“需要批准才能真正使用”，说明批准是长期策略状态，心跳只是短期在线状态。 | 批准状态必须 SQLite 持久化；心跳不持久化为可用性真相。 | Design 必须定义表结构、状态枚举、重启恢复、心跳与批准状态组合后的选择规则。 |
| HTTP 批准接口形状 | 用户要求导出 HTTP 批准接口，但尚未指定 URL、方法、请求体和权限细节。 | Proposal 要求控制面能力，不锁死具体 API 路径。 | Design 必须命名 API 路径、请求/响应模型、管理员权限要求和错误语义。 |
| 真实连接地址来源 | 用户要求显示代理节点通过连接进入的真实地址，而不是代理节点本地配置地址。 | Proposal 要求控制节点在运行时状态中记录并通过列表 API 暴露 observed/remote address。 | Design 必须定义从哪个连接/心跳上下文提取地址、TTL/失效行为、字段名和前端兼容策略。 |
| `port_mapping` 与 observed IP 的组合语义 | 用户要求设置了 `port_mapping` 的代理节点上线到控制面节点时，控制节点根据 `port_mapping` 端口和获取到的连接地址组成新的可代理节点地址；用户进一步澄清代理节点只上报 `port_mapping`，不替换本地监听端口。 | 控制节点应把 observed connection IP、代理节点上报的本地监听 `Endpoint` 协议和单独上报的 `port_mapping` 外网端口组合成 selectable/returned PN address；SN/control IP 和代理节点地址不作为数据库真相。 | Design 必须定义 observed IP 来源、协议端点选择、`PnServerInfo` 中 Endpoint 列表形状、`port_mapping` 上报字段、live-only 地址生命周期和重启后重新上线前不可选择语义。 |
| 代理节点地址类型 | 用户要求代理节点上报的地址和下发给客户端的地址使用 `Endpoint` 类型。 | 端点类型能同时携带协议、IP 和端口，避免 ip/port 拆散后丢失协议或错误组合。 | Design 必须同步 `vpn-frame` Endpoint 合同，并定义 server/client 序列化、raw codec 和兼容边界。 |
| NodeId base36 迁移 | 用户要求所有 NodeId 操作改成 base36，但当前 SQLite/API 多处使用 base58。 | Proposal 要求新写入、新响应、新请求解析以 base36 为 canonical；旧 base58 数据是否兼容读取由 design 明确。 | Design 必须列出数据库 key、HTTP API、日志、PN 匹配和前端/客户端调用的迁移策略。 |
| 纯代理节点的控制节点地址字段 | 纯代理节点需要一个或多个控制节点 bootstrap/control 地址。用户进一步要求“跟 SN 绑定的配置应该放到 SN 节点下面”，因此该字段不应继续位于 `pn` 下。 | 控制节点地址属于 `sn` 配置域；推荐结构由 design 固化为 `sn.control_server` 或等价 SN 子节点。 | Design 必须选择字段名、地址格式、必填/可选行为和从旧 `pn.control_server` 的迁移策略。 |
| HTTP 管理面配置归属 | 当前模板使用顶层 `http`，但 HTTP 管理 API 是控制节点管理面入口，不是 PN 代理角色能力。用户补充“http配置信息也是跟sn相关的”。 | HTTP 管理面监听配置属于 `sn`/control-node 配置域；推荐结构由 design 固化为 `sn.http` 或等价 SN 子节点。 | Design 必须选择最终 YAML 字段、环境变量兼容策略、默认监听行为和旧顶层 `http` 的迁移策略。 |
| 管理员账号配置归属 | 当前模板使用顶层 `admin`，但管理员账号用于控制节点 HTTP 管理面，不是 PN 代理角色能力。用户补充“admin也应该跟sn相关的”。 | 管理员账号 bootstrap 配置属于 `sn`/control-node 配置域；推荐结构由 design 固化为 `sn.admin` 或等价 SN 子节点。 | Design 必须选择最终 YAML 字段、环境变量兼容策略、默认账号行为和旧顶层 `admin` 的迁移策略。 |
| JWT 会话签名配置归属 | 当前模板使用顶层 `jwt`，但 JWT key 用于控制节点 HTTP 管理面的登录会话签名，不是 PN 代理角色能力。用户补充“jwt也应该是sn节点下的配置”。 | JWT 签名配置属于 `sn`/control-node 配置域；推荐结构由 design 固化为 `sn.jwt` 或等价 SN 子节点。 | Design 必须选择最终 YAML 字段、环境变量兼容策略、默认 key 行为和旧顶层 `jwt` 的迁移策略。 |
| 纯代理节点控制通道形状 | 当前实现用 SN client stack 把控制节点当作 SN bootstrap，但用户要求纯代理节点不要启动 SN client，且认为代理节点与控制节点已有通信通道。 | Proposal 收窄需求：纯代理节点控制面连接、注册、校验、心跳和流量上报不能依赖 `SNClientService` 在线/`ReportSn` 上报。 | Design 必须命名替代控制通道、连接建立方式、命令收发接口、重连/失败语义，并证明不再触发控制节点 SN report 日志。 |
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
- 纯代理节点的控制节点地址配置必须放在 `sn` 配置域下，`pn` 节点不得承载控制节点 bootstrap 地址。
- HTTP 管理面监听配置必须放在 `sn` 配置域下，推荐配置模板不得继续把 `http` 作为顶层节点。
- 管理员账号 bootstrap 配置必须放在 `sn` 配置域下，推荐配置模板不得继续把 `admin` 作为顶层节点。
- HTTP 登录会话 JWT 签名配置必须放在 `sn` 配置域下，推荐配置模板不得继续把 `jwt` 作为顶层节点。
- 推荐配置模板必须提供本机 P2P identity 证书名字字段，且该字段不得与 HTTP 登录 JWT key 或管理员账号 name 混用。
- 当证书名字变化且已有 identity 私钥可读取时，服务端必须保留旧私钥并重签生成带新名字的证书，不得隐式轮换私钥。
- 旧 identity 文件不可解析或无法提取私钥时，不得静默覆盖；design 必须选择 fail closed 或明确的新 identity 初始化路径。
- 纯代理节点不得把控制节点配置为 `P2pSn` 后启动 `SNClientService`，也不得依赖 SN `ReportSn` 完成控制面在线。
- 纯代理节点的控制命令、注册、校验、心跳和流量上报必须通过独立控制通道或已存在的非 SN-client 通道完成。
- 如果替代控制通道尚未设计清楚，implementation 不得只删除 `wait_online()` 或 `ReportSn` 调用后留下无心跳、无校验或无流量上报的半连接状态。
- 外部代理节点接受必须由控制节点策略控制，不能由不受信客户端选择。
- 外部代理节点批准状态必须写入 SQLite；控制节点重启后不得把所有曾心跳过的代理节点自动视为已批准。
- 只有已批准且当前 liveness 有效的外部代理节点才能参与新的代理选择。
- HTTP 批准接口必须走现有控制面认证/授权边界，不得提供匿名批准能力。
- 外部代理节点列表必须能表达控制节点观察到的真实连接来源地址；不得只返回代理节点本地配置地址供管理 UI 展示。
- 真实连接来源地址是运行时观察值，不得替代代理节点身份、批准状态或 selection 使用的 `pn_server` 标识。
- 配置了 `port_mapping` 的代理节点上线后，代理节点必须保留本地监听 Endpoint 端口并单独上报 `port_mapping`；控制节点必须用控制连接观测到的远端 IP、上报 Endpoint 协议和对应映射外网端口合成客户端可连接的代理节点地址。
- 合成后的代理节点地址必须进入 selector 选择结果和 `NodeNetwork.pn_server` 返回值，但不得作为代理节点地址持久化真相写入 SQLite。
- 控制/SN 连接 IP 和代理节点地址不得作为独立 SQLite 地址真相持久化；控制节点重启后必须等待代理节点重新上线并重新合成地址。
- 如果已有 schema 中存在 network/proxy-node PN address 字段，design 必须定义停止依赖、清理或兼容读取策略，不能仅因代理节点 id 相同就继续下发旧地址。
- 代理节点上报地址和控制节点下发地址必须使用 `Endpoint` 形状，不能在共享协议中只暴露拆散的 ip/port 地址。
- 服务端新写入 SQLite 的 joined-node、network-member、node、traffic-stat NodeId key 必须使用 base36。
- 服务端 HTTP API 中表示 NodeId 的字段必须以 base36 作为 canonical 输入/输出。
- 服务端可以在 approved design 指定的兼容窗口内读取旧 base58 NodeId，但不得继续把 base58 作为新写入格式。
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
| 外部代理节点列表应展示哪个地址？ | 应展示控制节点通过连接观察到的真实远端地址。`pn_server.ip:port` 可能只是代理节点本地配置/上报地址，在 NAT 或多网卡场景下会误导管理员。 | 需要在运行时状态或持久状态旁边增加 observed address，且该字段在离线或旧记录中可能缺失。 | `GET /pn_proxy_nodes` 需要暴露 observed/remote address；本地配置地址仍可作为身份/请求体数据保留，但 UI 展示地址应使用真实连接地址。 |
| `port_mapping` 是否只改端口、不改 IP？ | 对配置解析而言 `port_mapping` 只表达端口映射；但生成可代理地址时，仅替换端口还不够，NAT/LB/容器场景下本地 IP 往往不可达。代理节点本地上报不应把监听端口替换成映射端口，否则会丢失本机真实监听事实。 | 需要把 observed connection IP、上报 Endpoint 协议和单独上报的映射外网端口组合，才能形成客户端可用地址。风险是 observed IP 是运行时值，不能替代身份和审批 key。 | 代理节点上报本地监听 Endpoint + `port_mapping` 元数据；控制节点合成 selectable/returned PN 地址时使用 observed IP + mapped external port；SN/control observed IP 和代理节点地址不落库为长期真相。 |
| 代理节点地址是否应继续用 `ip`/`port` 字段下发？ | 不应。拆散字段无法完整表达协议，且当前 QUIC/TCP 多端点和端口映射需要端点整体语义。 | 改成 Endpoint 会影响 `vpn-frame` shared protocol、server API projection 和客户端连接代码，需要跨模块设计。 | 上报和下发代理节点地址使用 Endpoint 类型或 design 选定的等价共享 Endpoint 合同。 |
| NodeId 是否应继续用 base58？ | 不应继续作为 canonical。底层 P2P id 已使用 base36，继续混用 base58 会导致代理节点 id、成员 id 和前端显示/请求之间出现双格式。 | 直接硬切会影响旧 SQLite 行和旧前端/客户端请求。 | 新合同改为 base36；兼容旧 base58 的读路径或迁移由 design 决定。 |
| 本任务是否应直接改代码？ | 不应。该请求收窄受支持配置行为并改变范围/需求。 | 在 approved proposal/design 更新前编辑代码会绕过 Harness admission，并削弱实现可追踪性。 | 本任务只进入 proposal，并记录 design/implementation 下游回流。 |
| 内置代理节点是否仍应可配置？ | 应保留。用户移除的是外部静态地址，不是启用或关闭本地内置代理转发的能力。 | 如果移除 `pn.enabled`，部署方会失去关闭本地 relay 行为的简单开关。 | 保留 `pn.enabled`；只移除静态外部地址配置。 |
| 纯代理节点是否仍需要配置地址？ | 需要，但这是用于连接 control/bootstrap server 的控制节点地址，不是客户端选择代理节点的静态列表。 | 没有该地址时，纯代理节点无法确定如何找到控制节点；若继续建模为 `pn.server_addresses`，旧歧义会复现。 | 增加纯代理节点控制节点地址配置的独立 design 要求。 |
| 控制节点地址是否应继续放在 `pn.control_server`？ | 不应。该地址描述代理节点要连接的控制/SN 节点，而不是本机 PN 代理角色自身参数。 | 放在 `pn` 下会让 operator 误以为这是代理节点地址或 PN 选择配置，延续 `pn.server_addresses` 的语义混淆。 | 将该配置归入 `sn` 节点，design 选择最终字段名并定义旧字段迁移。 |
| HTTP 配置是否应保持顶层 `http`？ | 不应作为推荐结构。HTTP 管理 API 是控制节点管理面入口，和 SN/control-node 角色绑定。 | 顶层 `http` 在多角色配置中语义过宽，纯 PN 节点看到该字段时容易误以为本地也要启动管理面。 | 将 HTTP 管理面监听配置归入 `sn` 节点，design 定义旧顶层字段兼容或拒绝策略。 |
| 管理员账号是否应保持顶层 `admin`？ | 不应作为推荐结构。管理员账号服务于控制节点管理面和代理节点审批，和 SN/control-node 角色绑定更强。 | 顶层 `admin` 在多角色配置中语义过宽，纯 PN 节点看到该字段时也容易误以为需要本地管理员账号。 | 将管理员 bootstrap 配置归入 `sn` 节点，design 定义旧顶层字段兼容或拒绝策略。 |
| JWT 配置是否应保持顶层 `jwt`？ | 不应作为推荐结构。JWT key 服务于控制节点 HTTP 管理面的登录会话签名，和 SN/control-node 角色绑定更强。 | 顶层 `jwt` 在多角色配置中语义过宽，纯 PN 节点看到该字段时容易误以为本地也需要管理会话签名配置。 | 将 JWT 签名配置归入 `sn` 节点，design 定义旧顶层字段兼容或拒绝策略。 |
| 证书名字配置应放在哪里？ | 用户指向 `config.example.yaml` 中监听 `ip`/`port` 附近要求增加 `name` 字段。该名字描述本机 P2P identity 证书，而不是 HTTP 登录 JWT 或管理员账号。 | 将证书名字作为本机 identity/listener 启动配置；proposal 推荐在顶层 `ip`/`port` 附近增加 `name`，最终字段名和环境变量由 design 固化。 | Design 必须定义 YAML 字段、默认名字、环境变量兼容、旧配置行为和示例模板位置。 |
| 修改证书名字是否应轮换私钥？ | 不应。operator 改名字通常只想改变证书展示/subject；轮换私钥会改变节点身份材料，可能破坏控制节点、代理节点或已批准节点关系。 | 修改名字时必须复用已有私钥重签证书；仅当私钥缺失或不可解析时才走新 identity 生成路径或明确失败。 | Design 必须确认现有 `p2p-frame` X509 identity API 是否支持从旧私钥重签，并定义无法重签时的错误处理。 |
| 代理节点名字是否应该随上报进入控制面？ | 应该。用户要求代理节点设置名字后，上报控制节点时带上名字，控制节点再把该名字返回给客户端。 | 需要扩展代理上报/心跳 payload、运行时状态和返回给客户端的 `PnServerInfo`；若名字被当成 id，会破坏批准和选择稳定性。 | 将名字作为可变元数据随代理上报进入控制面，并通过代理节点信息返回；稳定身份仍使用 `pn_server` id。 |
| 未设置代理节点名字时如何处理？ | 旧部署和未命名代理节点必须继续可用。 | 强制名字会破坏旧配置；完全缺省则客户端可能只能回退到 id。 | Design 必须定义缺省/fallback：未上报名字时返回空/缺省字段，并由客户端按其 approved design 回退。 |
| 纯代理节点是否应继续通过 SN client 连接控制节点？ | 不应。当前实现把控制节点作为 `P2pSn` 后会启动 SN client 在线上报，导致控制节点打印 `report sn from ...`，这与“代理节点和控制面节点已有通信通道”的目标不一致。 | 直接禁用 SN client 有风险：现有命令发送、远端 tunnel 校验、PN 连接校验和流量/心跳上报都借用了该 stack 的 cmd client。 | 需求改为“不得启动 SN client”，但 design 必须先定义等价的非 SN-client 控制通道，不能只移除上报循环。 |
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
| data/schema | yes | 外部代理节点批准状态必须 SQLite 持久化，代理流量统计复用 database-backed 存储接口，列表 API 还需要表达 observed/remote address。 | Design 必须定义外部代理节点批准表或等价 schema、迁移行为、状态枚举、重启恢复语义、统计存储接口复用方式，以及 observed address 字段是否持久化或仅运行时返回。 | owner: design stage; acceptance impact: schema validation 延后到 design/implementation。 |
| data/schema | yes | 配置了 `port_mapping` 的代理节点需要上报本地监听 Endpoint + `port_mapping` 元数据，由控制节点用 observed IP + mapped external port 合成可代理地址；SN/control observed IP 和代理节点地址不作为 SQLite 真相。 | Design 必须定义停止持久化代理节点地址、旧 address 字段清理/兼容策略、只依赖 live 地址参与选择和下发。 | owner: design stage; risk: 旧 network/proxy-node 缓存可能继续下发过期地址。 |
| data/schema | yes | NodeId SQLite key 列需要从 base58 canonical 改成 base36 canonical。 | Design 必须定义旧 base58 行的读取/迁移策略，以及新写入列的编码 helper。 | owner: design/testing; risk: 旧数据库成员、流量和 joined-node 记录可能不可见。 |
| security/privacy/permission | yes | 控制节点策略决定外部代理节点是否可用，HTTP 批准接口会改变可用代理集合。 | Design 必须定义外部代理节点接受的认证/授权方式，以及 HTTP 批准接口的管理员权限要求。 | owner: implementation/testing stages after design; risk: 授权测试需要 approved policy 和接口。 |
| runtime/integration | yes | 代理节点选择将基于内置允许代理节点、已连接/接受的外部代理节点和心跳 liveness，而不是配置地址列表。 | Design 必须覆盖启动、内置默认允许、注册、心跳、选择、断连、超时、重连和失败行为。 | owner: implementation/testing stages after design; acceptance impact: integration evidence 延后到实现存在后。 |
| runtime/integration | yes | 纯代理节点不得启动 SN client，控制面连接必须走替代控制通道，且控制节点不应出现纯代理节点触发的 SN `ReportSn` 日志。 | Design 必须覆盖替代控制通道、命令收发、在线判断、重连、校验、心跳和流量上报，不得依赖 `SNClientService` active SN 状态。 | owner: design/implementation/testing stages; risk: 当前实现借用 SN cmd client，直接删除会中断控制面交互。 |
| ui/datamodel/workflow | yes | HTTP 控制面需要导出外部代理节点列表、批准和拒绝工作流；列表需要提供真实连接地址给管理 UI。 | Design 必须定义 HTTP API 数据模型和工作流，并明确 `vpn_web` 应显示 observed/remote address 而不是本地配置地址。 | owner: design stage; risk: API 字段未落地时 UI 只能显示旧地址或空值。 |
| build/dependency/config/deployment | yes | YAML 配置合同移除 `pn.server_addresses` 作为外部代理节点输入，同时纯代理节点仍需在 `sn` 配置域配置控制节点地址，HTTP 管理面监听配置、管理员 bootstrap 配置和 JWT 签名配置也迁入 `sn` 配置域。 | Proposal 和 design 必须记录配置迁移行为、纯代理节点控制节点地址字段、HTTP/admin/JWT 字段归属以及旧 `pn.control_server`/顶层 `http`/顶层 `admin`/顶层 `jwt` 的兼容策略。 | owner: design stage; risk: 部署验证依赖 approved compatibility behavior。 |
| build/dependency/config/deployment | yes | YAML 配置模板需要增加本机 identity 证书名字字段，并且重签行为会影响 identity 文件写入。 | Design 必须定义字段位置、默认值、环境变量、identity 文件兼容和重签失败行为；implementation admission 后才能更新 `config.example.yaml`、配置解析和 identity 生成流程。 | owner: design/implementation/testing; risk: 直接改代码可能轮换私钥或破坏已有节点身份。 |
| security/privacy/permission | yes | 私钥保留是节点身份连续性的安全边界；改名不应隐式生成新私钥。 | Design 必须规定改名时复用私钥重签，且不能静默删除或覆盖不可解析的旧私钥。 | owner: design/implementation; risk: 误轮换私钥会导致既有控制关系失效。 |
| contract/protocol | yes | 代理节点上报、心跳或返回给客户端的 `PnServerInfo` 需要携带代理节点名字。 | Design 必须定义名字字段在 `vpn-frame` shared protocol、代理控制命令、HTTP/API 响应和客户端消费路径中的映射。 | owner: vpn-frame/bucky-vpn-server/bucky-vpn design; risk: 任一模块缺失会导致客户端仍按 id 连接。 |
| contract/protocol | yes | 代理节点上报地址和控制节点下发地址需要使用 Endpoint 类型，而不是拆散的 ip/port 字段。 | Design 必须同步 `vpn-frame` shared protocol、server 构造/HTTP projection 和 client 连接消费。 | owner: vpn-frame/bucky-vpn-server/bucky-vpn design; risk: shared protocol 不同步会导致序列化或连接行为不一致。 |
| runtime/integration | yes | 控制节点必须把上报名字保存到运行时代理状态，并在选择/返回代理信息时保留该名字。 | Implementation 后 testing 必须覆盖有名字、无名字、名字变化和不会替代审批 key 的路径。 | owner: implementation/testing; risk: 名字变化可能错误影响批准状态或 liveness。 |
| harness/process | yes | 需求/范围变更必须先落在 proposal，再进入下游阶段。 | 运行 proposal doc structure 和 proposal stage scope 检查。 | owner: downstream stages; acceptance impact: proposal/design 更新并批准前 admission 继续延后。 |

## High-Level Outcomes
- 受支持的代理节点启动配置不再要求 operator 列出外部代理节点地址。
- 外部代理节点成为运行时参与者，主动连接/注册，并由控制节点策略接受或拒绝。
- 外部代理节点批准状态持久化到 SQLite，控制节点重启后保留管理员批准/拒绝决定。
- HTTP 控制面提供外部代理节点查询、批准和拒绝能力，并复用现有认证边界。
- HTTP 控制面的外部代理节点查询返回控制节点观察到的真实连接来源地址。
- 配置 `port_mapping` 的代理节点上线后，代理节点保留本地监听 Endpoint 端口并单独上报 `port_mapping`，控制节点根据连接观测 IP、Endpoint 协议和映射外网端口合成可下发、可选择的代理节点地址。
- SN/control 连接 IP 和代理节点地址不作为 SQLite 地址真相；选择和下发依赖当前 live 合成地址。
- 代理节点上报地址和控制节点下发给客户端的地址使用 Endpoint 形状。
- 服务端 HTTP NodeId 字段和 SQLite NodeId key 新写入使用 base36。
- 纯代理节点拥有明确的 `sn` 域控制节点地址配置，用于 bootstrap/control-plane 连接。
- HTTP 管理面监听配置归入 `sn` 域，表达其属于控制节点管理面。
- 管理员账号 bootstrap 配置归入 `sn` 域，表达其属于控制节点管理面。
- HTTP 登录会话 JWT 签名配置归入 `sn` 域，表达其属于控制节点管理面。
- 服务端启动配置提供本机 P2P identity 证书名字字段。
- 修改证书名字时保留已有私钥，并生成带新名字的证书。
- 代理节点向控制节点上报配置的名字。
- 控制节点返回给客户端的代理节点信息包含上报的代理节点名字。
- 纯代理节点不启动 SN client，不向控制节点 SN service 发送 `ReportSn`，控制面不再因纯代理节点接入打印 SN client 上报日志。
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
| PROP-external-pn-observed-address | CHG-external-pn-observed-address | 外部代理节点列表返回控制节点观察到的真实连接来源地址，区别于代理节点本地配置地址。 | Design 命名字段、来源、生命周期和缺失值语义；implementation 在 `/pn_proxy_nodes` 响应中提供该地址。 |
| PROP-pn-port-mapping-observed-address | CHG-pn-port-mapping-observed-address | 配置了 `port_mapping` 的代理节点上线时，代理节点上报本地监听 Endpoint 和独立 `port_mapping` 元数据；控制节点用观测到的连接 IP、Endpoint 协议和映射外网端口合成可代理地址；SN/control IP 和代理节点地址不持久化为地址真相。 | Design 命名 observed IP 来源、Endpoint 本地监听端口、`port_mapping` 上报字段、`PnServerInfo` Endpoint 列表形状、live-only 地址生命周期和旧地址字段停止依赖策略；implementation 后客户端下发地址使用合成结果。 |
| PROP-pn-server-endpoint-address-contract | CHG-pn-server-endpoint-address-contract | 代理节点上报地址和控制节点下发给客户端的代理节点地址使用 Endpoint 类型或等价共享 Endpoint 合同。 | Design 同步 `vpn-frame` shared protocol、server 上报/选择/API projection 和 client 连接消费；implementation 后协议地址不再只以拆散 ip/port 表达。 |
| PROP-server-node-id-base36 | CHG-server-node-id-base36 | 服务端 HTTP API、SQLite NodeId key 和节点识别日志使用 base36 作为 `NodeId` canonical string。 | Design 映射 `sqlite_store_factory.rs`、`api.rs`、`main.rs`、相关日志和旧 base58 兼容策略；implementation 新写入/响应不再使用 base58。 |
| PROP-pure-pn-sn-address | CHG-pure-pn-sn-address | 纯代理节点启动配置在 `sn` 配置域包含连接 control/bootstrap server 所需的控制节点地址。 | Design 命名 YAML 字段、地址格式、验证行为、旧 `pn.control_server` 迁移策略以及纯代理节点如何使用它。 |
| PROP-sn-http-config | CHG-sn-http-config | HTTP 管理面监听配置归入 `sn` 配置域，表达其属于控制节点管理面。 | Design 命名 YAML 字段、环境变量兼容策略、旧顶层 `http` 迁移行为，以及 implementation 更新配置模板和解析。 |
| PROP-sn-admin-config | CHG-sn-admin-config | 管理员账号 bootstrap 配置归入 `sn` 配置域，表达其属于控制节点管理面。 | Design 命名 YAML 字段、环境变量兼容策略、旧顶层 `admin` 迁移行为，以及 implementation 更新配置模板和解析。 |
| PROP-sn-jwt-config | CHG-sn-jwt-config | HTTP 登录会话 JWT 签名配置归入 `sn` 配置域，表达其属于控制节点管理面。 | Design 命名 YAML 字段、环境变量兼容策略、旧顶层 `jwt` 迁移行为，以及 implementation 更新配置模板和解析。 |
| PROP-server-identity-cert-name | CHG-server-identity-cert-name | 服务端启动配置提供本机 P2P identity 证书名字字段；名字变化时保持已有私钥并重签生成带新名字的证书。 | Design 命名 YAML 字段、默认值、环境变量、旧 identity 兼容、重签失败行为和 `p2p-frame` X509 API 使用方式；implementation 更新配置模板、解析和 identity 生成/更新流程。 |
| PROP-server-proxy-node-reported-name | CHG-server-proxy-node-reported-name | 代理节点向控制节点上报配置名字；控制节点在运行时状态和返回给客户端的代理节点信息中保留该名字，但不把名字作为身份或审批 key。 | Design 映射代理注册/心跳 payload、`PnServerInfo.name`、HTTP/API 响应、selector state 和无名字 fallback；implementation admission 后客户端可收到上报名字。 |
| PROP-pure-pn-no-sn-client | CHG-pure-pn-no-sn-client | 纯代理节点控制面通信不启动 `p2p-frame` SN client，不向控制节点 SN service 发送 `ReportSn`。 | Design 命名非 SN-client 控制通道、命令收发接口、在线/重连/失败语义；implementation 后控制节点不再出现纯代理节点触发的 `report sn from ...` 日志。 |
| PROP-pn-sn-heartbeat | CHG-pn-sn-heartbeat | 代理节点与控制节点在连接/注册后保持心跳，heartbeat liveness 控制代理节点可用性。 | Design 命名心跳间隔、超时、状态迁移、重连行为和选择影响。 |
| PROP-colocated-pn-default-allowed | CHG-colocated-pn-default-allowed | 与控制节点同进程的内置代理节点在本地代理启用时默认允许。 | Design 区分同进程内置代理节点权限和外部代理节点接受流程，并保持无配置默认行为。 |
| PROP-pn-traffic-db-interface | CHG-pn-traffic-db-interface | 内置代理节点流量统计直接使用现有 database-backed 存储接口。 | Design 命名存储接口，implementation 通过该接口写入流量统计且不新增平行 store。 |
| PROP-local-pn-toggle-preserved | CHG-local-pn-toggle-preserved | `pn.enabled` 继续控制内置本地代理节点。 | 缺少配置时仍默认启动本地代理节点，显式关闭时阻止本地代理节点启动。 |

## Success Criteria
- `proposal.md` 记录 `pn.server_addresses` 不再是受支持的外部代理节点机制。
- `proposal.md` 记录外部代理节点通过受控制节点策略控制的主动连接/注册路径接受。
- `proposal.md` 记录外部代理节点批准状态必须持久化到 SQLite。
- `proposal.md` 记录 HTTP 控制面必须导出代理节点列表、批准和拒绝接口。
- `proposal.md` 记录代理节点列表必须暴露真实连接来源地址用于管理 UI 展示。
- `proposal.md` 记录配置了 `port_mapping` 的代理节点必须上报本地监听 Endpoint + `port_mapping` 元数据，并由控制节点用 observed IP + mapped external port 合成客户端可连接地址。
- `proposal.md` 记录 SN/control 连接 IP 和代理节点地址不作为 SQLite 地址真相，旧 address 字段需要停止依赖或清理策略。
- `proposal.md` 记录代理节点上报地址和控制节点下发给客户端的代理节点地址使用 Endpoint 形状。
- `proposal.md` 记录服务端 NodeId 字符串 canonical 改为 base36。
- `proposal.md` 记录纯代理节点配置需要 `sn` 域控制节点地址。
- `proposal.md` 记录 HTTP 管理面监听配置归入 `sn` 配置域。
- `proposal.md` 记录管理员账号 bootstrap 配置归入 `sn` 配置域。
- `proposal.md` 记录 HTTP 登录会话 JWT 签名配置归入 `sn` 配置域。
- `proposal.md` 记录服务端启动配置提供本机 P2P identity 证书名字字段。
- `proposal.md` 记录修改证书名字时必须保留已有私钥并重签生成新名字证书。
- `proposal.md` 记录代理节点设置名字后，上报控制节点时必须带上该名字。
- `proposal.md` 记录控制节点返回给客户端的代理节点信息必须包含上报的代理节点名字。
- `proposal.md` 记录纯代理节点不得启动 SN client，控制面通信必须走非 SN-client 控制通道。
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
- 如果 UI 继续显示 `pn_server.ip:port`，管理员在 NAT、多网卡或容器部署中会看到代理节点本地配置地址而不是真实连接来源地址。
- 如果 observed address 被误用作代理节点身份或审批 key，NAT 地址变化可能破坏已有批准记录。
- 如果 `port_mapping` 只替换端口但继续使用代理节点本地 IP，客户端仍可能收到内网、容器或不可路由地址。
- 如果控制节点把 observed IP 或代理节点地址作为 SQLite 真相持久化，NAT 地址变化、重连或控制节点重启可能产生 stale 地址。
- 如果 network PN server 只按 id 判断有效，修改 `port_mapping`、Endpoint 或 observed IP 变化后可能继续下发旧地址。
- 如果共享协议继续用拆散 ip/port 字段表达代理节点地址，QUIC/TCP 多端点和 Endpoint 协议信息可能丢失或被错误重组。
- 如果 NodeId base36 切换没有迁移或兼容旧 base58 数据，已有 joined-node、network-member 和流量统计记录可能在升级后不可见。
- 如果心跳超时过于激进，短暂网络抖动可能把健康代理节点从选择中移除。
- 如果心跳超时过于宽松，死亡代理节点可能继续被选择并导致客户端连接失败。
- 如果内置代理节点被错误强制走外部接受流程，默认部署可能失败或需要不必要的注册配置。
- 如果内置代理节点绕过与其他代理流量相同的统计持久化接口，API 视图和存储累计值可能分叉。
- 如果旧 `pn.server_addresses` 配置兼容行为不明确，部署可能静默失败或继续使用不受支持语义。
- 如果纯代理节点的控制节点地址没有与被移除的代理地址列表分开建模，配置语义会继续含糊。
- 如果控制节点地址继续放在 `pn.control_server`，配置会把“要连接的控制节点”和“本机 PN 代理角色”混在一起，operator 可能继续误解为代理节点地址列表。
- 如果 HTTP 管理面监听配置继续作为顶层推荐配置，多角色部署中纯代理节点配置可能携带无意义 HTTP 管理面字段，控制节点配置边界也不清晰。
- 如果管理员账号继续作为顶层推荐配置，多角色部署中纯代理节点配置可能携带无意义账号字段，控制节点配置边界也不清晰。
- 如果 JWT 签名配置继续作为顶层推荐配置，多角色部署中纯代理节点配置可能携带无意义管理会话签名字段，控制节点配置边界也不清晰。
- 如果证书名字配置缺失，operator 只能接受默认生成证书名字，部署环境中多个服务端 identity 难以区分。
- 如果修改证书名字时重新生成私钥，节点身份材料会变化，可能导致已批准节点、控制节点关系或历史配置引用失效。
- 如果旧 identity 文件不可解析时静默覆盖，可能造成不可恢复的身份丢失；应 fail closed 或明确进入新 identity 初始化路径。
- 如果代理节点上报名字没有进入控制面返回值，客户端无法按 operator 配置的名字连接代理节点。
- 如果控制节点把名字当作审批 key 或 selector key，名字变化或重复名字会破坏已批准代理节点状态。
- 如果无名字代理节点没有 fallback 语义，旧配置或旧代理节点可能在升级后不可连接。
- 如果纯代理节点控制节点地址支持多种格式但缺少严格验证，启动可能延迟失败或注册到错误控制面。
- 如果只移除 SN client 而没有替代控制通道，纯代理节点会失去命令发送、远端 tunnel 校验、PN 连接校验、心跳或流量上报能力。
- 如果替代控制通道仍间接创建 `SNClientService` 或执行 `ReportSn`，控制节点仍会打印 SN 上报日志，需求未真正满足。
- 如果代理节点选择更新时没有保留 source-target relay 授权，现有安全边界可能回退。
- 如果 `pn.enabled` 语义与外部代理节点可用性混淆，关闭本地代理节点可能意外关闭外部代理节点发现或上报。

## Downstream Follow-Up
| stage | required_follow_up | reason |
| --- | --- | --- |
| Design | 更新 `design.md`：移除 `pn.server_addresses`，加入外部代理节点主动连接/注册模型，定义 `sn` 域纯代理节点控制节点地址字段、`sn` 域 HTTP 管理面字段、`sn` 域管理员账号字段、`sn` 域 JWT 签名字段、非 SN-client 控制通道、控制节点接受策略、SQLite 批准状态 schema、HTTP 批准接口、真实连接来源地址字段、心跳行为、内置代理节点默认允许、统计存储接口复用、状态 owner、选择行为和兼容处理。 | 当前 design 若没有覆盖 `sn.http`/`sn.admin`/`sn.jwt` 或等价 SN 子节点，不能准入新的配置结构。 |
| Design | 为 `CHG-pn-port-mapping-observed-address` 定义代理节点本地监听 Endpoint 与 `port_mapping` 分开上报/解析、控制节点 observed IP 获取、合成 `PnServerInfo` Endpoint 地址规则、selector 有效性比较、旧地址字段停止依赖/清理和控制节点重启后的 live-only 选择语义。 | 当前 design 只要求 observed address 展示，没有覆盖 observed IP + mapped external port 作为客户端可连接地址的生成，也没有覆盖不持久化代理节点地址。 |
| Design | 为 `CHG-pn-server-endpoint-address-contract` 定义代理节点上报 Endpoint、控制节点返回 Endpoint、HTTP projection 和客户端连接消费的跨模块合同。 | 当前 proposal/design 仍以 `PnServerInfo.ip/port/addresses` 为主，不能准入 Endpoint 地址合同变更。 |
| Design | 为 `CHG-server-identity-cert-name` 定义证书名字配置字段、默认值、环境变量、推荐模板位置、旧 identity 文件兼容、保留私钥重签流程、不可重签时的错误处理和 `p2p-frame` X509 API 使用方式。 | 当前 design 没有覆盖证书名字、identity subject/name 更新或保留私钥重签证书，不能准入生产代码变更。 |
| Design | 为 `CHG-server-proxy-node-reported-name` 定义代理节点名字字段如何从配置进入注册/心跳上报、控制节点运行时状态、HTTP 管理列表、返回给客户端的 `PnServerInfo.name`，以及未设置名字和名字变化的 fallback。 | 当前 design 没有覆盖代理节点上报名字或客户端代理节点信息中的名字字段，不能准入生产代码变更。 |
| Design | 为 `CHG-pure-pn-no-sn-client` 定义替代控制通道：命令发送接口、连接建立、在线判断、重连、失败处理、远端 tunnel 校验、PN 连接校验、心跳和流量上报全部不得依赖 `SNClientService`/`ReportSn`。 | 当前实现借用 `stack.sn_client().get_cmd_client()`，直接 implementation 会丢失控制面功能或继续触发 SN 上报。 |
| Design | 为 `CHG-server-node-id-base36` 定义服务端 NodeId base36 canonical 写入/解析、旧 base58 读取兼容或迁移策略，以及与 `vpn-frame`、`bucky-vpn`、`vpn_web` 的跨模块接口边界。 | 当前服务端代码和 design 多处默认 base58，无法直接 implementation。 |
| Implementation | 在 approved design 和 admission 后，更新 `vpn-server/src/server_config.rs`、`vpn-server/src/main.rs`、`vpn-server/src/api.rs`、`vpn-server/src/sqlite_store_factory.rs`、代理节点 selection/control/heartbeat code，以及代理 traffic persistence wiring 以匹配 approved design。 | 当前 `/pn_proxy_nodes` 只返回 `pn_server`、`status`、`live`、`updated_at` 和 `comment`，没有真实连接来源地址字段。 |
| Implementation | 在 approved design 和 admission 后，更新代理节点上线/心跳合并逻辑，使代理节点只上报本地监听 Endpoint + `port_mapping` 元数据，由控制节点用 observed IP + mapped external port 合成 live PN server 地址，并停止把代理节点地址作为控制节点 SQLite 真相。 | 当前逻辑可能把 observed endpoint 和 reported endpoint 仅追加到 addresses，或在代理节点本地提前替换监听端口，且 SQLite schema 中仍有 PN address 字段，可能继续下发旧地址。 |
| Implementation | 在 approved design 和 admission 后，更新 `vpn-server/config/config.example.yaml`、`vpn-server/src/server_config.rs` 和 `vpn-server/src/main.rs`，使配置可设置证书名字，并在名字变化时复用旧私钥重签证书。 | 当前 identity 生成只在 identity 文件不存在时调用默认名字生成路径，没有名字配置或重签流程。 |
| Implementation | 在 approved design 和 admission 后，更新代理控制 client/server、selector state、API DTO 和 `NodeNetwork.pn_server` 构造路径，使上报名字进入返回给客户端的代理节点信息。 | 当前代理节点信息只表达 id/ip/port/observed address 等字段，没有上报名字贯穿路径。 |
| Testing | 在 post-implementation testing 阶段新增或更新测试，覆盖配置解析、deprecated/removed 字段行为、`sn` 域纯代理节点控制节点地址验证、`sn` 域 HTTP 管理面监听解析、`sn` 域管理员账号解析、`sn` 域 JWT 签名解析、内置代理节点默认允许、批准状态持久化、HTTP 批准/拒绝接口权限与错误语义、统计持久化接口使用、外部代理节点接受、heartbeat timeout/recovery、选择和失败路径。 | 该行为变更影响配置、schema、HTTP 控制面、权限、liveness、统计持久化和 runtime integration。 |
| Testing | 为 `CHG-pn-port-mapping-observed-address` 设计验证：代理节点上报内网本地监听 Endpoint 与单独 `port_mapping` 时，控制节点用 observed IP 和映射外网端口合成下发 Endpoint；代理节点本地上报端口不被替换；修改映射或 observed IP 后不复用旧地址；SN/control IP 和代理节点地址不作为 SQLite 真相。 | 该行为直接影响 NAT/LB/容器部署下客户端能否连接代理节点。 |
| Testing | 为 `CHG-pn-server-endpoint-address-contract` 设计验证：代理节点上报 Endpoint、控制节点下发 Endpoint、客户端按 Endpoint 协议连接，覆盖 QUIC/TCP 地址并避免只依赖拆散 ip/port。 | 该行为改变共享协议地址形状，需要跨模块验证。 |
| Testing | 为 `CHG-server-identity-cert-name` 设计配置解析和 identity 更新验证：默认名字、新名字配置、已有 identity 同名不重写、已有私钥改名重签、旧 identity 不可解析时失败或显式初始化路径。 | 证书名字变更涉及 identity 持久化，缺少测试会难以及时发现私钥轮换或静默覆盖。 |
| Testing | 为 `CHG-server-proxy-node-reported-name` 设计验证：有名字上报并返回给客户端、无名字 fallback、名字变化不改变审批 key、HTTP 管理列表显示名字、客户端消费依赖由 `bucky-vpn` 测试覆盖。 | 名字贯穿代理上报和客户端连接路径，缺少测试会导致控制面或客户端继续按 id/name fallback 连接。 |
| Acceptance | 重新审计 proposal、design、implementation 和 testing 的一致性后再接受该变更。 | 当前 approved 下游 artifacts 在本 proposal 更新后已经 stale。 |

## Approval Record
- approver: auto-pipeline
- approval_date: 2026-07-07T00:41:31+08:00
- user_statement: "确认，自动处理后续步骤"
