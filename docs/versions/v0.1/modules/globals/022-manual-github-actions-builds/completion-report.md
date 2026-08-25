# Completion Report

## Object and Scope
- Task manifest: task.yaml
- Workflow tier: standard
- Change record: docs/changes/022-manual-github-actions-builds.md

## Delivery Summary
- Outcome: Ordinary pushes to `master` and pull-request updates no longer trigger the cross-platform workflow; maintainers can start ordinary builds with `workflow_dispatch`.
- Handoff: The existing `v*` Tag push event and push-Tag/repository/version publication guards remain, so matching release Tags still reach the GitHub Release and GHCR chain while manual runs remain build-only.

## Proposal Consistency
| change_id | requirement_or_boundary | proposal_source | delivery_evidence | finding | status |
|-----------|-------------------------|-----------------|-------------------|---------|--------|
| CHG-manual-github-actions-builds | Remove automatic branch/PR builds, retain manual ordinary builds and the existing automatic release Tag event without changing neighboring build/publication behavior | proposal.md P-001, Scope, Requirement Review, and Success Criteria | `.github/workflows/build.yml` has exactly `push.tags: ["v*"]` and `workflow_dispatch`; `tests/github_actions_build_contract.py` binds the complete event map and existing publication cases | Delivery matches the approved standard proposal; build jobs, artifacts, permissions, version checks, GitHub Release, and GHCR behavior were not changed by this task | pass |

## Implementation Review
| area | evidence | finding | status |
|------|----------|---------|--------|
| Trigger behavior | Parsed workflow event map is exactly `{push: {tags: ["v*"]}, workflow_dispatch: empty}`; no branch or pull-request event remains | Ordinary commits and PR updates cannot schedule this workflow, while manual dispatch remains available | pass |
| Release reachability | The `v*` push event remains and the version job still requires event `push`, ref type `tag`, an exact `v` plus Cargo-version Tag, and repository `buckyos/bucky-vpn` | The change does not strand or broaden release publication | pass |
| Manual publication boundary | Contract execution with `workflow_dispatch` and a matching Tag ref still produces `publish=false`; publication jobs remain gated on that output | Selecting a Tag in the manual-run UI builds it but cannot accidentally publish it | pass |
| Neighboring behavior and scope | Task-start baseline diff for `.github/workflows/build.yml` contains only removal of the `master` branch filter and the two pull-request lines; the separate pre-existing direct-artifact edits are present in both sides of the baseline | No build job, runner, action, command, artifact, permission, or publication implementation was changed by this task | pass |
| Regression and defect discovery | The new exact-event assertion failed against the task-start workflow because it found both forbidden automatic event paths, then all 16 current build/publication contracts passed; whitespace validation passed | The test detects reintroduction of either branch/PR trigger as well as loss or expansion of the retained event set; no blocking side effect was found | pass |
| Hosted boundary | Local YAML parsing and shell publication cases cover repository-controlled behavior; no external Actions run was launched | GitHub UI availability and hosted scheduling remain explicit post-push confirmation points rather than locally claimed evidence | pass |

## Verification
- Targeted check: `UV_CACHE_DIR=.harness/uv-cache uv run --active --with PyYAML==6.0.2 python ./tests/github_actions_build_contract.py` (16 passed); new trigger test against the pre-edit baseline (failed as intended); task-baseline workflow/test diff inspection; focused `git diff --check`
- Result: passed
- Exception reason: A real GitHub-hosted manual run or release Tag was not launched from this local workspace, so hosted scheduling and UI behavior remain external follow-up evidence.

## Findings
| id | severity | evidence | problem | blocking |
|----|----------|----------|---------|----------|
| F-000 | none | Exact baseline delta review, negative control against both former automatic triggers, 16 passing current contracts, and publication boundary inspection | No requirement, implementation, regression, or scope defect found | no |
| F-001 | none | No hosted workflow was launched; repository-controlled event and publication semantics are covered locally | The next pushed workflow version still needs a hosted manual run and later matching Tag run for platform-level confirmation | no |

## Conclusion
- Accepted / rejected / needs changes: accepted
- Reason: Automatic branch and pull-request builds are removed, manual builds remain available, matching release Tags retain the existing guarded publication path, and targeted negative/positive verification found no blocking defect or unrelated task delta.
