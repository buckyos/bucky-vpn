---
task_manifest: task.yaml
status: approved
---

# GitHub Actions Cargo Update Proposal

Risk profile: not-created (replace with ./risk-profile.yaml only after high-risk confirmation)

## Workflow Tier Judgment
- Proposed tier: high-risk
- Final tier: trivial
- Tier rationale / triggered boundaries: An explicit `cargo update` makes every CI run resolve the newest compatible registry and Git dependency revisions instead of building only the dependency graph committed in `Cargo.lock`. The resulting Debian, macOS, Windows, and server artifacts can therefore change without a source or lockfile commit. This is a confirmed dependency/build-graph, supply-chain trust, produced-artifact, and release reproducibility impact, so the high-risk lifecycle is proportionate.
- Proposal and tier confirmation: confirmed by the user's explicit `确认，按简单任务完成就好` instruction on 2026-08-26; the user-selected trivial tier is authoritative. The dependency, supply-chain, produced-artifact, and reproducibility risks that motivated the high-risk recommendation remain recorded for implementation review and handoff.

## Background and Goal
The current workflow checks out the repository's committed `Cargo.lock` and invokes `cargo metadata` plus the platform build scripts, but it never explicitly runs `cargo update`. The goal is to make every GitHub Actions workflow run update Cargo dependencies before compilation, while ensuring all platform jobs in that run compile the same resolved dependency graph.

## Scope
### In scope
- Run `cargo update` exactly once in the `version` job after checkout and before Cargo metadata/version validation.
- Store the generated workspace `Cargo.lock` as a short-lived workflow artifact.
- Make the Debian, macOS, Windows, and server build jobs download that generated lockfile after checking out the already-resolved source commit and before invoking their existing build scripts.
- Fail a build if the shared lockfile cannot be produced or downloaded; do not silently fall back to the committed lockfile.
- Extend the focused GitHub Actions contract test to prove update ordering, one-update-per-run behavior, all-build-job lockfile consumption, and immutable third-party Action pinning.

### Out of scope
- Committing the CI-generated `Cargo.lock` back to the repository or opening an automated pull request.
- Running `cargo update` independently in every platform job.
- Changing dependency declarations, Cargo version constraints, Rust toolchains, runner images, build scripts, package formats, release permissions, or publication gates.
- Adding Cargo registry or target-directory caching.

### Boundary with neighboring modules
The change is confined to GitHub Actions orchestration and its focused repository contract test. Cargo manifests, the committed `Cargo.lock`, application/runtime code, packaging scripts, installers, and container contents are not directly edited. All four build jobs consume the same workflow-generated lockfile so a single run does not mix independently resolved dependency graphs.

## Requirement Review
Executing `cargo update` on every workflow run is feasible, but running it separately in all four build jobs would be redundant and could resolve different moving Git revisions if upstream changes during the run. The chosen direction performs one resolution in the existing prerequisite `version` job and distributes the exact result to all consumers.

This deliberately weakens source-level reproducibility: rerunning the same commit or release Tag later may compile newer compatible crate versions or newer commits from unpinned Git dependencies. The shared generated lockfile preserves consistency only within one workflow run. The generated file remains an ephemeral build input and is not written back to the release Tag or default branch.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-actions-cargo-update | Execute one `cargo update` per GitHub Actions run and distribute its generated `Cargo.lock` to every Rust build job before compilation. | `.github/workflows/build.yml` plus focused workflow-contract coverage; no manifest, committed lockfile, build-script, runtime, or publication-policy edits. | Each rerun can select newer compatible registry crates or Git commits, so the same source Tag is no longer sufficient to reproduce a prior binary; resolving only once keeps all artifacts within the run mutually consistent. | Static workflow tests prove `cargo update` precedes metadata, the generated lockfile is stored once, every platform build consumes it before its build script, missing transfer fails closed, and all external Actions remain commit-pinned; available local validation passes and any hosted-runner gap is recorded. | No automated lockfile commit, per-platform independent update, dependency constraint change, Cargo cache, or release-policy change. |

## Success Criteria
- Concrete user-visible or system-visible result: every manual or version-Tag workflow run executes `cargo update` once, and its Debian, macOS, Windows, and server builds all use the resulting shared `Cargo.lock`.
- Required evidence: YAML parsing and focused workflow contract tests pass; test coverage rejects missing, late, duplicated, or partially consumed lockfile-update wiring; high-risk design/testing/acceptance records explicitly assess dependency consistency and release reproducibility; hosted execution is distinguished from local/static evidence.
- Explicit non-goals: no repository lockfile mutation, dependency declaration change, caching redesign, package-format change, or external GitHub publication during verification.

## Risks
- The same source commit or release Tag can produce different binaries on later runs because compatible registry versions and unpinned Git dependency heads may move.
- A newly published or compromised transitive dependency can enter release artifacts without a reviewed `Cargo.lock` diff committed to the repository.
- GitHub artifacts are an additional intra-workflow handoff; incorrect paths or download ordering could accidentally build with the committed lockfile instead.
- Local static tests cannot prove hosted macOS/Windows execution or external registry availability; missing hosted evidence must remain explicit.
