# Auto Pipeline Plan

## Trigger
- Approved proposal: yes
- User launch confirmed: confirmed
- User launch statement: "确认，自动处理后续步骤"
- Per-stage user confirmation: not required
- Auto-confirm completed document stages: yes
- Version: v0.1
- Module(s): bucky-vpn-server
- change_id values: CHG-pure-pn-sn-address, CHG-sn-http-config, CHG-sn-admin-config, CHG-sn-jwt-config

## Stage Graph
| task_id | stage | status | responsibility | scope | parent_task | depends_on | output | done_condition |
|---------|-------|--------|----------------|-------|-------------|------------|--------|----------------|
| bucky-vpn-server-proposal-sn-owned-config | proposal | complete | Approve SN-owned config hierarchy for control server, HTTP management, admin bootstrap, and JWT session signing settings | docs/versions/v0.1/modules/bucky-vpn-server/proposal.md | root | launch | approved proposal baseline for CHG-pure-pn-sn-address, CHG-sn-http-config, CHG-sn-admin-config, and CHG-sn-jwt-config | doc-structure-check proposal passed and auto-pipeline approval recorded |
| bucky-vpn-server-design-sn-owned-config | design | complete | Map approved SN-owned config proposal to YAML fields, compatibility behavior, and implementation scope paths | docs/versions/v0.1/modules/bucky-vpn-server/design.md | root | bucky-vpn-server-proposal-sn-owned-config | approved design mapping for sn.control_server, sn.http, sn.admin, and sn.jwt | doc-structure-check design and schema-check passed; stage-scope remains blocked by mixed dirty worktree and is recorded below |
| bucky-vpn-server-implementation-sn-owned-config | implementation | complete | Implement config template and parser changes for sn.control_server, sn.http, sn.admin, and sn.jwt with legacy compatibility | vpn-server/config/config.example.yaml, vpn-server/src/server_config.rs, vpn-server/src/main.rs | root | bucky-vpn-server-design-sn-owned-config | production code change and admission evidence | schema-check, admission-check, cargo fmt, focused server_config tests, and unified module validation passed; stage-scope remains blocked by mixed dirty worktree and is recorded below |
| bucky-vpn-server-testing-sn-owned-config | testing | complete | Record and run focused validation for SN-owned config parsing and template structure | docs/versions/v0.1/modules/bucky-vpn-server/testing.md, docs/versions/v0.1/modules/bucky-vpn-server/testplan.yaml, test-results/test-runs | root | bucky-vpn-server-implementation-sn-owned-config | testing metadata and runnable evidence | doc-structure-check testing, testing-coverage-check, and bucky-vpn-server unit/dv/integration test-run invocations passed, including `test-results/test-runs/20260702T044552Z-bucky-vpn-server-unit.json`, `test-results/test-runs/20260702T044638Z-bucky-vpn-server-dv.json`, and `test-results/test-runs/20260702T044656Z-bucky-vpn-server-integration.json` |
| bucky-vpn-server-acceptance-sn-owned-config | acceptance | blocked | Audit proposal, design, implementation, testing, and run evidence | docs/reviews/20260702-bucky-vpn-server-sn-owned-config-acceptance.md | root | bucky-vpn-server-testing-sn-owned-config | acceptance report | final acceptance is blocked until mixed dirty worktree is isolated or unrelated tracked changes are committed/stashed |

## Return Records
| issue_id | owning_stage | target_task | reason | expected_fix_output |
|----------|--------------|-------------|--------|---------------------|
| PIPELINE-mixed-dirty-worktree | external-environment | bucky-vpn-server-design-no-sn-client | `stage-scope-check.py --stage design --version v0.1 --module bucky-vpn-server --ignore-untracked` sees unrelated tracked changes across code, docs, pipeline plan, and other modules. | Isolate this task in a clean worktree or commit/stash unrelated changes, then rerun design stage-scope and auto-confirm design before implementation admission. |
| PIPELINE-mixed-dirty-worktree-sn-owned-config | external-environment | bucky-vpn-server-acceptance-sn-owned-config | `stage-scope-check.py` sees unrelated tracked changes across proposal/testing/code and other modules while each stage owns a narrower output set. Implementation scope also sees unrelated tracked files such as `build_win.bat`, `vpn-server/Cargo.toml`, `vpn-server/src/vpn_control_client.rs`, `vpn_web/README.md`, and `vpn_web/lib/base58.dart`. | Isolate this task in a clean worktree or commit/stash unrelated changes, then rerun stage-scope and acceptance checks; current run proceeds with this blocker recorded because schema, admission, doc, coverage, and module validation checks passed. |

## Exit Condition
- [x] Proposal-defined SN-owned config baseline is approved.
- [x] Proposal-defined SN-owned config behavior is implemented.
- [ ] Blocking issues are closed.
- [x] Required bucky-vpn-server SN-owned config validation evidence exists or blockers are recorded.
- [x] Final acceptance passes or remaining external blockers are reported.
