# Auto Pipeline Plan

## Trigger
- Approved proposal: yes
- User launch confirmed: confirmed
- User launch statement: "确认，自动处理后续步骤"
- Per-stage user confirmation: not required
- Auto-confirm completed document stages: yes
- Version: v0.1
- Module(s): vpn-frame, bucky-vpn-server, bucky-vpn
- change_id values: CHG-pn-server-endpoint-address-contract, CHG-pn-server-address-live-state-contract, CHG-pn-port-mapping-observed-address, CHG-client-pn-proxy-endpoint-address

## Stage Graph
| task_id | stage | status | responsibility | scope | parent_task | depends_on | output | done_condition |
|---------|-------|--------|----------------|-------|-------------|------------|--------|----------------|
| pn-endpoint-proposal-vpn-frame | proposal | complete | Approve shared Endpoint-shaped PN server address and live-address contract. | docs/versions/v0.1/modules/vpn-frame/proposal.md | root | launch | approved proposal baseline for CHG-pn-server-endpoint-address-contract and CHG-pn-server-address-live-state-contract | proposal doc-structure-check and schema-check pass |
| pn-endpoint-proposal-server | proposal | complete | Approve server live observed-IP plus mapped Endpoint port address synthesis and no proxy-address persistence. | docs/versions/v0.1/modules/bucky-vpn-server/proposal.md | root | launch | approved proposal baseline for CHG-pn-port-mapping-observed-address and CHG-pn-server-endpoint-address-contract | proposal doc-structure-check and schema-check pass |
| pn-endpoint-proposal-client | proposal | complete | Approve client consumption of server-returned Endpoint PN proxy addresses. | docs/versions/v0.1/modules/bucky-vpn/proposal.md | root | launch | approved proposal baseline for CHG-client-pn-proxy-endpoint-address | proposal doc-structure-check and schema-check pass |
| pn-endpoint-design-vpn-frame | design | complete | Map Endpoint-shaped PN server addresses and live-only address state through shared protocol and store-facing contracts. | docs/versions/v0.1/modules/vpn-frame/design.md | root | pn-endpoint-proposal-vpn-frame | approved design for CHG-pn-server-endpoint-address-contract and CHG-pn-server-address-live-state-contract | design doc-structure-check and schema-check pass; design auto-approved |
| pn-endpoint-design-server | design | complete | Map proxy-node reported Endpoint, observed IP synthesis, no address persistence, selector validity, API projection, and client-returned PN info. | docs/versions/v0.1/modules/bucky-vpn-server/design.md | root | pn-endpoint-proposal-server, pn-endpoint-design-vpn-frame | approved design for CHG-pn-port-mapping-observed-address and CHG-pn-server-endpoint-address-contract | design doc-structure-check and schema-check pass; design auto-approved |
| pn-endpoint-design-client | design | complete | Map client PN proxy connect to consume server-returned Endpoint values instead of split ip/port reconstruction. | docs/versions/v0.1/modules/bucky-vpn/design.md | root | pn-endpoint-proposal-client, pn-endpoint-design-vpn-frame | approved design for CHG-client-pn-proxy-endpoint-address | design doc-structure-check and schema-check pass; design auto-approved |
| pn-endpoint-implementation-vpn-frame | implementation | complete | Implement Endpoint-shaped PN server address protocol and live-only address store-facing contract. | vpn-frame/src, harness/evidence/admission | root | pn-endpoint-design-vpn-frame | production code and admission evidence | schema-check/admission-check passed; `cargo check -p vpn-frame` passed with existing dead-code warning |
| pn-endpoint-implementation-server | implementation | complete | Implement server observed-IP plus mapped Endpoint port synthesis and stop relying on persisted proxy-node addresses. | vpn-server/src, vpn-server/config, harness/evidence/admission | root | pn-endpoint-design-server, pn-endpoint-implementation-vpn-frame | production code and admission evidence | schema-check/admission-check passed; `cargo check -p bucky-vpn-server` passed with existing dead-code warnings |
| pn-endpoint-implementation-client | implementation | complete | Implement client consumption of Endpoint-shaped PN proxy addresses. | vpn-client/src, harness/evidence/admission | root | pn-endpoint-design-client, pn-endpoint-implementation-vpn-frame | production code and admission evidence | schema-check/admission-check passed; `cargo check -p bucky-vpn` passed with existing vpn-frame dead-code warning |
| pn-endpoint-testing | testing | complete | Design and run validation for shared Endpoint protocol shape, server address synthesis, no address persistence truth, and client Endpoint connect behavior. | docs/versions/v0.1/modules/*/testing.md, testplan.yaml, test-results/test-runs | root | pn-endpoint-implementation-vpn-frame, pn-endpoint-implementation-server, pn-endpoint-implementation-client | testing docs, testplan metadata, runnable evidence | doc-structure-check/testing-coverage/schema-check passed; unit/DV/workspace integration artifacts written; Multipass live integration blocked by local Multipass cert error |
| pn-endpoint-acceptance | acceptance | complete | Audit proposal, design, implementation, testing, and evidence for PN Endpoint/live-address behavior. | docs/reviews/20260706-pn-endpoint-live-address-acceptance.md | root | pn-endpoint-testing | acceptance report | acceptance-report-check passed; conclusion is needs changes due missing Multipass live integration and noisy stage-scope evidence |

## Return Records
| issue_id | owning_stage | target_task | reason | expected_fix_output |
|----------|--------------|-------------|--------|---------------------|
| ACC-PN-ENDPOINT-001 | testing | pn-endpoint-testing | Local Multipass is unavailable, so `bucky-vpn integration` / `test-run.py all all` live multi-VM evidence was not produced. | Restore Multipass state and run the live integration or full all-all test entry, then cite the fresh artifact. |
| ACC-PN-ENDPOINT-002 | implementation/testing | pn-endpoint-implementation-vpn-frame/pn-endpoint-implementation-server/pn-endpoint-implementation-client/pn-endpoint-testing | Stage-scope checks fail in this dirty shared worktree because unrelated tracked/untracked/generated files and earlier stage artifacts are visible. | Re-run scope checks from a clean task branch or an appropriate clean base that isolates the admitted implementation/testing diffs. |

## Stage Scope Notes
| stage | task | result | note |
|-------|------|--------|------|
| proposal | pn-endpoint-proposal-vpn-frame/pn-endpoint-proposal-server/pn-endpoint-proposal-client | scope-check failed | The repository has pre-existing tracked diffs and this approved proposal update intentionally touched three module proposal documents under one explicit auto-pipeline launch. Schema-check and doc-structure-check passed for all three proposal docs. |
| design | pn-endpoint-design-vpn-frame/pn-endpoint-design-server/pn-endpoint-design-client | scope-check failed | The repository has pre-existing tracked diffs and this design update intentionally touched three module design documents plus the active pipeline plan under one explicit auto-pipeline launch. Schema-check and doc-structure-check passed for all three design docs. |
| implementation | pn-endpoint-implementation-vpn-frame/pn-endpoint-implementation-server/pn-endpoint-implementation-client | scope-check failed | Implementation admission passed and edited production paths are within the admitted Scope Paths; the checker also reports pre-existing untracked/generated files, earlier proposal/design pipeline artifacts, and unrelated tracked diffs in this noisy worktree. |
| testing | pn-endpoint-testing | scope-check failed | Testing docs, testplans, unit test code, and test-results are valid testing artifacts, but the checker also reports pre-existing untracked/generated files, earlier stage artifacts, and implementation diffs in the shared dirty worktree. |

## Validation Evidence
| command | result | artifact_or_note |
|---------|--------|------------------|
| `cargo check -p vpn-frame` | passed | no artifact; existing `get_all_send` dead-code warning |
| `cargo check -p bucky-vpn-server` | passed | no artifact; existing dead-code warnings |
| `cargo check -p bucky-vpn` | passed | no artifact; existing `vpn-frame` dead-code warning |
| `uv run --active python ./harness/scripts/test-run.py vpn-frame unit` | passed | `test-results/test-runs/20260706T161358Z-vpn-frame-unit.json` |
| `uv run --active python ./harness/scripts/test-run.py bucky-vpn-server unit` | passed | `test-results/test-runs/20260706T161529Z-bucky-vpn-server-unit.json` |
| `uv run --active python ./harness/scripts/test-run.py bucky-vpn unit` | passed | `test-results/test-runs/20260706T162012Z-bucky-vpn-unit.json` |
| `uv run --active python ./harness/scripts/test-run.py vpn-frame dv` | passed | `test-results/test-runs/20260706T161943Z-vpn-frame-dv.json` |
| `uv run --active python ./harness/scripts/test-run.py bucky-vpn-server dv` | passed | `test-results/test-runs/20260706T162135Z-bucky-vpn-server-dv.json` |
| `uv run --active python ./harness/scripts/test-run.py bucky-vpn dv` | passed | `test-results/test-runs/20260706T162050Z-bucky-vpn-dv.json` |
| `uv run --active python ./harness/scripts/test-run.py vpn-frame integration` | passed | `test-results/test-runs/20260706T162332Z-vpn-frame-integration.json` |
| `uv run --active python ./harness/scripts/test-run.py bucky-vpn-server integration` | passed | `test-results/test-runs/20260706T162401Z-bucky-vpn-server-integration.json` |
| `multipass version` | failed | local Multipass is unavailable: missing `/var/snap/multipass/common/data/multipassd/multipass_root_cert.pem`; `bucky-vpn integration` live multi-VM test not run |
| `uv run --active python ./harness/scripts/quality-check.py` | passed | no quality gates configured; `harness/quality-gates.yaml` declares `gates: []` |
| `uv run --active python ./harness/scripts/acceptance-report-check.py docs/reviews/20260706-pn-endpoint-live-address-acceptance.md` | passed | report conclusion is `needs changes` |

## Exit Condition
- [x] User approval and auto-pipeline launch are recorded.
- [x] Design documents are updated and auto-approved.
- [x] Implementation admission passes for every affected module.
- [x] PN Endpoint/live-address behavior is implemented across shared protocol, server, and client.
- [x] Required validation evidence exists.
- [ ] Final acceptance passes. Blocked by `ACC-PN-ENDPOINT-001` and `ACC-PN-ENDPOINT-002`.
