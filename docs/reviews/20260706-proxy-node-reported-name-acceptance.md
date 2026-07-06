# Proxy Node Reported Name Acceptance Report

## Findings
| id | severity | stage | evidence | problem | fail_condition_hit |
| --- | --- | --- | --- | --- | --- |
| ACC-PROXY-NAME-001 | medium | external-environment | `stage-scope-check.py --stage testing --version v0.1 --module vpn-frame`, `bucky-vpn-server`, and `bucky-vpn` failed because the current worktree contains broad pre-existing cross-stage, generated, artifact, and unrelated tracked/untracked changes. | The reviewed implementation is mapped to admitted scope paths, but final stage-scope evidence cannot be cleanly isolated in this mixed worktree. | stage-scope evidence is not cleanly passable |
| ACC-PROXY-NAME-002 | medium | acceptance | `test-run.py all all` was not run. Fresh available artifacts are `test-results/test-runs/20260706T085738Z-vpn-frame-unit.json`, `test-results/test-runs/20260706T085801Z-bucky-vpn-server-unit.json`, and `test-results/test-runs/20260706T085748Z-bucky-vpn-unit.json`. | Final accepted conclusion requires whole-project unified test evidence, but this run only executed the relevant module unit levels plus direct cargo tests. | whole-project test evidence missing |

## Evidence Coverage
| documented_item | source_document | implementation_evidence | test_result_evidence | status |
| --- | --- | --- | --- | --- |
| `CHG-pn-server-reported-name-contract` | `docs/versions/v0.1/modules/vpn-frame/proposal.md`; `docs/versions/v0.1/modules/vpn-frame/design.md` | `vpn-frame/src/vpn_protocol.rs` adds optional `PnServerInfo.name`, normalizes names with `with_name`, and exposes `remote_name` fallback to id. | `test-results/test-runs/20260706T085738Z-vpn-frame-unit.json` reports 3 passed tests including `pn_server_info_normalizes_optional_remote_name`. | consistent |
| `CHG-server-identity-cert-name` | `docs/versions/v0.1/modules/bucky-vpn-server/proposal.md`; `docs/versions/v0.1/modules/bucky-vpn-server/design.md` | `vpn-server/config/config.example.yaml` documents top-level `name`; `vpn-server/src/server_config.rs` reads it; `vpn-server/src/main.rs` loads existing identity and re-signs the cert with the configured name while preserving the encoded private key bytes. | `test-results/test-runs/20260706T085801Z-bucky-vpn-server-unit.json` reports 36 passed tests including `yaml_can_configure_server_name`; direct `cargo test -p bucky-vpn-server` also passed. | consistent |
| `CHG-server-proxy-node-reported-name` | `docs/versions/v0.1/modules/bucky-vpn-server/proposal.md`; `docs/versions/v0.1/modules/bucky-vpn-server/design.md` | `vpn-server/src/main.rs` attaches configured name to local PN reports; `vpn-server/src/api.rs` and `vpn-server/src/sqlite_store_factory.rs` persist and project `pn_server_name`; `vpn-server/src/server_config.rs` preserves reported names while merging observed addresses. | `test-results/test-runs/20260706T085801Z-bucky-vpn-server-unit.json` covers selector and store-backed name preservation paths. | consistent |
| `CHG-client-pn-proxy-reported-name` | `docs/versions/v0.1/modules/bucky-vpn/proposal.md`; `docs/versions/v0.1/modules/bucky-vpn/design.md` | `vpn-client/src/p2p_vpn.rs` keeps `PnServerInfo.id` as remote id and uses `PnServerInfo::remote_name()` as the p2p-frame `TtpTarget.remote_name`. | `test-results/test-runs/20260706T085748Z-bucky-vpn-unit.json` reports 17 passed tests; shared name/fallback behavior is covered by the vpn-frame unit artifact. | consistent |
| Testing metadata and unified entries | `docs/versions/v0.1/modules/*/testing.md`; `docs/versions/v0.1/modules/*/testplan.yaml` | Testing docs and testplans include all four new change IDs with case-type coverage and runnable unit entries. | `doc-structure-check.py --docs testing`, `testing-coverage-check.py`, and `schema-check.py` passed for `vpn-frame`, `bucky-vpn-server`, and `bucky-vpn`. | consistent |

## Test Design Adequacy
| behavior_risk_change_id | required_case_types | test_design_evidence | runnable_test_evidence | status |
| --- | --- | --- | --- | --- |
| `CHG-pn-server-reported-name-contract` | normal, boundary, negative, error, compatibility, lifecycle, cross-module | `vpn-frame/testing.md` Direct Change Coverage, Case-Type Coverage, Design Element Coverage, Unit Tests, DV/Integration rows | `test-results/test-runs/20260706T085738Z-vpn-frame-unit.json` | adequate for shared value behavior |
| `CHG-server-identity-cert-name` | normal, boundary, negative, error, compatibility, lifecycle, cross-module | `bucky-vpn-server/testing.md` records YAML parsing, identity re-sign compile/reload path, and explicit live smoke gap | `test-results/test-runs/20260706T085801Z-bucky-vpn-server-unit.json`; direct cargo tests passed | adequate with recorded live certificate smoke gap |
| `CHG-server-proxy-node-reported-name` | normal, boundary, negative, error, compatibility, lifecycle, cross-module | `bucky-vpn-server/testing.md` maps heartbeat/report merge, API/store projection, and SQLite-backed list path | `test-results/test-runs/20260706T085801Z-bucky-vpn-server-unit.json` | adequate for server-owned propagation |
| `CHG-client-pn-proxy-reported-name` | normal, boundary, negative, error, compatibility, lifecycle, cross-module | `bucky-vpn/testing.md` maps client target-name wiring to shared unit behavior and client crate compatibility | `test-results/test-runs/20260706T085748Z-bucky-vpn-unit.json`; `test-results/test-runs/20260706T085738Z-vpn-frame-unit.json` | adequate for client-owned wiring |
| Whole-project final evidence | cross-module regression detection | Acceptance rules require `test-run.py all all` before accepted conclusion | no `all all` artifact produced in this run | gap |

## Generated Acceptance Rules
| rule_id | source | expected_result | evidence_required | status |
| --- | --- | --- | --- | --- |
| AR-PROTOCOL-NAME | `CHG-pn-server-reported-name-contract` | `PnServerInfo.name` is optional, blank names normalize to absent, and absent names fall back to id. | protocol code review plus vpn-frame unit artifact | pass |
| AR-SERVER-CERT-NAME | `CHG-server-identity-cert-name` | Configured server `name` is used for generated certs; changing name re-signs the certificate while keeping the existing private key. | server code review, config unit test, and server cargo/unit artifact | pass |
| AR-SERVER-REPORT-NAME | `CHG-server-proxy-node-reported-name` | Proxy node reports carry configured name, control node persists/returns it, and observed-address merges do not drop it. | server code review plus selector/store unit artifact | pass |
| AR-CLIENT-CONNECT-NAME | `CHG-client-pn-proxy-reported-name` | Client connects with reported proxy name when present while preserving id as the remote P2P identity. | client code review plus client/unit and shared protocol unit artifacts | pass |
| AR-HARNESS-SCOPE | acceptance review gate | Stage-scope checks can cleanly bind the reviewed changes to the allowed stage and admitted scope. | `stage-scope-check.py` results | gap |
| AR-WHOLE-PROJECT-EVIDENCE | acceptance review gate | A fresh `test-run.py all all` artifact exists before accepted conclusion. | `test-results/test-runs/*-all-all.json` | gap |

## Required Command Evidence
- `schema-check.py`: passed for `v0.1 / vpn-frame`, `v0.1 / bucky-vpn-server`, and `v0.1 / bucky-vpn` after proposal, design, implementation, and testing updates.
- `admission-check.py`: passed for `CHG-pn-server-reported-name-contract`, `CHG-server-identity-cert-name`, `CHG-server-proxy-node-reported-name`, and `CHG-client-pn-proxy-reported-name`; stamps exist under `harness/evidence/admission/20260706-*.stamp.json`.
- `stage-scope-check.py`: attempted for testing and failed for all three modules due the current mixed worktree with many pre-existing unrelated/cross-stage/generated changes; design-stage scope had the same recorded limitation.
- `test-run.py <module> all`: not run as a single all-level command; targeted module unit entries passed through the unified runner.
- `test-run.py all all`: not run; this blocks an accepted conclusion.
- `quality-check.py`: passed with no run artifact required because `harness/quality-gates.yaml` declares `gates: []`.
- `test-run.py vpn-frame unit`: passed; artifact `test-results/test-runs/20260706T085738Z-vpn-frame-unit.json`.
- `test-run.py bucky-vpn-server unit`: passed; artifact `test-results/test-runs/20260706T085801Z-bucky-vpn-server-unit.json`.
- `test-run.py bucky-vpn unit`: passed; artifact `test-results/test-runs/20260706T085748Z-bucky-vpn-unit.json`.
- `doc-structure-check.py --docs testing`: passed for `vpn-frame`, `bucky-vpn-server`, and `bucky-vpn`.
- `testing-coverage-check.py`: passed for `vpn-frame`, `bucky-vpn-server`, and `bucky-vpn`.
- `cargo fmt --all`: passed.
- `cargo test -p vpn-frame`: passed, 3 tests.
- `cargo test -p bucky-vpn-server`: passed, 36 tests.
- `cargo test -p bucky-vpn`: passed, 17 tests.

## Consistency Summary
- Proposal authority check: Approved proposal entries directly cover optional PN server reported names, server certificate naming with private-key preservation, server propagation through control/API/store, and client use when connecting.
- Proposal vs design: Designs preserve the proposal boundary and map the behavior to stable change IDs across `vpn-frame`, `bucky-vpn-server`, and `bucky-vpn`.
- Design vs implementation: Implementation follows the approved path list: shared protocol field/fallback, server config/identity/report/store/API propagation, and client connection target name wiring.
- Test design adequacy: Unit tests cover the lowest practical behavior for name normalization, config parsing, selector/store propagation, and client compile-time wiring; live certificate and whole-project integration evidence remain recorded gaps.
- change_id traceability: Proposal, design, admission evidence, testing metadata, testplans, and unit run artifacts all reference the four current change IDs.
- Document logic review: No contradiction found between proposal, design, testing metadata, and implementation; recorded gaps align with ownership boundaries and current environment limits.
- Implementation logic review: No defect found in reviewed code paths; identity id remains based on the preserved private key, `PnServerInfo.id` remains the P2P identity, and `name` is optional metadata used for certificate/remote-name matching.

## Conclusion
- accepted / rejected / needs changes: needs changes
- reason: The requested behavior is implemented and targeted module validation passed, but final acceptance is mechanically blocked by mixed-worktree stage-scope failures and missing whole-project `test-run.py all all` evidence.

## Follow-Up Tasks
- Iteration count: 1
- Proposal task: none for behavior; current proposal coverage is approved and consistent.
- Design task: none for behavior; current design coverage is approved and consistent.
- Implementation task: rerun implementation/testing stage-scope in an isolated worktree or after unrelated tracked/generated changes are separated; reviewed production paths match admission.
- Testing task: run broader `test-run.py <module> all` and `test-run.py all all` when final acceptance evidence is required.
