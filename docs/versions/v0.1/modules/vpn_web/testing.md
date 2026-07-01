---
module: vpn_web
version: v0.1
status: approved
approved_by: user-request
approved_at: 2026-07-01T17:44:43+08:00
approved_content_sha256: a88cf03a98776ce927a6ae15c555b5131f9c14d496ff0ea25debc8ec1a7787c4
---

# vpn_web Testing

## Test Document Index
| document | topic | scope |
|----------|-------|-------|
| `testing.md` | Proxy node management validation strategy | full change |

## Unified Test Entry
| level | entry | expected_result |
|-------|-------|-----------------|
| unit | `python3 ./harness/scripts/test-run.py vpn_web unit` | Runs existing Flutter tests; no new tests are added by default. |
| dv | `python3 ./harness/scripts/test-run.py vpn_web dv` | Runs `flutter analyze`. |
| integration | `python3 ./harness/scripts/test-run.py vpn_web integration` | Runs `flutter build web`. |

## Submodule Tests
| submodule | responsibility | detailed_test_doc | must_cover | boundary_failure | test_type | test_file |
|-----------|----------------|-------------------|------------|------------------|-----------|-----------|
| `home-shell` | Home tab composition and selected index state | none | Three tabs are wired to three children. | Tab count/child mismatch or missing import. | dv/integration/manual | `harness/scripts/test-run.py` |
| `page-widgets` | ProxyNodesPage table, loading, refresh, allow control, observed address display, and comment action | none | Renders allow control, id, observed-address-preferred address with `pn_server.ip:port` fallback, liveness, comment, and comment action from API data without raw status/update columns. | API failure shows toast and preserves old state; missing observed address falls back to local configured address. | dv/integration/manual | no new Flutter test by rule |
| `network-member-node-id` | Network member NodeId display and name lookup | none | Uses backend canonical base36 `nodeId` values directly without local base58 conversion. | Stale base58 server response is no longer normalized by UI. | dv/integration/manual | no new Flutter test by rule |
| `api-client` | Proxy-node models and HTTP wrappers | none | Typed wrappers compile and generated JSON glue matches annotations, including nullable `observedAddr`. | Missing generated code or backend key mismatch. | dv/integration | `api.dart`, `api.g.dart` |

## Module-Level Tests
| validation_id | coverage_boundary | entry | expected_result | test_type | test_file_or_script |
|---------------|-------------------|-------|-----------------|-----------|---------------------|
| VAL-vpn-web-existing-unit | Existing Flutter test signal only | `python3 ./harness/scripts/test-run.py vpn_web unit` | Existing tests are reachable; no new tests are added. | automated | `harness/scripts/test-run.py` |
| VAL-vpn-web-analyze | Frontend static analysis for proxy-node page/API wiring | `python3 ./harness/scripts/test-run.py vpn_web dv` | Analyzer exits 0. | automated | `harness/scripts/test-run.py` |
| VAL-vpn-web-build | Web build for generated model and tab integration | `python3 ./harness/scripts/test-run.py vpn_web integration` | Web build exits 0. | automated | `harness/scripts/test-run.py` |

## External Interface Tests
| interface | responsibility | success_case | failure_boundary | test_type | test_doc_or_file |
|-----------|----------------|--------------|------------------|-----------|------------------|
| Backend proxy-node HTTP API usage | Frontend consumes list/approve/reject endpoints with bearer session, optional observed address, and optional comment | Analyzer/build compile typed wrappers and request bodies. | Invalid sessions and permission semantics remain backend-owned; UI displays API failures through toast; missing observed address falls back to local address. | dv/integration/manual | `vpn_web/lib/api.dart` |
| Backend NodeId base36 contract | Frontend consumes `node_id` fields already encoded as base36 | Analyzer/build compile direct string comparison without base58 dependency in `NetworkMembersPage`. | Backend returning old base58 values may break display/name matching and is treated as backend migration risk. | dv/integration/manual | `vpn_web/lib/network_members_page.dart` |

## Direct Change Coverage
| change_id | design_source | validation_id | testplan_level | testplan_step_id | gap | gap_manual_reason |
|-----------|---------------|---------------|----------------|------------------|-----|-------------------|
| CHG-proxy-node-management-tab | design.md Directly Mapped Change Items | VAL-vpn-web-analyze | dv | vpn-web-dv | no | none |
| CHG-proxy-node-real-address-display | design.md Directly Mapped Change Items | VAL-vpn-web-real-address | dv | vpn-web-dv | no | Analyzer/build compile the nullable `observedAddr` model and Address column fallback logic; live backend/browser smoke is manual. |
| CHG-vpn-web-node-id-base36 | design.md Directly Mapped Change Items | VAL-vpn-web-node-id-base36 | dv | vpn-web-dv | no | Analyzer/build compile direct NodeId use; no new widget tests by rule. |

## Case-Type Coverage
| change_id | case_type | required | validation_id | level | status | gap_manual_reason |
|-----------|-----------|----------|---------------|-------|--------|-------------------|
| CHG-proxy-node-management-tab | normal | yes | VAL-vpn-web-analyze | dv | covered | none |
| CHG-proxy-node-management-tab | boundary | yes | VAL-vpn-web-build | integration | covered | Empty and populated table states compile through the same widget path; manual visual verification remains useful. |
| CHG-proxy-node-management-tab | negative | yes | VAL-vpn-web-analyze | dv | manual | Toast behavior for API failure needs runtime/browser verification; owner frontend implementer, risk UI-only, acceptance impact recorded. |
| CHG-proxy-node-management-tab | error | yes | VAL-vpn-web-analyze | dv | manual | Backend error responses require a running API to reproduce; owner frontend implementer, risk toast path, acceptance impact recorded. |
| CHG-proxy-node-management-tab | compatibility | yes | VAL-vpn-web-build | integration | covered | Existing backend endpoints are consumed without changing their contract. |
| CHG-proxy-node-management-tab | lifecycle | yes | VAL-vpn-web-analyze | dv | covered | `initState` load and refresh paths compile and preserve widget lifecycle checks. |
| CHG-proxy-node-management-tab | cross-module | yes | VAL-vpn-web-build | integration | manual | Full frontend-to-backend runtime smoke needs a running server; owner integration environment, risk endpoint mismatch, acceptance impact recorded. |
| CHG-proxy-node-real-address-display | normal | yes | VAL-vpn-web-real-address | dv | covered | Analyzer compiles `observedAddr` parsing and preferred Address rendering path. |
| CHG-proxy-node-real-address-display | boundary | yes | VAL-vpn-web-real-address | integration | covered | Build compiles the fallback to `pnServer.ip:pnServer.port` when `observedAddr` is absent or empty. |
| CHG-proxy-node-real-address-display | negative | yes | VAL-vpn-web-real-address | dv | manual | Runtime backend response with malformed address needs live API/browser verification; owner frontend implementer, risk display-only. |
| CHG-proxy-node-real-address-display | error | yes | VAL-vpn-web-real-address | dv | manual | Backend/API failure path reuses list error toast; no new widget test by rule. |
| CHG-proxy-node-real-address-display | compatibility | yes | VAL-vpn-web-build | integration | covered | Nullable `observedAddr` keeps older/missing backend responses build-compatible with fallback display. |
| CHG-proxy-node-real-address-display | lifecycle | yes | VAL-vpn-web-real-address | dv | covered | Address value is derived during row build after each load/refresh; no separate lifecycle state is introduced. |
| CHG-proxy-node-real-address-display | cross-module | yes | VAL-vpn-web-build | integration | manual | Requires `bucky-vpn-server` to expose `observed_addr`; live frontend-to-backend smoke remains manual. |
| CHG-vpn-web-node-id-base36 | normal | yes | VAL-vpn-web-node-id-base36 | dv | covered | Analyzer compiles direct base36 NodeId comparison/display path. |
| CHG-vpn-web-node-id-base36 | boundary | yes | VAL-vpn-web-build | integration | covered | Empty/missing node-name lookup still compiles through fallback display. |
| CHG-vpn-web-node-id-base36 | negative | yes | VAL-vpn-web-node-id-base36 | dv | manual | Live stale base58 backend response requires backend/runtime smoke; no new widget test by rule. |
| CHG-vpn-web-node-id-base36 | error | no | VAL-vpn-web-node-id-base36 | dv | not-applicable | Removing local conversion does not add a new UI error branch. |
| CHG-vpn-web-node-id-base36 | compatibility | yes | VAL-vpn-web-build | integration | manual | Compatibility depends on backend base36 rollout; UI no longer compensates for base58. |
| CHG-vpn-web-node-id-base36 | lifecycle | no | VAL-vpn-web-node-id-base36 | dv | not-applicable | Encoding selection has no lifecycle state. |
| CHG-vpn-web-node-id-base36 | cross-module | yes | VAL-vpn-web-build | integration | manual | Requires backend serving base36 NodeIds and browser/runtime smoke. |

## Design Element Coverage
| element_type | design_source | derived_cases | level | status | gap_manual_reason |
|--------------|---------------|---------------|-------|--------|-------------------|
| parameter-domain | design.md Interfaces and Dependencies | Proxy server id/ip/port/observed_addr/status/comment fields compile through generated JSON wrappers; observed address drives Address display, while status drives allow control and comment preservation but is not shown as a raw column. | integration | covered | none |
| parameter-domain | design.md Data and State | observed address present, absent, and empty values map to preferred address or fallback display. | dv | manual | Compile coverage exists; live value permutations need runtime/browser verification because no new widget tests are added by rule. |
| state-transition | design.md Data and State | allow status transitions approved/rejected are represented by the allow checkbox; comment edits preserve current approved/rejected state through existing approve/reject API calls. | dv | manual | Needs running API/browser to verify state refresh; owner frontend implementer, risk stale UI, acceptance impact recorded. |
| failure-path | design.md Key Call Flows | list/allow/comment API failure toast paths are present. | dv | manual | Toast failure paths need runtime fault injection; owner frontend implementer, risk notification regression, acceptance impact recorded. |
| error-handling | design.md Key Call Flows | API wrapper error tuples propagate to page toast handling. | dv | manual | Automated UI tests are not added by rule; owner frontend implementer, risk UI-only. |
| invariant | design.md Invariants to Preserve | Existing tabs and API wrapper style preserved by analyzer/build. | dv | covered | none |
| concurrency | design.md Data and State | not-applicable: no concurrent mutation ordering is introduced. | dv | not-applicable | Design does not introduce concurrency beyond existing async refresh calls. |
| parameter-domain | design.md Interfaces and Dependencies | backend base36 `nodeId` strings are compared directly; base58 conversion dependency removed from member page | dv | covered | none |

## Validation Rationale
`vpn_web` has a repository rule against adding new frontend tests by default, so this change uses the existing harness entries: analyzer for typed wiring and build for generated code/page integration. Runtime browser verification remains valuable for toast paths and live backend responses, and those gaps are recorded explicitly rather than filled with unrequested tests.

## Unit Tests
| function_or_unit | branch_or_condition | covered_behavior | test_file | status | gap_manual_reason |
|------------------|---------------------|------------------|-----------|--------|-------------------|
| ProxyNodesPage | UI branches for loading/list/observed-address fallback/allow/comment-action callbacks | Existing no-new-tests rule prevents adding widget branch tests by default. | no new test | manual | User did not request frontend test exception; analyzer/build/manual verification are used. |
| NetworkMembersPage NodeId lookup | base36 `nodeId` direct comparison | Existing no-new-tests rule prevents adding widget branch tests by default. | no new test | manual | Analyzer/build verify removal of base58 conversion; runtime stale backend response remains manual. |

## DV Tests
| workflow | kind | entry | expected_result | test_file_or_script | status | gap_manual_reason |
|----------|------|-------|-----------------|---------------------|--------|-------------------|
| Analyze proxy node management wiring | main | `python3 ./harness/scripts/test-run.py vpn_web dv` | `flutter analyze` exits 0. | `harness/scripts/test-run.py` | covered | none |
| Existing app lifecycle signal | lifecycle | `python3 ./harness/scripts/test-run.py vpn_web unit` | Existing Flutter tests are reachable if still valid. | `harness/scripts/test-run.py` | manual | Existing widget smoke test may not match the real app shell; no new tests by rule. |
| API failure toast paths | failure | manual browser/API fault injection | List, allow, and comment API failures show toast and keep prior state. | manual | manual | Requires a running backend or injected API failure; owner frontend implementer, acceptance impact recorded. |
| NodeId base36 frontend workflow | main | `python3 ./harness/scripts/test-run.py vpn_web dv` | analyzer passes after removing base58-to-base36 conversion from network member page | `harness/scripts/test-run.py` | covered | none |

## Integration Tests
| contract_or_flow | modules_involved | success_case | failure_case | test_file | status | gap_manual_reason |
|------------------|------------------|--------------|--------------|-----------|--------|-------------------|
| Proxy-node frontend build | `vpn_web`, backend API contract | `python3 ./harness/scripts/test-run.py vpn_web integration` builds web bundle. | Generated model mismatch or bad imports fail build. | `harness/scripts/test-run.py` | covered | none |
| Live proxy-node backend smoke | `vpn_web`, `bucky-vpn-server` | Browser page lists proxy nodes from a running server. | Invalid session or endpoint mismatch shows API failure toast. | manual | manual | Requires running server/browser; owner integration environment, risk endpoint drift, acceptance impact recorded. |
| Frontend NodeId base36 smoke | `vpn_web`, `bucky-vpn-server` | Browser member page receives base36 node ids and resolves member names through direct comparison. | Backend returns stale base58 ids and UI no longer normalizes them. | manual | manual | Requires running server/browser; owner integration environment, risk backend migration. |

## Definition of Done
- [x] Testing document maps `CHG-proxy-node-management-tab` to validation coverage.
- [x] `testplan.yaml` includes the change id for reachable harness entries.
- [x] No new frontend tests are added without explicit user exception.
- [x] NodeId base36 frontend behavior is mapped without adding new widget tests.
- [x] `doc-structure-check.py --docs testing` passes.
- [ ] `testing-coverage-check.py` passes.
- [ ] Allowed validation runs are executed or blockers are recorded.

## Approval Record
- approver: user-request
- approval_date: 2026-07-01T17:44:43+08:00
- user_statement: "确认，自动处理后续步骤"
