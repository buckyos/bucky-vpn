# GitHub Actions Cross-Platform Build and Release Acceptance Report

## Findings
| ID | Severity | Kind | Evidence | Problem | Blocking |
|----|----------|------|----------|---------|----------|
| F-000 | none | implementation | `proposal.md` P-001 through P-005; `pipeline/plan.md`; `.github/workflows/build.yml`; four build entrypoints; `tests/github_actions_build_contract.py`; successful task artifact `.harness/test-results/test-runs/20260812T141110Z-globals+017-add-github-actions-builds-all.json` | Independent falsification found no blocking requirement, design, implementation, or automated-test defect. Native macOS/Windows packaging and real Release/GHCR mutation remain intentionally unexecuted until the first matching hosted Tag run. | no |

## Result Summary
- Overall result: accepted
- Outcome: ordinary pushes, pull requests, and manual runs build without publishing; an official repository push of the exact `v<vpn-client Cargo version>` Tag builds all deliverables, publishes the server image to GHCR, and creates a GitHub Release with the three installers while GitHub supplies its automatic source archives.
- What was verified: version ownership, four build entrypoints, runner/tool setup, artifact paths, Tag/repository gates, job-local permissions, immutable Action references, Release inputs, GHCR tag identity, script cleanup, negative version handling, and affected Rust consumer compilation.
- Evidence used: approved proposal and plan, current implementation and test source, 13 focused contract tests, POSIX syntax checks, structured YAML parsing, fake-tool packaging executions, and the successful task-scoped compile/test artifact.
- Blocking issues: none.
- Next action: close the task packet; the maintainer's first exact version Tag push supplies the recorded hosted-runner and external-publication evidence.

## Object and Scope
- Task manifest: `task.yaml`
- Module: globals, targeting `bucky-vpn` and `bucky-vpn-server` build/release surfaces.
- Version: v0.1 task packet; current product version read from `vpn-client/Cargo.toml` is `1.0.0`.
- Task name: `017-add-github-actions-builds`.
- change_id values reviewed: `CHG-github-deb-build`, `CHG-github-macos-build`, `CHG-github-windows-build`, `CHG-github-server-build`, `CHG-github-release`.
- Review date: 2026-08-12.
- In scope: `.github/workflows/build.yml`, `build_deb.sh`, `build_macos.sh`, `build_win.bat`, `install.iss`, `build_server.sh`, `Dockerfile`, version/package metadata boundaries, focused tests, and task-local Harness evidence.
- Out of scope: creating or pushing a real Git Tag, mutating GitHub Release/GHCR from this workspace, deployment, code signing/notarization, multi-architecture server images, unrelated dirty-worktree files, and broad product runtime suites.
- Task-relevant acceptance scope: the repository-defined build and conditional publication contract, not the external state produced by a future Tag.
- Out-of-scope checks not run: native hosted macOS universal package build, hosted Windows Inno Setup build, live GHCR push, live GitHub Release creation, installer execution, and broad runtime/integration suites.

## Optional Diff / Status Evidence
- `git status --short` summary: six tracked task files are modified and the workflow/test/task packet are new; the worktree also contains pre-existing unrelated tracked and untracked files that were excluded from this review.
- `git diff --stat` summary: task-scoped inspection covered the six tracked implementation changes plus new workflow, test, and task-local Harness files.
- `git diff --name-status` summary: implementation paths match the five plan bindings; tests and task-local evidence are owned by T-1/A-1.
- `git diff --check` result: task-owned tracked implementation diffs pass; the only whole-worktree warnings are pre-existing CRLF/trailing-whitespace changes under unrelated `vpn_web` paths.
- Note: diff/status output was used only to locate task evidence, not as proof of correctness.

## Requirement Review
| Requirement or Boundary | Source | Implementation Evidence | Finding | Status |
|-------------------------|--------|-------------------------|---------|--------|
| Debian job invokes `build_deb.sh` and uploads a Cargo-versioned `.deb`. | proposal P-001 / `CHG-github-deb-build` | `build_deb.sh`; workflow `build-deb` job | Temporary package staging preserves the template, aligns control metadata and filename, and fails before upload on missing output. | pass |
| macOS job invokes `build_macos.sh` and uploads a universal Cargo-versioned `.pkg`. | proposal P-002 / `CHG-github-macos-build` | `build_macos.sh`; workflow `build-macos` job | Both Darwin targets feed `lipo`; staged bundle metadata and pkgbuild version use the Cargo version and the final path is checked. | pass |
| Windows job invokes `build_win.bat` and uploads an Inno Setup installer. | proposal P-003 / `CHG-github-windows-build` | `build_win.bat`; `install.iss`; workflow `build-windows` job | PowerShell resolves the unique Cargo package version, ISCC receives it as a required define, and the expected `.exe` is checked before upload. | pass |
| Server job builds the existing inputs and matching Tag publishes version and latest to GHCR with least privilege. | proposal P-004 / `CHG-github-server-build` | `build_server.sh`; `Dockerfile`; workflow build/publish jobs | Ordinary events do not export/login/push; official matching Tags publish one loaded image under both names and compare remote digests. | pass |
| Matching Tag creates a GitHub Release with three installers and automatic GitHub source archives. | proposal P-005 / `CHG-github-release` | workflow version and release jobs | Exact Tag mismatch fails early; Release waits for all builds and GHCR, accepts exactly `.deb`, `.pkg`, `.exe`, and runs no custom archive command. | pass |

## Evidence Coverage
| Documented Item | Source Document | Implementation Evidence | Test / Result Evidence | Status |
|-----------------|-----------------|-------------------------|------------------------|--------|
| `CHG-github-deb-build` | proposal P-001; plan Debian binding | `build_deb.sh` stages metadata in a temporary root and `.github/workflows/build.yml` verifies/uploads the Cargo-versioned `.deb` | fake Cargo/dpkg execution, template immutability assertion, and compile closure in the successful task artifact | implemented |
| `CHG-github-macos-build` | proposal P-002; plan macOS binding | `build_macos.sh` builds both architectures, creates a staged universal app, and emits a versioned `.pkg`; workflow uses `macos-15-intel` | fake Cargo/lipo/pkgbuild execution, staged metadata/output assertions, and workflow contract | implemented |
| `CHG-github-windows-build` | proposal P-003; plan Windows binding | `build_win.bat` resolves Cargo metadata, passes `/DAppVersion`, checks ISCC and final output; `install.iss` requires the define | static batch/Inno version-propagation and workflow runner/tool/output assertions | implemented |
| `CHG-github-server-build` | proposal P-004; plan server binding | `build_server.sh`, `Dockerfile`, build job, Tag-only image handoff, least-privilege GHCR publisher, version/latest digest comparison | fake Cargo/Flutter/Docker execution plus workflow permission, gate, tag, and digest contracts | implemented |
| `CHG-github-release` | proposal P-005; plan Release binding | version job rejects mismatched pushed Tags; release job depends on successful GHCR publication, downloads exactly three installer classes, and invokes `gh release create` without custom archives | executable event matrix plus structured workflow assertions for dependencies, assets, permissions, and absent custom archive commands | implemented |

## Test Design Adequacy
| Behavior / Risk / change_id | Required Case Types | Test Design Evidence | Runnable Test Evidence | Status |
|-----------------------------|---------------------|------------------------|------------------------|--------|
| client packagers / three client change IDs | normal, boundary, negative, error, compatibility, cleanup, cross-boundary | testplan unit step exercises staged outputs and metadata; contract closure compiles affected Rust targets; hosted native execution is explicitly manual | 13/13 contract tests and Rust all-target compile closure passed | adequate |
| server image / `CHG-github-server-build` | normal, negative event, error propagation, compatibility, publication lifecycle, cross-module | fake server entrypoint execution checks Cargo-Flutter-Docker ordering and version build arg; workflow tests inspect permission/event/digest boundaries; live registry is manual | local automated evidence passed; hosted Tag/GHCR mutation intentionally not fabricated | adequate |
| Release / `CHG-github-release` | matching/mismatched Tag, fork/manual event, missing/extra asset error, permission boundary, publication lifecycle | executable version-job matrix and structured three-asset/dependency/permission checks; live external mutation is manual | matching official push publishes=true, fork/manual publish=false, mismatch exits nonzero | adequate |

## Implementation Review
| Area | Evidence | Finding | Status |
|------|----------|---------|--------|
| requirement-and-behavior | proposal P-001..P-005 mapped against scripts, workflow jobs, image tags, Release command, and source-archive exclusion | Every requested build entrypoint is called; Cargo owns the version; Release receives `.deb`, `.pkg`, `.exe`; GHCR receives version and latest; no custom source archive is created. | pass |
| logic-and-control-flow | version job executable matrix; `needs` edges; shell `set -euo pipefail`; build-script exit checks | Only an official matching pushed Tag enables publication. All build jobs must succeed before GHCR, and Release additionally waits for GHCR success. Mismatch terminates before mutation. | pass |
| boundary-and-input | unique package filtering, strict three-component version regex, exact Tag comparison, exact three-asset count, missing ISCC/output checks | Missing, ambiguous, prerelease, wrong Tag, fork repository, and missing/extra artifact paths fail or remain non-publishing; no unchecked user path becomes a command. | pass |
| state-and-data-integrity | Debian/macOS temporary staging, artifact handoff, Release/GHCR dependency order, digest equality | Local templates are not mutated; failed builders cannot promote artifacts; version/latest are created from one loaded image and compared after push. A rerun against an existing Release fails visibly. | pass |
| error-handling-and-recovery | strict POSIX scripts, batch `errorlevel`, artifact `if-no-files-found`, image/digest assertions, `gh release create` exit behavior | Errors propagate as failed jobs and stop downstream mutation. No personal-token fallback or partial-success masking exists. | pass |
| resource-lifetime-and-cleanup | `mktemp` plus EXIT traps; server image retention 1 day; installers 14 days | Debian/macOS staging is cleaned on success and error; Action artifacts have bounded retention; Docker runner resources are ephemeral hosted-job resources. | pass |
| concurrency-and-ordering | `needs` graph, distinct artifact names, merged installer download, server image handoff | Builds have no shared runner filesystem; publication cannot race ahead of artifacts. Release waits for GHCR success. | pass |
| interface-and-compatibility | existing entrypoint names, Cargo lookup replacing hard-coded versions, plan migration table, artifact assertions | Commands and naming shapes remain compatible with corrected versions; new Tag/GHCR contracts are explicit and Rust public APIs are unchanged. | pass |
| security-and-capacity | workflow permissions, official-repository gate, immutable Action SHAs, GITHUB_TOKEN, artifact retention | Builds are read-only; only publisher jobs receive narrowly scoped writes; no long-lived secret is added; the large image tar is Tag-only and expires after one day. | pass |
| test-adequacy | inspected 13 tests and successful compile/unit artifact | Tests expose wrong event/repository/Tag, mutable Actions, excess permissions, asset errors, divergent tags, invalid versions, and dirty templates. Hosted/external success remains explicit residual risk, not an invented pass. | pass |

## Document Consistency
| Document | Source | Implementation Consistency | Finding | Status |
|----------|--------|----------------------------|---------|--------|
| design | `pipeline/plan.md` | All five scope bindings, interfaces, failure flows, permissions, and output contracts are implemented in the named paths. | No design/implementation contradiction or missing mapped outcome was found. | pass |
| testing | `testplan.yaml` | The successful task artifact ran exactly the declared compile contract and focused unit step for every change ID. | Test code matches the declared risks; hosted/native and external mutations remain explicitly manual. | pass |

## Generated Acceptance Rules
| Rule ID | Source | Expected Result | Evidence Required | Status |
|---------|--------|-----------------|-------------------|--------|
| AR-001 | proposal P-001..P-003 | all client installers derive metadata and filenames from the unique stable `bucky-vpn` Cargo version | script execution contracts and workflow asset checks | pass |
| AR-002 | proposal P-004; security risk profile | ordinary events cannot publish; official matching Tag publishes one server image under version and latest with least privilege | event/permission/tag/digest contracts | pass |
| AR-003 | proposal P-005; contract risk profile | mismatched Tag fails before mutation; matching Tag attaches exactly three installers and relies on automatic GitHub source archives | executable version gate and Release workflow assertions | pass |
| AR-004 | build risk profile | each repository entrypoint remains the workflow's packaging authority and affected Rust consumers compile | structured workflow check, fake-tool entrypoint runs, compile-only closure | pass |

## Consistency Summary
- Proposal authority check: the user-confirmed proposal remains the acceptance baseline and contains all later refinements.
- Proposal vs design: plan bindings cover all five proposal IDs without narrowing Release assets, version ownership, or GHCR output.
- Design vs testing implementation: testplan and test source cover every change ID and all risk-profile build/contract/security requirements.
- Design vs long-lived boundary doc: not applicable; no architecture or public runtime boundary document is changed.
- Design vs implementation: workflow, scripts, permissions, failure flows, and output names conform to the plan.
- Test implementation vs test code vs results: the two testplan commands exactly match the successful task artifact.
- Test design adequacy: adequate for locally testable contracts; native hosted builds and external mutations are accurately manual.
- change_id traceability: complete for all five IDs in proposal, plan, testplan, test artifact, and this report.
- Acceptance criteria traceability: AR-001 through AR-004 cover all approved outcomes and risk checks.
- Cross-module admission: server binary, Flutter Web, Docker, Actions artifacts, Release, and GHCR handoffs are explicitly ordered and tested at the contract boundary.
- Public API / codec / runtime semantics review: no public API, codec, protocol, or deployed VPN runtime behavior changes.
- Document logic review: no contradiction or impossible acceptance state found; hosted execution is an explicit residual, not a local pass claim.
- Implementation logic review: all ten defect-discovery categories above were independently inspected.
- Implementation correctness audit completeness and routing: complete; no return to design, implementation, or testing is required.
- Document approval timing: proposal approval predates implementation; auto-pipeline plan and risk checks passed before testing.
- Implementation task paths bound to design Scope Paths: all task implementation paths match their plan bindings; T-1/A-1 own test and task-local evidence paths.
- Bugfix red-green regression evidence: not applicable; this is a new build/publication capability rather than a production bugfix.

## Validation Evidence
- Existing schema result: proposal/task/plan inputs passed `harness-check.py --profile pre-edit` on 2026-08-12.
- Existing admission stamp: auto-pipeline launch is bound to the user's explicit `确认，自动完成` statement in `pipeline/plan.md`.
- Existing stage-scope result: task implementation files match all five `Implementation Scope Bindings`; test and acceptance artifacts are owned by T-1/A-1.
- Existing pipeline-plan result: `pipeline-plan-check.py` passed against final plan hash `7f94cde8c3ea1a2883e525fd55c2912421ac08854d345d5bf06f250278a123a2` after the consumer-path mapping was normalized.
- Task-relevant test run artifact: `.harness/test-results/test-runs/20260812T141110Z-globals+017-add-github-actions-builds-all.json`, exit code 0.
- Commands rerun because checker-owned inputs changed after their previous pass: final acceptance-report and complete pipeline checks are run after this report and runtime state are finalized.
- Direct package/module runtime suites, whole-project suites, and root shortcuts: not run; the risk-triggered compile-only consumer closure ran only inside the task artifact.
- Risk-triggered task-local contract kinds and assertions: `repository-compile-closure` / `repository-consumers-compile`, plus the focused unit contract for all five change IDs.
- Scoped evidence input hash current: the successful artifact records the current workflow, scripts, Dockerfile, installer definition, Cargo metadata, Debian template, testplan, and test source as evidence inputs.
- Quality gates: not run automatically because the user did not explicitly request a repository quality gate.
- Explicitly requested quality run artifact: none.
- Architecture doc check: not applicable; no architecture evidence is relevant to this CI/release task.
- Acceptance report check after this report was created or modified: passed on 2026-08-12; it is rerun by the complete pipeline check.
- Targeted migration search: hard-coded installer version ownership was removed from Debian, macOS, Windows, and Release paths; unsupported Cargo versions are rejected.

## Automated Test Exception
- Applies: no
- Reason: a successful automated task run exists.
- Owner: not applicable because no exception is used.
- Risk: native hosted and external publication execution remains a documented manual boundary, not an automated-test exception.
- Acceptance impact: no blocking impact under the approved plan; first matching Tag is the operational confirmation.
- Alternative evidence: structured workflow checks and fake-tool entrypoint executions cover local contracts without fabricating external mutations.

## Conclusion
- Accepted / rejected / needs changes: accepted
- Reason: every approved build and publication outcome is present with exact Cargo version ownership, fail-closed publication gates, least-privilege tokens, bounded artifact handling, and focused falsification coverage; no blocking defect was found.
- Supporting task-relevant test evidence: `.harness/test-results/test-runs/20260812T141110Z-globals+017-add-github-actions-builds-all.json`, exit code 0, two successful steps covering all five change IDs.
- Residual risk: macOS/Windows GitHub runner images, Inno Setup availability, Flutter/Docker hosted behavior, GitHub Release creation, automatic source archive presentation, and GHCR permissions/digest observation are not executable locally. The first exact `v1.0.0` Tag run is the required external confirmation. Reproducibility also remains bounded by pre-existing `nginx:latest`, the repository's Flutter package mirror lockfile, and the currently untracked Cargo lockfile, none of which this approved task changes.

## Follow-Up Tasks
- Requirement task: none.
- User decision required for proposal issue: none.
- Design task: none.
- Implementation task: none.
- Testing task: none; hosted-runner and external publication checks occur operationally on the first matching Tag.
- Testing return reason if coverage is incomplete: none; manual hosted boundaries are explicitly approved and recorded.
- Iteration count: 1.
- Stop reason if more than 5 unsuccessful iterations: not applicable.
