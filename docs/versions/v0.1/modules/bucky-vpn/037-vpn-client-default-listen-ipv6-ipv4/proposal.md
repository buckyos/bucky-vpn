---
task_manifest: task.yaml
status: approved
---

# Vpn-client 默认同时监听 IPv4 与 IPv6 Proposal

## Workflow Tier Judgment
- Proposed tier: standard
- Final tier: standard
- Tier rationale / triggered boundaries: 变更范围集中 vpn-client 监听地址的默认值，属于单一模块的生产默认/运行时行为变更（`trivial` 明确要求无素材性的生产默认影响，故不成立）；无持久化 schema、公共协议/CLI、安全边界剥离、依赖图或发布面变更，不触发 `high-risk`。
- Proposal and tier confirmation: 用户于 2026-09-16 明确回复「确认」，批准本提案按 `standard` 层级执行。

## Background and Goal
当前 `vpn-client` 的 p2p 监听默认只绑定 IPv4 通配地址 `0.0.0.0`（`DEFAULT_LISTEN_IP`），因此 IPv6 网络的节点无法直接与客户端建立 p2p 连接。目标是将默认监听地址改为同时覆盖 IPv4 与 IPv6 通配地址（`0.0.0.0, ::`），使客户端开箱即用地在双栈环境监听。已有 `parse_listen_ips` 支持逗号/空白分隔的地址列表，且持久化配置 `VPN_P2P_LISTEN_IP` 可覆盖默认值。

## Scope
### In scope
- 将 `vpn-client/src/cli.rs` 的 `DEFAULT_LISTEN_IP` 从 `"0.0.0.0"` 改为 `"0.0.0.0, ::"`。
- 同步更新受默认值变化影响的单元测试（`cli.rs` 的默认值断言与 `main.rs` 的 `p2p_listen_endpoints` 断言），使其反映双栈默认监听。
- 保留 `parse_listen_ips` 的既有校验、env/config 覆盖机制与 4 个地址上限不变。

### Out of scope
- 修改 p2p 端口、协议栈绑定逻辑或 `p2p_listen_endpoints` 的地址→EndPoint 映射规则。
- 修改本地 API（`api.ip`/`api.port`，默认仍为 `127.0.0.1:4536`）。
- 修改 `vpn-server`、`vpn-frame`、`vpn_web` 或其他模块。
- 引入新的配置项或文档化的运维迁移流程。

### Boundary with neighboring modules
仅 `bucky-vpn` 模块的 vpn-client 默认值与对应单元测试变化；`vpn-frame` 的 Endpoint/P2P 栈绑定逻辑保持不变。

## Requirement Review
该需求合理：默认绑定双栈是双栈 VPN 网络的常见运维期望，且代码已具备多地址监听能力，只需调整默认值。唯一注意点是非双栈主机上绑定 `::` 的可用性；由于地址列表允许按主机能力覆盖且失败仍由既有 `ServerRuntime` 路径处理，采用双通配默认是稳妥方向。`::` 通配（而非 `::1`）由 `parse_listen_ips` 的 "at most one wildcard per family" 校验支持下与 `0.0.0.0` 并存。

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-vpn-client-default-dual-stack | vpn-client 无显式 `p2p.listen_ip` 配置时默认同时监听 IPv4 通配 `0.0.0.0` 与 IPv6 通配 `::`，并保持显式配置与 `parse_listen_ips` 校验行为不变。 | 仅 `vpn-client/src/cli.rs` 的 `DEFAULT_LISTEN_IP` 常量及受默认值影响的两处单元测试。 | 非双栈主机可能无法绑定 `::`，但默认仅多出 IPv6 通配地址，operator 可继续通过 env/config 覆盖；共享 EndPoint 映射与协议栈绑定不随之改变。 | `cargo test -p vpn-client` 全部通过；新增/更新测试证明无配置时默认解析为 `[0.0.0.0, ::]` 且 `p2p_listen_endpoints` 生成 IPv4+IPv6 的 Quic/Tcp 各两条 EndPoint。 | 不改端口、绑定逻辑、显式配置覆盖、本地 API 默认或其他模块。 |

## Success Criteria
- 可见结果：`vpn-client` 在无 `p2p.listen_ip` 配置启动时，日志显示同时监听 `0.0.0.0` 与 `::`（p2p 端口）。
- 必要证据：`cargo test -p vpn-client` 通过；`DEFAULT_LISTEN_IP` 默认解析/EndPoint 生成测试覆盖 IPv4 与 IPv6 双栈。
- 显式非目标：不改变 `ServerRuntime` 绑定失败处理、不修改其他模块、不引入新配置项。

## Risks
- 非双栈（IPv6 不可用）主机上绑定 `::` 可能失败并影响 p2p 监听；既有错误处理路径负责上报，operator 可通过 `p2p.listen_ip` 覆盖。
- 显式配置的用户不受影响，因为默认值仅在配置缺失时生效。