# Auto Pipeline Plan

## Trigger
- Approved proposal: yes
- User launch confirmed: confirmed
- User launch statement: "确认，自动处理后续步骤"
- Per-stage user confirmation: not required
- Auto-confirm completed document stages: yes
- Version: v0.1
- Module(s): bucky-vpn
- change_id values: CHG-client-pn-proxy-route-resolver, CHG-client-configurable-local-api-address

## Stage Graph
| task_id | stage | status | responsibility | scope | parent_task | depends_on | output | done_condition |
|---------|-------|--------|----------------|-------|-------------|------------|--------|----------------|
| bucky-vpn-proposal-pn-proxy-route-resolver | proposal | complete | Define the client PN proxy route resolver requirement and acceptance boundary | docs/versions/v0.1/modules/bucky-vpn/proposal.md | root | launch | approved proposal baseline for CHG-client-pn-proxy-route-resolver | doc-structure-check proposal passed and user approval recorded |
| bucky-vpn-design-pn-proxy-route-resolver | design | complete | Map the approved proposal to client submodules, resolver contract, call flow, and scope paths | docs/versions/v0.1/modules/bucky-vpn/design.md | root | bucky-vpn-proposal-pn-proxy-route-resolver | approved design mapping and scope paths | doc-structure-check design passed and document auto-approved; stage-scope blocked by pre-existing dirty worktree |
| bucky-vpn-implementation-pn-proxy-route-resolver | implementation | complete | Implement the smallest production change for client PN proxy route resolution | vpn-client/src/p2p_vpn.rs and admitted supporting paths | root | bucky-vpn-design-pn-proxy-route-resolver | production code change and admission evidence | schema-check and admission-check passed before code edits; cargo check and harness unit/DV passed; stage-scope blocked by pre-existing dirty worktree |
| bucky-vpn-testing-pn-proxy-route-resolver | testing | complete | Record post-implementation validation coverage and run required client checks | docs/versions/v0.1/modules/bucky-vpn/testing.md, testplan.yaml, test code only if required | root | bucky-vpn-implementation-pn-proxy-route-resolver | testing metadata and run artifacts | doc-structure-check testing and testing-coverage-check passed; unit/DV passed; integration attempted and failed in unrelated server test |
| bucky-vpn-acceptance-pn-proxy-route-resolver | acceptance | blocked | Audit proposal, design, implementation, testing, and run evidence | docs/reviews/20260626-bucky-vpn-pn-proxy-route-resolver-acceptance.md | root | bucky-vpn-testing-pn-proxy-route-resolver | acceptance report | acceptance-report-check passed; report conclusion is needs changes due external dirty-worktree and unrelated integration-test blockers |

## Return Records
| issue_id | owning_stage | target_task | reason | expected_fix_output |
|----------|--------------|-------------|--------|---------------------|
| PIPE-mixed-dirty-worktree | external-environment | bucky-vpn-design-pn-proxy-route-resolver | Stage scope checks see pre-existing unrelated modified/tracked files outside this task scope | Isolate in a clean worktree or commit/stash unrelated changes before relying on stage-scope as clean evidence |
| TEST-workspace-server-config-failure | external-environment | bucky-vpn-testing-pn-proxy-route-resolver | `test-run.py bucky-vpn integration` fails in `bucky-vpn-server::server_config::tests::default_config_falls_back_to_legacy_toml_when_yaml_missing`, outside admitted client path | Fix or isolate the server config test before requiring green workspace integration evidence |
| TEST-vpn-frame-clippy-failure | external-environment | bucky-vpn-testing-pn-proxy-route-resolver | `cargo clippy -p bucky-vpn --all-targets --all-features -- -D warnings` fails in existing `vpn-frame` warnings outside admitted client path | Fix or explicitly allow the existing vpn-frame clippy findings before requiring lint-green client evidence |
| ACC-external-acceptance-blockers | acceptance | bucky-vpn-acceptance-pn-proxy-route-resolver | Acceptance report is structurally valid but concludes needs changes because the dirty-worktree and unrelated integration-test blockers remain open | Resolve the external blockers and rerun stage-scope plus integration evidence before accepting |

## Exit Condition
- [x] Proposal-defined client PN proxy resolver baseline is approved.
- [x] Proposal-defined client PN proxy resolver behavior is implemented.
- [ ] Blocking issues are closed.
- [x] Required bucky-vpn unit/DV validation evidence exists.
- [ ] Final acceptance passes.
