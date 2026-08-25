---
task_manifest: task.yaml
status: approved
---

# Manual GitHub Actions Build Trigger Proposal

Risk profile: not-created (replace with ./risk-profile.yaml only after high-risk confirmation)

## Workflow Tier Judgment
- Proposed tier: standard
- Final tier: standard
- Tier rationale / triggered boundaries: The implementation is localized to the event stanza in one workflow, but it changes repository-wide CI execution policy. Keeping the existing version-Tag release trigger avoids changing produced artifacts, publication permissions, versioning, or the established GitHub Release/GHCR boundary, so a bounded standard flow is proportionate.
- Proposal and tier confirmation: confirmed by the user's explicit `确认` instruction on 2026-08-14; the displayed proposal had no unresolved questions.

## Background and Goal
The build workflow currently runs all platform packaging jobs for every push to `master` and every pull request targeting `master`. This consumes hosted-runner capacity even when maintainers do not need a full cross-platform build.

The goal is to make ordinary cross-platform builds opt-in from the GitHub Actions UI while preserving the existing explicit version-Tag release path.

## Scope
### In scope
- Remove automatic workflow runs for pushes to `master`.
- Remove automatic workflow runs for pull requests targeting `master`.
- Keep `workflow_dispatch` so maintainers can manually run all build jobs against a selected branch or Tag.
- Keep the `push.tags: ["v*"]` event so a matching `v<vpn-client Cargo version>` Tag still triggers the existing GitHub Release and GHCR publication flow.
- Add or update focused workflow-contract coverage for the trigger policy if current checks do not cover it.

### Out of scope
- Changing any build command, runner image, platform, installer artifact, or server image.
- Changing the Cargo version authority, exact Tag/version validation, GitHub Release contents, GHCR tags, permissions, or credentials.
- Making a normal manual build publish a Release or container image.
- Adding schedules, path filters, branch filters, or additional manual inputs.

### Boundary with neighboring modules
Only workflow event routing changes. All Debian, macOS, Windows, and server jobs keep their current behavior after a run starts. Publication remains restricted to a matching version Tag pushed to `buckyos/bucky-vpn`; a manually dispatched branch build remains build-only.

## Requirement Review
Stopping automatic branch and pull-request builds directly addresses the repeated compilation concern. Retaining the version-Tag push event is necessary because the current publication gate requires a Tag `push`; removing every non-manual event without redesigning that gate would silently make GitHub Release and GHCR publication unreachable. The chosen direction therefore makes day-to-day builds manual without weakening or disabling release controls.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-manual-github-actions-builds | Stop automatic builds for `master` pushes and `master` pull requests, retain manual dispatch for ordinary builds, and preserve matching `v*` Tag pushes as the only automatic release event. | GitHub Actions event configuration and focused static contract coverage only. | Pull requests and ordinary commits no longer receive automatic cross-platform feedback; maintainers must start those runs manually. | Focused validation proves `workflow_dispatch` and `push.tags: ["v*"]` remain, branch/PR triggers are absent, manual runs cannot publish, and matching Tag runs retain the existing publication route. | No build, artifact, version, permission, GHCR, or Release-job redesign. |

## Success Criteria
- Concrete user-visible or system-visible result: commits and pull-request updates no longer start the workflow automatically; maintainers can run it manually; pushing the exact release Tag still starts the existing build-and-publish chain.
- Required evidence: workflow syntax/static validation and focused assertions covering allowed and forbidden event paths; no unrelated workflow behavior changes.
- Explicit non-goals: fully manualizing release publication, changing package outputs, or changing application code.

## Risks
- Removing automatic PR runs reduces pre-merge feedback and makes manual maintainer discipline necessary.
- Removing the `v*` Tag event as well would strand the current publication jobs, so the release event must remain unless a separately approved manual-release design replaces it.
