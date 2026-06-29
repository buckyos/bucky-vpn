# bucky-vpn PN Proxy Route Resolver Acceptance

## Findings
| id | severity | stage | evidence | problem | fail_condition_hit |
| --- | --- | --- | --- | --- | --- |
| ACC-bucky-vpn-001 | high | testing | `test-results/test-runs/20260626T031906Z-bucky-vpn-integration.json` | Workspace integration was attempted but failed in `bucky-vpn-server::server_config::tests::default_config_falls_back_to_legacy_toml_when_yaml_missing`, so cross-module green evidence is unavailable. | Required test evidence includes a failing run artifact. |
| ACC-bucky-vpn-002 | high | implementation | `stage-scope-check.py --stage implementation --ignore-untracked` output | Implementation scope check is blocked by pre-existing tracked dirty files outside admitted `vpn-client/src/p2p_vpn.rs`. | Implementation diff is not cleanly bound to admitted Scope Paths in current worktree. |
| ACC-bucky-vpn-003 | medium | testing | `cargo clippy -p bucky-vpn --all-targets --all-features -- -D warnings` output | Client crate clippy fails before checking this task's client code because dependency crate `vpn-frame` has existing denied warnings such as dead code, type complexity, result-large-err, and unwrap/style lints. | Recommended Rust lint evidence is not green in the current workspace. |

## Evidence Coverage
| documented_item | source_document | implementation_evidence | test_result_evidence | status |
| --- | --- | --- | --- | --- |
| Client-owned PN proxy route resolver exists and implements p2p-frame resolver hook. | `proposal.md` and `design.md` `CHG-client-pn-proxy-route-resolver` | `vpn-client/src/p2p_vpn.rs` defines `P2pVpnPnProxyRouteResolver` and implements `p2p_frame::stack::PnProxyRouteResolver`. | `test-results/test-runs/20260626T031819Z-bucky-vpn-dv.json` passed. | consistent |
| P2P stack receives the resolver before pntunnel creation. | `design.md` Overall Approach and Interfaces | `P2pVpnClientFactory::create_client` calls `P2pStackConfig::set_proxy_route_resolver`. | `cargo check -p bucky-vpn` passed; DV artifact passed. | consistent |
| Resolver route cache refreshes from VPN info. | `design.md` Key Call Flows and Data and State | `P2pVpnTunnelFactory::on_vpn_info_received` calls `update_routes(vpn_infos)`. | `test-results/test-runs/20260626T032431Z-bucky-vpn-unit.json` covers resolver route population, no-PN skip, stale replacement, invalid id, and missing route. | consistent |
| Existing client settings, join, and local API remain outside scope. | `proposal.md` Scope and `design.md` Invariants | Scope Paths admit only `vpn-client/src/p2p_vpn.rs`. | Stage-scope evidence is blocked by dirty worktree, but no intentional edits were made to settings, CLI, or API. | consistent |

## Test Design Adequacy
| behavior_risk_change_id | required_case_types | test_design_evidence | runnable_test_evidence | status |
| --- | --- | --- | --- | --- |
| CHG-client-pn-proxy-route-resolver | normal, boundary, negative, error, compatibility, lifecycle | `testing.md` maps resolver branch behavior to unit and DV evidence. | `test-results/test-runs/20260626T032431Z-bucky-vpn-unit.json`; `test-results/test-runs/20260626T032458Z-bucky-vpn-dv.json`. | covered |
| CHG-client-pn-proxy-route-resolver | cross-module | `testing.md` records attempted workspace integration. | `test-results/test-runs/20260626T031906Z-bucky-vpn-integration.json` exists but failed in server config test. | gap |

## Generated Acceptance Rules
| rule_id | source | expected_result | evidence_required | status |
| --- | --- | --- | --- | --- |
| AR-client-resolver-wired | proposal/design | Client p2p stack is configured with a PN proxy resolver. | Code inspection plus DV artifact. | pass |
| AR-route-refresh | design Data and State | Resolver route cache is refreshed from VPN info before pntunnel use. | Code inspection plus unit evidence. | pass |
| AR-failure-semantics | design Key Call Flows | Missing/invalid route behavior matches design. | Code/design comparison and unit coverage. | pass |
| AR-cross-module-green | testing rules | Workspace compatibility has passing evidence. | Fresh passing integration artifact. | fail |
| AR-scope-binding | implementation admission rules | Diff is bound to admitted Scope Paths. | Passing stage-scope check. | fail |

## Required Command Evidence
- schema-check.py: `uv run --active python ./harness/scripts/schema-check.py --version v0.1 --module bucky-vpn` passed.
- admission-check.py: `uv run --active python ./harness/scripts/admission-check.py --version v0.1 --module bucky-vpn --change-id CHG-client-pn-proxy-route-resolver --evidence-file harness/evidence/admission/20260626-pn-proxy-route-resolver.md` passed and wrote `harness/evidence/admission/20260626-pn-proxy-route-resolver.bucky-vpn.stamp.json`.
- stage-scope-check.py: implementation/design/testing scope checks were run with `--ignore-untracked` and failed because unrelated tracked dirty files are present outside admitted/task scope.
- test-run.py <module> all: not run as one command; equivalent module levels were run individually: unit passed, DV passed, integration failed. Artifacts: `test-results/test-runs/20260626T032431Z-bucky-vpn-unit.json`, `test-results/test-runs/20260626T032458Z-bucky-vpn-dv.json`, `test-results/test-runs/20260626T031906Z-bucky-vpn-integration.json`.
- test-run.py all all: not run because module integration already fails and whole-project evidence would be blocked by the same workspace failure.
- cargo fmt: `cargo fmt --all -- --check` passed.
- cargo clippy: `cargo clippy -p bucky-vpn --all-targets --all-features -- -D warnings` failed in pre-existing `vpn-frame` lint findings outside this task's admitted client path.
- quality-check.py: not run; `harness/quality-gates.yaml` declares `gates: []`, so no quality artifact is required for a non-accepted conclusion.

## Consistency Summary
- Proposal authority check: The implementation targets `CHG-client-pn-proxy-route-resolver`, which is present in approved proposal and approved design.
- Proposal vs design: Consistent on adding a client-owned PN proxy route resolver and avoiding new persistence/UI behavior.
- Design vs implementation: Consistent after design was updated to specify invalid PN id error propagation.
- Test design adequacy: Resolver unit, DV, and metadata coverage are adequate for client-local behavior; cross-module green evidence remains blocked by unrelated workspace failure.
- change_id traceability: Proposal, design, testing, and admission evidence all map `CHG-client-pn-proxy-route-resolver`.
- Document logic review: The proposal/design structure is coherent; testing accurately records gaps instead of claiming full coverage.
- Implementation logic review: The resolver is wired to p2p-frame's existing hook, refreshed from VPN info, and branch behavior is covered by unit tests; remaining acceptance risk is external evidence cleanliness.

## Conclusion
- Accepted / rejected / needs changes: needs changes
- Reason: The core implementation compiles and bucky-vpn unit/DV validation passed, including resolver unit coverage. Acceptance cannot pass while implementation stage-scope evidence is failing due to unrelated dirty files, workspace integration evidence is failing in an unrelated server config test, and clippy evidence is blocked by existing vpn-frame warnings.

## Follow-Up Tasks
- Iteration count: 1
- External cleanup task: Fix or isolate `bucky-vpn-server::server_config::tests::default_config_falls_back_to_legacy_toml_when_yaml_missing` before requiring green workspace integration.
- Environment/worktree task: Isolate this pipeline in a clean worktree or clear unrelated tracked dirty files before rerunning stage-scope checks.
- Lint cleanup task: Resolve or explicitly allow the existing `vpn-frame` clippy findings before requiring `cargo clippy -p bucky-vpn --all-targets --all-features -- -D warnings` to pass.
