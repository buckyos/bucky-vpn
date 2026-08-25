# PN Endpoint Live Address Acceptance Report

## Findings
| id | severity | stage | evidence | problem | fail_condition_hit |
|----|----------|-------|----------|---------|--------------------|
| ACC-PN-ENDPOINT-001 | high | testing | `multipass version` failed with missing `/var/snap/multipass/common/data/multipassd/multipass_root_cert.pem`; `test-run.py bucky-vpn integration` not run | Full live multi-VM PN proxy topology evidence is unavailable in this environment. Unit, DV, and workspace integration passed, but live client/proxy/control behavior is not fully proven. | required whole-project / live integration evidence missing |
| ACC-PN-ENDPOINT-002 | medium | implementation/testing | `stage-scope-check.py --stage implementation ...` and `stage-scope-check.py --stage testing ...` failed | The shared worktree contains pre-existing untracked/generated files, prior admission artifacts, earlier stage docs, and unrelated tracked diffs, so mechanical stage scope could not produce a clean pass. | implementation diff scope binding not cleanly proven by checker |

## Evidence Coverage
| documented_item | source_document | implementation_evidence | test_result_evidence | status |
|-----------------|-----------------|-------------------------|----------------------|--------|
| Shared PN address is Endpoint-shaped, not split `ip`/`port` | `docs/versions/v0.1/modules/vpn-frame/proposal.md`, `design.md` | `vpn-frame/src/vpn_protocol.rs` uses `Endpoint` and `PnServerInfo.endpoints`; `vpn-client/src/p2p_vpn.rs` consumes endpoints directly | `test-results/test-runs/20260706T161358Z-vpn-frame-unit.json`, `test-results/test-runs/20260706T162012Z-bucky-vpn-unit.json`, `test-results/test-runs/20260706T162332Z-vpn-frame-integration.json` | consistent |
| Control node combines observed proxy-control IP with reported/mapped Endpoint port | `docs/versions/v0.1/modules/bucky-vpn-server/proposal.md`, `design.md` | `vpn-server/src/server_config.rs` rewrites reported endpoints with observed IP in both heartbeat orderings | `test-results/test-runs/20260706T161529Z-bucky-vpn-server-unit.json`, `test-results/test-runs/20260706T162401Z-bucky-vpn-server-integration.json` | consistent |
| Proxy-node transport endpoints are live state, not SQLite truth | `docs/versions/v0.1/modules/vpn-frame/proposal.md`, `docs/versions/v0.1/modules/bucky-vpn-server/proposal.md` | `vpn-server/src/sqlite_store_factory.rs` stores id/name with blank endpoint columns; selector returns live endpoints when available | `test-results/test-runs/20260706T161529Z-bucky-vpn-server-unit.json` | consistent |
| Client connects to server-returned Endpoint values | `docs/versions/v0.1/modules/bucky-vpn/proposal.md`, `design.md` | `vpn-client/src/p2p_vpn.rs` orders/deduplicates `PnServerInfo.endpoints` and converts protocol to p2p-frame `Endpoint` | `test-results/test-runs/20260706T162012Z-bucky-vpn-unit.json`, `test-results/test-runs/20260706T162050Z-bucky-vpn-dv.json` | consistent |
| Full live PN proxy runtime across real processes | `docs/versions/v0.1/modules/bucky-vpn/testing.md`, `testplan.yaml` | implementation is present, but runtime proof needs Multipass integration | Multipass unavailable; no `bucky-vpn integration` artifact for this run | missing |

## Test Design Adequacy
| behavior_risk_change_id | required_case_types | test_design_evidence | runnable_test_evidence | status |
|-------------------------|---------------------|----------------------|------------------------|--------|
| `CHG-pn-server-endpoint-address-contract` | normal, boundary, compatibility, cross-module | `vpn-frame/testing.md` maps Endpoint IPv4/IPv6 and cross-crate compatibility | `test-results/test-runs/20260706T161358Z-vpn-frame-unit.json`, `test-results/test-runs/20260706T162332Z-vpn-frame-integration.json` | adequate |
| `CHG-pn-server-address-live-state-contract` | normal, boundary, negative, lifecycle, cross-module | `vpn-frame/testing.md` records live selector/store behavior; server tests cover store-backed list | `test-results/test-runs/20260706T161529Z-bucky-vpn-server-unit.json`, `test-results/test-runs/20260706T162401Z-bucky-vpn-server-integration.json` | adequate |
| `CHG-pn-port-mapping-observed-address` | normal, boundary, lifecycle, cross-module | `bucky-vpn-server/testing.md` maps observed IP plus mapped/reported port synthesis | `test-results/test-runs/20260706T161529Z-bucky-vpn-server-unit.json` | adequate |
| `CHG-client-pn-proxy-endpoint-address` | normal, boundary, negative, error, compatibility, cross-module | `bucky-vpn/testing.md` maps endpoint ordering, dedupe, protocol conversion, and invalid protocol | `test-results/test-runs/20260706T162012Z-bucky-vpn-unit.json`, `test-results/test-runs/20260706T162050Z-bucky-vpn-dv.json` | adequate |
| live multi-process PN proxy topology | lifecycle, cross-module, data-plane | Existing `bucky-vpn/testplan.yaml` has a Multipass integration path | Not runnable here because Multipass is unavailable | gap |

## Generated Acceptance Rules
| rule_id | source | expected_result | evidence_required | status |
|---------|--------|-----------------|-------------------|--------|
| ACC-RULE-001 | approved proposals/designs | PN server addresses are Endpoint-shaped at shared protocol, server API, and client connect boundaries | code inspection plus unit/DV/workspace artifacts | pass |
| ACC-RULE-002 | approved server design | Control node must synthesize live proxy Endpoint from observed IP and reported/mapped port | server selector tests and implementation inspection | pass |
| ACC-RULE-003 | approved persistence design | SQLite must not be the source of truth for proxy transport endpoints | store implementation inspection and unit tests | pass |
| ACC-RULE-004 | testing and acceptance rules | Final acceptance must not be marked accepted without full required runtime/scope evidence | Multipass check and stage-scope results | fail |

## Required Command Evidence
- schema-check.py: passed for `vpn-frame`, `bucky-vpn-server`, and `bucky-vpn` after testing docs were re-approved.
- admission-check.py: passed for all three modules; stamps written under `harness/evidence/admission/20260706-*`.
- stage-scope-check.py: failed for implementation and testing because this worktree contains unrelated/pre-existing untracked and tracked changes; details recorded in `harness/pipeline-plan.md`.
- test-run.py <module> all: not run as a single command; covered by module unit/DV plus workspace integration artifacts listed above, with `bucky-vpn integration` blocked by Multipass availability.
- test-run.py all all: not run because it would invoke the unavailable Multipass integration path; `multipass version` failed before launch.
- quality-check.py: passed with no configured quality gates; `harness/quality-gates.yaml` declares `gates: []`.

## Consistency Summary
- Proposal authority check: approved proposal rows directly cover Endpoint-shaped PN addresses, observed-IP/mapped-port synthesis, no persisted endpoint truth, and client Endpoint consumption.
- Proposal vs design: design rows map the same `change_id` values and scope paths for `vpn-frame`, `bucky-vpn-server`, and `bucky-vpn`.
- Design vs implementation: implementation follows the design by moving `PnServerInfo` to endpoints, synthesizing server live endpoints from observed IP plus reported port, and leaving SQLite endpoint columns blank.
- Test design adequacy: focused unit tests cover changed branches and workspace integration covers cross-crate contract drift; full live Multipass topology remains a documented gap.
- change_id traceability: admission evidence binds all four reviewed `change_id` values to approved proposal/design hashes and testing docs map each to testplan steps.
- Document logic review: no proposal/design contradiction found; acceptance cannot pass because required runtime/scope evidence is incomplete.
- Implementation logic review: no logic defect found in the reviewed implementation; residual risk is live topology behavior without Multipass execution in this environment.

## Conclusion
- Accepted / Rejected / Needs Changes: needs changes
- Reason: The implementation and focused validation are consistent with the approved documents, but final acceptance is blocked by missing live `bucky-vpn` Multipass integration evidence and failing mechanical stage-scope checks in a noisy worktree.

## Follow-Up Tasks
- Iteration count: 1
- Testing: restore/fix local Multipass state, then run `uv run --active python ./harness/scripts/test-run.py bucky-vpn integration` or `uv run --active python ./harness/scripts/test-run.py all all` and cite the artifact.
- Governance/worktree: rerun stage-scope checks from a clean task branch or with an appropriate clean base so unrelated generated/untracked files do not mask the admitted scope.
- Acceptance: rerun acceptance after the missing runtime and scope evidence is available.
