---
task_manifest: task.yaml
status: approved
---

# Windows Action NASM Provisioning Proposal

Risk profile: ./risk-profile.yaml

## Workflow Tier Judgment
- Proposed tier: high-risk
- Final tier: high-risk
- Tier rationale / triggered boundaries: The change is localized to the Windows GitHub Actions job, but it changes the toolchain used to compile `aws-lc-sys` assembly into the released installer. The failure currently blocks a produced artifact, and selecting/provisioning an external assembler has material release-build and supply-chain consequences, so the repository's build/config trigger requires `high-risk` rather than classifying by the small YAML diff alone.
- Proposal and tier confirmation: confirmed by the user's explicit `确认， 自动完成任务` instruction on 2026-08-13; the same instruction launches automatic Design, Implementation, Testing, and Acceptance without separate stage confirmations.

## Background and Goal
The `build-windows` job reaches `aws-lc-sys` successfully with MSVC, including successful C compilation, then panics because `nasm` cannot be found. The checked-in workflow prepares Inno Setup but does not provision NASM. The current dependency chain for the Windows client is `bucky-vpn -> p2p-frame -> rcgen -> aws-lc-rs -> aws-lc-sys`; AWS-LC documents NASM as required for Windows x86/x86-64 source assembly builds and recommends installing it when available.

The goal is to provision and verify a pinned NASM tool before `build_win.bat` runs so the existing Windows installer job can compile `aws-lc-sys` and continue to the installer verification step.

## Scope
### In scope
- Add a Windows-job setup step before `Build Windows installer` that installs a fixed NASM version through the package manager already present on the `windows-2022` runner.
- Fail early with a clear setup error if installation fails or `nasm -v` is not executable from the build environment.
- Keep the real build command and expected installer verification unchanged.
- Validate workflow structure locally and use a rerun of the hosted Windows job as the decisive platform evidence.

### Out of scope
- Changing `aws-lc-rs`, `aws-lc-sys`, `rcgen`, `p2p-frame`, or Rust crypto-provider features.
- Enabling AWS-LC's crate-provided prebuilt NASM objects.
- Adding or adopting the currently untracked root `Cargo.lock`, upgrading dependencies, or otherwise changing dependency resolution.
- Changing `build_win.bat`, installer contents, signing, release triggers, other platform jobs, or product runtime behavior.

### Boundary with neighboring modules
The workflow owns hosted-runner tool provisioning. `build_win.bat` remains the Windows packaging entrypoint, and the Rust dependency graph remains unchanged. NASM is used only during native compilation of the existing transitive AWS-LC dependency; no new runtime component is shipped separately.

## Requirement Review
Installing NASM is the direct fix and matches AWS-LC's documented preferred path. Enabling prebuilt objects would avoid installing an assembler but changes the provenance of cryptographic native objects and is therefore not the preferred default. Pinning the NASM package version reduces runner-image drift; an explicit `nasm -v` check makes failures occur before the long Cargo build and records the actual tool used.

The supplied log resolved `aws-lc-sys 0.44.0`, while the workspace's untracked local `Cargo.lock` currently contains `0.43.0`. That confirms dependency resolution is not reproducible in the hosted checkout, but adopting a user-owned untracked lockfile is a separate dependency-management decision and is intentionally excluded from this repair.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-provision-windows-nasm | Provision a pinned NASM version in `build-windows`, prove it is callable before Cargo starts, then run the unchanged Windows installer build. | `.github/workflows/build.yml` Windows job only. | Adds one network-fetched build tool and modest setup time; version pinning and explicit verification bound the drift. | Workflow validation passes; the hosted setup prints the expected NASM version; the Windows job gets past `aws-lc-sys` and produces the existing expected installer artifact. | No dependency, crypto feature, prebuilt-object, packaging-script, release-trigger, or runtime change. |

## Success Criteria
- Concrete user-visible or system-visible result: the GitHub Actions Windows job no longer fails with `NASM command not found` and produces the expected versioned `.exe` installer.
- Required evidence: local workflow/configuration checks, an explicit hosted `nasm -v` result, successful `aws-lc-sys` compilation, and successful existing installer-file verification. If hosted execution cannot be triggered from this workspace, that missing platform evidence remains an explicit manual gate rather than being claimed as passed.
- Explicit non-goals: dependency locking/upgrades, use of AWS-LC prebuilt assembly objects, crypto-provider changes, installer behavior changes, and changes to Linux/macOS/server jobs.

## Risks
- NASM compiles assembly used by a cryptographic dependency; package source and version must be explicit, and the job must report the installed version.
- A runner/package-repository outage can fail earlier during provisioning; this is preferable to an opaque build-script panic but remains an external availability dependency.
- Local Linux validation cannot prove Windows PATH propagation or native compilation; hosted Windows execution is required for final platform evidence.
- The untracked `Cargo.lock` means hosted dependency versions can continue to drift independently of this fix; resolving that broader reproducibility issue is outside this task.
