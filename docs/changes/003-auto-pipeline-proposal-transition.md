# Auto-pipeline Proposal Transition

- Status: complete
- Task manifest: docs/versions/v0.1/modules/repo-governance/003-auto-pipeline-proposal-transition/task.yaml
- Approved proposal: docs/versions/v0.1/modules/repo-governance/003-auto-pipeline-proposal-transition/proposal.md
- Affected paths: harness/scripts/harness-check.py, harness/scripts/test-run.py, harness/tests/test_auto_pipeline_proposal_transition.py, docs/architecture/repository-workflow.md

## Approach
Select proposal schema validation from the existing automatic-design policy boundary: launch-confirmed automatic Design uses ordinary schema validation, while manual Proposal retains approval-enforcing validation. Register a focused red-green regression and document the boundary.

## Risk Screen
The user selected the standard tier despite Harness-process impact. The focused test asserts both automatic and manual command construction, and the blocked real task transition provides an end-to-end signal without changing schema or lifecycle policy.

## Verification
- Targeted check: `python3 ./harness/scripts/test-run.py repo-governance unit` plus the real `002-align-proxy-node-api` proposal transition
- Result: passed
- Residual risk or follow-up: The user-selected standard tier omits the full high-risk staged lifecycle; focused red-green coverage and the successful real transition mitigate the known governance risk.
