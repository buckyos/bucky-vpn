---
task_manifest: task.yaml
status: approved
---

# GitHub Actions Cross-Platform Build Proposal

Risk profile: ./risk-profile.yaml

## Workflow Tier Judgment
- Proposed tier: high-risk
- Final tier: high-risk
- Tier rationale / triggered boundaries: The workflow will exercise four platform-specific packaging entrypoints and produce or validate Debian, macOS, Windows, and Docker artifacts. It crosses the `bucky-vpn`, `bucky-vpn-server`, and `vpn_web` build surfaces and introduces third-party Actions/toolchain setup into the supply-chain boundary. These are confirmed produced-artifact, build-graph, cross-project, and release-entry consequences.
- Proposal and tier confirmation: confirmed by the user's explicit “确认，自动完成” instruction on 2026-08-12; the same instruction launches automatic Design, Implementation, Testing, and Acceptance without separate stage confirmations.

## Background and Goal
The repository documents four local build entrypoints but has no checked-in GitHub Actions workflow. The goal is to add one maintainable CI workflow that provisions each required runner/toolchain, invokes the same functional build path as `build_deb.sh`, `build_macos.sh`, `build_server.sh`, and `build_win.bat`, retains useful build outputs, publishes client installers to a GitHub Release, and publishes the server image to GitHub Container Registry (GHCR) for an explicitly pushed matching version Tag.

## Scope
### In scope
- Add `.github/workflows/build.yml` with independent Debian client, macOS client, Windows client, and Linux server-image jobs on the matching hosted runner operating systems.
- Trigger builds for pushes and pull requests targeting `master`, with `workflow_dispatch` for manual runs.
- Install or select the Rust targets and platform tools required by each existing script, including musl tooling, Flutter for the server Web build, macOS universal-binary packaging tools, and Inno Setup for Windows.
- Invoke the existing build scripts as the canonical packaging entrypoints wherever runner shell semantics permit, rather than duplicating their packaging commands in YAML.
- Upload the Debian package, macOS package, and Windows installer as workflow artifacts. Verify that the server Docker image is produced locally by `build_server.sh` on ordinary CI runs.
- Use `vpn-client/Cargo.toml` `package.version` as the single version source for the Debian package metadata/filename, macOS package metadata/filename, Windows installer metadata/filename, and GitHub Release.
- On a pushed `v*` Tag, require the Tag without its leading `v` to equal the exact `vpn-client` Cargo package version, then create the corresponding GitHub Release only after all required package jobs pass and attach the `.deb`, `.pkg`, and Windows installer.
- Use GitHub's automatically generated Tag source archives (`Source code (zip)` and `Source code (tar.gz)`) for release source downloads; do not generate or upload a duplicate custom source archive.
- On the same matching Tag, publish the already validated server image to `ghcr.io/buckyos/bucky-vpn-server:<cargo-version>` and update `ghcr.io/buckyos/bucky-vpn-server:latest`; ordinary pushes, pull requests, and manual CI builds must not log in to or push to GHCR.
- Make the minimal build-script and packaging-metadata adjustments needed to consume the Cargo version reliably in local and CI builds, while preserving local behavior and the user's existing uncommitted `build_win.bat` content.

### Out of scope
- Pushing Docker images to Harbor or any registry other than GHCR, signing/notarizing packages, or introducing long-lived repository secrets or personal access tokens.
- Automatically incrementing or selecting the version in `vpn-client/Cargo.toml`; maintainers remain responsible for changing that source value before release.
- Changing application runtime behavior, service definitions, installer behavior, or Docker runtime configuration.
- Adding non-x86 Linux packages, separate per-architecture macOS packages, Flutter mobile/desktop builds, or test-suite execution unrelated to proving these build paths.
- Refactoring all four scripts or introducing a new release/version-management system.
- Creating a custom source archive, source bundle with generated files, or full-history repository archive; GitHub's automatic Tag archives are sufficient.
- Automatically changing the GHCR package visibility or organization package-access policy; the workflow publishes a repository-linked package and leaves public/private visibility to repository or organization administrators.

### Boundary with neighboring modules
`bucky-vpn` remains the owner of the Debian, macOS, and Windows client binaries/installers, with `vpn-client/Cargo.toml` owning the shared product release version. `bucky-vpn-server` remains the owner of the server binary and Docker image; `vpn_web` contributes the Flutter Web files copied into that image. GitHub Actions orchestrates existing entrypoints, stores temporary CI artifacts, promotes the three client packages to a Tag-bound GitHub Release, and promotes the validated server image to the repository-linked GHCR package under the same product version. It does not change application contracts or deploy the image.

## Requirement Review
Adding CI coverage for the existing build scripts is reasonable and removes the current dependence on undocumented developer-machine setup. Independent jobs are preferable to one matrix because the four platforms need materially different setup and produce different artifact paths. The proposed trigger set gives pull-request feedback and manual rebuilds, while artifact uploads make successful packaging inspectable. Tag-only promotion keeps ordinary pushes and pull requests unable to publish Releases or container packages. Using the client Cargo package version as the shared product version is preferable to retaining conflicting hard-coded versions because it is committed with the exact source being packaged. Versioning the server image with the same value makes one product release resolve to one client/server set; the `latest` tag is updated only after that versioned image push succeeds. The cost is that macOS and multi-platform packaging are heavier than a compile-only check and packaging scripts must reliably derive one Cargo value; keeping signing and external registry credentials out of scope limits the security surface.

The implementation should pin Actions to immutable revisions where practical, use read-only permissions for build jobs, grant `contents: write` only to the Tag-gated GitHub Release job, and grant `packages: write` only to the Tag-gated GHCR publication job. Both publishers use the ephemeral repository `GITHUB_TOKEN` rather than a personal token. It should avoid silently downloading unreviewed build helpers inside scripts. Exact hosted-runner labels and preinstalled-tool assumptions will be verified against current primary documentation during design/implementation rather than guessed from old runner images. Publication must fail before any external mutation when the Tag/version check fails.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-github-deb-build | Add a Linux job that provisions musl and Debian packaging prerequisites, runs `build_deb.sh`, and uploads a `.deb` whose version is derived from `vpn-client/Cargo.toml`. | Existing x86_64 Debian client package only. | musl/package setup adds CI time and Debian metadata must be generated without leaving tracked-file drift. | Workflow syntax validation plus a successful job producing `dist/bucky-vpn_<cargo-version>_amd64.deb`, with matching Debian control metadata, or a clearly recorded external-runner gap before merge. | No new architectures or package-layout redesign. |
| P-002 | CHG-github-macos-build | Add a macOS job that installs both Rust targets, runs `build_macos.sh`, and uploads the universal `.pkg` using the client Cargo version. | Existing unsigned/unnotarized macOS package only. | Universal compilation is slower and hosted-runner architecture must be selected deliberately. | The script produces `dist/BuckyVPN-<cargo-version>.pkg`; package metadata and universal binary are checked where the runner is available. | No signing, notarization, DMG, or separate architecture packages. |
| P-003 | CHG-github-windows-build | Add a Windows job that makes Inno Setup available, runs `build_win.bat`, and uploads the installer using the client Cargo version. | Existing x86_64 Inno Setup package and tracked `wintun.dll`. | Relies on a Windows hosted runner and must pass the Cargo-derived version into Inno Setup without retaining a second authoritative constant. | The job produces `dist/BuckyVPN_<cargo-version>_amd64_Setup.exe`, with any unavailable live runner evidence explicitly recorded. | No MSI, signing, installer-semantic change, or ARM build. |
| P-004 | CHG-github-server-build | Add a Linux job that provisions musl and Flutter, runs `build_server.sh`, verifies the local image on ordinary CI, and on a matching release Tag publishes it to GHCR with `<cargo-version>` and `latest` tags. | Existing server binary, Flutter Web bundle, Dockerfile, and repository-linked `ghcr.io/buckyos/bucky-vpn-server`; no runtime deployment. | This is the heaviest job, introduces a registry write on release Tags, and deliberately couples the server image version to the client product version. | Ordinary CI proves `docker image inspect bucky-vpn-server:latest`; negative validation proves non-release events cannot authenticate/push; a matching Tag publishes both GHCR tags to the same image digest using job-local `packages: write`. | No Harbor/other registry, branch/SHA tags, multi-architecture image, deployment, or Docker runtime behavior change. |
| P-005 | CHG-github-release | For a pushed `v<version>` Tag matching `vpn-client/Cargo.toml`, create a GitHub Release after all package jobs pass and attach the `.deb`, `.pkg`, and Windows installer; rely on GitHub's automatic ZIP and tar.gz source archives. | Tag-gated client-package publication only; ordinary pushes, pull requests, and manual CI builds cannot publish. | Adds `contents: write` to one gated job and makes a failed or mismatched platform build block release publication; automatic source archives avoid duplicate storage but are GitHub-generated rather than workflow-produced assets. | Negative validation rejects a mismatched Tag; a matching Tag maps to the Cargo version; release logic consumes exactly the three expected installer packages, does not upload a custom source bundle, and uses the repository token with job-local permission. | No automatic version bump, release from an untagged commit, custom/full-history source archive, signing, or external installer registry. |

## Success Criteria
- Concrete user-visible or system-visible result: GitHub shows one build workflow with four independent jobs covering every existing root build script on its intended platform; pushing a matching version Tag publishes one Release containing the three client installers plus GitHub's automatic ZIP/tar.gz source archives, and publishes `ghcr.io/buckyos/bucky-vpn-server:<cargo-version>` plus `latest`, all from the same product version.
- Required evidence: workflow syntax/static validation; one-source version extraction and Tag-match/mismatch validation; package metadata/filename checks; event/permission checks proving publication is Tag-only; current runner/action/tool assumptions verified from primary sources; each job's command and expected output recorded; locally runnable validation executed; platform-only gaps explicitly identified; and, when GitHub-hosted execution is available, all four jobs pass and the matching Tag produces the expected Release assets and GHCR tags.
- Explicit non-goals: automatic version increments, Harbor/other registry push, Docker image as a Release tarball, multi-architecture images, code signing/notarization, personal tokens, deployment, application changes, and unrelated test or packaging redesign.

## Risks
- Hosted-runner images and preinstalled tools change over time; undocumented assumptions can make the workflow flaky.
- Third-party setup Actions expand the supply-chain trust boundary and must be minimized and pinned deliberately.
- macOS universal compilation depends on the selected runner architecture and Rust target availability.
- A passing Rust compile does not prove package assembly; each job must verify its final package or image rather than stop after `cargo build`.
- Cargo, Debian, macOS, and Inno Setup impose different version syntax constraints; the workflow and scripts must reject an unsupported client Cargo version rather than silently rewrite it into different identities.
- GitHub Release publication is an external, user-visible mutation; it must occur only for an existing matching Tag after every required build succeeds, with least-privilege permissions.
- GHCR publication adds a package-write trust boundary; build jobs must remain read-only, credentials must not be exposed to pull requests, and both version and `latest` tags must resolve to the same pushed digest.
- The GHCR package may default to organization-controlled visibility; publishing successfully does not imply anonymous pull access, and this task does not silently change that policy.
- The current worktree already contains unrelated modifications, including line-ending-only changes to `build_win.bat`; implementation must preserve and separate those changes from this task.
- Full validation requires GitHub-hosted macOS and Windows runners. If this environment cannot launch the workflow, acceptance must clearly distinguish static/local evidence from missing remote execution evidence.
