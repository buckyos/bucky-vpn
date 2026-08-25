# Completion Report

## Object and Scope
- Task manifest: task.yaml
- Workflow tier: standard
- Change record: docs/changes/025-manual-controlled-release.md

## Delivery Summary
- Outcome: `workflow_dispatch` now defaults to build-only but can explicitly publish an existing validated version tag. The workflow resolves that tag to one commit, pins every platform build to the resolved SHA, revalidates the hosted tag after all builds, and only then allows the existing GHCR and GitHub Release jobs to mutate external state.
- Handoff: Automatic matching-tag publication remains supported. Manual publication uses the corrected default-branch workflow while building the entered tag's source, so the existing `v1.2.0` can be released without moving the tag. The hosted dispatch itself remains an explicit operator action after these changes are committed and pushed.

## Proposal Consistency
| change_id | requirement_or_boundary | proposal_source | delivery_evidence | finding | status |
|-----------|-------------------------|-----------------|-------------------|---------|--------|
| CHG-manual-controlled-release | Add a default-safe manual release request, validate repository/tag/version, pin all builds to one source commit, reject ref drift, and route only valid requests through existing publishers without changing packaging | proposal.md P-001, Scope, Requirement Review, Success Criteria, and Risks | `.github/workflows/build.yml` adds typed inputs, exact manual-release validation, `source_sha`/`release_tag` outputs, SHA-pinned checkouts, a read-only post-build tag authorization job, and validated-tag Release invocation; `tests/github_actions_build_contract.py` exercises the new decision and failure paths | Delivery matches the approved standard proposal and retains all named non-goals | pass |

## Implementation Review
| area | evidence | finding | status |
|------|----------|---------|--------|
| Default behavior and admission | `publish` is a required boolean with `default: false`; a normal dispatch with no tag produces `publish=false`; supplying a tag while publication is false is rejected | Ordinary manual runs cannot reach either mutation job, and ambiguous input fails rather than being ignored | pass |
| Repository, version, and tag boundary | Manual publication requires `GITHUB_REPOSITORY == buckyos/bucky-vpn`, a non-empty tag, and exact Cargo-version agreement such as `release_tag == v1.2.0`; checkout addresses the explicit `refs/tags/` namespace | Fork, missing-tag, non-tag-ref, and version-mismatch publication attempts fail before any external mutation | pass |
| Source integrity and ref movement | The version job compares checked-out HEAD with the local tag commit, exports one `source_sha`, and all four builders checkout that SHA; after all builds, a read-only authorization job resolves lightweight or nested annotated GitHub tag objects and compares the current hosted commit with `source_sha` | Artifacts cannot silently come from the dispatch branch, separate build refs, or a tag moved during the build | pass |
| Publication dependencies and permissions | `authorize-publication` needs all four builds and has only `contents: read`; `publish-server` additionally needs authorization and retains `packages: write`; Release remains downstream of server publication with `contents: write` | Publication stays ordered after successful builds and ref authorization, with write scopes restricted to the two existing mutation jobs | pass |
| Release identity and prior fix | The Release command consumes `needs.version.outputs.release_tag`, retains `--repo "$GITHUB_REPOSITORY"`, verifies the tag, checks exactly three installer assets, and does not use dispatch `GITHUB_REF_NAME` | The manual run cannot create a Release named after `master`, and the checkout-free repository-binding repair remains intact | pass |
| Existing automatic path and artifacts | Pushed matching tags still set `publish=true`; artifact names, builders, retention, generated notes, title, server version/`latest` tags, and immutable external Action references remain unchanged | Manual support does not replace the existing automatic release path or redesign produced artifacts | pass |
| Failure paths and regression evidence | Ten decision-script cases cover matching push/manual release, build-only default, fork, missing tag, mismatched tag, contradictory inputs, and invalid boolean; fake GitHub responses cover lightweight, annotated, and moved tags; the task-start workflow fails the new trigger contract | The new controls have executable positive and negative coverage rather than source-shape assertions alone | pass |
| Platform and hosted boundary | The 19-test suite executes existing Debian/macOS/server fixture contracts and Windows/package source contracts; no real hosted runners or external publication were launched | Local coverage is proportionate for the standard tier, but native hosted builds and real Release/GHCR mutation remain an explicit operational evidence gap | pass |
| Scope and dirty-worktree isolation | The pre-edit baseline captured the pre-existing `--repo` workflow correction, untracked contract test, and unrelated `vpn_web` edits before this task; review compares current delivery with that baseline | This task preserves but does not claim unrelated or prior-task work | pass |

## Verification
- Targeted check: `UV_CACHE_DIR=.harness/uv-cache uv run --active --with PyYAML==6.0.2 python ./tests/github_actions_build_contract.py` (19 passed); pre-edit negative control (one expected contract failure); live read-only `v1.2.0` tag resolution with correct and deliberately incorrect SHA; focused workflow/test whitespace and diff inspection
- Result: passed
- Exception reason: Native hosted platform builds and the actual manual Release/GHCR mutation were not executed locally. The user-selected standard tier does not create high-risk testing artifacts; platform coverage and the remaining hosted proof are recorded in this report and the bound change record.

## Findings
| id | severity | evidence | problem | blocking |
|----|----------|----------|---------|----------|
| F-000 | none | Approved proposal comparison, baseline-aware diff, 19 passing focused contracts, hosted tag positive/negative checks, permission/dependency review, and failure-path execution | No unresolved implementation, boundary, failure-path, regression, or scope defect found | no |
| F-001 | none | No Actions run or external write was launched from the local task | GitHub-hosted runner behavior and successful creation of the `v1.2.0` Release remain to be confirmed by the first deliberately authorized dispatch | no |
| F-002 | none | The release trigger custom rule requests high-risk testing artifacts, while the user explicitly selected the standard tier whose flow excludes them | Applied the higher-priority explicit tier selection and preserved the rule's substantive platform-evidence requirement in the standard change record and completion review | no |

## Conclusion
- Accepted / rejected / needs changes: accepted
- Reason: The delivery provides an explicit false-by-default publication path, binds it to one validated tag commit and the canonical repository, rechecks ref stability before mutation, preserves existing least-privilege and artifact contracts, and passes focused positive/negative verification with no blocking review finding.
