---
task_manifest: task.yaml
status: approved
---

# Bind GitHub Release Repository Proposal

Risk profile: not-created (replace with ./risk-profile.yaml only after high-risk confirmation)

## Workflow Tier Judgment
- Proposed tier: high-risk
- Final tier: standard
- Tier rationale / triggered boundaries: The implementation is a localized workflow correction, but it directly controls the externally mutating GitHub Release publication step. The current tag run built and transferred all produced artifacts successfully, then failed before Release creation. Because the repair affects a material release/deployment boundary, the repository rules classify it as high-risk by consequence rather than diff size.
- Proposal and tier confirmation: confirmed by the user's explicit `确认，按standard任务完成` instruction on 2026-08-14; the user-selected standard tier is authoritative for this delivery.

## Background and Goal
GitHub Actions run `31784491251` for tag `v1.2.0` completed version validation, all client/server builds, direct installer uploads and downloads, and GHCR publication. The only failing job was `Publish GitHub Release`. That job intentionally does not check out source, so its working directory has no `.git`. The unqualified `gh release create` command attempts to infer the repository through Git and exits with `fatal: not a git repository` before creating the Release.

The goal is to make Release publication independent of a local checkout by explicitly binding the command to the repository already supplied by GitHub Actions.

## Scope
### In scope
- Pass the Actions-provided `GITHUB_REPOSITORY` value to `gh release create` through its supported explicit repository option.
- Preserve the existing tag, version, official-repository, permissions, asset-count, generated-notes, and title behavior.
- Extend the focused GitHub Actions workflow contract test to require explicit repository binding in the checkout-free Release job.
- Run the focused local contract suite and Harness lifecycle verification appropriate to the confirmed tier.

### Out of scope
- Adding a source checkout to the Release job.
- Rebuilding or changing any Debian, macOS, Windows, or server artifact.
- Changing artifact names, direct-upload/download behavior, GHCR publication, release notes, tag/version authority, permissions, or workflow triggers.
- Creating, deleting, or moving tags or Releases from this local task.

### Boundary with neighboring modules
Only the GitHub Actions release invocation and its existing repository-level contract test are affected. Packaging scripts, application runtime code, installer contents, and the server image remain unchanged.

## Requirement Review
The request is reasonable and the supplied failure is reproducible from the hosted log. Adding a checkout would make Git repository discovery work but would add unnecessary network work and retain an implicit dependency. `gh release create` supports the inherited `--repo` option, so `--repo "$GITHUB_REPOSITORY"` is the smallest direct fix and uses GitHub Actions' canonical `owner/repository` identity.

The existing contract test already validates Release inputs and permissions; extending it to assert explicit repository selection is sufficient local regression coverage. A new hosted tag run after the fix is committed remains the decisive proof that GitHub Release creation succeeds.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-bind-release-repository | Bind `gh release create` explicitly to `GITHUB_REPOSITORY` so the checkout-free Release job does not invoke Git repository discovery, and lock that behavior with focused contract coverage. | `.github/workflows/build.yml` Release command and `tests/github_actions_build_contract.py` only. | Couples the command explicitly to the standard GitHub Actions repository variable; this is clearer and cheaper than adding checkout. | Focused contract tests pass and prove the release job retains exactly three assets while supplying an explicit repository; the next hosted tag run successfully creates the Release. | No checkout, artifact, packaging, GHCR, permission, trigger, version, tag mutation, or release-content redesign. |

## Success Criteria
- Concrete user-visible or system-visible result: the checkout-free `Publish GitHub Release` job can create the tag-bound GitHub Release instead of failing with `not a git repository`.
- Required evidence: the repository contract suite proves explicit `GITHUB_REPOSITORY` binding and preservation of the three-installer Release contract; Harness checks pass; a new hosted tag execution remains required for external publication proof.
- Explicit non-goals: changing artifacts, builds, GHCR, versioning, permissions, triggers, release notes, or external tag/Release state during this local task.

## Risks
- A typo or quoting error in the explicit repository argument would still block Release creation; the focused test must assert the exact variable binding.
- Local tests cannot execute the external Release mutation with `GITHUB_TOKEN`; hosted evidence must remain distinct from local verification.
- The already-published GHCR tags from failed run `31784491251` are external state and are not rolled back or republished by this fix.
