# bucky-vpn SN QUIC/TCP Endpoint Acceptance Report

## Findings
| id | severity | stage | evidence | problem | fail_condition_hit |
| --- | --- | --- | --- | --- | --- |
| ACC-SN-ENDPOINT-001 | medium | external-environment | `stage-scope-check.py --stage implementation --version v0.1 --module bucky-vpn --change-id CHG-client-sn-quic-tcp-priority` failed because the worktree contains mixed tracked/untracked changes outside this task, including `build_win.bat`, `vpn_web/*`, logs, DB files, generated platform files, and prior review/admission artifacts. | The production change is confined to the admitted client files, but final stage-scope evidence cannot be cleanly isolated in the current dirty worktree. | implementation diff scope evidence is not cleanly passable |
| ACC-SN-ENDPOINT-002 | medium | acceptance | `test-run.py all all` was not run; available fresh artifacts are `test-results/test-runs/20260704T161545Z-bucky-vpn-unit.json` and `test-results/test-runs/20260704T161424Z-bucky-vpn-dv.json`. | Final accepted conclusion requires whole-project test evidence, but this run only executed the relevant bucky-vpn unit and DV levels. | whole-project test evidence missing |

## Evidence Coverage
| documented_item | source_document | implementation_evidence | test_result_evidence | status |
| --- | --- | --- | --- | --- |
| `CHG-client-sn-quic-tcp-priority` proposal item | `docs/versions/v0.1/modules/bucky-vpn/proposal.md` Proposal Items | `vpn-client/src/main.rs` builds local P2P endpoints as QUIC then TCP on `p2p.port`; `vpn-client/src/p2p_vpn.rs` builds remote SN endpoints as QUIC then TCP and passes them to `P2pSn::new`. | `test-results/test-runs/20260704T161545Z-bucky-vpn-unit.json`; `test-results/test-runs/20260704T161424Z-bucky-vpn-dv.json` | consistent |
| Design scope path and admission | `docs/versions/v0.1/modules/bucky-vpn/design.md` Directly Mapped Change Items | `harness/evidence/admission/20260705-bucky-vpn-sn-quic-tcp-priority.md` and generated stamp bind implementation to `vpn-client/src/main.rs` and `vpn-client/src/p2p_vpn.rs`. | admission-check passed for `CHG-client-sn-quic-tcp-priority` | consistent |
| Testing coverage for endpoint and listener order | `docs/versions/v0.1/modules/bucky-vpn/testing.md`; `testplan.yaml` | `p2p_listen_endpoints_register_quic_and_tcp` asserts local QUIC/TCP endpoints; `sn_endpoints_register_quic_before_tcp` asserts remote SN QUIC/TCP endpoints. | `test-results/test-runs/20260704T161545Z-bucky-vpn-unit.json` reports 11 passed tests | consistent |

## Test Design Adequacy
| behavior_risk_change_id | required_case_types | test_design_evidence | runnable_test_evidence | status |
| --- | --- | --- | --- | --- |
| `CHG-client-sn-quic-tcp-priority` | normal, boundary, compatibility, cross-module; negative/error/lifecycle recorded not-applicable with reasons | `testing.md` Direct Change Coverage, Case-Type Coverage, Design Element Coverage, Unit Tests, DV Tests | `test-results/test-runs/20260704T161545Z-bucky-vpn-unit.json`; `test-results/test-runs/20260704T161424Z-bucky-vpn-dv.json` | adequate for module-level behavior |
| p2p-frame endpoint selection boundary | integration semantics delegated to p2p-frame by proposal and design | `testing.md` Integration Tests records live endpoint selection as manual because client does not own retry/selection | no live QUIC/TCP p2p-frame selection artifact produced in this task | gap |

## Generated Acceptance Rules
| rule_id | source | expected_result | evidence_required | status |
| --- | --- | --- | --- | --- |
| AR-LOCAL-ENDPOINT-LIST | proposal/design `CHG-client-sn-quic-tcp-priority` | Client supplies local QUIC and TCP P2P endpoints on the configured `p2p.port` | code review plus unit artifact | pass |
| AR-SN-ENDPOINT-ORDER | proposal/design `CHG-client-sn-quic-tcp-priority` | Client supplies a QUIC endpoint followed by a TCP endpoint for the resolved SN socket address | code review plus unit artifact | pass |
| AR-NO-CLIENT-SELECTION | proposal/design boundary | Client does not implement its own SN endpoint selection or retry logic | code review of `run_daemon`, `P2pVpnClientFactory::create_client`, `p2p_listen_endpoints`, and `sn_endpoints` | pass |
| AR-HARNESS-SCOPE | harness stage-scope rules | Stage scope checks can cleanly bind changes to allowed files | stage-scope command results | gap |
| AR-WHOLE-PROJECT-EVIDENCE | acceptance rules | Whole-project unified test evidence exists before accepted conclusion | `test-run.py all all` artifact | gap |

## Required Command Evidence
- `schema-check.py`: passed for `v0.1 / bucky-vpn` after proposal, design, implementation, and testing updates.
- `admission-check.py`: passed with `harness/evidence/admission/20260705-bucky-vpn-sn-quic-tcp-priority.md`; stamp written for `vpn-client/src/main.rs` and `vpn-client/src/p2p_vpn.rs`.
- `cargo fmt --all`: passed.
- `cargo check -p bucky-vpn`: passed; existing `vpn-frame::TunnelManager::get_all_send` dead code warning remains.
- `test-run.py bucky-vpn unit`: passed; artifact `test-results/test-runs/20260704T161545Z-bucky-vpn-unit.json`.
- `test-run.py bucky-vpn dv`: passed; artifact `test-results/test-runs/20260704T161424Z-bucky-vpn-dv.json`.
- `test-run.py <module> all`: not run as a single all-level command; targeted `bucky-vpn` unit and DV entries passed, but integration/all-level module evidence is not present.
- `doc-structure-check.py --docs testing`: passed.
- `testing-coverage-check.py`: passed.
- `stage-scope-check.py --stage implementation`: failed due mixed dirty worktree; unrelated/ambient files are listed in command output.
- `test-run.py all all`: not run; this blocks an accepted conclusion.
- `quality-check.py`: passed; no quality gates configured because `harness/quality-gates.yaml` declares an explicitly empty gates list.

## Consistency Summary
- Proposal authority check: approved proposal directly contains `PROP-client-sn-quic-tcp-priority` and states the client must supply QUIC/TCP SN endpoints and enable local QUIC/TCP P2P listeners while p2p-frame chooses the connection endpoint.
- Proposal vs design: design preserves the proposal boundary and maps `CHG-client-sn-quic-tcp-priority` to `vpn-client/src/main.rs` and `vpn-client/src/p2p_vpn.rs`.
- Design vs implementation: implementation adds `p2p_listen_endpoints(u16)` returning local `[Protocol::Quic, Protocol::Tcp]`, and `sn_endpoints(SocketAddr)` returning remote `[Protocol::Quic, Protocol::Tcp]`, matching design.
- Test design adequacy: unit and DV coverage are appropriate for client-owned endpoint construction; live p2p-frame selection remains outside client ownership and is documented as a manual integration gap for final acceptance.
- change_id traceability: proposal, design, admission evidence, testing metadata, and testplan all name `CHG-client-sn-quic-tcp-priority`.
- Document logic review: no contradiction found between proposal, design, testing, and the p2p-frame ownership boundary.
- Implementation logic review: no client-side retry or endpoint-selection logic was added; default key parsing, identity directory behavior, and p2p-frame ownership are unchanged.

## Conclusion
- accepted / rejected / needs changes: needs changes
- reason: The implementation and targeted validation satisfy the requested client-owned behavior, including the missing local TCP listener, but final acceptance is blocked by mixed-worktree stage-scope failures and missing whole-project `test-run.py all all` evidence.

## Follow-Up Tasks
- Iteration count: 2
- Proposal task: none for behavior; proposal is approved and consistent.
- Design task: rerun design/stage-scope in an isolated worktree or after unrelated tracked changes are committed/stashed.
- Implementation task: rerun implementation stage-scope for `CHG-client-sn-quic-tcp-priority` in an isolated worktree; production edit is confined to `vpn-client/src/main.rs` and `vpn-client/src/p2p_vpn.rs`.
- Testing task: run broader `test-run.py bucky-vpn all` or `test-run.py all all` when final acceptance evidence is required.
