# Auto Pipeline Plan

## Trigger
- Approved proposal: yes
- User launch confirmed: confirmed
- User launch statement: "确认，自动处理后续步骤"
- Per-stage user confirmation: not required
- Auto-confirm completed document stages: yes
- Version: v0.1
- Module(s): vpn-frame, bucky-vpn-server, bucky-vpn
- change_id values: CHG-pn-server-reported-name-contract, CHG-server-identity-cert-name, CHG-server-proxy-node-reported-name, CHG-client-pn-proxy-reported-name

## Stage Graph
| task_id | stage | status | responsibility | scope | parent_task | depends_on | output | done_condition |
|---------|-------|--------|----------------|-------|-------------|------------|--------|----------------|
| proxy-name-proposal-vpn-frame | proposal | complete | Approve shared `PnServerInfo.name` contract. | docs/versions/v0.1/modules/vpn-frame/proposal.md | root | launch | approved proposal baseline for CHG-pn-server-reported-name-contract | proposal doc-structure-check and schema-check pass |
| proxy-name-proposal-server | proposal | complete | Approve server certificate name and proxy node reported-name propagation requirements. | docs/versions/v0.1/modules/bucky-vpn-server/proposal.md | root | launch | approved proposal baseline for CHG-server-identity-cert-name and CHG-server-proxy-node-reported-name | proposal doc-structure-check and schema-check pass |
| proxy-name-proposal-client | proposal | complete | Approve client PN proxy reported-name connection requirement. | docs/versions/v0.1/modules/bucky-vpn/proposal.md | root | launch | approved proposal baseline for CHG-client-pn-proxy-reported-name | proposal doc-structure-check and schema-check pass |
| proxy-name-design-vpn-frame | design | complete | Map optional reported proxy node name through shared protocol and store-facing contracts. | docs/versions/v0.1/modules/vpn-frame/design.md | root | proxy-name-proposal-vpn-frame | approved design for CHG-pn-server-reported-name-contract | design doc-structure-check and schema-check pass; design auto-approved |
| proxy-name-design-server | design | complete | Map certificate name and proxy node configured name through server config, identity lifecycle, proxy control reporting, runtime selector state, HTTP/API projection, and client-returned PN info. | docs/versions/v0.1/modules/bucky-vpn-server/design.md | root | proxy-name-proposal-server, proxy-name-design-vpn-frame | approved design for CHG-server-identity-cert-name and CHG-server-proxy-node-reported-name | design doc-structure-check and schema-check pass; design auto-approved |
| proxy-name-design-client | design | complete | Map `PnServerInfo.name` consumption to PN proxy connection name, fallback, and cache/worker-key behavior. | docs/versions/v0.1/modules/bucky-vpn/design.md | root | proxy-name-proposal-client, proxy-name-design-vpn-frame | approved design for CHG-client-pn-proxy-reported-name | design doc-structure-check and schema-check pass; design auto-approved |
| proxy-name-implementation-vpn-frame | implementation | complete | Implement optional reported proxy node name in shared protocol/store-facing contracts. | vpn-frame/src, harness/evidence/admission | root | proxy-name-design-vpn-frame | production code and admission evidence | schema-check and admission-check pass before code edit; relevant cargo/harness checks pass or blockers recorded |
| proxy-name-implementation-server | implementation | complete | Implement server identity certificate name and server config/report/API/selector propagation of reported proxy node name. | vpn-server/src, vpn-server/config, harness/evidence/admission | root | proxy-name-design-server, proxy-name-implementation-vpn-frame | production code and admission evidence | schema-check and admission-check pass before code edit; relevant cargo/harness checks pass or blockers recorded |
| proxy-name-implementation-client | implementation | complete | Implement client use of `PnServerInfo.name` when connecting to PN proxy. | vpn-client/src, harness/evidence/admission | root | proxy-name-design-client, proxy-name-implementation-vpn-frame | production code and admission evidence | schema-check and admission-check pass before code edit; relevant cargo/harness checks pass or blockers recorded |
| proxy-name-testing | testing | complete | Design and run validation for shared protocol shape, server propagation, and client connection name usage. | docs/versions/v0.1/modules/*/testing.md, testplan.yaml, test-results/test-runs | root | proxy-name-implementation-vpn-frame, proxy-name-implementation-server, proxy-name-implementation-client | testing docs, testplan metadata, runnable evidence | doc-structure-check testing, testing-coverage-check, and relevant test-run entries pass or blockers recorded |
| proxy-name-acceptance | acceptance | complete | Audit proposal, design, implementation, testing, and evidence for proxy node reported-name behavior. | docs/reviews/20260706-proxy-node-reported-name-acceptance.md | root | proxy-name-testing | acceptance report | acceptance-report-check passes and conclusion is accepted, or return records are added |

## Return Records
| issue_id | owning_stage | target_task | reason | expected_fix_output |
|----------|--------------|-------------|--------|---------------------|
| ACC-PROXY-NAME-001 | environment/scope | proxy-name-acceptance | Current worktree contains broad pre-existing cross-stage, generated, artifact, and unrelated changes, so stage-scope checks cannot isolate this task. | Rerun stage-scope checks in an isolated worktree or after unrelated changes are separated. |
| ACC-PROXY-NAME-002 | testing | proxy-name-testing | `test-run.py all all` was not run, so final accepted conclusion lacks whole-project unified evidence. | Run `uv run --active python ./harness/scripts/test-run.py all all` and cite the fresh artifact in a follow-up acceptance report. |

## Stage Scope Notes
| stage | task | result | note |
|-------|------|--------|------|
| design | proxy-name-design-vpn-frame/proxy-name-design-server/proxy-name-design-client | scope-check failed | The repository had broad pre-existing cross-stage and generated/artifact diffs before this design stage; schema-check and doc-structure-check passed for all three design docs. No unrelated files were reverted. |
| testing | proxy-name-testing | scope-check failed | Testing docs, testplans, schema checks, doc-structure checks, coverage checks, and unit test-run artifacts passed, but stage-scope-check failed because the worktree still contains broad pre-existing unrelated/cross-stage/generated diffs. |
| acceptance | proxy-name-acceptance | needs changes | `docs/reviews/20260706-proxy-node-reported-name-acceptance.md` passed acceptance-report-check with conclusion `needs changes`; final acceptance remains blocked by ACC-PROXY-NAME-001 and ACC-PROXY-NAME-002. |

## Exit Condition
- [x] User approval and auto-pipeline launch are recorded.
- [x] Design documents are updated and auto-approved.
- [x] Implementation admission passes for every affected module.
- [x] Proxy node reported-name behavior is implemented across shared protocol, server, and client.
- [x] Required validation evidence exists.
- [ ] Final acceptance passes.
