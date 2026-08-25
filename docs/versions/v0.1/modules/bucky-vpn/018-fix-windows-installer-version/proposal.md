---
task_manifest: task.yaml
status: approved
---

# Cross-Platform Packaging Version Audit and Windows Fix Proposal

Risk profile: ./risk-profile.yaml

## Workflow Tier Judgment
- Proposed tier: high-risk
- Final tier: high-risk
- Tier rationale / triggered boundaries: The confirmed defects block or can mis-version the Windows installer, and the requested audit covers all produced package/image paths. This has a confirmed produced-artifact and cross-platform release-build consequence, so repository policy classifies it as high-risk even though the expected code fix is small.
- Proposal and tier confirmation: confirmed by the user's explicit “确认， 自动完成” instruction on 2026-08-13; the same instruction launches automatic Design, Implementation, Testing, and Acceptance without separate stage confirmations.

## Background and Goal
`build_win.bat` currently passes a caret-escaped call operator (`^&`) into PowerShell. Executing the checked-in batch file under `cmd.exe` reproduces a PowerShell `AmpersandNotAllowed` parser failure, so the Cargo-owned version is never reliably delivered to Inno Setup. The reported Inno error is the downstream form of the same contract failure: compilation reaches `[Setup]` without a usable `AppVersion`. A direct Windows invocation of `ISCC.exe /DAppVersion=1.0.0 install.iss` succeeds, proving the installer script and documented `/D` interface work when the value is present.

The cross-platform audit also found that the POSIX entrypoints do not share this failure mode: Debian uses the resolved version in staged control metadata and the `.deb` name; macOS uses it in generated bundle metadata, `pkgbuild`, and the `.pkg` name; the server passes it as a Docker build argument for the image label. Their existing focused fake-tool executions passed. A second Windows-only drift defect does exist: `[package.metadata.winres]` hard-codes `FileVersion` and `ProductVersion` even though `winres` defaults these values from `package.version`, so a future package-version update can leave the executable metadata stale.

The goal is therefore to make the checked-in Windows entrypoint resolve `vpn-client/Cargo.toml` version `1.0.0`, pass it to Inno Setup without cmd/PowerShell escaping damage, inherit Windows executable and installer metadata from that one version, and produce the expected versioned installer with a clear failure for a missing or empty version. The same delivery must retain explicit evidence that Debian, macOS, and server version propagation remains coherent.

## Scope
### In scope
- Correct the `cmd.exe` to PowerShell boundary in `build_win.bat` so Cargo metadata executes rather than passing a literal caret to PowerShell.
- Preserve `vpn-client/Cargo.toml` as the only version source and pass the resolved value to ISCC using its documented `/D<name>=<value>` interface.
- Remove the redundant `FileVersion` and `ProductVersion` overrides under `[package.metadata.winres]` so `winres` inherits the Cargo package version while retaining non-version resource metadata.
- Make `install.iss` reject both an undefined and an empty compile-time version before `[Setup]` parsing can degrade into the generic missing-AppVersion error.
- Strengthen the focused Windows packaging contract so this exact escaping failure cannot pass static validation again.
- Execute the actual `build_win.bat` through Windows `cmd.exe` in this workspace and require `dist/BuckyVPN_1.0.0_amd64_Setup.exe` as the success signal.
- Recheck `build_deb.sh`, `build_macos.sh`, and `build_server.sh` for the same version-source, shell-boundary, metadata, output-name, fail-closed, and cleanup properties; preserve them unchanged when no defect is found.
- Execute the real Debian entrypoint when the installed Linux toolchain permits it; retain focused fake-tool execution for macOS/server boundaries that this host cannot natively satisfy, without claiming hosted-runner success.

### Out of scope
- Changing the product version, installer contents/service behavior, output naming, code signing, GitHub Actions triggers, Release/GHCR behavior, or non-Windows build scripts.
- Refactoring the shared release workflow or introducing a new version parser/dependency.
- Treating a direct `ISCC.exe install.iss` invocation without the build entrypoint as a supported release command.

### Boundary with neighboring modules
`vpn-client/Cargo.toml` continues to own the version. `build_win.bat` owns extraction and compiler invocation, while `winres` and `install.iss` consume the Cargo-owned value for executable and installer metadata. Debian, macOS, and server scripts consume the same package version through Cargo metadata but retain their existing platform-specific outputs. The existing GitHub Actions jobs remain callers and need no behavior change.

## Requirement Review
The reported compiler failure is valid and release-blocking. Current-source reproduction reveals that the batch file fails even earlier than the supplied log because `cmd.exe` leaves a caret in the quoted PowerShell command. The smallest robust fix is to use ordinary PowerShell native-command assignment (`$json = cargo metadata ...`), quote the complete ISCC `/D` argument, and validate an empty macro explicitly. Removing the two redundant winres version overrides closes the second-source drift without changing the package version itself. This preserves the approved version ownership and avoids duplicating TOML parsing in batch syntax. Because a real Windows compiler is available from this workspace, acceptance should require the actual entrypoint to produce the installer rather than relying only on text assertions.

The other-platform audit found no equivalent defect. All three POSIX scripts use strict Bash mode, change to the repository directory before Cargo lookup, require exactly one `bucky-vpn` package with a stable three-component version, and fail through `pipefail` when Cargo or Python fails. Debian stages and verifies `Version:` without dirtying the template; macOS generates package metadata in a temporary root and passes the same value to `pkgbuild`; server passes the same value to Docker. These conclusions must be backed by the existing executable fake-tool tests and, for Debian, a real local package build when available. Native macOS packaging and Docker image assembly remain explicit environment gaps until their hosted runners execute.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-fix-windows-installer-version | Resolve the unique stable `bucky-vpn` Cargo version through PowerShell, let winres inherit that version, pass a non-empty value into Inno Setup, and build the expected versioned Windows installer. | `vpn-client/Cargo.toml`, `build_win.bat`, `install.iss`, and the focused Windows build contract only. | Keeps a compact inline PowerShell command, but validates it by executing the real batch/compiler path rather than trusting source-shape checks. | Before-fix reproduction records `AmpersandNotAllowed`; after the fix, `cmd.exe /d /c build_win.bat` exits 0 and produces `dist/BuckyVPN_1.0.0_amd64_Setup.exe`; focused tests also pass and no second Windows resource version remains. | No installer semantic, workflow, signing, release, or version-ownership change. |
| P-002 | CHG-audit-cross-platform-version-packaging | Audit Debian, macOS, server, and Windows entrypoints for single-source version propagation, shell-boundary correctness, metadata/output alignment, fail-closed behavior, and temporary-state cleanup; repair only confirmed defects. | Root packaging scripts and directly consumed package/image metadata; no installer runtime or workflow redesign. | macOS and Docker cannot be fully built on this host, so their local evidence is executable contract validation rather than invented native success. | Focused entrypoint tests pass for all four scripts; Debian real build passes when locally supported; Windows real batch/ISCC build passes; macOS/server hosted gaps are explicit. | No unrelated packaging refactor, platform expansion, installer behavior change, or deployment test. |

## Success Criteria
- Concrete user-visible or system-visible result: running `build_win.bat` on Windows successfully compiles the correctly versioned installer instead of failing extraction or reporting a missing `AppVersion`; the other platform packaging paths retain one Cargo-owned version through their metadata and output contracts.
- Required evidence: exact before-fix reproduction; real Windows batch and ISCC success; expected versioned output exists; Windows executable metadata no longer has a second hard-coded version; missing/empty version guard remains fail-closed; all four focused build contracts pass; real Debian build runs when supported; macOS/server environment gaps are stated accurately.
- Explicit non-goals: version bump, installer redesign, workflow changes, signing, and non-Windows packaging changes.

## Risks
- Batch, `for /f`, and nested PowerShell quoting are sensitive to cmd escaping; a static substring assertion is insufficient.
- An empty but defined ISPP variable bypasses `#ifndef`, so the installer script needs an explicit empty-value guard.
- Cargo `[package.metadata.winres]` overrides take precedence over winres defaults; retaining explicit version strings would preserve a hidden second version source.
- A successful direct ISCC invocation does not prove the repository entrypoint works; verification must exercise `build_win.bat` itself.
- POSIX fake-tool contracts prove orchestration and metadata flow but not native macOS toolchain or Docker-runner availability; those boundaries remain explicit until hosted execution.
