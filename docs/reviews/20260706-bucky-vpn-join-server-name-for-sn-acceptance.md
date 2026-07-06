# bucky-vpn Join Server Name Acceptance Report

## Findings
| id | severity | stage | evidence | problem | fail_condition_hit |
| --- | --- | --- | --- | --- | --- |
| ACC-JOIN-SERVER-NAME-001 | medium | external-environment | `stage-scope-check.py --stage implementation --version v0.1 --module bucky-vpn --change-id CHG-client-join-server-name-for-sn --ignore-untracked` failed because the worktree contains upstream proposal/design/testing/plan changes from this auto-pipeline plus unrelated tracked changes outside this task, including `build_win.bat`, `vpn-client/Cargo.toml`, `vpn-server/Cargo.toml`, `vpn_web/README.md`, and `vpn_web/lib/base58.dart`. | The admitted production edit is confined to `vpn-client/src/main.rs`, `vpn-client/src/cli.rs`, `vpn-client/src/api.rs`, and `vpn-client/src/p2p_vpn.rs`, but final implementation stage-scope evidence cannot be cleanly isolated in the current mixed worktree. | implementation diff scope evidence is not cleanly passable |
| ACC-JOIN-SERVER-NAME-002 | medium | acceptance | `test-run.py all all` was not run; available fresh artifacts are `test-results/test-runs/20260706T073344Z-bucky-vpn-unit.json` and `test-results/test-runs/20260706T073427Z-bucky-vpn-dv.json`. | Final accepted conclusion requires whole-project test evidence, but this run only executed the relevant bucky-vpn unit and DV levels. | whole-project test evidence missing |

## Evidence Coverage
| documented_item | source_document | implementation_evidence | test_result_evidence | status |
| --- | --- | --- | --- | --- |
| `CHG-client-join-server-name-for-sn` proposal item | `docs/versions/v0.1/modules/bucky-vpn/proposal.md` Proposal Items | `vpn-client/src/main.rs` adds `--server_name`; `vpn-client/src/cli.rs` forwards `server_name`; `vpn-client/src/api.rs` stores it in `JoinRecord`; `vpn-client/src/p2p_vpn.rs` passes the effective name to `P2pSn::new`. | `test-results/test-runs/20260706T073344Z-bucky-vpn-unit.json`; `test-results/test-runs/20260706T073427Z-bucky-vpn-dv.json` | consistent |
| Design scope path and admission | `docs/versions/v0.1/modules/bucky-vpn/design.md` Directly Mapped Change Items | `harness/evidence/admission/20260706-bucky-vpn-join-server-name-for-sn.md` and generated stamp bind implementation to the four admitted client paths. | admission-check passed for `CHG-client-join-server-name-for-sn` | consistent |
| Testing coverage for server_name defaulting and compatibility | `docs/versions/v0.1/modules/bucky-vpn/testing.md`; `testplan.yaml` | Unit tests cover explicit name priority, domain default, IP default, blank fallback, structured manager-key roundtrip, and legacy manager-key parsing. | `test-results/test-runs/20260706T073344Z-bucky-vpn-unit.json` reports 17 passed tests | consistent |

## Test Design Adequacy
| behavior_risk_change_id | required_case_types | test_design_evidence | runnable_test_evidence | status |
| --- | --- | --- | --- | --- |
| `CHG-client-join-server-name-for-sn` | normal, boundary, negative, error, compatibility, lifecycle, cross-module | `testing.md` Direct Change Coverage, Case-Type Coverage, Design Element Coverage, Unit Tests, DV Tests | `test-results/test-runs/20260706T073344Z-bucky-vpn-unit.json`; `test-results/test-runs/20260706T073427Z-bucky-vpn-dv.json` | adequate for module-level behavior |
| p2p-frame live certificate/SNI behavior | cross-module live semantics delegated to p2p-frame by proposal and design | `testing.md` records client-owned behavior as pure name selection before p2p-frame validates live tunnels | no live certificate mismatch integration artifact produced in this task | adequate for client-owned scope |

## Generated Acceptance Rules
| rule_id | source | expected_result | evidence_required | status |
| --- | --- | --- | --- | --- |
| AR-SERVER-NAME-CLI-API | proposal/design `CHG-client-join-server-name-for-sn` | CLI and local `/join` API carry optional `server_name` without changing `--name` network-member semantics | code review plus DV artifact | pass |
| AR-SERVER-NAME-DEFAULTS | proposal/design defaulting rule | Explicit non-empty `server_name` wins; omitted domain server defaults to domain; omitted IP server defaults to `server_id`; blank value is absent | unit artifact | pass |
| AR-SERVER-NAME-PERSISTENCE | design data/state | `JoinRecord` persists optional `server_name`, and missing old records still parse as `None` | code review plus unit artifact | pass |
| AR-P2PSN-NAME-WIRING | design interface `P2pSn::new` name parameter | `P2pSn::new` receives the computed effective SN remote name instead of unconditional `sn_id.to_string()` | code review plus DV artifact | pass |
| AR-HARNESS-SCOPE | harness stage-scope rules | Stage scope checks can cleanly bind changes to allowed files | stage-scope command results | gap |
| AR-WHOLE-PROJECT-EVIDENCE | acceptance rules | Whole-project unified test evidence exists before accepted conclusion | `test-run.py all all` artifact | gap |

## Required Command Evidence
- `schema-check.py`: passed for `v0.1 / bucky-vpn` after proposal, design, implementation, and testing updates.
- `admission-check.py`: passed with `harness/evidence/admission/20260706-bucky-vpn-join-server-name-for-sn.md`; stamp written for `vpn-client/src/main.rs`, `vpn-client/src/cli.rs`, `vpn-client/src/api.rs`, and `vpn-client/src/p2p_vpn.rs`.
- `stage-scope-check.py`: proposal/design/implementation/testing scope checks were attempted and failed due the current mixed worktree; implementation output shows admitted production paths are separate from unrelated tracked files but scope cannot pass until the worktree is isolated.
- `test-run.py <module> all`: not run as a single all-level command; targeted `bucky-vpn` unit and DV entries passed.
- `test-run.py all all`: not run; this blocks an accepted conclusion.
- `quality-check.py`: passed; no quality gates configured because `harness/quality-gates.yaml` declares an explicitly empty gates list.
- `test-run.py bucky-vpn unit`: passed; artifact `test-results/test-runs/20260706T073344Z-bucky-vpn-unit.json`.
- `test-run.py bucky-vpn dv`: passed; artifact `test-results/test-runs/20260706T073427Z-bucky-vpn-dv.json`.
- `doc-structure-check.py --docs testing`: passed.
- `testing-coverage-check.py`: passed.
- `cargo fmt --all`: passed.

## Consistency Summary
- Proposal authority check: approved proposal directly contains `PROP-client-join-server-name-for-sn` and states that `join` / `/join` support optional `server_name`, with domain/IP defaults.
- Proposal vs design: design preserves the proposal boundary and maps `CHG-client-join-server-name-for-sn` to CLI, local API, persisted records, client key, and `P2pSn::new` name wiring.
- Design vs implementation: implementation adds the CLI/API field, persists `JoinRecord.server_name`, introduces structured `P2pVpnClientKey`, preserves legacy key parsing, and passes `effective_server_name` to `P2pSn::new`.
- Test design adequacy: unit coverage is appropriate for pure defaulting/key behavior, and DV validates the changed CLI/API/factory code compiles together.
- change_id traceability: proposal, design, admission evidence, testing metadata, and testplan all name `CHG-client-join-server-name-for-sn`.
- Document logic review: no contradiction found between proposal, design, testing, and the p2p-frame ownership boundary for live SN tunnel validation.
- Implementation logic review: no change was made to `join --name` semantics, identity directory naming, VPN protocol schema, or service-side behavior; old manager keys remain parseable.

## Conclusion
- accepted / rejected / needs changes: needs changes
- reason: The implementation and targeted validation satisfy the requested client-owned behavior, but final acceptance is blocked by mixed-worktree stage-scope failures and missing whole-project `test-run.py all all` evidence.

## Follow-Up Tasks
- Iteration count: 1
- Proposal task: none for behavior; proposal is approved and consistent.
- Design task: rerun proposal/design stage-scope in an isolated worktree or after unrelated tracked changes are committed/stashed.
- Implementation task: rerun implementation stage-scope for `CHG-client-join-server-name-for-sn` in an isolated worktree; production edit is confined to the admitted four client paths.
- Testing task: run broader `test-run.py bucky-vpn all` or `test-run.py all all` when final acceptance evidence is required.
