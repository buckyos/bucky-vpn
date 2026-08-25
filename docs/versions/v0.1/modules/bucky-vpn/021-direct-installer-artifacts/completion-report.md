# Completion Report

## Object and Scope
- Task manifest: task.yaml
- Workflow tier: standard
- Change record: docs/changes/021-direct-installer-artifacts.md

## Delivery Summary
- Outcome: Debian, macOS, and Windows installers now use `actions/upload-artifact@v7` direct single-file uploads, so each Actions artifact is named after the real versioned installer and is not wrapped in an additional ZIP archive.
- Handoff: The tag-gated Release job now downloads those three direct artifacts by exact filename into `release-assets`; package contents, version derivation, server-image handling, GHCR publication, triggers, permissions, retention, and exactly-three-assets validation remain unchanged.

## Proposal Consistency
| change_id | requirement_or_boundary | proposal_source | delivery_evidence | finding | status |
|-----------|-------------------------|-----------------|-------------------|---------|--------|
| CHG-direct-installer-artifacts | Directly upload the three versioned client installers and preserve tag-gated Release retrieval without changing neighboring package/server behavior | proposal.md P-001, Scope, Requirement Review, and Success Criteria | `.github/workflows/build.yml` uses `archive: false` on the three exact single-file paths and has three matching versioned `download-artifact@v8` names; `tests/github_actions_build_contract.py` binds the actions, paths, policies, and Release handoff | Delivery matches the approved standard proposal; no server-image, package-content, version-source, trigger, permission, or publication-gate expansion was introduced | pass |

## Implementation Review
| area | evidence | finding | status |
|------|----------|---------|--------|
| Direct-upload behavior | All three client upload steps use pinned `upload-artifact@v7`, exactly one concrete versioned path, `archive: false`, `if-no-files-found: error`, and 14-day retention; their obsolete `name: installer-*` inputs are removed | Each upload satisfies v7's one-file direct mode and exposes the installer basename as its artifact name | pass |
| Release handoff | Debian, macOS, and Windows download steps use pinned `download-artifact@v8`; each `name` exactly equals the basename of its corresponding upload path and all target `release-assets` | Case, separators, version interpolation, architecture, and extensions match across producer and consumer, avoiding the former `installer-*` dependency | pass |
| Completeness and failure paths | Existing release script still requires exactly three files across `.deb`, `.pkg`, and `.exe`; uploads still fail on missing output; downloads fail if an exact artifact name is unavailable | Missing platform output or producer/consumer name drift remains fail-closed instead of producing a partial Release | pass |
| Neighboring publication behavior | Server upload retains `name: server-image`, one-day retention, zero compression, and its existing archived transfer; GHCR and tag/repository gates are unchanged | The client direct-upload change does not alter server image publication or broaden external release behavior | pass |
| Regression and side effects | The new assertions failed against all six pre-change producer/consumer expectations, then 16 build/publication contracts passed; all 7 Windows NASM contracts also passed; focused whitespace check passed | Tests detect removal of direct mode, return of logical artifact names, action-version downgrade, path/name drift, and disruption to the existing Windows setup | pass |
| Hosted boundary | Official action contracts support v7 direct uploads and v8 direct downloads, but no hosted workflow or GitHub Release was mutated in this local run | Local evidence is adequate for the YAML contract; GitHub UI appearance and actual hosted transfer remain explicit post-push confirmation points | pass |

## Verification
- Targeted check: `UV_CACHE_DIR=.harness/uv-cache uv run --active --with PyYAML==6.0.2 python ./tests/github_actions_build_contract.py` (16 passed); `UV_CACHE_DIR=.harness/uv-cache uv run --active --with PyYAML==6.0.2 python ./tests/windows_action_nasm_contract.py` (7 passed); focused `git diff --check`; exact producer/consumer filename inspection
- Result: passed
- Exception reason: A real hosted run was not launched from this workspace, so direct presentation in the Actions UI, native cross-platform builds, and a real tag-gated Release download/upload remain externally verifiable follow-up evidence rather than locally claimed results.

## Findings
| id | severity | evidence | problem | blocking |
|----|----------|----------|---------|----------|
| F-000 | none | Fresh producer/consumer boundary review, red-green contract run, 23 passing focused tests, and whitespace validation | No blocking requirement, implementation, regression, or scope defect found | no |
| F-001 | none | Hosted execution was not performed; the workflow uses the official pinned v7/v8 direct-file contract | GitHub UI and real artifact-service behavior require confirmation on the next hosted run, but this does not invalidate the locally verified workflow contract | no |

## Conclusion
- Accepted / rejected / needs changes: accepted
- Reason: All three client installers are configured for unarchived direct upload, their Release consumers use exact matching filenames, adjacent server/release safeguards remain intact, and focused red-green plus regression verification passed without a blocking finding.
