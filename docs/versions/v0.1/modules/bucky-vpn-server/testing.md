---
module: bucky-vpn-server
version: v0.1
status: approved
approved_by: auto-pipeline
approved_at: 2026-06-25
approved_content_sha256: 9582f003bd7d374d77fc96de0d090453ab232fd390897201dcda93989fa656c1
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
| `server-config` | 解析本地开关和纯代理节点控制节点地址，移除静态外部代理地址合同。 | 默认启用、禁用 SN/PN、`pn.control_server` 解析、不再注入 `pn.server_addresses`。 | unit | `vpn-server/src/server_config.rs` | none |
| `process-assembly` | 组装控制节点、内置代理节点、外部代理控制 client 和统计服务。 | `pn.enabled=true` 时内置代理默认可选；纯代理节点连接控制节点后启动心跳。 | dv | `python3 ./harness/scripts/test-run.py bucky-vpn-server dv` | runtime branch covered by compile/DV only; full network smoke deferred |
| `control-node-control` | 管理外部代理节点批准、心跳 liveness 和选择状态。 | 使用控制命令上报 endpoint heartbeat，控制节点 selector 只选择 approved + live 的远端代理。 | unit/dv | `vpn-server/src/server_config.rs`, `vpn-server/src/pn_traffic_service.rs`, dv build | full multi-process smoke deferred |
| `sqlite-persistence` | 持久化外部代理节点批准状态。 | `pn_proxy_node` schema、pending/approved/rejected 状态和旧库迁移可编译。 | unit/dv | `cargo check -p bucky-vpn-server`, `python3 ./harness/scripts/test-run.py bucky-vpn-server unit` | direct SQLite restart fixture deferred |
| `http-api` | 暴露外部代理节点列表、批准和拒绝接口。 | 新接口复用 Bearer session，注册到 HTTP 控制面。 | dv/integration | `cargo check -p bucky-vpn-server`, workspace test | direct HTTP smoke deferred |
| `traffic-statistics` | 通过 SQLite-backed 接口持久化代理流量。 | 真实 delta 和 heartbeat zero delta 复用 remote reporter / store 接口。 | unit/integration | `vpn-server/src/pn_traffic_service.rs`, integration entry | no full restart test in this task |

## Module-Level Tests
| behavior | validation | entry | expected_result |
| --- | --- | --- | --- |
| Config contract | Rust unit tests in `server_config.rs` | `python3 ./harness/scripts/test-run.py bucky-vpn-server unit` | config tests pass and crate tests compile |
| Server assembly | crate build | `python3 ./harness/scripts/test-run.py bucky-vpn-server dv` | `bucky-vpn-server` builds |
| Workspace compatibility | workspace test | `python3 ./harness/scripts/test-run.py bucky-vpn-server integration` | workspace tests pass or failures are recorded |

## External Interface Tests
| interface | success_case | failure_case | validation |
| --- | --- | --- | --- |
| YAML config | Missing optional PN fields keeps local default; `pn.control_server` supplies pure proxy control-node address. | Invalid control endpoint fails parsing. | unit config tests |
| External proxy control channel | Pure proxy node can create control client and use control commands. | Missing `pn.control_server` while SN disabled logs warning and rejects proxy connections. | dv build plus future integration |
| Heartbeat over traffic report | Remote reporter sends zero delta heartbeat with proxy endpoint through existing command; selector admits and expires remote proxy by TTL. | Reporter error logs warning and next tick retries. | unit selector TTL tests plus dv build |
| HTTP proxy approval API | Authenticated users can list, approve and reject proxy nodes. | Missing/invalid Bearer token is rejected by existing session decode path. | dv compile plus future HTTP smoke |
| Traffic persistence | Non-zero deltas write through existing SQLite-backed interface. | Zero heartbeat does not create local parallel store. | unit/integration future |

## Direct Change Coverage
| change_id | design_source | validation_id | testplan_level | testplan_step_id | gap | gap_manual_reason |
| --- | --- | --- | --- | --- | --- | --- |
| CHG-pn-config-no-static-addresses | `Directly Mapped Change Items` | VAL-config-no-static-addresses | unit | bucky-vpn-server-unit | no | not-applicable: covered by config tests and compile |
| CHG-external-pn-active-control | `Directly Mapped Change Items` | VAL-external-proxy-control | unit | bucky-vpn-server-unit | no | Selector registration/expiry is covered by unit tests; full multi-process registration integration is deferred. |
| CHG-external-pn-approval-persistence | `Directly Mapped Change Items` | VAL-proxy-approval-persistence | unit | bucky-vpn-server-unit | no | `pn_proxy_node` schema and store/selector integration compile; direct restart fixture is deferred. |
| CHG-external-pn-approval-http-api | `Directly Mapped Change Items` | VAL-proxy-approval-http-api | dv | bucky-vpn-server-dv | no | API route registration and request/response types compile; direct HTTP smoke is deferred. |
| CHG-pure-pn-sn-address | `Directly Mapped Change Items` | VAL-pure-proxy-control-address | unit | bucky-vpn-server-unit | no | not-applicable: covered by `pn.control_server` parsing test |
| CHG-pn-sn-heartbeat | `Directly Mapped Change Items` | VAL-proxy-heartbeat | unit | bucky-vpn-server-unit | no | Heartbeat endpoint liveness is covered by selector TTL tests; reporter error log assertion remains deferred. |
| CHG-colocated-pn-default-allowed | `Directly Mapped Change Items` | VAL-colocated-default | unit | bucky-vpn-server-unit | no | not-applicable: selector uses local endpoint when `pn.enabled=true` |
| CHG-pn-traffic-db-interface | `Directly Mapped Change Items` | VAL-traffic-db-interface | unit | bucky-vpn-server-unit | no | Existing storage path is covered; full restart persistence remains integration follow-up. |
| CHG-local-pn-toggle-preserved | `Directly Mapped Change Items` | VAL-local-toggle | unit | bucky-vpn-server-unit | no | not-applicable: config tests cover default and disabled paths |

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
| CHG-pure-pn-sn-address | normal | yes | VAL-pure-proxy-control-address | unit | covered | not-applicable: valid endpoint parsed |
| CHG-pure-pn-sn-address | boundary | yes | VAL-pure-proxy-control-address | unit | covered | not-applicable: IPv4 endpoint parsing covered |
| CHG-pure-pn-sn-address | negative | yes | VAL-pure-proxy-control-address-invalid | unit | covered | not-applicable: invalid endpoint parse fails |
| CHG-pure-pn-sn-address | error | yes | VAL-pure-proxy-control-address-invalid | unit | covered | not-applicable: parse error is returned |
| CHG-pure-pn-sn-address | compatibility | yes | VAL-pure-proxy-control-address | unit | covered | not-applicable: `pn.control_server` existing shape is preserved |
| CHG-pure-pn-sn-address | lifecycle | no | VAL-pure-proxy-control-address | unit | not-applicable | Address parsing has no lifecycle by itself. |
| CHG-pure-pn-sn-address | cross-module | no | VAL-pure-proxy-control-address | unit | not-applicable | Parsed config is consumed by server assembly only. |
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

## Design Element Coverage
| element_type | design_source | derived_cases | level | status | gap_manual_reason |
| --- | --- | --- | --- | --- | --- |
| parameter-domain | `Data and State` config rows | default true, disabled false, valid/invalid control endpoint | unit | covered | not-applicable: unit config coverage |
| state-transition | `Data and State` heartbeat row | alive -> timeout unavailable -> restored | unit | covered | not-applicable: `remote_proxy_heartbeat_adds_temporary_selectable_proxy` and `remote_proxy_heartbeat_expires_from_selection` cover selector liveness |
| state-transition | `Data and State` approval row | absent -> pending -> approved/rejected, approved + live selectable | unit | manual | Store/selector path compiles; direct SQLite fixture assertion deferred. |
| parameter-domain | `Interfaces and Dependencies` HTTP approval rows | list, approve and reject proxy nodes with Bearer auth | dv | manual | Route registration compiles; direct HTTP smoke deferred. |
| failure-path | `Key Call Flows` heartbeat maintenance | reporter failure logs warning and retries | unit | gap | No assertion hook for log/retry in current implementation. |
| error-handling | `Key Call Flows` config parse failure | invalid control endpoint errors at startup/config parse | unit | covered | not-applicable: parser returns error |
| invariant | `Invariants to Preserve` | no config keeps local SN/PN default enabled | unit | covered | not-applicable: unit config coverage |
| concurrency | `Key Call Flows` background flush/heartbeat | heartbeat task and flush task use locked reporter state safely | dv | manual | Concurrency stress is deferred; compile plus manual review only. |

## Validation Rationale
The lowest reliable layer for config and selector behavior is unit tests in `server_config.rs`. Heartbeat reuses the control command path and now carries the proxy endpoint used by selector liveness. Full multi-process proxy registration smoke and restart persistence tests require a broader integration harness and remain recorded as gaps for acceptance visibility.

## Unit Tests
| function_or_unit | branch_or_condition | covered_behavior | test_file | status | gap_manual_reason |
| --- | --- | --- | --- | --- | --- |
| `get_pn_server_config` | default config | local proxy defaults enabled | `vpn-server/src/server_config.rs` | covered | not-applicable: existing test updated |
| `get_pn_server_config` | `pn.control_server` valid | pure proxy control-node address parses | `vpn-server/src/server_config.rs` | covered | not-applicable: existing test |
| `parse_quic_endpoint` | invalid endpoint | invalid control endpoint fails | `vpn-server/src/server_config.rs` | covered | not-applicable: via config parsing |
| `resolve_service_endpoints` | proxy enabled with no static list | no static proxy endpoint injection | `vpn-server/src/server_config.rs` | covered | not-applicable: updated test |
| `ConfigPnServerSelector::report_heartbeat` | remote proxy heartbeat | remote proxy endpoint becomes selectable before TTL | `vpn-server/src/server_config.rs` | covered | not-applicable: selector unit test |
| `ConfigPnServerSelector::is_valid/select` | remote proxy TTL expired | remote proxy endpoint is removed from selection | `vpn-server/src/server_config.rs` | covered | not-applicable: selector unit test |
| `SqliteVpnStore` proxy approval methods | pending/approved/rejected states | approval state schema and store API compile | none | manual | Direct SQLite fixture test not added in this stage. |
| `ConfigPnServerSelector` with store | approved + live selection | unapproved proxy should stay out of selection | none | manual | Direct selector-with-store fixture not added in this stage. |
| `PnTrafficService::start_remote_heartbeat` | reporter failure branch | warning and retry behavior | none | gap | Needs fake reporter/log assertion not added in this task. |

## DV Tests
| workflow | kind | entry | expected_result | test_file_or_script | status | gap_manual_reason |
| --- | --- | --- | --- | --- | --- | --- |
| bucky-vpn-server crate build | lifecycle | `python3 ./harness/scripts/test-run.py bucky-vpn-server dv` | crate builds after config/control/heartbeat changes | `harness/scripts/test-run.py` | covered | not-applicable: unified entry |
| control-node assembly | main | `python3 ./harness/scripts/test-run.py bucky-vpn-server dv` | main assembly compiles with local and remote proxy branches | `harness/scripts/test-run.py` | covered | not-applicable: build-level DV |
| proxy approval API registration | main | `python3 ./harness/scripts/test-run.py bucky-vpn-server dv` | HTTP control plane compiles with selector-backed approval routes | `harness/scripts/test-run.py` | covered | not-applicable: build-level DV |
| pure proxy control failure | failure | `python3 ./harness/scripts/test-run.py bucky-vpn-server dv` | missing control server branch remains non-panicking compile path | `harness/scripts/test-run.py` | covered | not-applicable: branch compiles |

## Integration Tests
| contract_or_flow | modules_involved | success_case | failure_case | test_file | status | gap_manual_reason |
| --- | --- | --- | --- | --- | --- | --- |
| workspace compatibility | bucky-vpn-server, vpn-frame, bucky-vpn | workspace test command passes | compile/API contract mismatch fails | `harness/scripts/test-run.py` | covered | not-applicable: unified integration entry |
| external proxy lifecycle | bucky-vpn-server, vpn-frame, p2p-frame | external proxy reports endpoint heartbeat and selector admits it | heartbeat timeout removes selection | `vpn-server/src/server_config.rs` unit tests plus workspace test | manual | Selector lifecycle is unit covered; full multi-process runtime harness remains future work. |
| proxy approval lifecycle | bucky-vpn-server, SQLite, HTTP API | admin approves proxy and approved + live proxy becomes selectable | rejected or unapproved proxy is not selected | none | gap | Needs HTTP/runtime fixture and SQLite restart fixture. |
| durable traffic persistence | bucky-vpn-server, SQLite | restart continues cumulative values | write failure does not advance baseline | none | gap | Requires restart fixture not present in this task. |

## Definition of Done
- Proposal and design are approved and traceable by `change_id`.
- Production code compiles with `cargo check -p bucky-vpn-server`.
- `testplan.yaml` maps every changed `change_id` to unified test entries.
- Unit validation is run through `python3 ./harness/scripts/test-run.py bucky-vpn-server unit`.
- Known integration gaps are explicit in this testing document.
