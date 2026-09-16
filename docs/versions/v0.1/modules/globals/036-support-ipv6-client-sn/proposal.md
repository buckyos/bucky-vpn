---
task_manifest: task.yaml
status: approved
---

# 客户端与 SN 通信 IPv6 支持提案

Risk profile: ./risk-profile.yaml

## Workflow Tier Judgment
- Proposed tier: high-risk
- Final tier: high-risk
- Tier rationale / triggered boundaries:
  - 跨模块且跨仓库：需求覆盖 `bucky-vpn`（客户端监听与 SN 端点选择）、`bucky-vpn-server`（SN 服务端监听与 HTTP API）、上游 `p2p-frame`（`/mnt/f/work/p2p`，用户已授权直接修改）。单个模块 packet 已不是 canonical 位置，因此在 `globals` 建包；上游改动将在 `/mnt/f/work/p2p` 按其仓库流程登记对应任务包并做 `change_id` 映射。
  - 生产运行时行为变更：客户端与服务端的监听/绑定行为都会改变。客户端在 `create_p2p_env(...).await.unwrap()` 上绑定失败会直接 panic 终止进程；服务端 SN 监听地址解析当前是 IPv4-only 硬编码（`Ipv4Addr::from_str(ip).unwrap()`），改错会直接让 SN 起不来。
  - 已确认的兼容性反例：本机 Linux（`IPV6_V6ONLY` 默认 0）下 `0.0.0.0:<port>` 与 `[::]:<port>` 两个绑定顺序都 `EADDRINUSE`。也就是说“在现有通配监听之外追加 IPv6 通配监听”这一直观做法会打死进程，不是降级。
  - 能力边界会被连带改变：SN 的 NAT probe 端口反射器只绑定 IPv4（`0.0.0.0:<port>`），客户端 `validate_nat_probe_target` 明确拒绝非 IPv4 的 active SN 端点（`ActiveEndpointIpv4`，fail closed）。该限制已由用户明确接受（见 Decisions 3）。
  - 变更同时影响协议/运行时/配置契约/依赖图（上游 git 依赖 rev 需要升级）与多平台兼容性，且验收证据部分只能来自上游测试与代码分支审查。
- Proposal and tier confirmation: 用户在 2026-09-15 对四个待裁决问题逐条给出裁决后回复“确认，自动完成”，确认了本提案与 `high-risk` tier，并显式启动 auto-pipeline（整个下游流程自动推进，不再逐阶段确认）。本 packet 复用 036 序号，因为这是同一任务的 requirement 修订，未另开新任务号。

## Background and Goal
现状（均已核对本仓库与锁定上游 `p2p-frame` `a9875f0` 源码）：
- 客户端监听只有 IPv4：`vpn-client/src/main.rs` 的 `p2p_listen_endpoints(...)` 只构造 `0.0.0.0:<p2p.port>` 的 QUIC 与 TCP 端点，然后原样交给 `P2pConfig::new`，进程不会在 IPv6 上接受入站 P2P 连接。
- 客户端到 SN 的出站方向已有 IPv6 基础：`P2pVpnClientKey::server_addr()` 对 IPv6 字面量会组装成 `[v6]:port`；`sn_endpoints()` 在同一地址上构造 QUIC/TCP 端点；上游 QUIC 客户端端点按 `remote.addr().is_ipv4()` 分别缓存 v4/v6 endpoint。但域名解析仍是 `lookup_host(...).next()`，依赖系统返回顺序，没有明确地址族策略。
- SN 服务端是主要阻塞点：`vpn-server/src/main.rs` 用 `Ipv4Addr::from_str(ip.as_str()).unwrap()` 构造 `sn_endpoint`，`resolve_service_endpoints` 据此生成服务端点，而 `SnService::start_inner` 正是用 `local_identity.endpoints()` 去 `net_manager.listen(...)`。配置写 IPv6 会在解析阶段失败。
- 其余 SN 通信面现状不一：`pn.control_server.endpoint` 走 `parse::<SocketAddr>()`，已支持 IPv6 字面量；`sn/inter_sn` 与 `ttp` 代码路径未发现 IPv4-only 硬编码；SN HTTP API 用 `HttpServerConfig::new(ip: String, port)`，底层 sfo-http 以 `format!("{ip}:{port}")` 拼接后 `listen`，裸 IPv6 字面量会拼接失败，只有带方括号形式才可能通过。
- 服务器配置层其余部分基本与地址族无关：`pn.advertised_ip`、`advertised_ip_for`、`route_local_ip`、`first_non_loopback_interface_ip`、`same_ip_family` 都按 `IpAddr` 通用处理；`resolve_sn_identity_endpoints` 现在只是原样返回服务端点。

目标是让所有 SN 通信面在 IPv6 上可用（客户端 ↔ SN 命令/隧道、PN ↔ SN、SN 互通、SN HTTP API），默认行为可控且允许配置为只监听某一家族，同时不破坏现有 IPv4 行为与启动失败语义。

## Scope
### In scope
- 客户端启动装配注册/绑定 IPv6 监听端点，并定义与现有 `0.0.0.0:<p2p.port>` 的共存策略（含同端口通配双栈冲突、平台 `IPV6_V6ONLY` 差异）与失败/降级行为。
- 监听地址族可配置：客户端与服务端都能通过配置选择监听地址族（例如仅 IPv4、仅 IPv6、或双栈并存），默认值保持不变。
- SN 服务端（`bucky-vpn-server`）接受 IPv6 监听配置：`ip` 允许 IPv6 字面量与 IPv6 通配地址，据此构造服务端点与 SN identity 端点，使内嵌 SN 真正在 IPv6 上监听；非法或不可绑定地址给出明确失败而不是未捕获 panic。
- 客户端到 SN 的端点选择：域名解析出 AAAA 时按明确策略选择地址族并保留回退，IPv6 字面量配置可用，不再依赖解析结果顺序。
- 其余 SN 通信面支持 IPv6：PN ↔ SN 传输端点、SN 互通（inter-SN）路径，以及 SN HTTP API（含监听地址的 IPv6 解析/校验与方括号地址语义）。
- 上游 `p2p-frame` 的必要改动（用户已授权直接修改 `/mnt/f/work/p2p`），例如显式 `IPV6_V6ONLY`/双栈监听支持、listener 绑定语义、SN 端点相关地址族处理；上游改动按 p2p 仓库流程登记任务包。
- 能力边界与可观测性：明确并验证 IPv6 SN 端点下 NAT 探测 fail closed（已接受）、SN 侧准入仍丢弃公网 IPv6、rendezvous 仍为 IPv4-only；提供日志/测试断言与文档说明，不修改 wire 契约。

### Out of scope
- 不实现 NAT probe 反射器/预测的 IPv6 支持（用户已明确接受“经 IPv6 连 SN 的客户端不做 NAT 探测/打洞”）。
- 不放宽 SN 侧 `sanitize_reported_endpoints` 的公网 IPv6 准入，也不放宽 `rendezvous_ipv4_eligible`/`rendezvous_eligible_area` 的 IPv4-only 规则。
- 不修改 `ReportSn`/`SnQuery`/NAT probe 的 wire 字段与协议语义。
- 不修改 `vpn-frame` 的 VPN 虚拟地址语义（`ipv6`/`ipv6_mask`、TUN 配置）。
- 不承诺公网 IPv6 可达性、运营商/防火墙放行或 IPv6-only 端到端打洞能力。

### Boundary with neighboring modules
`bucky-vpn`（`vpn-client`）拥有本地监听端点构造、SN 远端端点选择与客户端启动装配；`bucky-vpn-server`（`vpn-server`）拥有 `ip`/`port`、`http.ip` 等监听配置解析、服务端点与 SN identity 端点装配、以及内嵌 SN 的启动；`p2p-frame`（上游仓库 `/mnt/f/work/p2p`）拥有 listener 绑定/`IPV6_V6ONLY` 语义、SN 准入、rendezvous 与 inter-SN/TTP 端点处理；`vpn-frame` 拥有 VPN 虚拟网络语义。本提案不越界修改 `vpn-frame`。

## Requirement Review
需求方向合理：客户端完全没有 IPv6 入站能力，SN 服务端监听地址解析是 IPv4-only 硬编码，SN HTTP API 的地址拼接方式对裸 IPv6 不友好。需要澄清的一点是：客户端到 SN 的出站连接并不缺 IPv6 代码路径，真正的阻塞在服务端监听装配、地址族配置策略与部分通信面的地址格式处理。

已由用户裁决的范围与边界：
1. 目标面为“所有 SN 通信都支持 IPv6”，包含客户端 ↔ SN、PN ↔ SN、SN 互通与 SN HTTP API。
2. 监听地址族由配置控制，允许配置为只监听某一家族；默认值保持现有行为。
3. 接受 IPv6 SN 端点下 NAT 探测/打洞不可用（fail closed），不纳入本任务。
4. 授权直接修改 `/mnt/f/work/p2p` 中的 `p2p-frame` 代码，因此上游改动在本任务范围内。

关键事实与取舍：
1. Linux 默认 `IPV6_V6ONLY=0`，`0.0.0.0:<port>` 与 `[::]:<port>` 互斥；本机实测两个绑定顺序都 `EADDRINUSE`。因此“追加一个 `[::]` 端点”会打死进程，必须先确定共存策略（配置只监听某一家族，或以显式 `IPV6_V6ONLY` 控制双栈）。
2. 只保留 `[::]` 依赖双栈覆盖 IPv4 同样不可接受：Linux 上远端观测会变成 v4-mapped 地址，破坏 SN 侧 `is_non_lan_ipv4_addr` 与“观测 IP == 上报 IP”的 WAN 判定；Windows/macOS 默认 `IPV6_V6ONLY=1`，会直接丢失 IPv4 入站能力。
3. 服务端侧最小改动面清晰：把 `ip` 解析从 `Ipv4Addr` 放宽到 `IpAddr`（含 `::` 通配）并保持 `resolve_service_endpoints`/identity 端点一致，就能让内嵌 SN 在 IPv6 上监听；HTTP API 需要显式的地址格式化/校验规则。
4. 上游改动不可避免时，`Cargo.toml`/`Cargo.lock` 中的 `p2p-frame` git rev 需要升级，这属于依赖图变更，必须在实现阶段与 p2p 仓库的提交一起闭环。

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-client-ipv6-listen | 客户端启动装配支持 IPv6 监听端点，地址族由配置控制（含只监听某一家族），并在不改变现有 IPv4 默认行为、不改变启动成功/失败语义的前提下完成绑定。 | 仅 `bucky-vpn` 的监听端点构造、配置项与启动装配。 | 新增配置项与平台绑定差异需要明确默认值；纯配置化方案在部分平台仍需上游 `IPV6_V6ONLY` 支持才能双栈共存。 | 定向测试覆盖默认值、仅 v4、仅 v6 与双栈配置；双通配冲突控制用例可复现并解释；受影响 crate 编译/测试通过；`git diff --check` 通过；完成独立缺陷复查。 | 不声称公网 IPv6 可达性；不改 SN 准入策略。 |
| P-002 | CHG-server-ipv6-listen | `bucky-vpn-server` 支持 IPv6 监听配置（IPv6 字面量、`::` 通配、只监听某一家族），据此构造服务端点与 SN identity 端点，使内嵌 SN 在配置的地址族上监听；非法或不可绑定地址给出明确失败。 | 仅 `bucky-vpn-server` 的监听配置解析、端点装配与启动校验。 | 放宽 `ip` 语义会改变配置契约；双栈并存需解决同端口通配冲突或引入多地址配置。 | 配置/端点装配定向测试覆盖 IPv6 字面量、`::` 通配、单族配置与非法输入；受影响 crate 编译/测试通过；启动失败路径可诊断。 | 不改变 `pn.advertised_ip` 既有语义；不承诺运营商侧 IPv6 可达。 |
| P-003 | CHG-client-sn-ipv6-endpoint | 客户端连接 SN 时支持 IPv6 端点：域名解析出 AAAA 时按明确策略选择地址族并保留回退，IPv6 字面量配置可用，不再依赖解析结果顺序。 | 仅 `bucky-vpn` 的 SN 地址解析与端点列表构造。 | 地址族选择/回退会改变连接时序与失败模式；严格偏好可能与运行环境实际可达性不一致。 | 定向测试覆盖 IPv6 字面量、域名多地址（A+AAAA）与解析失败路径；端点列表仍为 QUIC 优先、TCP 次之。 | 不实现复杂调度策略；不改 SN 服务端。 |
| P-004 | CHG-sn-comms-ipv6-surfaces | 其余 SN 通信面支持 IPv6：PN ↔ SN 传输端点、SN 互通（inter-SN）路径与 SN HTTP API 的 IPv6 监听/连接与地址格式处理。 | 涉及 `bucky-vpn-server` 配置与装配，以及上游 `p2p-frame` 中 SN 互通/HTTP 相关路径；不改 wire 协议。 | SN HTTP API 及部分配置项当前以 IPv4 风格拼接地址，改造会改变配置写法与向后兼容边界。 | 各通信面的定向测试/断言（含 IPv6 监听地址格式化、inter-SN 端点族处理）与上游测试证据；受影响 crate 编译/测试通过。 | 不改 SN 准入与 rendezvous 规则；不覆盖 NAT probe。 |
| P-005 | CHG-ipv6-capability-boundaries | 明确并验证 IPv6 通信的能力边界与上游改动闭环：客户端 IPv6 监听地址的上报可见性、SN 侧丢弃公网 IPv6、IPv6 active SN 端点下 NAT 探测 fail closed，并将必要的 `p2p-frame` 改动登记、升级 rev 并映射 `change_id`。 | 仅断言、日志、文档与依赖 rev/上游任务登记；不改 wire 字段与 IPv4-only 准入规则。 | 说明性与依赖闭环工作不产生新连通能力，但可避免把“能连”误判为“IPv6 打洞可用”，并防止仓库间改动失去可追溯性。 | 定向测试或日志断言证明上述边界；上游任务包与 `change_id` 映射记录；`Cargo.lock` rev 与上游提交一致。 | 不改 `ReportSn`/`SnQuery`/NAT probe 协议，不放宽 IPv4-only 准入规则。 |

## Success Criteria
- 具体可见结果：客户端可在配置指定的地址族（含 IPv6）上监听；`bucky-vpn-server` 可按配置在 IPv6 上提供 SN 服务与 HTTP API；客户端可经 IPv6 连接 SN；PN ↔ SN 与 SN 互通路径在 IPv6 端点下不被 IPv4 假设阻断；原有 IPv4 默认行为与启动失败语义不变。
- 必需证据：客户端与服务端定向测试覆盖监听配置（默认/仅 v4/仅 v6/双栈）、IPv6 配置解析、地址族选择与失败路径；涉及上游的改动有对应上游任务包、测试与 `Cargo.lock` rev 记录；受影响 crate 的 `cargo check`/定向测试通过；`git diff --check` 通过；完成报告包含行为/边界/回归三维缺陷复查。
- 明确非目标：不声称公网 IPv6 可达性、不做 IPv6-only 打洞、不实现 NAT probe IPv6、不放宽 SN 准入与 rendezvous 的 IPv4-only 规则；不以 loopback 单测替代真实双栈/公网证据。

## Risks
- 平台绑定语义风险：`IPV6_V6ONLY` 默认值、同端口通配互斥与 reuseport 行为在 Windows/macOS/Linux 不一致；错误策略会导致进程启动失败。
- 配置契约风险：新增监听地址族配置与放宽 `ip` 语义会改变部署配置，默认值与旧配置兼容必须明确。
- 跨仓库风险：`p2p-frame` 是 git 依赖（锁定 `a9875f0`），改动需要上游任务包、提交与 rev 升级同时闭环，否则本仓库改动无法编译或无法追溯。
- 能力回退风险：客户端经 IPv6 连 SN 后 NAT 探测与 rendezvous 仍不可用（已接受）；若后续期望改变，需要新的提案。
- 安全面风险：客户端与服务端新增 IPv6 通配监听都会扩大入站暴露面，防火墙策略需同步；HTTP API 若暴露 IPv6 需明确管理面边界。
- 验收证据缺口：CI 与本机无法证明真实公网 IPv6、跨 NAT、多平台与运营商行为，只能作为已知缺口记录。

## Decisions（已由用户裁决）
1. 目标面：所有 SN 通信都支持 IPv6（客户端 ↔ SN、PN ↔ SN、SN 互通、SN HTTP API）。
2. 监听地址族：由配置控制，允许配置为只监听某一家族；默认保持现有行为。
3. NAT 探测/打洞：接受 IPv6 SN 端点下 fail closed，不纳入本任务。
4. 上游修改：授权直接修改 `/mnt/f/work/p2p` 中的 `p2p-frame` 代码；上游改动按 p2p 仓库流程登记任务包并与本 packet 建立 `change_id` 映射。

配置项的具体键名与取值域、HTTP API 的 IPv6 地址写法、以及上游需要改动的精确清单，属于 design 阶段产物，确认后在 design 中定稿。
