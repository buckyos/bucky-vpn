---
module: bucky-vpn-server
version: v0.1
status: approved
approved_by: auto-pipeline
approved_at: 2026-07-07T00:41:31+08:00
approved_content_sha256: df5709a08c83948be788fb3888870852aa6c64c5ca13123eacb99a3c80562c8d
---

# bucky-vpn-server Testing

## Test Document Index
| document | topic | scope |
| --- | --- | --- |
| `testing.md` | 当前测试设计 | 控制节点/代理节点配置、心跳、内置代理默认允许和统计持久化 |
| `testplan.yaml` | 机器可读测试计划 | unified test entry levels |

## Unified Test Entry
- Unit: `python3 ./harness/scripts/test-run.py bucky-vpn-server unit`
- DV: `python3 ./harness/scripts/test-run.py bucky-vpn-server dv`
- Integration: `python3 ./harness/scripts/test-run.py bucky-vpn-server integration`
- Machine plan: `docs/versions/v0.1/modules/bucky-vpn-server/testplan.yaml`

## Submodule Tests
| submodule | responsibility | required_behavior | test_level | test_file_or_entry | gap |
| --- | --- | --- | --- | --- | --- |
| `server-config` | 解析本地开关和 SN-owned 控制节点配置，移除静态外部代理地址合同。 | 默认启用、禁用 SN/PN、`sn.control_server`、`sn.http`、`sn.admin`、`sn.jwt` 解析，旧字段兼容，不再注入 `pn.server_addresses`。 | unit | `vpn-server/src/server_config.rs` | none |
| `process-assembly` | 组装控制节点、内置代理节点、外部代理控制 client 和统计服务。 | `pn.enabled=true` 时内置代理默认可选；纯代理节点连接控制节点后启动心跳。 | dv | `python3 ./harness/scripts/test-run.py bucky-vpn-server dv` | runtime branch covered by compile/DV only; full network smoke deferred |
| `control-node-control` | 管理外部代理节点批准、心跳 liveness、port_mapping 合成和选择状态。 | 使用控制命令上报本地监听 Endpoint + `port_mapping`，控制节点 selector 只选择 approved + live 的远端代理，并用 observed IP + mapped external port 合成下发 Endpoint。 | unit/dv | `vpn-server/src/server_config.rs`, `vpn-server/src/pn_traffic_service.rs`, dv build | full multi-process smoke deferred |
| `control-node-control` | 管理 proxy-control 专用 purpose 接入。 | 代理控制通道使用专用 purpose，不能复用 SN command purpose，控制节点 listener 使用独立 proxy-control command service 而不是 `SnService`。 | unit | `vpn-server/src/vpn_control_client.rs` | identity-gate full runtime smoke deferred |
| `sqlite-persistence` | 持久化外部代理节点批准状态。 | `pn_proxy_node` schema、pending/approved/rejected 状态和旧库迁移可编译。 | unit/dv | `cargo check -p bucky-vpn-server`, `python3 ./harness/scripts/test-run.py bucky-vpn-server unit` | direct SQLite restart fixture deferred |
| `http-api` | 暴露外部代理节点列表、真实连接来源地址、批准和拒绝接口。 | 新接口复用 Bearer session，注册到 HTTP 控制面；列表响应在可用时返回 `observed_addr`，缺失时不改变审批状态。 | dv/integration | `cargo check -p bucky-vpn-server`, workspace test | direct HTTP smoke deferred |
| `traffic-statistics` | 通过 SQLite-backed 接口持久化代理流量。 | 真实 delta 和 heartbeat zero delta 复用 remote reporter / store 接口。 | unit/integration | `vpn-server/src/pn_traffic_service.rs`, integration entry | no full restart test in this task |
| `node-id-text` | HTTP/API、SQLite key、selector 比较和日志中的 NodeId 文本格式。 | 新输出和新写入使用 base36；非 NodeId base58 编码保持不变。 | unit/dv/integration | `vpn-server/src/api.rs`, `vpn-server/src/sqlite_store_factory.rs`, `vpn-server/src/server_config.rs`, `vpn-server/src/pn_traffic_service.rs` | old database migration fixture deferred |

## Module-Level Tests
| behavior | validation | entry | expected_result |
| --- | --- | --- | --- |
| Config contract | Rust unit tests in `server_config.rs` | `python3 ./harness/scripts/test-run.py bucky-vpn-server unit` | config tests pass and crate tests compile |
| Server assembly | crate build | `python3 ./harness/scripts/test-run.py bucky-vpn-server dv` | `bucky-vpn-server` builds |
| Workspace compatibility | workspace test | `python3 ./harness/scripts/test-run.py bucky-vpn-server integration` | workspace tests pass or failures are recorded |

## External Interface Tests
| interface | success_case | failure_case | validation |
| --- | --- | --- | --- |
| YAML config | Missing optional PN fields keeps local default; `sn.control_server` supplies pure proxy control-node address; `sn.http`, `sn.admin`, and `sn.jwt` supply control-node management settings. | Invalid control endpoint fails parsing; legacy `pn.control_server`, top-level `http`, top-level `admin`, and top-level `jwt` remain compatible. | unit config tests |
| External proxy control channel | Pure proxy node can create control client and use control commands. | Missing `sn.control_server` while SN disabled logs warning and rejects proxy connections. | dv build plus future integration |
| Heartbeat over traffic report | Remote reporter sends zero delta heartbeat with proxy local listen Endpoint and optional port_mapping through existing command; selector applies observed IP + mapped external port and expires remote proxy by TTL. | Reporter error logs warning and next tick retries. | unit selector TTL tests plus dv build |
| HTTP proxy approval API | Authenticated users can list, approve and reject proxy nodes, and list responses expose observed address when runtime peer WAN address is available. | Missing/invalid Bearer token is rejected by existing session decode path; missing peer WAN observation leaves `observed_addr` absent/null. | dv compile plus future HTTP smoke |
| Traffic persistence | Non-zero deltas write through existing SQLite-backed interface. | Zero heartbeat does not create local parallel store. | unit/integration future |
| Server NodeId text contract | API responses, SQLite new key writes, selector comparisons and NodeId logs use base36. | Old base58 SQLite rows are not automatically migrated; password hashes still use existing non-NodeId base58. | unit/dv/integration |

## Direct Change Coverage
| change_id | design_source | validation_id | testplan_level | testplan_step_id | gap | gap_manual_reason |
| --- | --- | --- | --- | --- | --- | --- |
| CHG-pn-config-no-static-addresses | `Directly Mapped Change Items` | VAL-config-no-static-addresses | unit | bucky-vpn-server-unit | no | not-applicable: covered by config tests and compile |
| CHG-external-pn-active-control | `Directly Mapped Change Items` | VAL-external-proxy-control | unit | bucky-vpn-server-unit | no | Selector registration/expiry is covered by unit tests; full multi-process registration integration is deferred. |
| CHG-pure-pn-no-sn-client | `Directly Mapped Change Items` | VAL-pure-proxy-control-purpose | unit | bucky-vpn-server-unit | no | Dedicated purpose is covered by `proxy_control_purpose_is_dedicated`; independent command service construction is covered by `proxy_control_cmd_service_is_independent_from_sn_service`; full runtime identity-gate smoke is deferred. |
| CHG-external-pn-approval-persistence | `Directly Mapped Change Items` | VAL-proxy-approval-persistence | unit | bucky-vpn-server-unit | no | `pn_proxy_node` schema and store/selector integration compile; direct restart fixture is deferred. |
| CHG-external-pn-approval-http-api | `Directly Mapped Change Items` | VAL-proxy-approval-http-api | dv | bucky-vpn-server-dv | no | API route registration and request/response types compile; direct HTTP smoke is deferred. |
| CHG-external-pn-observed-address | `Directly Mapped Change Items` | VAL-proxy-observed-address | dv | bucky-vpn-server-dv | no | `/pn_proxy_nodes` response DTO and runtime peer WAN lookup wiring compile; direct live peer observation smoke is deferred. |
| CHG-pure-pn-sn-address | `Directly Mapped Change Items` | VAL-pure-proxy-control-address | unit | bucky-vpn-server-unit | no | not-applicable: covered by `sn.control_server` parsing and legacy `pn.control_server` compatibility tests |
| CHG-sn-http-config | `Directly Mapped Change Items` | VAL-sn-http-config | unit | bucky-vpn-server-unit | no | not-applicable: covered by `sn.http` parsing and legacy top-level `http` compatibility tests |
| CHG-sn-admin-config | `Directly Mapped Change Items` | VAL-sn-admin-config | unit | bucky-vpn-server-unit | no | not-applicable: covered by `sn.admin` parsing and legacy top-level `admin` compatibility tests |
| CHG-sn-jwt-config | `Directly Mapped Change Items` | VAL-sn-jwt-config | unit | bucky-vpn-server-unit | no | not-applicable: covered by `sn.jwt` parsing and legacy top-level `jwt` compatibility tests |
| CHG-pn-sn-heartbeat | `Directly Mapped Change Items` | VAL-proxy-heartbeat | unit | bucky-vpn-server-unit | no | Heartbeat endpoint liveness is covered by selector TTL tests; reporter error log assertion remains deferred. |
| CHG-colocated-pn-default-allowed | `Directly Mapped Change Items` | VAL-colocated-default | unit | bucky-vpn-server-unit | no | not-applicable: selector uses local endpoint when `pn.enabled=true` |
| CHG-pn-traffic-db-interface | `Directly Mapped Change Items` | VAL-traffic-db-interface | unit | bucky-vpn-server-unit | no | Existing storage path is covered; full restart persistence remains integration follow-up. |
| CHG-local-pn-toggle-preserved | `Directly Mapped Change Items` | VAL-local-toggle | unit | bucky-vpn-server-unit | no | not-applicable: config tests cover default and disabled paths |
| CHG-server-node-id-base36 | `Directly Mapped Change Items` | VAL-server-node-id-base36 | dv | bucky-vpn-server-dv | no | cargo check and workspace compatibility cover API/store/log call sites; old database migration remains out of scope. |
| CHG-server-identity-cert-name | `Directly Mapped Change Items` | VAL-server-identity-cert-name | unit | bucky-vpn-server-unit | no | Config parsing is unit covered; certificate re-signing with existing key is covered by compile/review of identity load path and dependency wiring. |
| CHG-server-proxy-node-reported-name | `Directly Mapped Change Items` | VAL-server-proxy-reported-name | unit | bucky-vpn-server-unit | no | Selector merge and SQLite-backed proxy node listing preserve reported names; API/store DTOs compile. |
| CHG-pn-port-mapping-observed-address | `Directly Mapped Change Items` | VAL-server-observed-mapped-endpoint | unit | bucky-vpn-server-unit | no | Selector tests cover local listen Endpoint reporting, observed IP plus reported port_mapping synthesis in both heartbeat orders, no local port rewrite, and store-backed listing. |
| CHG-pn-server-endpoint-address-contract | `Directly Mapped Change Items` | VAL-server-pn-endpoint-contract | unit | bucky-vpn-server-unit | no | Config projection, proxy-control observed heartbeat, HTTP DTOs, and store paths compile and unit tests assert Endpoint-shaped PN server values. |

## Case-Type Coverage
| change_id | case_type | required | validation_id | level | status | gap_manual_reason |
| --- | --- | --- | --- | --- | --- | --- |
| CHG-pn-config-no-static-addresses | normal | yes | VAL-config-no-static-addresses | unit | covered | not-applicable: static list no longer parsed |
| CHG-pn-config-no-static-addresses | boundary | yes | VAL-config-no-static-addresses | unit | covered | not-applicable: empty static list behavior collapses to local endpoint only |
| CHG-pn-config-no-static-addresses | negative | no | VAL-config-no-static-addresses | unit | not-applicable | Static list is removed rather than rejected in this implementation. |
| CHG-pn-config-no-static-addresses | error | no | VAL-config-no-static-addresses | unit | not-applicable | No static list parser remains to produce parse errors. |
| CHG-pn-config-no-static-addresses | compatibility | yes | VAL-config-no-static-addresses | unit | covered | not-applicable: no-config default is preserved |
| CHG-pn-config-no-static-addresses | lifecycle | no | VAL-config-no-static-addresses | unit | not-applicable | Config removal has no runtime lifecycle by itself. |
| CHG-pn-config-no-static-addresses | cross-module | no | VAL-config-no-static-addresses | unit | not-applicable | Change is contained in server config and assembly. |
| CHG-external-pn-active-control | normal | yes | VAL-external-proxy-control | unit | covered | not-applicable: heartbeat registers remote proxy endpoint into selector |
| CHG-external-pn-active-control | boundary | no | VAL-external-proxy-control | dv | not-applicable | Boundary cases require multi-process harness. |
| CHG-external-pn-active-control | negative | yes | VAL-external-proxy-control | dv | covered | not-applicable: missing control server branch remains reject-all |
| CHG-external-pn-active-control | error | yes | VAL-external-proxy-control | dv | covered | not-applicable: control client creation failure logs and continues |
| CHG-external-pn-active-control | compatibility | yes | VAL-external-proxy-control | dv | covered | not-applicable: local control node path remains unchanged |
| CHG-external-pn-active-control | lifecycle | yes | VAL-external-proxy-control | unit | covered | not-applicable: selector admits heartbeat endpoint and removes it after TTL expiry |
| CHG-external-pn-active-control | cross-module | yes | VAL-external-proxy-control | integration | manual | Uses vpn-frame control commands; workspace compatibility is current coverage; full network smoke deferred. |
| CHG-external-pn-approval-persistence | normal | yes | VAL-proxy-approval-persistence | unit | covered | not-applicable: schema/store/selector path compiles through crate tests |
| CHG-external-pn-approval-persistence | boundary | yes | VAL-proxy-approval-persistence | unit | covered | not-applicable: missing row maps to unapproved and first heartbeat creates pending |
| CHG-external-pn-approval-persistence | negative | yes | VAL-proxy-approval-persistence | unit | manual | Rejected proxy exclusion is implemented in selector but lacks a direct SQLite fixture assertion. |
| CHG-external-pn-approval-persistence | error | yes | VAL-proxy-approval-persistence | dv | manual | Database failure should fail closed; no injected DB failure fixture added. |
| CHG-external-pn-approval-persistence | compatibility | yes | VAL-proxy-approval-persistence | dv | covered | not-applicable: `CREATE TABLE IF NOT EXISTS` preserves old DB startup |
| CHG-external-pn-approval-persistence | lifecycle | yes | VAL-proxy-approval-persistence | unit | manual | Restart persistence is designed but direct restart fixture is deferred. |
| CHG-external-pn-approval-persistence | cross-module | no | VAL-proxy-approval-persistence | unit | not-applicable | State is local to `bucky-vpn-server` SQLite/selector. |
| CHG-external-pn-approval-http-api | normal | yes | VAL-proxy-approval-http-api | dv | covered | not-applicable: API routes and DTOs compile |
| CHG-external-pn-approval-http-api | boundary | yes | VAL-proxy-approval-http-api | dv | manual | Repeated approve/reject is implemented as upsert but lacks direct HTTP assertion. |
| CHG-external-pn-approval-http-api | negative | yes | VAL-proxy-approval-http-api | dv | manual | Missing/invalid Bearer path uses existing decode branch but no direct HTTP smoke added. |
| CHG-external-pn-approval-http-api | error | yes | VAL-proxy-approval-http-api | dv | manual | Store errors propagate through `Resp::from_result`; no injected store failure test added. |
| CHG-external-pn-approval-http-api | compatibility | yes | VAL-proxy-approval-http-api | dv | covered | not-applicable: existing API registration pattern is preserved |
| CHG-external-pn-approval-http-api | lifecycle | yes | VAL-proxy-approval-http-api | integration | manual | End-to-end approve -> select flow needs HTTP/runtime harness and is deferred. |
| CHG-external-pn-approval-http-api | cross-module | no | VAL-proxy-approval-http-api | dv | not-applicable | API is local to `bucky-vpn-server`; Flutter Web UI is explicitly out of scope. |
| CHG-external-pn-observed-address | normal | yes | VAL-proxy-observed-address | dv | covered | not-applicable: DTO field and runtime peer WAN query path compile through crate build. |
| CHG-external-pn-observed-address | boundary | yes | VAL-proxy-observed-address | dv | manual | Missing peer WAN observation should return absent/null; direct handler assertion is deferred. |
| CHG-external-pn-observed-address | negative | yes | VAL-proxy-observed-address | dv | manual | Invalid or unparsable proxy id should not break list response; no direct fixture added. |
| CHG-external-pn-observed-address | error | yes | VAL-proxy-observed-address | dv | manual | Peer WAN query failure should leave `observed_addr` absent/null; no injected runtime failure fixture added. |
| CHG-external-pn-observed-address | compatibility | yes | VAL-proxy-observed-address | dv | covered | not-applicable: existing `pn_server`, status, live, updated_at, and comment fields remain in response. |
| CHG-external-pn-observed-address | lifecycle | yes | VAL-proxy-observed-address | integration | manual | Address changes with runtime observation; full live proxy connection smoke is deferred. |
| CHG-external-pn-observed-address | cross-module | yes | VAL-proxy-observed-address | integration | manual | Consumed by `vpn_web`; workspace/front-backend runtime smoke requires a running server and browser. |
| CHG-pure-pn-sn-address | normal | yes | VAL-pure-proxy-control-address | unit | covered | not-applicable: valid endpoint parsed |
| CHG-pure-pn-sn-address | boundary | yes | VAL-pure-proxy-control-address | unit | covered | not-applicable: IPv4 endpoint parsing covered |
| CHG-pure-pn-sn-address | negative | yes | VAL-pure-proxy-control-address-invalid | unit | covered | not-applicable: invalid endpoint parse fails |
| CHG-pure-pn-sn-address | error | yes | VAL-pure-proxy-control-address-invalid | unit | covered | not-applicable: parse error is returned |
| CHG-pure-pn-sn-address | compatibility | yes | VAL-pure-proxy-control-address | unit | covered | not-applicable: legacy `pn.control_server` existing shape is preserved |
| CHG-pure-pn-sn-address | lifecycle | no | VAL-pure-proxy-control-address | unit | not-applicable | Address parsing has no lifecycle by itself. |
| CHG-pure-pn-sn-address | cross-module | no | VAL-pure-proxy-control-address | unit | not-applicable | Parsed config is consumed by server assembly only. |
| CHG-sn-http-config | normal | yes | VAL-sn-http-config | unit | covered | not-applicable: valid `sn.http` values parse |
| CHG-sn-http-config | boundary | yes | VAL-sn-http-config | unit | covered | not-applicable: default HTTP listener fallback remains available |
| CHG-sn-http-config | negative | yes | VAL-sn-http-config | unit | manual | Invalid port is guarded by parser but no dedicated invalid-port assertion was added. |
| CHG-sn-http-config | error | yes | VAL-sn-http-config | unit | manual | Invalid port returns config error; direct assertion deferred. |
| CHG-sn-http-config | compatibility | yes | VAL-sn-http-config | unit | covered | not-applicable: legacy top-level `http` remains compatible |
| CHG-sn-http-config | lifecycle | no | VAL-sn-http-config | unit | not-applicable | HTTP config parsing has no lifecycle by itself. |
| CHG-sn-http-config | cross-module | no | VAL-sn-http-config | unit | not-applicable | Parsed config is consumed by server assembly only. |
| CHG-sn-admin-config | normal | yes | VAL-sn-admin-config | unit | covered | not-applicable: valid `sn.admin` values parse |
| CHG-sn-admin-config | boundary | yes | VAL-sn-admin-config | unit | manual | Empty name/password validation is not added in this task. |
| CHG-sn-admin-config | negative | yes | VAL-sn-admin-config | unit | manual | Missing admin fields follow existing config error behavior; no dedicated assertion added. |
| CHG-sn-admin-config | error | yes | VAL-sn-admin-config | unit | manual | Config errors propagate to startup; no direct assertion added. |
| CHG-sn-admin-config | compatibility | yes | VAL-sn-admin-config | unit | covered | not-applicable: legacy top-level `admin` remains compatible |
| CHG-sn-admin-config | lifecycle | no | VAL-sn-admin-config | unit | not-applicable | Admin config parsing has no lifecycle by itself. |
| CHG-sn-admin-config | cross-module | no | VAL-sn-admin-config | unit | not-applicable | Parsed config is consumed by server assembly only. |
| CHG-sn-jwt-config | normal | yes | VAL-sn-jwt-config | unit | covered | not-applicable: valid `sn.jwt` values parse |
| CHG-sn-jwt-config | boundary | yes | VAL-sn-jwt-config | unit | manual | Empty JWT key validation is not added in this task. |
| CHG-sn-jwt-config | negative | yes | VAL-sn-jwt-config | unit | manual | Missing JWT fields follow existing config error behavior; no dedicated assertion added. |
| CHG-sn-jwt-config | error | yes | VAL-sn-jwt-config | unit | manual | Config errors propagate to startup; no direct assertion added. |
| CHG-sn-jwt-config | compatibility | yes | VAL-sn-jwt-config | unit | covered | not-applicable: legacy top-level `jwt` remains compatible |
| CHG-sn-jwt-config | lifecycle | no | VAL-sn-jwt-config | unit | not-applicable | JWT config parsing has no lifecycle by itself. |
| CHG-sn-jwt-config | cross-module | no | VAL-sn-jwt-config | unit | not-applicable | Parsed config is consumed by server assembly only. |
| CHG-pure-pn-no-sn-client | normal | yes | VAL-pure-proxy-control-purpose | unit | covered | not-applicable: proxy-control purpose helper returns a stable dedicated purpose and proxy-control service is independently constructible |
| CHG-pure-pn-no-sn-client | boundary | yes | VAL-pure-proxy-control-purpose | unit | covered | not-applicable: unit test proves proxy-control purpose is distinct from SN command purpose |
| CHG-pure-pn-no-sn-client | negative | yes | VAL-pure-proxy-control-purpose | unit | covered | not-applicable: direct `sn_cmd_purpose` reuse would fail the dedicated-purpose assertion and `SnService` reuse would fail the independent-service assertion |
| CHG-pure-pn-no-sn-client | error | yes | VAL-pure-proxy-control-purpose-runtime | dv | manual | Listener registration and missing remote endpoint reject path compile; no injected TTP accept error fixture added. |
| CHG-pure-pn-no-sn-client | compatibility | yes | VAL-pure-proxy-control-purpose | dv | covered | not-applicable: existing VPN command request/response types are reused unchanged |
| CHG-pure-pn-no-sn-client | lifecycle | yes | VAL-pure-proxy-control-purpose-runtime | dv | manual | Start listener -> register heartbeat -> serve command tunnel needs process/TTP runtime smoke and is deferred. |
| CHG-pure-pn-no-sn-client | cross-module | yes | VAL-pure-proxy-control-purpose-runtime | integration | manual | Uses p2p-frame TTP control streams and vpn-frame command server; full network smoke deferred. |
| CHG-pn-sn-heartbeat | normal | yes | VAL-proxy-heartbeat | unit | covered | not-applicable: heartbeat task calls existing reporter |
| CHG-pn-sn-heartbeat | boundary | no | VAL-proxy-heartbeat | unit | not-applicable | Interval boundary is not separately configurable in this task. |
| CHG-pn-sn-heartbeat | negative | yes | VAL-proxy-heartbeat-error | unit | manual | Reporter failure branch logs and retries; no log assertion added. |
| CHG-pn-sn-heartbeat | error | yes | VAL-proxy-heartbeat-error | unit | gap | Heartbeat reporter error is logged and retried by code path; no log assertion added. |
| CHG-pn-sn-heartbeat | compatibility | yes | VAL-proxy-heartbeat | unit | covered | not-applicable: heartbeat reuses existing traffic report command |
| CHG-pn-sn-heartbeat | lifecycle | yes | VAL-proxy-heartbeat | unit | covered | not-applicable: selector TTL test covers heartbeat timeout removing remote proxy from selection |
| CHG-pn-sn-heartbeat | cross-module | yes | VAL-proxy-heartbeat | integration | manual | Uses vpn-frame report command; workspace compatibility is current coverage. |
| CHG-colocated-pn-default-allowed | normal | yes | VAL-colocated-default | unit | covered | not-applicable: selector uses local endpoint when `pn.enabled=true` |
| CHG-colocated-pn-default-allowed | boundary | yes | VAL-colocated-default | unit | covered | not-applicable: `pn.enabled=false` disables local proxy |
| CHG-colocated-pn-default-allowed | negative | no | VAL-colocated-default | unit | not-applicable | No external acceptance is required for co-located proxy. |
| CHG-colocated-pn-default-allowed | error | no | VAL-colocated-default | unit | not-applicable | Local default allowance has no new error path. |
| CHG-colocated-pn-default-allowed | compatibility | yes | VAL-colocated-default | unit | covered | not-applicable: default `pn.enabled=true` preserved |
| CHG-colocated-pn-default-allowed | lifecycle | yes | VAL-colocated-default | dv | covered | not-applicable: build covers startup assembly branch |
| CHG-colocated-pn-default-allowed | cross-module | no | VAL-colocated-default | unit | not-applicable | Behavior is local server assembly. |
| CHG-pn-traffic-db-interface | normal | yes | VAL-traffic-db-interface | unit | covered | not-applicable: non-zero delta path exists |
| CHG-pn-traffic-db-interface | boundary | yes | VAL-traffic-db-interface | unit | covered | not-applicable: zero delta heartbeat does not create parallel store |
| CHG-pn-traffic-db-interface | negative | no | VAL-traffic-db-interface | unit | not-applicable | No new negative storage input is introduced. |
| CHG-pn-traffic-db-interface | error | yes | VAL-traffic-db-interface | unit | gap | Store write failure branch is not newly asserted in this task. |
| CHG-pn-traffic-db-interface | compatibility | yes | VAL-traffic-db-interface | unit | covered | not-applicable: existing persisted stat interfaces are reused |
| CHG-pn-traffic-db-interface | lifecycle | yes | VAL-traffic-db-interface | integration | gap | Full restart persistence remains future integration work. |
| CHG-pn-traffic-db-interface | cross-module | yes | VAL-traffic-db-interface | integration | manual | Workspace-level integration is available but full persistence restart scenario is deferred. |
| CHG-local-pn-toggle-preserved | normal | yes | VAL-local-toggle | unit | covered | not-applicable: default `pn.enabled=true` covered |
| CHG-local-pn-toggle-preserved | boundary | yes | VAL-local-toggle | unit | covered | not-applicable: explicit `pn.enabled=false` covered |
| CHG-local-pn-toggle-preserved | negative | no | VAL-local-toggle | unit | not-applicable | No invalid toggle value path is added. |
| CHG-local-pn-toggle-preserved | error | no | VAL-local-toggle | unit | not-applicable | Toggle fallback preserves previous behavior. |
| CHG-local-pn-toggle-preserved | compatibility | yes | VAL-local-toggle | unit | covered | not-applicable: no-config default preserved |
| CHG-local-pn-toggle-preserved | lifecycle | yes | VAL-local-toggle | dv | covered | not-applicable: startup assembly compiles |
| CHG-local-pn-toggle-preserved | cross-module | no | VAL-local-toggle | unit | not-applicable | Toggle is local server assembly behavior. |
| CHG-server-node-id-base36 | normal | yes | VAL-server-node-id-base36 | dv | covered | not-applicable: NodeId output call sites compile with base36. |
| CHG-server-node-id-base36 | boundary | yes | VAL-server-node-id-base36 | unit | covered | not-applicable: parse helper can accept canonical NodeId text at API boundaries. |
| CHG-server-node-id-base36 | negative | yes | VAL-server-node-id-base36 | dv | manual | Malformed HTTP NodeId rejection is not directly smoked in this task. |
| CHG-server-node-id-base36 | error | yes | VAL-server-node-id-base36 | dv | manual | Database key lookup failure follows existing store error handling; no injected DB fixture added. |
| CHG-server-node-id-base36 | compatibility | yes | VAL-server-node-id-base36 | integration | manual | Old base58 SQLite migration is explicitly deferred; non-NodeId base58 password hashes remain unchanged by review. |
| CHG-server-node-id-base36 | lifecycle | no | VAL-server-node-id-base36 | unit | not-applicable | Encoding selection has no runtime lifecycle by itself. |
| CHG-server-node-id-base36 | cross-module | yes | VAL-server-node-id-base36-integration | integration | covered | Workspace compatibility covers direct consumers of server API/store contract. |
| CHG-server-identity-cert-name | normal | yes | VAL-server-identity-cert-name | unit | covered | none |
| CHG-server-identity-cert-name | boundary | yes | VAL-server-identity-cert-name | unit | covered | none |
| CHG-server-identity-cert-name | negative | no | VAL-server-identity-cert-name | unit | not-applicable | Blank names normalize to absent rather than producing a startup error. |
| CHG-server-identity-cert-name | error | yes | VAL-server-identity-cert-name-dv | dv | covered | none |
| CHG-server-identity-cert-name | compatibility | yes | VAL-server-identity-cert-name-dv | dv | covered | none |
| CHG-server-identity-cert-name | lifecycle | yes | VAL-server-identity-cert-name-dv | dv | covered | none |
| CHG-server-identity-cert-name | cross-module | yes | VAL-server-identity-cert-name-integration | integration | covered | none |
| CHG-server-proxy-node-reported-name | normal | yes | VAL-server-proxy-reported-name | unit | covered | none |
| CHG-server-proxy-node-reported-name | boundary | yes | VAL-server-proxy-reported-name | unit | covered | none |
| CHG-server-proxy-node-reported-name | negative | no | VAL-server-proxy-reported-name | unit | not-applicable | Name is optional metadata and does not reject heartbeat/report payloads. |
| CHG-server-proxy-node-reported-name | error | no | VAL-server-proxy-reported-name | unit | not-applicable | Missing name falls back to existing id behavior. |
| CHG-server-proxy-node-reported-name | compatibility | yes | VAL-server-proxy-reported-name-integration | integration | covered | none |
| CHG-server-proxy-node-reported-name | lifecycle | yes | VAL-server-proxy-reported-name | unit | covered | none |
| CHG-server-proxy-node-reported-name | cross-module | yes | VAL-server-proxy-reported-name-integration | integration | covered | none |
| CHG-pn-port-mapping-observed-address | normal | yes | VAL-server-observed-mapped-endpoint | unit | covered | none |
| CHG-pn-port-mapping-observed-address | boundary | yes | VAL-server-observed-mapped-endpoint | unit | covered | none |
| CHG-pn-port-mapping-observed-address | negative | yes | VAL-server-observed-mapped-endpoint | unit | covered | none |
| CHG-pn-port-mapping-observed-address | error | no | VAL-server-observed-mapped-endpoint | unit | not-applicable | Address synthesis is pure merge logic and introduces no new error return. |
| CHG-pn-port-mapping-observed-address | compatibility | yes | VAL-server-observed-mapped-endpoint | dv | covered | none |
| CHG-pn-port-mapping-observed-address | lifecycle | yes | VAL-server-observed-mapped-endpoint | unit | covered | none |
| CHG-pn-port-mapping-observed-address | cross-module | yes | VAL-server-observed-mapped-endpoint-integration | integration | covered | none |
| CHG-pn-server-endpoint-address-contract | normal | yes | VAL-server-pn-endpoint-contract | unit | covered | none |
| CHG-pn-server-endpoint-address-contract | boundary | yes | VAL-server-pn-endpoint-contract | unit | covered | none |
| CHG-pn-server-endpoint-address-contract | negative | no | VAL-server-pn-endpoint-contract | unit | not-applicable | Server accepts shared Endpoint values; protocol validation remains at config/API parse boundaries. |
| CHG-pn-server-endpoint-address-contract | error | yes | VAL-server-pn-endpoint-contract | unit | covered | none |
| CHG-pn-server-endpoint-address-contract | compatibility | yes | VAL-server-pn-endpoint-contract | dv | covered | none |
| CHG-pn-server-endpoint-address-contract | lifecycle | yes | VAL-server-pn-endpoint-contract | unit | covered | none |
| CHG-pn-server-endpoint-address-contract | cross-module | yes | VAL-server-pn-endpoint-contract-integration | integration | covered | none |

## Design Element Coverage
| element_type | design_source | derived_cases | level | status | gap_manual_reason |
| --- | --- | --- | --- | --- | --- |
| parameter-domain | `Data and State` config rows | default true, disabled false, valid/invalid control endpoint, `sn.http`, `sn.admin`, `sn.jwt`, and legacy compatibility | unit | covered | not-applicable: unit config coverage |
| state-transition | `Data and State` heartbeat row | alive -> timeout unavailable -> restored | unit | covered | not-applicable: `remote_proxy_heartbeat_adds_temporary_selectable_proxy` and `remote_proxy_heartbeat_expires_from_selection` cover selector liveness |
| state-transition | `Data and State` approval row | absent -> pending -> approved/rejected, approved + live selectable | unit | manual | Store/selector path compiles; direct SQLite fixture assertion deferred. |
| parameter-domain | `Interfaces and Dependencies` HTTP approval rows | list, approve and reject proxy nodes with Bearer auth | dv | manual | Route registration compiles; direct HTTP smoke deferred. |
| parameter-domain | `Interfaces and Dependencies` `observed_addr` row | observed peer WAN address present vs absent/null | dv | manual | DTO and wiring compile; direct handler/runtime assertion deferred. |
| failure-path | `Key Call Flows` heartbeat maintenance | reporter failure logs warning and retries | unit | gap | No assertion hook for log/retry in current implementation. |
| error-handling | `Key Call Flows` config parse failure | invalid control endpoint errors at startup/config parse | unit | covered | not-applicable: parser returns error |
| invariant | `Invariants to Preserve` | no config keeps local SN/PN default enabled | unit | covered | not-applicable: unit config coverage |
| concurrency | `Key Call Flows` background flush/heartbeat | heartbeat task and flush task use locked reporter state safely | dv | manual | Concurrency stress is deferred; compile plus manual review only. |
| parameter-domain | `Interfaces and Dependencies` server NodeId text | base36 NodeId request/response/key/log values; malformed values; non-NodeId base58 exclusion | dv | covered | none |
| parameter-domain | `Interfaces and Dependencies` proxy-control `TunnelPurpose` listener | dedicated proxy-control purpose vs SN command purpose; independent proxy-control command service vs `SnService` | unit | covered | `proxy_control_purpose_is_dedicated` and `proxy_control_cmd_service_is_independent_from_sn_service` cover the domain split. |
| failure-path | `Key Call Flows` pure proxy connection | wrong SN purpose or missing remote endpoint must not enter proxy-control command handling | dv | manual | Dedicated purpose is unit covered; runtime accept error and missing endpoint require TTP fixture. |
| parameter-domain | `Interfaces and Dependencies` server identity name | configured `name`, blank/absent values, and existing identity with mismatched certificate name | dv | covered | Config parsing is unit covered; re-sign path compiles and reloads through p2p-frame identity factory. |
| state-transition | `Data and State` proxy-node reported name | heartbeat/report without name -> with name -> selected/listed proxy carries latest non-empty reported name | unit | covered | Selector merge and SQLite-backed list path assertions cover preservation. |

## Validation Rationale
The lowest reliable layer for config and selector behavior is unit tests in `server_config.rs`. Heartbeat reuses the control command path and now carries the proxy endpoint used by selector liveness. Full multi-process proxy registration smoke and restart persistence tests require a broader integration harness and remain recorded as gaps for acceptance visibility.

## Unit Tests
| function_or_unit | branch_or_condition | covered_behavior | test_file | status | gap_manual_reason |
| --- | --- | --- | --- | --- | --- |
| `get_pn_server_config` | default config | local proxy defaults enabled | `vpn-server/src/server_config.rs` | covered | not-applicable: existing test updated |
| `get_pn_server_config` | `sn.control_server` valid | pure proxy control-node address parses | `vpn-server/src/server_config.rs` | covered | not-applicable: `yaml_can_configure_sn_control_server` |
| `get_pn_server_config` | legacy `pn.control_server` valid | legacy pure proxy control-node address parses | `vpn-server/src/server_config.rs` | covered | not-applicable: `legacy_pn_control_server_remains_compatible` |
| `get_sn_http_config` | `sn.http` valid | HTTP management listener parses from SN config domain | `vpn-server/src/server_config.rs` | covered | not-applicable: `yaml_can_configure_sn_owned_http_and_admin` |
| `get_sn_http_config` | legacy top-level `http` valid | legacy HTTP config remains compatible | `vpn-server/src/server_config.rs` | covered | not-applicable: `legacy_top_level_http_and_admin_remain_compatible` |
| `get_sn_admin_config` | `sn.admin` valid | admin bootstrap parses from SN config domain | `vpn-server/src/server_config.rs` | covered | not-applicable: `yaml_can_configure_sn_owned_http_and_admin` |
| `get_sn_admin_config` | legacy top-level `admin` valid | legacy admin config remains compatible | `vpn-server/src/server_config.rs` | covered | not-applicable: `legacy_top_level_http_and_admin_remain_compatible` |
| `get_sn_jwt_config` | `sn.jwt` valid | JWT signing config parses from SN config domain | `vpn-server/src/server_config.rs` | covered | not-applicable: `yaml_can_configure_sn_owned_management_config` |
| `get_sn_jwt_config` | legacy top-level `jwt` valid | legacy JWT config remains compatible | `vpn-server/src/server_config.rs` | covered | not-applicable: `legacy_top_level_management_config_remains_compatible` |
| `parse_quic_endpoint` | invalid endpoint | invalid control endpoint fails | `vpn-server/src/server_config.rs` | covered | not-applicable: via config parsing |
| `resolve_service_endpoints` | proxy enabled with no static list | no static proxy endpoint injection | `vpn-server/src/server_config.rs` | covered | not-applicable: updated test |
| `ConfigPnServerSelector::report_heartbeat` | remote proxy heartbeat | remote proxy endpoint becomes selectable before TTL | `vpn-server/src/server_config.rs` | covered | not-applicable: selector unit test |
| `ConfigPnServerSelector::is_valid/select` | remote proxy TTL expired | remote proxy endpoint is removed from selection | `vpn-server/src/server_config.rs` | covered | not-applicable: selector unit test |
| `endpoints_to_pn_server` | local listen Endpoint with configured `port_mapping` | reports local listen Endpoint ports unchanged and carries mapping metadata separately | `vpn-server/src/server_config.rs` | covered | none |
| `ConfigPnServerSelector::report_observed_heartbeat` + `report_heartbeat` | observed then reported heartbeat | selected proxy uses observed connection IP and mapped external port from separately reported port_mapping | `vpn-server/src/server_config.rs` | covered | none |
| `ConfigPnServerSelector::report_heartbeat` + `report_observed_heartbeat` | reported then observed heartbeat | selected proxy is rewritten from reported private IP/listen port to observed connection IP and mapped external port | `vpn-server/src/server_config.rs` | covered | none |
| `SqliteVpnStore` PN server fields | proxy node and network PN server rows | persists stable id/name only and returns live selector endpoints when online | `vpn-server/src/sqlite_store_factory.rs`, `vpn-server/src/server_config.rs` | covered | none |
| `get_server_name_config` | YAML `name` with surrounding whitespace | returns normalized server identity/proxy reported name | `vpn-server/src/server_config.rs` | covered | none |
| `ConfigPnServerSelector` reported name merge | reported and observed heartbeat order | selected proxy keeps the reported name while using the observed address | `vpn-server/src/server_config.rs` | covered | none |
| `ConfigPnServerSelector` store-backed reported name merge | observed then reported heartbeat | listed proxy node keeps the reported name in SQLite-backed path | `vpn-server/src/server_config.rs` | covered | none |
| `proxy_control_purpose` | dedicated purpose value | proxy-control purpose differs from SN command purpose | `vpn-server/src/vpn_control_client.rs` | covered | not-applicable: `proxy_control_purpose_is_dedicated` |
| `ProxyControlCmdService` | independent command service | proxy-control command service is constructible without `SnService` and has a distinct service type | `vpn-server/src/vpn_control_client.rs` | covered | not-applicable: `proxy_control_cmd_service_is_independent_from_sn_service` |
| `SqliteVpnStore` proxy approval methods | pending/approved/rejected states | approval state schema and store API compile | none | manual | Direct SQLite fixture test not added in this stage. |
| `ConfigPnServerSelector` with store | approved + live selection | unapproved proxy should stay out of selection | none | manual | Direct selector-with-store fixture not added in this stage. |
| `/pn_proxy_nodes` DTO mapping | observed peer WAN address present or missing | response can include `observed_addr` without changing approval state | none | manual | Direct HTTP handler/runtime fixture not added in this stage. |
| `PnTrafficService::start_remote_heartbeat` | reporter failure branch | warning and retry behavior | none | gap | Needs fake reporter/log assertion not added in this task. |
| service NodeId text helpers | API parse/output and SQLite key formatting | base36 is used for NodeId strings and password hash base58 is untouched | `vpn-server/src/api.rs`, `vpn-server/src/sqlite_store_factory.rs` | covered | none |

## DV Tests
| workflow | kind | entry | expected_result | test_file_or_script | status | gap_manual_reason |
| --- | --- | --- | --- | --- | --- | --- |
| bucky-vpn-server crate build | lifecycle | `python3 ./harness/scripts/test-run.py bucky-vpn-server dv` | crate builds after config/control/heartbeat changes | `harness/scripts/test-run.py` | covered | not-applicable: unified entry |
| control-node assembly | main | `python3 ./harness/scripts/test-run.py bucky-vpn-server dv` | main assembly compiles with local and remote proxy branches | `harness/scripts/test-run.py` | covered | not-applicable: build-level DV |
| proxy approval API registration | main | `python3 ./harness/scripts/test-run.py bucky-vpn-server dv` | HTTP control plane compiles with selector-backed approval routes | `harness/scripts/test-run.py` | covered | not-applicable: build-level DV |
| proxy observed address response | main | `python3 ./harness/scripts/test-run.py bucky-vpn-server dv` | `/pn_proxy_nodes` response compiles with observed address field and runtime peer WAN lookup wiring | `harness/scripts/test-run.py` | covered | not-applicable: build-level DV |
| pure proxy control failure | failure | `python3 ./harness/scripts/test-run.py bucky-vpn-server dv` | missing control server branch remains non-panicking compile path | `harness/scripts/test-run.py` | covered | not-applicable: branch compiles |
| proxy-control dedicated listener | main | `python3 ./harness/scripts/test-run.py bucky-vpn-server dv` | control node assembly compiles with proxy-control listener on SN TTP server and independent proxy-control command service | `harness/scripts/test-run.py` | covered | not-applicable: build-level DV |
| server NodeId base36 contract | main | `python3 ./harness/scripts/test-run.py bucky-vpn-server dv` | server crate builds after replacing NodeId base58 output/key call sites with base36 | `harness/scripts/test-run.py` | covered | none |

## Integration Tests
| contract_or_flow | modules_involved | success_case | failure_case | test_file | status | gap_manual_reason |
| --- | --- | --- | --- | --- | --- | --- |
| workspace compatibility | bucky-vpn-server, vpn-frame, bucky-vpn | workspace test command passes | compile/API contract mismatch fails | `harness/scripts/test-run.py` | covered | not-applicable: unified integration entry |
| external proxy lifecycle | bucky-vpn-server, vpn-frame, p2p-frame | external proxy reports endpoint heartbeat and selector admits it | heartbeat timeout removes selection | `vpn-server/src/server_config.rs` unit tests plus workspace test | manual | Selector lifecycle is unit covered; full multi-process runtime harness remains future work. |
| proxy approval lifecycle | bucky-vpn-server, SQLite, HTTP API | admin approves proxy and approved + live proxy becomes selectable | rejected or unapproved proxy is not selected | none | gap | Needs HTTP/runtime fixture and SQLite restart fixture. |
| durable traffic persistence | bucky-vpn-server, SQLite | restart continues cumulative values | write failure does not advance baseline | none | gap | Requires restart fixture not present in this task. |
| server NodeId base36 compatibility | bucky-vpn-server, vpn-frame, bucky-vpn, vpn_web | workspace/front-end consumers use base36 NodeId strings consistently | stale base58-only consumer fails build or manual review | `harness/scripts/test-run.py` | covered | none |

## Definition of Done
- Proposal and design are approved and traceable by `change_id`.
- Production code compiles with `cargo check -p bucky-vpn-server`.
- `testplan.yaml` maps every changed `change_id` to unified test entries.
- Unit validation is run through `python3 ./harness/scripts/test-run.py bucky-vpn-server unit`.
- Known integration gaps are explicit in this testing document.

## Approval Record
- approver: auto-pipeline
- approval_date: 2026-07-07T00:41:31+08:00
- user_statement: "确认，自动处理后续步骤"
