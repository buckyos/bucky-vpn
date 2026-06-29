# vpn-frame PN Server Info Acceptance

## Findings
| id | severity | stage | evidence | problem | fail_condition_hit |
|----|----------|-------|----------|---------|--------------------|
| ACC-flutter-sdk-permission | high | acceptance | `test-results/test-runs/20260626T023834Z-all-all.json` | Whole-project `test-run.py all all` fails at `flutter test` because Flutter cannot move `/mnt/c/flutter/bin/cache/dart-sdk` to `/mnt/c/flutter/bin/cache/dart-sdk.old` due permission denied. Rust and repo-governance steps pass before this failure. | accepted conclusion requires a fresh passing whole-project test run artifact |
| ACC-mixed-dirty-worktree | high | implementation | `stage-scope-check.py --stage implementation --version v0.1 --module vpn-frame --change-id CHG-pn-server-info-contract --ignore-untracked` | Implementation scope check sees pre-existing modified tracked files outside admitted Scope Paths, including README/build/UI/runtime files not owned by this task. The task cannot produce clean mechanical stage-scope evidence in the current mixed worktree. | implementation diff was not mechanically isolated to admitted Scope Paths |
| ACC-clippy-baseline | medium | validation | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Clippy fails on existing broad workspace warnings such as `dead_code`, `type_complexity`, `result_large_err`, and `unnecessary_unwrap` across client/server files. Fixing these would be a broad cleanup outside the PN server contract scope. | repository delivery checklist asks for clippy, but the current baseline is not clippy-clean |

## Conclusion
- Accepted / rejected / needs changes: needs changes
- Reason: The corrected PN server id/ip/port contract is implemented and vpn-frame validation passes, but final acceptance is blocked by the whole-project Flutter SDK permission failure, mixed dirty-worktree stage-scope evidence, and non-clean clippy baseline.

## Evidence Coverage
| documented_item | source_document | implementation_evidence | test_result_evidence | status |
|-----------------|-----------------|-------------------------|----------------------|--------|
| `NodeNetwork.pn_server` carries structured id/ip/port | docs/versions/v0.1/modules/vpn-frame/proposal.md `PROP-pn-server-info` | `vpn-frame/src/vpn_protocol.rs` defines `PnServerInfo` and changes `NodeNetwork.pn_server` to `Option<PnServerInfo>` | `test-results/test-runs/20260626T023236Z-vpn-frame-unit.json`; `test-results/test-runs/20260626T023418Z-vpn-frame-all.json` | consistent |
| PN server id is the `vpn-server` P2P node id | docs/versions/v0.1/modules/vpn-frame/proposal.md Assumptions and Success Criteria | `vpn-server/src/main.rs` constructs local PN server info with `local_id.to_string()`; `vpn-server/src/server_config.rs` constructs `PnServerInfo` from id plus endpoint address | `test-results/test-runs/20260626T023345Z-vpn-frame-integration.json` | consistent |
| SQLite PN server storage is id/ip/port, not endpoint | docs/versions/v0.1/modules/vpn-frame/design.md SQLite interfaces | `vpn-server/src/sqlite_store_factory.rs` uses `pn_server_id`, `pn_server_ip`, and `pn_server_port` for `network` and `pn_proxy_node` | `test-results/test-runs/20260626T023345Z-vpn-frame-integration.json` | consistent |
| Old endpoint-string compatibility is not preserved | docs/versions/v0.1/modules/vpn-frame/proposal.md Out of scope | `PnServerInfo::from_endpoint_string` and `endpoint()` helpers were removed; no SQL path reads old `pn_server` endpoint columns | `test-results/test-runs/20260626T023418Z-vpn-frame-all.json` | consistent |
| Client derives endpoint only at P2P boundary | docs/versions/v0.1/modules/vpn-frame/design.md Key Call Flows | `vpn-client/src/p2p_vpn.rs` parses `PnServerInfo.id` as the remote P2P id and derives `Endpoint` from ip/port for the P2P call | `test-results/test-runs/20260626T023345Z-vpn-frame-integration.json` | consistent |
| HTTP API no longer exposes endpoint string PN server values | docs/versions/v0.1/modules/vpn-frame/design.md HTTP API interface | `vpn-server/src/api.rs` uses `JsonPnServerInfo { id, ip, port }` for proxy-node approval/list and network responses | `test-results/test-runs/20260626T023345Z-vpn-frame-integration.json` | consistent |

## Test Design Adequacy
| behavior_risk_change_id | required_case_types | test_design_evidence | runnable_test_evidence | status |
|-------------------------|---------------------|----------------------|------------------------|--------|
| CHG-pn-server-info-contract | normal, boundary, negative, error, compatibility, lifecycle, cross-module | docs/versions/v0.1/modules/vpn-frame/testing.md Case-Type Coverage and Design Element Coverage | `test-results/test-runs/20260626T023236Z-vpn-frame-unit.json`; `test-results/test-runs/20260626T023250Z-vpn-frame-dv.json`; `test-results/test-runs/20260626T023345Z-vpn-frame-integration.json`; `test-results/test-runs/20260626T023418Z-vpn-frame-all.json` | runnable |
| final project gate | whole-project | harness/rules/acceptance-review-rules.md requires `test-run.py all all` | `test-results/test-runs/20260626T023834Z-all-all.json` | not passing |

## Generated Acceptance Rules
| rule_id | source | expected_result | evidence_required | status |
|---------|--------|-----------------|-------------------|--------|
| AR-pn-server-info-shape | proposal/design | `NodeNetwork.pn_server` is structured as id/ip/port | code inspection plus vpn-frame unit tests | pass |
| AR-pn-server-id-source | proposal/design | PN server id comes from the server P2P node id, not endpoint/ip/port | `main.rs` and `server_config.rs` inspection | pass |
| AR-no-endpoint-storage | proposal/design | network and proxy-node PN server storage uses id/ip/port fields | SQLite schema and SQL inspection | pass |
| AR-consumer-compatibility | design Interfaces and Dependencies | client/server consumers compile with structured PN server data | workspace test evidence | pass |
| AR-final-project-gate | acceptance-review-rules.md | `test-run.py all all` passes with fresh artifact | whole-project test-run artifact | fail |
| AR-stage-scope | task-entry-gate-rules.md | implementation diff is mechanically bound to admitted Scope Paths | stage-scope-check result | fail |

## Required Command Evidence
- schema-check.py: passed with `uv run --active python ./harness/scripts/schema-check.py --version v0.1 --module vpn-frame`
- admission-check.py: passed with `uv run --active python ./harness/scripts/admission-check.py --version v0.1 --module vpn-frame --change-id CHG-pn-server-info-contract --evidence-file harness/evidence/admission/20260626-pn-server-info-contract.md`
- cargo check: passed with `cargo check --workspace`
- cargo clippy: failed with `cargo clippy --workspace --all-targets --all-features -- -D warnings` due existing workspace warnings; see finding ACC-clippy-baseline
- stage-scope-check.py: failed because the worktree contains unrelated modified files outside admitted scope; see finding ACC-mixed-dirty-worktree
- test-run.py vpn-frame unit: passed, artifact `test-results/test-runs/20260626T023236Z-vpn-frame-unit.json`
- test-run.py vpn-frame dv: passed, artifact `test-results/test-runs/20260626T023250Z-vpn-frame-dv.json`
- test-run.py vpn-frame integration: passed, artifact `test-results/test-runs/20260626T023345Z-vpn-frame-integration.json`
- test-run.py vpn-frame all: passed, artifact `test-results/test-runs/20260626T023418Z-vpn-frame-all.json`
- test-run.py <module> all: passed for vpn-frame, artifact `test-results/test-runs/20260626T023418Z-vpn-frame-all.json`
- test-run.py all all: failed at Flutter SDK cache permission, artifact `test-results/test-runs/20260626T023834Z-all-all.json`
- quality-check.py: passed; `harness/quality-gates.yaml` declares an explicitly empty `gates: []` list, so no quality run artifact was written

## Consistency Summary
- Proposal authority check: proposal now requires no old endpoint compatibility, P2P-node-id identity, and structured SQLite storage; implementation follows that scope.
- Proposal vs design: design maps `PROP-pn-server-info` to `CHG-pn-server-info-contract` and includes protocol, selector, heartbeat, storage, client connection, and HTTP API scope paths.
- Design vs implementation: implementation removes endpoint helper semantics, passes `PnServerInfo` through selector/report/store/API flows, and derives transport endpoints only at the client P2P boundary.
- Test design adequacy: vpn-frame unit, DV, and integration evidence cover protocol value behavior and workspace consumer compatibility; final whole-project gate remains blocked by Flutter SDK permissions.
- change_id traceability: proposal, design, testing, admission evidence, and testplan all reference `CHG-pn-server-info-contract`.
- Document logic review: no contradiction found between proposal/design/testing for the corrected PN server info contract.
- Implementation logic review: no endpoint string remains as PN server id or structured storage format in the admitted implementation paths.

## Follow-Up Tasks
- Iteration count: 2
- Fix `ACC-flutter-sdk-permission` by making `/mnt/c/flutter/bin/cache` writable for the test user, pre-populating the Flutter/Dart cache, or running Flutter tests outside root with a writable SDK cache.
- Fix `ACC-mixed-dirty-worktree` by isolating this task in a clean worktree or committing/stashing unrelated existing changes before rerunning implementation `stage-scope-check.py`.
- Fix `ACC-clippy-baseline` with a separate cleanup change_id that brings the workspace to a clippy-clean baseline under `-D warnings`.
