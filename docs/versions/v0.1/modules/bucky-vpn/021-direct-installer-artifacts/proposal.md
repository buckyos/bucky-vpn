---
task_manifest: task.yaml
status: approved
---

# Direct Installer Artifacts Proposal

Risk profile: not-created (replace with ./risk-profile.yaml only after high-risk confirmation)

## Workflow Tier Judgment
- Proposed tier: high-risk
- Final tier: standard
- Tier rationale / triggered boundaries: The YAML edit is small, but it changes the externally visible Actions artifacts and the artifact contract consumed by the tag-gated GitHub Release job. Because a broken handoff could prevent release publication, the repository's produced-artifact and release/deployment boundaries require the high-risk workflow.
- Proposal and tier confirmation: confirmed by the user's explicit `确认，作为standard任务完成就好` instruction on 2026-08-14; the user-selected standard tier is authoritative for this delivery.

## Background and Goal
The three client packaging jobs currently upload single installer files using named archived artifacts (`installer-deb`, `installer-macos`, and `installer-windows`). GitHub therefore presents downloads using those container names and ZIP packaging instead of showing the actual versioned `.deb`, `.pkg`, and `.exe` filenames.

The goal is to use the single-file direct-upload support already available in the pinned `actions/upload-artifact@v7`, so the Actions run exposes each installer directly under its real versioned filename without an additional ZIP wrapper, while preserving successful tag-gated GitHub Release publication.

## Scope
### In scope
- Set `archive: false` for the Debian, macOS, and Windows single-file artifact uploads.
- Remove or stop relying on the ignored `name:` fields for those direct uploads.
- Update the Release job to download the three versioned direct artifacts explicitly and merge them into `release-assets`.
- Preserve current installer filenames, version derivation, retention periods, build commands, release tag gate, and the exactly-three-assets validation.
- Add focused workflow-contract regression coverage for direct uploads and Release retrieval if the existing test suite has no adequate assertion.

### Out of scope
- Publishing installers for branch or pull-request runs as GitHub Releases.
- Changing installer contents, signing, compression inside the installer formats, package metadata, or product version authority.
- Changing the server-image artifact or GHCR publication path.
- Changing build triggers, supported platforms, runner images, or retention policy.

### Boundary with neighboring modules
Only the client artifact transport in `.github/workflows/build.yml` changes. Debian, macOS, and Windows build scripts continue producing the same files. The server image remains an archived job-transfer artifact because it is not a directly downloadable client installer and has a separate GHCR publication flow.

## Requirement Review
The requested outcome is supported by the pinned `upload-artifact@v7`: `archive: false` accepts one file, ignores the configured artifact name, and uses the uploaded filename as the artifact name. This is preferable to manufacturing alternate links or creating extra Releases for non-tag builds.

The required companion change is to replace the Release job's `installer-*` lookup, because those logical names cease to exist in direct-upload mode. Explicit versioned names make the handoff auditable and retain the existing three-platform completeness check.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-direct-installer-artifacts | Upload each client installer as an unarchived single-file Artifact under its actual versioned filename, and update the tag Release job to retrieve all three direct artifacts without changing their contents or publication gate. | `.github/workflows/build.yml` client upload and Release download steps only, plus focused contract coverage where needed. | Direct uploads require one file per artifact and remove the convenient `installer-*` logical naming pattern; the Release job must name each platform file explicitly. | Static workflow/contract checks prove all three uploads use `archive: false`, their paths remain versioned, Release retrieval matches those filenames, and the final Release validation still requires exactly one `.deb`, `.pkg`, and `.exe`; hosted builds remain platform evidence. | No package-content, build-script, server-image, GHCR, signing, trigger, version-source, or non-tag Release change. |

## Success Criteria
- Concrete user-visible or system-visible result: the Actions run lists/downloads `bucky-vpn_<version>_amd64.deb`, `BuckyVPN-<version>.pkg`, and `BuckyVPN_<version>_amd64_Setup.exe` directly rather than `installer-*.zip` wrappers.
- Required evidence: focused workflow-contract validation covering all three direct uploads and their Release download names; existing release completeness validation remains intact; actual hosted runs provide final GitHub UI/platform confirmation.
- Explicit non-goals: changing installer internals, server-image handling, versioning, signing, build triggers, or creating Releases for ordinary branch/PR builds.

## Risks
- If any direct-upload path resolves to zero or multiple files, `archive: false` fails; the existing exact output paths and `if-no-files-found: error` must remain.
- If Release retrieval names differ in case, separators, or version interpolation from the uploaded filenames, tag publication fails despite successful platform builds.
- Local validation cannot prove GitHub's hosted UI rendering; a hosted workflow run remains the decisive confirmation of direct-download presentation.
