# bucky-vpn-server Acceptance Review

## Findings
| id | severity | stage | evidence | problem | fail_condition_hit |
| --- | --- | --- | --- | --- | --- |
| ACC-001 | high | implementation | `vpn-server/src/server_config.rs:103` stores only static constructor-provided `pn_servers`; `vpn-server/src/main.rs:163` constructs `ConfigPnServerSelector::new(pn_servers)` before any external proxy heartbeat; `vpn-server/src/pn_traffic_service.rs:87` sends heartbeat as zero traffic delta only. | Approved external proxy behavior requires external proxy nodes to actively connect and be controlled by the control node for usability. The delivered code wires pure proxy control client validation and heartbeat, but does not register a connected external proxy into the control node selector or make heartbeat liveness affect selectable proxy membership. | Acceptance Must Fail If: approved behavior is not implemented; implementation lifecycle/state defect. |
| ACC-002 | high | implementation | `python3 ./harness/scripts/stage-scope-check.py --stage implementation --version v0.1 --module bucky-vpn-server --change-id ...` exited 1 and reported many changed paths outside admitted Scope Paths, including unrelated docs, harness files, vpn-client/vpn-frame/vpn_web paths, generated artifacts, and local data. | The implementation diff could not be mechanically bound to the admitted design Scope Paths in the current dirty worktree. The reviewed code may be scoped, but the required gate fails closed. | Acceptance Must Fail If: implementation diff was not bound to admitted design Scope Paths. |
| ACC-003 | high | testing | `test-results/test-runs/20260624T155609Z-all-all.json` has `requested_module=all`, `requested_level=all`, `exit_code=1`; failing step is `vpn_web` `flutter test` with exit code 1. Console output shows Flutter tried to move `/mnt/c/flutter/bin/cache/dart-sdk` and failed with permission denied while running as root. | Whole-project unified test evidence is failing. Rust, repo-governance, and bucky-vpn-server module evidence passed, but final pipeline acceptance cannot pass with a failing all/all artifact. | Acceptance Must Fail If: required test evidence is failing. |
| ACC-004 | medium | testing | `docs/versions/v0.1/modules/bucky-vpn-server/testing.md` records lifecycle gaps for external proxy connect/register/disconnect, heartbeat timeout/recover, and persistence restart; module all artifact `test-results/test-runs/20260624T155310Z-bucky-vpn-server-all.json` passes only compile/unit/workspace test coverage. | Testing correctly records gaps, but the gaps are for behavior central to the approved external proxy and heartbeat requirements. These gaps cannot support an accepted conclusion for this proposal. | Acceptance Must Fail If: approved behavior cannot be verified. |

## Evidence Coverage
| documented_item | source_document | implementation_evidence | test_result_evidence | status |
| --- | --- | --- | --- | --- |
| Remove `pn.server_addresses` / no static external proxy addresses | `proposal.md` CHG-pn-config-no-static-addresses; `design.md` Directly Mapped Change Items | `vpn-server/src/server_config.rs:17` `PnServerConfig` has no static address list; `resolve_service_endpoints` returns only the base SN endpoint at `vpn-server/src/server_config.rs:91`. | `test-results/test-runs/20260624T155310Z-bucky-vpn-server-all.json` exit_code 0. | consistent |
| Pure proxy node requires control-node address | `proposal.md` CHG-pure-pn-sn-address; `design.md` Data and State | `vpn-server/src/server_config.rs:128` reads `pn.control_server.id` and endpoint; `vpn-server/src/main.rs:192` requires this config when SN is disabled and PN is enabled. | `test-results/test-runs/20260624T155310Z-bucky-vpn-server-all.json` exit_code 0. | consistent |
| Pure proxy connects back to control node | `proposal.md` CHG-external-pn-active-control; `design.md` Key Call Flows | `vpn-server/src/vpn_control_client.rs:36` creates a control client; `vpn-server/src/main.rs:196` creates it for pure proxy mode. | `test-results/test-runs/20260624T155310Z-bucky-vpn-server-all.json` exit_code 0. | partial |
| Control node controls whether proxy can be used | `proposal.md` CHG-external-pn-active-control; `design.md` Invariants to Preserve | `vpn-server/src/vpn_control_client.rs:107` validates PN connection through control command, but `ConfigPnServerSelector` remains static and heartbeat does not update selector membership. | No runnable lifecycle test proves selectable proxy admission/removal. | inconsistent |
| PnServer and SnServer keep heartbeat | `proposal.md` CHG-pn-sn-heartbeat; `design.md` Key Call Flows | `vpn-server/src/pn_traffic_service.rs:87` starts a zero-delta remote heartbeat through existing traffic reporter. | Module tests pass, but no timeout/liveness selection evidence exists. | partial |
| Co-located control/proxy defaults allowed | `proposal.md` CHG-colocated-pn-default-allowed; `design.md` Key Decisions | `vpn-server/src/server_config.rs:51` defaults `pn.enabled=true`; local selector receives local endpoint when PN starts. | `test-results/test-runs/20260624T155310Z-bucky-vpn-server-all.json` exit_code 0. | consistent |
| Traffic stats use DB-backed storage interface | `proposal.md` CHG-pn-traffic-db-interface; `design.md` Data and State | `vpn-server/src/pn_traffic_service.rs:217` applies deltas through store factory; remote reporter uses command path at `vpn-server/src/vpn_control_client.rs:87`. | Existing persistence tests pass in `test-results/test-runs/20260624T155310Z-bucky-vpn-server-all.json`. | consistent |

## Test Design Adequacy
| behavior_risk_change_id | required_case_types | test_design_evidence | runnable_test_evidence | status |
| --- | --- | --- | --- | --- |
| CHG-pn-config-no-static-addresses | normal,boundary,compatibility | `testing.md` Direct Change Coverage and Case-Type Coverage rows. | `test-results/test-runs/20260624T155310Z-bucky-vpn-server-all.json` unit step exit_code 0. | covered |
| CHG-pure-pn-sn-address | normal,boundary,negative,error,compatibility | `testing.md` covers valid and invalid `pn.control_server` parsing. | `test-results/test-runs/20260624T155310Z-bucky-vpn-server-all.json` unit step exit_code 0. | covered |
| CHG-external-pn-active-control | normal,negative,error,compatibility,lifecycle,cross-module | `testing.md` records lifecycle integration gap for full connect/register/disconnect. | Module compile/tests pass, but no runnable evidence proves control-node selector membership or removal. | gap |
| CHG-pn-sn-heartbeat | normal,negative,error,compatibility,lifecycle,cross-module | `testing.md` records heartbeat timeout/recover and error assertion gaps. | Module compile/tests pass, but no fake-clock or runtime lifecycle evidence exists. | gap |
| CHG-colocated-pn-default-allowed | normal,boundary,compatibility,lifecycle | `testing.md` maps default enabled and disabled branches to config tests/build. | `test-results/test-runs/20260624T155310Z-bucky-vpn-server-all.json` exit_code 0. | covered |
| CHG-pn-traffic-db-interface | normal,boundary,error,compatibility,lifecycle,cross-module | `testing.md` maps storage paths and records restart/write failure gaps. | Existing persistence unit test passes in module all artifact. | partial |
| CHG-local-pn-toggle-preserved | normal,boundary,compatibility,lifecycle | `testing.md` maps default and explicit disabled config behavior. | `test-results/test-runs/20260624T155310Z-bucky-vpn-server-all.json` exit_code 0. | covered |

## Generated Acceptance Rules
| rule_id | source | expected_result | evidence_required | status |
| --- | --- | --- | --- | --- |
| AR-001 | Approved proposal CHG-pn-config-no-static-addresses | Server config has no `pn.server_addresses` contract and does not add static proxy addresses to service endpoints. | Config implementation and module all tests. | pass |
| AR-002 | Approved proposal CHG-external-pn-active-control | External proxy active connection must make the control node able to allow/deny proxy usability. | Runtime selector/liveness implementation and runnable lifecycle evidence. | fail |
| AR-003 | Approved proposal CHG-pure-pn-sn-address | Pure proxy config has a control-node address. | `pn.control_server` parsing and pure proxy assembly branch. | pass |
| AR-004 | Approved proposal CHG-pn-sn-heartbeat | Proxy and control node keep heartbeat and liveness impacts usability. | Heartbeat implementation plus timeout/recover evidence. | gap |
| AR-005 | Approved proposal CHG-colocated-pn-default-allowed | Co-located control/proxy node defaults proxy allowed. | Default PN enabled config and local selector evidence. | pass |
| AR-006 | Approved proposal CHG-pn-traffic-db-interface | Traffic stats use existing DB-backed store interfaces. | Store-backed delta tests and implementation review. | pass |
| AR-007 | Harness acceptance rules | Schema, admission, implementation scope, module tests, all/all, quality, and report checks must pass or be explicitly rejected. | Required command evidence below. | fail |

## Required Command Evidence
- schema-check.py: `python3 ./harness/scripts/schema-check.py --version v0.1 --module bucky-vpn-server` passed.
- admission-check.py: `python3 ./harness/scripts/admission-check.py --version v0.1 --module bucky-vpn-server --change-id ... --evidence-file harness/evidence/admission/20260624-proxy-node-control.md` passed and wrote `harness/evidence/admission/20260624-proxy-node-control.bucky-vpn-server.stamp.json`.
- stage-scope-check.py: `python3 ./harness/scripts/stage-scope-check.py --stage implementation --version v0.1 --module bucky-vpn-server --change-id ...` failed with changed paths outside admitted Scope Paths.
- test-run.py <module> all: `python3 ./harness/scripts/test-run.py bucky-vpn-server all` passed; artifact `test-results/test-runs/20260624T155310Z-bucky-vpn-server-all.json`.
- test-run.py all all: `python3 ./harness/scripts/test-run.py all all` failed at `vpn_web` `flutter test`; artifact `test-results/test-runs/20260624T155207Z-all-all.json`; root shortcut `./test-run.sh` reproduced the same failure and wrote `test-results/test-runs/20260624T155609Z-all-all.json`.
- quality-check.py: `python3 ./harness/scripts/quality-check.py` passed; `harness/quality-gates.yaml` declares an explicitly empty gates list, so no quality run artifact was required or written.

## Consistency Summary
- Proposal authority check: The approved proposal requires active external proxy participation controlled by the control node; this remains the authoritative behavior for ACC-001.
- Proposal vs design: Design preserves the proposal terminology and maps all seven `change_id` values, but the implemented selector/liveness path does not satisfy the external proxy usability part.
- Design vs implementation: Config removal, pure proxy control address, control client, heartbeat report, and DB stats are present; dynamic external proxy registration/selection is missing.
- Test design adequacy: Testing artifacts are approved and coverage-check passed, but they explicitly record lifecycle gaps for central heartbeat/external proxy behavior.
- change_id traceability: All seven change IDs exist across proposal, design, admission evidence, testing.md, and testplan.yaml; implementation scope binding still fails due dirty worktree paths.
- Document logic review: Proposal/design/testing are mostly coherent; testing correctly exposes gaps instead of claiming complete lifecycle coverage.
- Implementation logic review: Zero-delta heartbeat reports activity but does not by itself create selectable proxy state, update liveness, or remove stale external proxy candidates.

## Follow-Up Tasks
- Iteration count: 1
- Implementation task: Add or wire a control-node registry/liveness path so externally connected proxy nodes can become selectable only when allowed, and are removed or made unavailable when heartbeat expires.
- Testing task: Add runnable lifecycle evidence for external proxy register/allow/deny/heartbeat-timeout/recover, or narrow proposal/design scope with approved deferral.
- Governance task: Re-run implementation `stage-scope-check.py` from a clean or task-isolated worktree so admitted Scope Paths can be verified.
- Environment task: Fix Flutter SDK cache permissions or run whole-project tests as a non-root user with writable Flutter cache, then rerun `test-run.py all all` and `./test-run.sh`.

## Conclusion
- Accepted / Rejected / Needs changes: needs changes
- Reason: The config and partial control/heartbeat implementation compile and pass module tests, but external proxy usability is not fully implemented, implementation scope binding fails, and whole-project unified evidence fails at Flutter.
