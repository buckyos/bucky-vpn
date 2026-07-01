# Auto Pipeline Plan

## Trigger
- Approved proposal: yes
- User launch confirmed: confirmed
- User launch statement: "批准，后续步骤自动处理"
- Per-stage user confirmation: not required
- Auto-confirm completed document stages: yes
- Version: v0.1
- Module(s): bucky-vpn-server
- change_id values: CHG-pure-pn-no-sn-client, CHG-pure-pn-sn-address, CHG-pn-sn-heartbeat, CHG-external-pn-active-control

## Stage Graph
| task_id | stage | status | responsibility | scope | parent_task | depends_on | output | done_condition |
|---------|-------|--------|----------------|-------|-------------|------------|--------|----------------|
| bucky-vpn-server-proposal-no-sn-client | proposal | complete | Approve pure proxy node no-SN-client requirement baseline | docs/versions/v0.1/modules/bucky-vpn-server/proposal.md | root | launch | approved proposal baseline for pure proxy no-SN-client control channel | doc-structure-check proposal passed and user approval recorded |
| bucky-vpn-server-design-no-sn-client | design | blocked | Map approved proposal to non-SN-client control channel, scope paths, and failure handling | docs/versions/v0.1/modules/bucky-vpn-server/design.md | root | bucky-vpn-server-proposal-no-sn-client | draft design mapping for CHG-pure-pn-no-sn-client and dependent control/heartbeat changes | doc-structure-check design and schema-check passed; stage-scope blocked by mixed dirty worktree |
| bucky-vpn-server-implementation-no-sn-client | implementation | pending | Implement smallest production change so pure proxy control path does not start SNClientService or ReportSn | vpn-server/src/vpn_control_client.rs, vpn-server/src/main.rs, vpn-server/src/server_config.rs | root | bucky-vpn-server-design-no-sn-client | production code and admission evidence | schema-check and admission-check pass before code edits; implementation scope check passes or records mixed-worktree blocker |
| bucky-vpn-server-testing-no-sn-client | testing | pending | Record and run focused validation for pure proxy no-SN-client behavior | docs/versions/v0.1/modules/bucky-vpn-server/testing.md, docs/versions/v0.1/modules/bucky-vpn-server/testplan.yaml, test-results/test-runs | root | bucky-vpn-server-implementation-no-sn-client | testing metadata and runnable evidence | doc-structure-check testing, testing-coverage-check, and relevant test-run invocation pass or blockers recorded |
| bucky-vpn-server-acceptance-no-sn-client | acceptance | pending | Audit proposal, design, implementation, testing, and run evidence | docs/reviews/20260701-bucky-vpn-server-no-sn-client-acceptance.md | root | bucky-vpn-server-testing-no-sn-client | acceptance report | acceptance-report-check passes or reports blocking issues |

## Return Records
| issue_id | owning_stage | target_task | reason | expected_fix_output |
|----------|--------------|-------------|--------|---------------------|
| PIPELINE-mixed-dirty-worktree | external-environment | bucky-vpn-server-design-no-sn-client | `stage-scope-check.py --stage design --version v0.1 --module bucky-vpn-server --ignore-untracked` sees unrelated tracked changes across code, docs, pipeline plan, and other modules. | Isolate this task in a clean worktree or commit/stash unrelated changes, then rerun design stage-scope and auto-confirm design before implementation admission. |

## Exit Condition
- [x] Proposal-defined pure proxy no-SN-client baseline is approved.
- [ ] Proposal-defined pure proxy no-SN-client behavior is implemented.
- [ ] Blocking issues are closed.
- [ ] Required bucky-vpn-server validation evidence exists or blockers are recorded.
- [ ] Final acceptance passes or remaining external blockers are reported.
