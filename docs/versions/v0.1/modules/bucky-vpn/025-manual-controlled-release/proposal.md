---
task_manifest: task.yaml
status: approved
---

# Controlled Manual Release Proposal

Risk profile: not-created (replace with ./risk-profile.yaml only after high-risk confirmation)

## Workflow Tier Judgment
- Proposed tier: high-risk
- Final tier: standard
- Tier rationale / triggered boundaries: The change deliberately enables `workflow_dispatch` to mutate GitHub Releases and GHCR, including the `latest` server-image tag. It therefore changes a material release/deployment surface and must keep source-ref, version, repository, permission, and publication gates fail-closed.
- Proposal and tier confirmation: confirmed by the user's explicit `确认，按standard完成就好` instruction on 2026-08-14; the user-selected standard tier is authoritative, with the documented release/deployment risk retained for review.

## Background and Goal
The workflow currently accepts `workflow_dispatch`, but every manual run resolves `publish=false`; only a matching pushed tag in `buckyos/bucky-vpn` can publish. The existing `v1.2.0` tag run completed all builds and GHCR publication but failed to create the GitHub Release, and rerunning it would reuse the old workflow definition.

The goal is to let an authorized user launch a new workflow from the corrected default-branch workflow, explicitly request publication for an existing release tag, build the exact commit named by that tag, and reuse the existing guarded GitHub Release and GHCR jobs.

## Scope
### In scope
- Add manual-dispatch inputs with a safe build-only default and an explicit release mode plus release-tag value.
- Preserve ordinary manual dispatch as a non-publishing build when release mode is not selected.
- For manual publication, require the canonical `buckyos/bucky-vpn` repository, a non-empty existing tag, exact `v<Cargo version>` agreement, and a resolved immutable source commit.
- Make every platform build use the same resolved commit so workflow code loaded from the default branch cannot accidentally package default-branch source under the requested tag.
- Feed the validated release tag to the existing GitHub Release command; retain explicit `--repo "$GITHUB_REPOSITORY"` binding in the checkout-free release job.
- Keep pushed matching tags as an automatic publication path unless implementation evidence exposes a conflict.
- Extend focused workflow contract tests for default-safe dispatch, valid manual-release routing, rejection paths, immutable source checkout, and preservation of existing permissions/artifact gates.

### Out of scope
- Creating, deleting, moving, or force-updating any Git tag.
- Triggering the hosted manual release or otherwise mutating GitHub Release/GHCR state as part of local verification.
- Changing `vpn-client/Cargo.toml` version, packaging scripts, installer contents/names, server-image contents/names, release notes, or retention periods.
- Adding repository Environment protection or changing GitHub repository settings; those can be layered on separately if a second human approval gate is desired.
- Making publication available from forks, arbitrary repositories, branches, or unvalidated commits.

### Boundary with neighboring modules
The implementation is confined to `.github/workflows/build.yml` and its repository-level contract test. Packaging scripts and application/runtime modules remain consumers of the selected immutable source commit and are not otherwise changed.

The worktree already contains the prior task's uncommitted explicit `gh release create --repo "$GITHUB_REPOSITORY"` correction. This task preserves that correction but does not claim it as newly implemented manual-dispatch behavior; the Harness baseline must distinguish pre-existing content from this delivery.

## Requirement Review
The requested outcome is reasonable, but a bare `publish: true` input would be unsafe because a dispatch normally builds the selected branch and because publication jobs currently derive the release tag from `GITHUB_REF_NAME`. The safer direction is to treat the entered release tag as a request that must be resolved and validated once, output both the validated tag and immutable commit SHA, and make every downstream build consume that SHA.

The release flag remains false by default. Repository identity, exact Cargo-version/tag agreement, existing job-local write permissions, successful completion of all builds, three-installer validation, and server-image publication ordering remain mandatory. Invalid or incomplete manual publication requests fail before publication rather than silently degrading to a build or selecting another ref.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-manual-controlled-release | Add a default-safe manual release request, validate its existing tag/repository/version, pin all builds to one resolved commit, and route only a valid request through the existing GitHub Release and GHCR publishers. | `.github/workflows/build.yml` and `tests/github_actions_build_contract.py`; existing automatic matching-tag releases remain supported. | Adds ref-resolution and validation complexity to the version job in exchange for avoiding arbitrary-branch, ref-drift, and accidental-publication behavior. | Focused contracts prove build-only default behavior, valid manual publication, fail-closed invalid requests, identical downstream checkout SHA, exact release-tag propagation, canonical repository gating, and unchanged least-privilege/artifact dependencies. | No tag/version mutation, package redesign, repository-setting change, or hosted publication during implementation verification. |

## Success Criteria
- Concrete user-visible or system-visible result: an authorized operator can dispatch the default-branch workflow with explicit release mode and `v1.2.0`, causing all artifacts to be rebuilt from the existing `v1.2.0` commit and, after successful validation/builds, published through the existing Release and GHCR jobs.
- Required evidence: workflow/YAML parsing succeeds; focused repository contracts cover both release and rejection paths; Harness stage checks pass; final handoff gives the exact UI and `gh workflow run` invocation. The actual hosted dispatch remains a separately authorized external release action.
- Explicit non-goals: no external tag/Release/GHCR mutation, version bump, artifact-format change, or repository-settings change during this task.

## Risks
- A manual workflow loaded from the default branch can build the wrong source unless every checkout is pinned to the validated tag commit.
- A movable tag could create cross-job drift unless the first job resolves one commit SHA and downstream jobs consume that immutable output.
- Weak input handling could publish from a fork, mismatched version, empty tag, or ordinary branch; each case must fail closed before publication.
- Manual retry or concurrent publication may encounter an already-existing Release or already-pushed GHCR tags. The current create/push semantics remain explicit external-state gates; this task does not add overwrite behavior.
- Local tests can validate workflow structure and decision logic but cannot prove GitHub-hosted permissions or external publication; a deliberately authorized hosted dispatch is the final operational proof.
