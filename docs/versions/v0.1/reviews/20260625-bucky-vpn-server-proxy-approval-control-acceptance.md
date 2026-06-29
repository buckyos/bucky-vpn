# bucky-vpn-server Proxy Approval Control Acceptance

## Findings
| id | severity | stage | evidence | problem | fail_condition_hit |
| --- | --- | --- | --- | --- | --- |
| ACC-003 | high | testing/environment | `test-results/test-runs/20260625T093341Z-all-all.json` | Whole-project `test-run.py all all` reached `vpn_web` and failed at `flutter test` because Flutter attempted to move `/mnt/c/flutter/bin/cache/dart-sdk` to `dart-sdk.old` and received permission denied. Rust workspace and `bucky-vpn-server` steps passed before this environment failure. | Required fresh passing whole-project test artifact is missing, so final acceptance cannot be `accepted`. |
| ACC-005 | high | implementation/testing | `stage-scope-check.py --stage implementation --ignore-untracked` and `stage-scope-check.py --stage acceptance --ignore-untracked` | Stage scope checks are blocked by pre-existing tracked dirty paths outside the admitted proxy approval scope, including `vpn-frame`, `vpn-client`, `vpn_web`, README, build script, and older server files. | Required stage-scope evidence is failing in normal working-tree mode. |
| ACC-006 | medium | testing | `docs/versions/v0.1/modules/bucky-vpn-server/testing.md` | The new SQLite approval and HTTP approval behavior is represented by compile, unit, DV and workspace evidence, but direct HTTP smoke, selector-with-store assertion, DB restart fixture, and rejected-proxy runtime assertion remain documented gaps. | Test design has explicit gaps for lifecycle, negative, error and HTTP boundary behavior. |

## Evidence Coverage
| documented_item | source_document | implementation_evidence | test_result_evidence | status |
| --- | --- | --- | --- | --- |
| External proxy approval state is persisted in SQLite as pending, approved or rejected and is separate from heartbeat liveness. | `proposal.md` and `design.md` rows for `CHG-external-pn-approval-persistence` | `vpn-server/src/sqlite_store_factory.rs` defines `pn_proxy_node`, `ProxyNodeApprovalStatus`, pending insert, approval upsert, approved lookup and list query. `vpn-server/src/server_config.rs` uses the store-backed selector. | `test-results/test-runs/20260625T092629Z-bucky-vpn-server-all.json` passed; `test-results/test-runs/20260625T093341Z-all-all.json` passed Rust/server steps before Flutter failure. | consistent with testing gaps |
| Remote proxy selection requires both heartbeat liveness and approval when SQLite store is attached. | `design.md` Key Call Flows and Directly Mapped Change Items | `ConfigPnServerSelector::is_valid` and `select` require `live_remote_pn_servers` plus `is_remote_approved`; `report_heartbeat` creates pending rows only. | Existing selector TTL tests pass in the server test artifact; store-backed branch has compile/DV coverage but no direct fixture assertion. | gap |
| HTTP approval control API lists, approves and rejects proxy nodes through the existing Bearer session boundary. | `proposal.md` and `design.md` rows for `CHG-external-pn-approval-http-api` | `vpn-server/src/api.rs` registers `GET /pn_proxy_nodes`, `POST /approve_pn_proxy_node`, and `POST /reject_pn_proxy_node`; `vpn-server/src/main.rs` injects the shared selector into API registration. | Server artifact passed crate and workspace Rust tests; no direct HTTP smoke fixture exists. | gap |
| The implementation is constrained to the admitted design Scope Paths for this change. | Admission evidence `harness/evidence/admission/20260625-proxy-approval-control.md` | Admission passed and listed `vpn-server/src/sqlite_store_factory.rs`, `vpn-server/src/server_config.rs`, `vpn-server/src/main.rs`, and `vpn-server/src/api.rs`. | Normal stage-scope-check failed because unrelated tracked dirty paths are present in the working tree. | inconsistent |
| Whole-project validation remains invokable through the unified test entrypoint. | Acceptance task rules and `testplan.yaml` | `harness/scripts/test-run.py all all` executed and wrote a machine artifact. | `test-results/test-runs/20260625T093341Z-all-all.json` exists with `exit_code: 1` due Flutter cache permission failure. | missing |

## Test Design Adequacy
| behavior_risk_change_id | required_case_types | test_design_evidence | runnable_test_evidence | status |
| --- | --- | --- | --- | --- |
| CHG-external-pn-approval-persistence | normal, boundary, negative, error, compatibility, lifecycle | `testing.md` maps schema/store/selector integration, missing-row pending creation, old DB compatibility, rejected proxy exclusion and restart persistence. It records direct SQLite restart and DB failure fixtures as deferred. | `test-results/test-runs/20260625T092629Z-bucky-vpn-server-all.json` passed server unit/DV/integration; no direct SQLite fixture artifact exists. | gap |
| CHG-external-pn-approval-http-api | normal, boundary, negative, error, compatibility, lifecycle | `testing.md` maps route registration and DTO compile coverage, and explicitly records missing HTTP smoke for auth failure, repeated approve/reject and approve-to-select lifecycle. | Server DV and workspace Rust steps passed; no HTTP runtime fixture artifact exists. | gap |
| CHG-external-pn-active-control and CHG-pn-sn-heartbeat | normal, negative, error, compatibility, lifecycle, cross-module | Existing selector heartbeat unit tests cover heartbeat admission and expiry. Testing document records reporter error log assertion and full multi-process runtime smoke as deferred. | Server artifact passed 20 tests and workspace Rust steps; all/all failed only after Rust steps. | gap |

## Generated Acceptance Rules
| rule_id | source | expected_result | evidence_required | status |
| --- | --- | --- | --- | --- |
| AR-001 | Proposal approval persistence requirement | First heartbeat creates a durable pending row; approved remote proxies may be selected only while live; rejected or missing approval proxies are not selected. | Code review of store and selector plus runnable tests or explicit gap evidence. | gap |
| AR-002 | Proposal HTTP API requirement | Authenticated HTTP clients can list proxy nodes and approve or reject them; invalid sessions fail through the existing Bearer session path. | API route implementation plus HTTP smoke or documented gap. | gap |
| AR-003 | Harness implementation admission rule | Production code changes are limited to admitted Scope Paths for the approved change IDs. | Passing `stage-scope-check.py --stage implementation` in normal working-tree mode. | fail |
| AR-004 | Acceptance whole-project evidence rule | `test-run.py all all` produces a fresh passing artifact. | `test-results/test-runs/*.json` with requested module and level `all all`, exit code 0. | fail |
| AR-005 | Quality gate rule | Repository quality gate command is executed and configured gates are passing. | `quality-check.py` result and `harness/quality-gates.yaml` state. | pass |

## Required Command Evidence
- schema-check.py: `uv run --active python ./harness/scripts/schema-check.py --version v0.1 --module bucky-vpn-server` passed.
- admission-check.py: `uv run --active python ./harness/scripts/admission-check.py --version v0.1 --module bucky-vpn-server --change-id CHG-external-pn-approval-persistence --change-id CHG-external-pn-approval-http-api --evidence-file harness/evidence/admission/20260625-proxy-approval-control.md` passed and rewrote the admission stamp.
- stage-scope-check.py: `uv run --active python ./harness/scripts/stage-scope-check.py --stage implementation --version v0.1 --module bucky-vpn-server --change-id CHG-external-pn-approval-persistence --change-id CHG-external-pn-approval-http-api --ignore-untracked` failed due unrelated tracked dirty paths outside admitted Scope Paths.
- test-run.py <module> all: `python3 ./harness/scripts/test-run.py bucky-vpn-server all` passed; artifact `test-results/test-runs/20260625T092629Z-bucky-vpn-server-all.json`.
- test-run.py all all: `uv run --active python ./harness/scripts/test-run.py all all` failed at `vpn_web` Flutter cache update; artifact `test-results/test-runs/20260625T093341Z-all-all.json`.
- quality-check.py: `uv run --active python ./harness/scripts/quality-check.py` passed because `harness/quality-gates.yaml` declares `gates: []`.

## Consistency Summary
- Proposal authority check: Approved proposal directly requires SQLite-backed approval state and HTTP approval APIs for external proxy nodes.
- Proposal vs design: Approved design maps the new requirements to `CHG-external-pn-approval-persistence` and `CHG-external-pn-approval-http-api` and names concrete state, status and endpoint behavior.
- Design vs implementation: Implementation follows the design shape: SQLite table and status enum, pending creation on heartbeat, approved-and-live selector filtering, and three HTTP routes wired from `main.rs`.
- Test design adequacy: Testing metadata maps both new change IDs and records explicit gaps; the gaps are meaningful enough to block final accepted status until direct fixtures or a conscious later acceptance decision exists.
- change_id traceability: Proposal, design, admission evidence, testing metadata and testplan all contain the two new change IDs.
- Document logic review: No contradiction found between proposal and design; testing document honestly narrows runtime proof to compile/unit/DV evidence and gap statements.
- Implementation logic review: No direct code defect found in the reviewed approval path; remaining risks are untested runtime behavior around HTTP auth/error branches, SQLite restart and rejected-proxy selection with a real store.

## Follow-Up Tasks
- Iteration count: 2
- Return target: testing/environment and implementation working-tree hygiene.
- Required next action: Fix the Flutter SDK cache permission issue or provide a runnable Flutter toolchain so `test-run.py all all` can pass.
- Required next action: Re-run implementation and acceptance stage-scope checks in a working tree without unrelated tracked dirty paths, or isolate this task in a clean branch/worktree.
- Required next action: Add direct coverage or explicitly re-scope acceptance for proxy approval lifecycle, including selector-with-store, reject/unapproved exclusion, HTTP auth failure and approve/reject routes.
- Unresolved risk: The service behavior likely satisfies the approved implementation intent, but the Harness acceptance bar is intentionally evidence-based and is not met by compile-only coverage plus a failing all-project artifact.

## Conclusion
Accepted / Rejected / Needs Changes: needs changes

The implementation is present and coherent for the requested SQLite approval state and HTTP approval API, but final acceptance is blocked by missing passing whole-project evidence, failing scope checks in the current dirty worktree, and explicit direct-test gaps for approval lifecycle behavior.
