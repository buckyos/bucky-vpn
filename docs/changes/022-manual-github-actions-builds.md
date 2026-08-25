# Manual GitHub Actions Build Triggers

- Status: complete
- Task manifest: docs/versions/v0.1/modules/globals/022-manual-github-actions-builds/task.yaml
- Approved proposal: docs/versions/v0.1/modules/globals/022-manual-github-actions-builds/proposal.md
- Affected paths: .github/workflows/build.yml, tests/github_actions_build_contract.py

## Approach
Remove the `master` branch push and pull-request events from the existing workflow. Retain `workflow_dispatch` for opt-in cross-platform builds and retain only `push.tags: ["v*"]` as the automatic event because the existing release gate requires a pushed version Tag. Update the focused workflow contract to reject branch and pull-request triggers while preserving build jobs, publication gating, and permissions.

## Risk Screen
The change reduces automatic CI feedback across the repository, so maintainers must explicitly launch ordinary builds before relying on them. The release event must remain because publication is intentionally gated on `GITHUB_EVENT_NAME=push` and `GITHUB_REF_TYPE=tag`; removing it would make GitHub Release and GHCR jobs unreachable. No build graph, artifact, credential, permission, or publication implementation changes are planned.

## Verification
- Targeted check: `UV_CACHE_DIR=.harness/uv-cache uv run --active --with PyYAML==6.0.2 python ./tests/github_actions_build_contract.py` (16 tests); the new trigger assertion executed against the task-start baseline as a negative control; focused baseline diff and `git diff --check`
- Result: passed
- Residual risk or follow-up: The negative control failed on the old `master`/PR events as intended and all 16 current contracts passed. A local static contract proves event configuration and publication guards, but only the next GitHub-hosted manual and Tag runs can confirm hosted UI/execution behavior.
