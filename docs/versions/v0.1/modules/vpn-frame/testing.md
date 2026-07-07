---
module: vpn-frame
version: v0.1
status: approved
approved_by: auto-pipeline
approved_at: 2026-07-07T00:41:31+08:00
approved_content_sha256: d8a3aa8b94779181fb96c621aba72fa46819d1c1654df40d040d540d63e03230
---

# vpn-frame Testing

## Test Document Index
| document | topic | scope |
|----------|-------|-------|
| `testing.md` | PN server info protocol/runtime/storage validation | full module |
| `testplan.yaml` | Machine-readable harness entries | full module |

## Unified Test Entry
- Machine-readable plan: `docs/versions/v0.1/modules/vpn-frame/testplan.yaml`
- Unified runner: `harness/scripts/test-run.py`
- Unit: `python3 ./harness/scripts/test-run.py vpn-frame unit`
- DV: `python3 ./harness/scripts/test-run.py vpn-frame dv`
- Integration: `python3 ./harness/scripts/test-run.py vpn-frame integration`

## Submodule Tests
| submodule | responsibility | detailed_testing_doc | required_behavior | boundary_failure_cases | test_type | test_file |
|-----------|----------------|----------------------|-------------------|------------------------|-----------|-----------|
| protocol | Shared protocol type | none | `PnServerInfo` carries id, Endpoint values, and optional report-time port_mapping without endpoint-string identity helpers. | absent PN server remains `None`; port_mapping does not rewrite local Endpoint ports | unit | `vpn-frame/src/vpn_protocol.rs` |
| protocol-name | Shared proxy display/certificate name metadata | none | `PnServerInfo.name` carries an optional reported proxy name and `remote_name()` falls back to id when absent. | blank names normalize to absent | unit | `vpn-frame/src/vpn_protocol.rs` |
| server-runtime | Builds `NodeNetwork` values and receives heartbeat reports | none | selected and reported PN servers are structured values whose id is the server P2P node id. | selector returns none or rejects stale value | dv | `harness/scripts/test-run.py` |
| persistence | Stores stable PN server identity/policy while endpoints stay live | none | network and proxy-node rows no longer treat transport endpoints as SQLite truth. | live endpoint is unavailable until a proxy reports/refreshes it | unit/dv | `vpn-server/src/sqlite_store_factory.rs`, harness runner |
| api | Exposes server-side PN server JSON models | none | proxy-node and network APIs use Endpoint-shaped JSON values. | invalid endpoint protocol is rejected by consumers at the p2p boundary | integration | `vpn-server/src/api.rs`, harness runner |
| client-runtime | Consumes `NodeNetwork.pn_server` | none | client code consumes server-returned Endpoint values directly at the P2P boundary. | absent PN info is skipped | integration | `harness/scripts/test-run.py` |
| node-id-contract | Shared NodeId text helpers and runtime logs | none | base36 is the canonical NodeId external text output while raw bytes/codecs stay unchanged. | legacy base58 text is not used for new output | unit/dv | `vpn-frame/src/server/node_store.rs`, `harness/scripts/test-run.py` |

## Module-Level Tests
| test_item | coverage_boundary | entry | expected_result | test_type | test_file_or_script |
|-----------|-------------------|-------|-----------------|-----------|---------------------|
| vpn-frame unit | Protocol and focused storage tests | `python3 ./harness/scripts/test-run.py vpn-frame unit` | cargo test for vpn-frame exits 0 | automated | `harness/scripts/test-run.py` |
| vpn-frame dv | Single crate build with direct runtime consumers | `python3 ./harness/scripts/test-run.py vpn-frame dv` | cargo build for vpn-frame exits 0 | automated | `harness/scripts/test-run.py` |
| vpn-frame integration | Workspace consumers compile/test with structured PN info | `python3 ./harness/scripts/test-run.py vpn-frame integration` | cargo test workspace exits 0 | automated | `harness/scripts/test-run.py` |

## External Interface Tests
| interface | responsibility | success_case | failure_boundary_case | test_type | test_doc_or_file |
|-----------|----------------|--------------|-----------------------|-----------|------------------|
| `NodeNetwork.pn_server` | Shared protocol field consumed by client and server binaries | valid selector produces `PnServerInfo` with P2P node id and Endpoint values | no valid selector produces `None` | unit/integration | `vpn-frame/src/vpn_protocol.rs`, harness integration |
| `ReportPnTrafficStatsReq.pn_server` | Heartbeat/report command payload | remote PN server report carries structured id/Endpoint plus optional port_mapping metadata | absent PN server skips selector heartbeat | integration | harness integration |
| `PnServerInfo.name` | Optional reported proxy name consumed by server/client binaries | non-empty name is preserved as PN server metadata | blank or absent name falls back to `PnServerInfo.id` for remote connection name | unit/integration | `vpn-frame/src/vpn_protocol.rs`, harness integration |
| SQLite PN server fields | Store selected and approved PN server identity/policy | network/proxy-node rows bind id/name while endpoint columns are not address truth | stale endpoint columns are ignored in favor of live selector state | unit/dv | `vpn-server/src/sqlite_store_factory.rs`, harness runner |
| HTTP API PN server JSON | Server-side admin API data model | list/approve/reject bodies use Endpoint-shaped objects | invalid endpoint protocol fails at consumer conversion | integration | `vpn-server/src/api.rs`, harness runner |
| NodeId external text | Shared NodeId helper contract | `NodeId` round-trips through base36 and runtime call sites emit base36 | old base58 output is not accepted as canonical output | unit/dv | `vpn-frame/src/server/node_store.rs`, harness runner |

## Direct Change Coverage
| change_id | design_source | validation_id | testplan_level | testplan_step_id | gap | gap_manual_reason |
|-----------|---------------|---------------|----------------|------------------|-----|-------------------|
| CHG-pn-server-info-contract | design.md Directly Mapped Change Items | VAL-pn-server-info-unit | unit | vpn-frame-unit | no | none |
| CHG-node-id-base36-contract | design.md Directly Mapped Change Items | VAL-node-id-base36-contract | unit | vpn-frame-unit | no | none |
| CHG-pn-server-reported-name-contract | design.md Directly Mapped Change Items | VAL-pn-server-reported-name-unit | unit | vpn-frame-unit | no | none |
| CHG-pn-server-endpoint-address-contract | design.md Directly Mapped Change Items | VAL-pn-server-endpoint-address-unit | unit | vpn-frame-unit | no | none |
| CHG-pn-server-address-live-state-contract | design.md Directly Mapped Change Items | VAL-pn-server-live-address-integration | integration | vpn-frame-integration | no | none |

## Case-Type Coverage
| change_id | case_type | required | validation_id | level | status | gap_manual_reason |
|-----------|-----------|----------|---------------|-------|--------|-------------------|
| CHG-pn-server-info-contract | normal | yes | VAL-pn-server-info-unit | unit | covered | none |
| CHG-pn-server-info-contract | boundary | yes | VAL-pn-server-info-unit | unit | covered | none |
| CHG-pn-server-info-contract | negative | yes | VAL-pn-server-info-unit | unit | covered | none |
| CHG-pn-server-info-contract | error | yes | VAL-pn-server-info-dv | dv | covered | none |
| CHG-pn-server-info-contract | compatibility | yes | VAL-pn-server-info-integration | integration | covered | none |
| CHG-pn-server-info-contract | lifecycle | yes | VAL-pn-server-info-dv | dv | covered | none |
| CHG-pn-server-info-contract | cross-module | yes | VAL-pn-server-info-integration | integration | covered | none |
| CHG-node-id-base36-contract | normal | yes | VAL-node-id-base36-contract | unit | covered | none |
| CHG-node-id-base36-contract | boundary | yes | VAL-node-id-base36-contract | unit | covered | none |
| CHG-node-id-base36-contract | negative | yes | VAL-node-id-base36-dv | dv | covered | none |
| CHG-node-id-base36-contract | error | yes | VAL-node-id-base36-dv | dv | covered | none |
| CHG-node-id-base36-contract | compatibility | yes | VAL-node-id-base36-integration | integration | manual | Old base58 text is only a legacy read concern in owning modules; vpn-frame defines the canonical helper. |
| CHG-node-id-base36-contract | lifecycle | no | VAL-node-id-base36-contract | unit | not-applicable | Text encoding has no runtime lifecycle by itself. |
| CHG-node-id-base36-contract | cross-module | yes | VAL-node-id-base36-integration | integration | covered | none |
| CHG-pn-server-reported-name-contract | normal | yes | VAL-pn-server-reported-name-unit | unit | covered | none |
| CHG-pn-server-reported-name-contract | boundary | yes | VAL-pn-server-reported-name-unit | unit | covered | none |
| CHG-pn-server-reported-name-contract | negative | no | VAL-pn-server-reported-name-unit | unit | not-applicable | Name is optional metadata and introduces no reject path. |
| CHG-pn-server-reported-name-contract | error | no | VAL-pn-server-reported-name-unit | unit | not-applicable | Blank names normalize to absent instead of returning an error. |
| CHG-pn-server-reported-name-contract | compatibility | yes | VAL-pn-server-reported-name-integration | integration | covered | none |
| CHG-pn-server-reported-name-contract | lifecycle | no | VAL-pn-server-reported-name-unit | unit | not-applicable | Name metadata has no independent lifecycle in vpn-frame. |
| CHG-pn-server-reported-name-contract | cross-module | yes | VAL-pn-server-reported-name-integration | integration | covered | none |
| CHG-pn-server-endpoint-address-contract | normal | yes | VAL-pn-server-endpoint-address-unit | unit | covered | none |
| CHG-pn-server-endpoint-address-contract | boundary | yes | VAL-pn-server-endpoint-address-unit | unit | covered | none |
| CHG-pn-server-endpoint-address-contract | negative | no | VAL-pn-server-endpoint-address-unit | unit | not-applicable | Endpoint values are accepted as structured data; protocol validation happens at p2p boundary consumers. |
| CHG-pn-server-endpoint-address-contract | error | no | VAL-pn-server-endpoint-address-unit | unit | not-applicable | Shared Endpoint construction does not introduce a new error return. |
| CHG-pn-server-endpoint-address-contract | compatibility | yes | VAL-pn-server-endpoint-address-integration | integration | covered | none |
| CHG-pn-server-endpoint-address-contract | lifecycle | no | VAL-pn-server-endpoint-address-unit | unit | not-applicable | Endpoint value shape has no independent runtime lifecycle. |
| CHG-pn-server-endpoint-address-contract | cross-module | yes | VAL-pn-server-endpoint-address-integration | integration | covered | none |
| CHG-pn-server-address-live-state-contract | normal | yes | VAL-pn-server-live-address-integration | integration | covered | none |
| CHG-pn-server-address-live-state-contract | boundary | yes | VAL-pn-server-live-address-integration | integration | covered | none |
| CHG-pn-server-address-live-state-contract | negative | yes | VAL-pn-server-live-address-integration | integration | covered | none |
| CHG-pn-server-address-live-state-contract | error | yes | VAL-pn-server-live-address-integration | integration | covered | none |
| CHG-pn-server-address-live-state-contract | compatibility | yes | VAL-pn-server-live-address-integration | integration | covered | none |
| CHG-pn-server-address-live-state-contract | lifecycle | yes | VAL-pn-server-live-address-integration | integration | covered | none |
| CHG-pn-server-address-live-state-contract | cross-module | yes | VAL-pn-server-live-address-integration | integration | covered | none |

## Design Element Coverage
| element_type | design_source | derived_cases | level | status | gap_manual_reason |
|--------------|---------------|---------------|-------|--------|-------------------|
| parameter-domain | design.md Interfaces and Dependencies | id string, IPv4 Endpoint, IPv6 Endpoint, port range | unit | covered | none |
| state-transition | design.md Data and State | selected none to tuple, tuple to none, proxy-node missing to pending to approved/rejected | dv | covered | none |
| failure-path | design.md Key Call Flows | no valid selector sends none; missing approval returns false; connect failure remains existing `VpnResult` | dv | covered | none |
| error-handling | design.md Key Call Flows | database errors propagate through existing `VpnResult`; P2P connect errors remain at boundary | dv | covered | none |
| invariant | design.md Invariants to Preserve | approval, membership authorization, raw codec/serde derive preserved by build/test | integration | covered | none |
| concurrency | design.md Overall Approach | selector heartbeat map keeps existing mutex shape | dv | covered | none |
| parameter-domain | design.md Interfaces and Dependencies | HTTP API request/response compile with structured PN server JSON | integration | covered | none |
| parameter-domain | design.md Interfaces and Dependencies | NodeId base36 encode/decode and no base58 output call sites in vpn-frame runtime consumers | dv | covered | none |
| parameter-domain | design.md Interfaces and Dependencies | `PnServerInfo.name` accepts non-empty text and blank/absent values fall back to id | unit | covered | none |

## Validation Rationale
The lowest useful validation for the protocol struct is unit level. Structured SQLite binding deserves focused test coverage when practical because it is the main data-shape change. DV build validation catches crate-local producer and consumer type drift. Integration validation is required because `bucky-vpn` and `bucky-vpn-server` consume the shared protocol field.

## Unit Tests
| function_or_unit | branch_or_condition | covered_behavior | test_file | status | gap_manual_reason |
|------------------|---------------------|------------------|-----------|--------|-------------------|
| `PnServerInfo::new` | valid id/ip/port compatibility constructor | constructs Endpoint values without endpoint-string parsing | `vpn-frame/src/vpn_protocol.rs` | covered | none |
| `PnServerInfo::with_name` / `remote_name` | non-empty and blank optional names | trims reported names and falls back to id when absent | `vpn-frame/src/vpn_protocol.rs` | covered | none |
| `PnServerInfo::new` / `primary_endpoint` | IPv4 and IPv6 endpoint values | stores PN address as Endpoint values rather than split `ip`/`port` fields | `vpn-frame/src/vpn_protocol.rs` | covered | none |
| `PnServerInfo::with_port_mapping` | non-empty mapping with local Endpoint port | stores report-time port_mapping while preserving the local Endpoint listen port | `vpn-frame/src/vpn_protocol.rs` | covered | none |
| `PnServerInfo::new_with_endpoints` / `add_endpoint` | duplicate endpoint list entries | deduplicates Endpoint values while preserving protocol/address/port together | `vpn-frame/src/vpn_protocol.rs`, `vpn-client/src/p2p_vpn.rs` | covered | none |
| SQLite PN server binding | network/proxy-node structured rows | stores and reads id/name while live endpoints come from selector state | `vpn-server/src/sqlite_store_factory.rs` | covered | none |
| old endpoint compatibility | old endpoint string | no parser/helper accepts old endpoint strings as PN server data | compile/build contract | covered | none |
| `NodeId::to_base36` / `NodeId::from_base36` | valid and invalid text | round-trips canonical NodeId text and rejects malformed values | `vpn-frame/src/server/node_store.rs` | covered | none |

## DV Tests
| workflow | kind | entry | expected_result | test_file_or_script | status | gap_manual_reason |
|----------|------|-------|-----------------|---------------------|--------|-------------------|
| protocol crate unit workflow | main | `python3 ./harness/scripts/test-run.py vpn-frame unit` | unit tests pass | `harness/scripts/test-run.py` | covered | none |
| protocol crate build lifecycle | lifecycle | `python3 ./harness/scripts/test-run.py vpn-frame dv` | crate builds with changed protocol type | `harness/scripts/test-run.py` | covered | none |
| structured storage build | failure | `python3 ./harness/scripts/test-run.py vpn-frame integration` | workspace compile fails if endpoint-string storage remains in direct consumers | `harness/scripts/test-run.py` | covered | none |
| NodeId base36 build | main | `python3 ./harness/scripts/test-run.py vpn-frame dv` | crate build fails if vpn-frame runtime still depends on base58 NodeId output | `harness/scripts/test-run.py` | covered | none |

## Integration Tests
| contract_or_flow | modules_involved | success_case | failure_case | test_file | status | gap_manual_reason |
|------------------|------------------|--------------|--------------|-----------|--------|-------------------|
| shared protocol consumer compatibility | `vpn-frame`, `bucky-vpn`, `bucky-vpn-server` | workspace tests compile consumers of structured PN server data | compile failure or test failure exposes stale `Option<String>` usage | `harness/scripts/test-run.py` | covered | none |
| NodeId base36 consumer compatibility | `vpn-frame`, `bucky-vpn`, `bucky-vpn-server`, `vpn_web` | direct consumers compile after switching NodeId text operations to base36 | stale base58-only NodeId operation fails review/build checks | `harness/scripts/test-run.py` | covered | none |

## Definition of Done
- [x] Testing metadata maps `CHG-pn-server-info-contract` to unit, DV, and integration validation.
- [x] Case-type coverage records required protocol, data, runtime, lifecycle, and cross-module cases.
- [x] Unified test entrypoint is referenced by every automated validation row.
- [x] Required validation commands have produced fresh run artifacts after the structured-storage implementation.

## Approval Record
- approver: auto-pipeline
- approval_date: 2026-07-07T00:41:31+08:00
- user_statement: "确认，自动处理后续步骤"
