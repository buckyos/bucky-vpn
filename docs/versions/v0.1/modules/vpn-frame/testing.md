---
module: vpn-frame
version: v0.1
status: approved
approved_by: user-request
approved_at: 2026-07-01T17:44:43+08:00
approved_content_sha256: a473f0725d5821e882069f3128241f017f7a758cf473c6280481a7df08eba5ee
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
| protocol | Shared protocol type | none | `PnServerInfo` carries id, ip, and port without endpoint-string identity helpers. | absent PN server remains `None` | unit | `vpn-frame/src/vpn_protocol.rs` |
| server-runtime | Builds `NodeNetwork` values and receives heartbeat reports | none | selected and reported PN servers are structured values whose id is the server P2P node id. | selector returns none or rejects stale value | dv | `harness/scripts/test-run.py` |
| persistence | Stores network and proxy-node PN server data | none | network and proxy-node rows bind PN server id/ip/port separately. | old endpoint-string data is unsupported | unit/dv | `vpn-server/src/sqlite_store_factory.rs`, harness runner |
| api | Exposes server-side PN server JSON models | none | proxy-node and network APIs use id/ip/port JSON objects. | endpoint-string request bodies no longer match server models | integration | `vpn-server/src/api.rs`, harness runner |
| client-runtime | Consumes `NodeNetwork.pn_server` | none | client code derives a P2P endpoint from ip/port only at the P2P boundary. | absent PN info is skipped | integration | `harness/scripts/test-run.py` |
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
| `NodeNetwork.pn_server` | Shared protocol field consumed by client and server binaries | valid selector produces `PnServerInfo` with P2P node id, ip, and port | no valid selector produces `None` | unit/integration | `vpn-frame/src/vpn_protocol.rs`, harness integration |
| `ReportPnTrafficStatsReq.pn_server` | Heartbeat/report command payload | remote PN server report carries structured id/ip/port | absent PN server skips selector heartbeat | integration | harness integration |
| SQLite PN server fields | Store selected and approved PN servers | network/proxy-node rows bind id/ip/port columns | endpoint-string columns are not read as compatible data | unit/dv | `vpn-server/src/sqlite_store_factory.rs`, harness runner |
| HTTP API PN server JSON | Server-side admin API data model | list/approve/reject bodies use id/ip/port objects | endpoint-string request body is not supported | integration | `vpn-server/src/api.rs`, harness runner |
| NodeId external text | Shared NodeId helper contract | `NodeId` round-trips through base36 and runtime call sites emit base36 | old base58 output is not accepted as canonical output | unit/dv | `vpn-frame/src/server/node_store.rs`, harness runner |

## Direct Change Coverage
| change_id | design_source | validation_id | testplan_level | testplan_step_id | gap | gap_manual_reason |
|-----------|---------------|---------------|----------------|------------------|-----|-------------------|
| CHG-pn-server-info-contract | design.md Directly Mapped Change Items | VAL-pn-server-info-unit | unit | vpn-frame-unit | no | none |
| CHG-node-id-base36-contract | design.md Directly Mapped Change Items | VAL-node-id-base36-contract | unit | vpn-frame-unit | no | none |

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

## Design Element Coverage
| element_type | design_source | derived_cases | level | status | gap_manual_reason |
|--------------|---------------|---------------|-------|--------|-------------------|
| parameter-domain | design.md Interfaces and Dependencies | id string, IPv4, IPv6, port range | unit | covered | none |
| state-transition | design.md Data and State | selected none to tuple, tuple to none, proxy-node missing to pending to approved/rejected | dv | covered | none |
| failure-path | design.md Key Call Flows | no valid selector sends none; missing approval returns false; connect failure remains existing `VpnResult` | dv | covered | none |
| error-handling | design.md Key Call Flows | database errors propagate through existing `VpnResult`; P2P connect errors remain at boundary | dv | covered | none |
| invariant | design.md Invariants to Preserve | approval, membership authorization, raw codec/serde derive preserved by build/test | integration | covered | none |
| concurrency | design.md Overall Approach | selector heartbeat map keeps existing mutex shape | dv | covered | none |
| parameter-domain | design.md Interfaces and Dependencies | HTTP API request/response compile with structured PN server JSON | integration | covered | none |
| parameter-domain | design.md Interfaces and Dependencies | NodeId base36 encode/decode and no base58 output call sites in vpn-frame runtime consumers | dv | covered | none |

## Validation Rationale
The lowest useful validation for the protocol struct is unit level. Structured SQLite binding deserves focused test coverage when practical because it is the main data-shape change. DV build validation catches crate-local producer and consumer type drift. Integration validation is required because `bucky-vpn` and `bucky-vpn-server` consume the shared protocol field.

## Unit Tests
| function_or_unit | branch_or_condition | covered_behavior | test_file | status | gap_manual_reason |
|------------------|---------------------|------------------|-----------|--------|-------------------|
| `PnServerInfo::new` | valid id/ip/port | constructs the same structured tuple without endpoint parsing | `vpn-frame/src/vpn_protocol.rs` | covered | none |
| SQLite PN server binding | network/proxy-node structured rows | stores and reads id/ip/port fields separately | `vpn-server/src/sqlite_store_factory.rs` | covered | none |
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
- approver: user-request
- approval_date: 2026-07-01T17:44:43+08:00
- user_statement: "确认，自动处理后续步骤"
