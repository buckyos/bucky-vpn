---
task_manifest: task.yaml
status: approved
---

# Refresh README Release and Installation Proposal

Risk profile: not-created (replace with ./risk-profile.yaml only after high-risk confirmation)

## Workflow Tier Judgment
- Proposed tier: standard
- Final tier: standard
- Tier rationale / triggered boundaries: The delivery rewrites one public documentation file and changes no build, package, image, runtime, release, or deployment behavior. The README spans the client and server release surfaces and needs durable handoff value beyond a localized wording correction, so the bounded standard flow is proportionate; no high-risk consequence is introduced by the documentation itself.
- Proposal and tier confirmation: confirmed by the user's explicit `确认` instruction on 2026-08-14; the displayed proposal had no unresolved questions.

## Background and Goal
The root `README.md` still describes local packaging as the main installation path and points server deployment at the obsolete `harbor.mynode.site` image. The current `.github/workflows/build.yml` instead produces directly downloadable Debian, macOS, and Windows installers, publishes them to a matching version GitHub Release, and publishes the server image to GHCR.

The goal is to regenerate the root README as a user-oriented Chinese guide whose download, installation, server deployment, release, and source-build information matches the current repository sources of truth.

## Scope
### In scope
- Rewrite the root `README.md` with a concise project introduction and supported-platform summary.
- Document GitHub Release as the client distribution channel, using the stable Releases/latest page and the exact versioned `.deb`, `.pkg`, and `.exe` filename patterns from the workflow.
- Provide current Windows, Debian/Ubuntu, and macOS installation commands or UI steps appropriate to each generated installer format.
- Document GHCR as the server image channel, including versioned and `latest` tags, the current config template/mount, Web/API port 80, and P2P TCP/UDP port 3624.
- Explain the workflow boundary: manual dispatch performs builds only; an exact pushed `v<vpn-client Cargo version>` Tag in `buckyos/bucky-vpn` publishes the three client assets and two server image tags.
- Retain a compact source-build section based on the four current build scripts and a client join example verified against the live CLI implementation.

### Out of scope
- Changing `.github/workflows/build.yml`, build scripts, package contents, Docker image contents, runtime configuration, ports, versioning, signing, or release permissions.
- Claiming that a particular hosted workflow run, Release, or GHCR image currently exists unless separately verified online.
- Rewriting `vpn_web/README.md`, translating the README to additional languages, or adding screenshots, badges, changelogs, or architecture documentation.

### Boundary with neighboring modules
The root README describes both `bucky-vpn` client installation and `bucky-vpn-server` image deployment, so the packet is stored under `globals`. All behavioral details remain owned by the workflow, packaging scripts, Docker files, server config template, and CLI source; the README only reflects those contracts.

## Requirement Review
Regenerating the README from the current release workflow is reasonable and fixes a misleading deployment path. A user-first order—install released clients first, deploy the published server image second, then show source builds and release mechanics—better serves installation readers than leading with compilation.

Version-specific asset names should use a `<version>` placeholder while the download entry points use stable GitHub URLs. This keeps the instructions accurate after the Cargo version changes. The document will distinguish repository-configured publication behavior from externally confirmed availability so it does not overstate hosted release status.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-readme-client-release-installation | Replace the stale client build-first guidance with GitHub Release download and platform-specific installation instructions matching the workflow's exact installer formats and filenames. | Root `README.md` only; GitHub Actions and package scripts remain unchanged. | Placeholder versions require users to substitute the downloaded release version, but avoid hard-coded version drift. | README links to `buckyos/bucky-vpn` Releases, names all three exact asset patterns, and gives valid Windows, Debian/Ubuntu, and macOS installation steps cross-checked against current package outputs. | No installer, signing, package metadata, version, or publication change. |
| P-002 | CHG-readme-server-image-deployment | Replace the obsolete Harbor example with GHCR image tags and a current config-backed Docker deployment example matching the image entrypoint and server ports. | Root `README.md` only; Dockerfile, config, runtime, and GHCR workflow remain unchanged. | A secure production deployment still requires operators to customize credentials, secrets, public reachability, storage, and any reverse proxy. | README uses `ghcr.io/buckyos/bucky-vpn-server`, mounts `/bucky-vpn/config.yaml`, persists the configured data path, exposes port 80 and P2P TCP/UDP 3624, and warns users to replace template secrets. | No production-hardening automation or server behavior change. |

## Success Criteria
- Concrete user-visible or system-visible result: a new reader can install the current client packages, deploy the current server image, understand where release outputs are published, and find source-build commands without relying on obsolete Harbor or port information.
- Required evidence: focused comparison of every documented asset name, repository URL, image tag, port, mount path, release gate, and build command against `.github/workflows/build.yml`, `Dockerfile`, `start.sh`, `nginx.conf`, `vpn-server/config/config.example.yaml`, the build scripts, Cargo version metadata, and the live client CLI source; Markdown/whitespace validation passes.
- Explicit non-goals: any behavior, workflow, package, image, UI, or external publication change.

## Risks
- Documentation can drift when the workflow or config changes; stable links and version placeholders reduce but do not eliminate that risk.
- A wrong container mount or port would make the example unusable, so those values require direct source cross-checking rather than reuse of the old README.
- GitHub Actions configuration proves intended publication behavior, not that a hosted Release or package is presently available; the README must not blur that distinction.
