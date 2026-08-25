# Windows Action NASM Provisioning Acceptance Report

## Findings
| ID | Severity | Kind | Evidence | Problem | Blocking |
|----|----------|------|----------|---------|----------|
| F-001 | low | testing-consistency | Hosted `windows-2022` run 31716578412 / job 94502776485 is real red evidence for the superseded Machine/User PATH discovery design. Fresh task artifact `20260813T160607Z-bucky-vpn+019-install-nasm-for-windows-action-all.json` covers the corrected source contract and available local Windows packaging path, while `testplan.yaml` keeps hosted integration manual. | The corrected `%ProgramFiles%\NASM` discovery and `GITHUB_PATH` handoff have not run on a new hosted runner because doing so requires a pushed commit or external workflow dispatch. The platform must not be reported green until that rerun succeeds. | no |

## Result Summary
- Overall result: accepted
- Outcome: the workflow now validates Chocolatey NASM 2.16.3 at its package-defined `$env:ProgramFiles\NASM\nasm.exe` location and publishes `$env:ProgramFiles\NASM` through `GITHUB_PATH` before the unchanged verification and Windows installer build.
- What was verified: proposal and design coverage, exact workflow behavior, failure paths, unchanged build and installer consumers, seven focused regression tests, Rust compile closure, and a real local Windows native/package build.
- Evidence used: `.github/workflows/build.yml`, `tests/windows_action_nasm_contract.py`, `pipeline/plan.md`, `testplan.yaml`, runtime state, hosted runs 31710104500 and 31716578412, and fresh task artifact `20260813T160607Z-bucky-vpn+019-install-nasm-for-windows-action-all.json`.
- Blocking issues: none; F-001 remains a visible manual hosted-platform gate and is not a hosted-success claim.
- Next action: commit and push the correction, then require a hosted `build-windows` job to print NASM 2.16.03, compile `aws-lc-sys`, and pass the existing installer verification.

## Object and Scope
- Task manifest: `task.yaml`
- Module: bucky-vpn.
- Version: v0.1; current product version is `1.2.0` from `vpn-client/Cargo.toml`.
- Task name: `019-install-nasm-for-windows-action`.
- change_id values reviewed: `CHG-provision-windows-nasm`.
- Review date: 2026-08-14.
- In scope: the `build-windows` job in `.github/workflows/build.yml`, deterministic `$env:ProgramFiles\NASM\nasm.exe` validation, `GITHUB_PATH` handoff to later pwsh/cmd steps, unchanged `build_win.bat` and installer checks, focused contract tests, testplan, runtime state, and fresh task artifact.
- Out of scope: the untracked root `Cargo.lock`, dependency or crypto-provider changes, AWS-LC prebuilt objects, `build_win.bat` changes, signing, release triggers, non-Windows jobs, and product runtime behavior.
- Task-relevant acceptance scope: whether the returned implementation closes the two observed hosted PATH failures without broadening the approved build and supply-chain boundary.
- Out-of-scope checks not run: a pushed or dispatched GitHub Actions rerun of the uncommitted correction and any claim that hosted native compilation is already green.

## Optional Diff / Status Evidence
- `.github/workflows/build.yml` changes only the NASM setup block; the separate `Verify NASM`, `build_win.bat`, installer verification, and other jobs remain unchanged.
- `git diff --check -- .github/workflows/build.yml` passed.
- Unrelated tracked edits remain in `vpn_web/README.md` and `vpn_web/lib/base58.dart`; they were not modified or attributed to this task.
- The task-owned untracked sources remain the `019-install-nasm-for-windows-action` packet and `tests/windows_action_nasm_contract.py`; other pre-existing untracked files remain outside this delivery.

## Requirement Review
| Requirement or Boundary | Source | Implementation Evidence | Finding | Status |
|-------------------------|--------|-------------------------|---------|--------|
| `CHG-provision-windows-nasm`: install exact NASM 2.16.3 before Cargo, prove discovery, and keep existing build and installer checks. | proposal P-001 and Success Criteria; risk profile contract/build checks | `.github/workflows/build.yml` `Install NASM` and `Verify NASM` steps | The exact Chocolatey pin remains. The workflow derives `$env:ProgramFiles\NASM`, validates its `nasm.exe`, publishes the directory, and runs standalone `nasm -v` before the unchanged build. | pass |
| Fail early and clearly if installation does not provide the expected executable. | proposal In scope; plan Failure Flows | `Test-Path -LiteralPath $nasm -PathType Leaf` and explicit throw in `.github/workflows/build.yml` | Chocolatey failure remains nonzero; a successful package command with a missing expected file now fails before any PATH publication or Cargo invocation. | pass |
| Make NASM available to later PowerShell and cmd processes. | hosted runs 31710104500 and 31716578412; plan State Ownership | `$nasmDirectory` append to `$env:GITHUB_PATH`; separate `nasm -v`; unchanged `shell: cmd` build | The workflow no longer expects the package to mutate Machine/User PATH. GitHub's ordered step handoff supplies the validated directory to both later consumers. | pass |
| Preserve dependency, crypto, packaging, release, and non-Windows non-goals. | proposal Out of scope; plan Rejected Alternatives | scoped workflow diff; unchanged `build_win.bat`; forbidden-token assertions | No dependency, feature, prebuilt-object, batch, installer-path, trigger, or other-job behavior changed. | pass |
| Keep unavailable hosted evidence explicit. | proposal Success Criteria; plan Exit Conditions; testplan integration level | manual integration row; F-001; runtime lifecycle coverage | Local success is not substituted for hosted execution; the next hosted rerun remains named and measurable. | pass |

## Evidence Coverage
| Documented Item | Source Document | Implementation Evidence | Test / Result Evidence | Status |
|-----------------|-----------------|-------------------------|------------------------|--------|
| `CHG-provision-windows-nasm` | proposal P-001; plan D-1/I-1/T-1/A-1; risk profile contract/security/build checks | deterministic Program Files validation, explicit missing-file error, and `GITHUB_PATH` publication in `.github/workflows/build.yml` | fresh `20260813T160607Z` artifact exits 0 for compile closure, seven focused tests, local NASM probe, real unchanged Windows build, and versioned installer check; hosted rerun remains manual | implemented |

## Test Design Adequacy
| Behavior / Risk / change_id | Required Case Types | Test Design Evidence | Runnable Test Evidence | Status |
|-----------------------------|---------------------|----------------------|------------------------|--------|
| provisioning and PATH handoff / `CHG-provision-windows-nasm` | normal, boundary, negative, error, ordering | `tests/windows_action_nasm_contract.py` requires the exact pin, Program Files directory and executable derivation, leaf-file guard, explicit error, directory publication, standalone verification, and unchanged consumer order | seven tests pass; mutations reject missing setup, unpinned/wrong versions, wrong package path, the hosted-proven Machine/User discovery strategy, missing `GITHUB_PATH`, and crypto/prebuilt workarounds | adequate |
| native compiler and packaging compatibility / `CHG-provision-windows-nasm` | compatibility, lifecycle, cross-module | testplan compile closure plus unchanged `build_win.bat` and Cargo-versioned installer verification | Cargo all-target no-run passes; local Windows NASM 2.16.01 runs; unchanged batch build succeeds; `dist/BuckyVPN_1.2.0_amd64_Setup.exe` exists | adequate |
| hosted Chocolatey/package-path/job lifecycle / `CHG-provision-windows-nasm` | external dependency, hosted process boundary, produced artifact | integration remains manual and names the exact required observations; two hosted failures falsified earlier implementations | no hosted green exists for the corrected uncommitted workflow; F-001 retains this limitation without treating source or local evidence as platform proof | adequate |

## Implementation Review
| Area | Evidence | Finding | Status |
|------|----------|---------|--------|
| requirement-and-behavior | proposal P-001; workflow `Install NASM` through installer verification; hosted failure history | The delivery implements the approved pin, fail-fast discovery, cross-step PATH publication, unchanged build path, and truthful hosted-evidence boundary. | pass |
| logic-and-control-flow | ordered install block and later job steps | Chocolatey runs first; the package-defined directory and executable are derived; an absent leaf file throws; only a validated directory is published; verification, build, and installer checks remain failure-stopping. | pass |
| boundary-and-input | `$env:ProgramFiles`, literal `NASM` and `nasm.exe`, `Test-Path -LiteralPath -PathType Leaf` | The x64 package's declared Program Files boundary is explicit. Missing environment/path components or a non-file target fail before publication, and no recursive or ambiguous command search is used. | pass |
| state-and-data-integrity | append-only `GITHUB_PATH`; ephemeral hosted runner | The step does not overwrite Machine/User PATH or persistent task data. It appends one validated directory for later steps, and runner-local installation state is discarded with the job. | pass |
| error-handling-and-recovery | Chocolatey exit, missing-file throw, standalone `nasm -v`, batch and installer exits | Installation, discovery, invocation, native compilation, packaging, or artifact verification failures remain nonzero and prevent upload; no fallback masks failure. | pass |
| resource-lifetime-and-cleanup | job-local tool and existing artifact retention | No service, handle, cache policy, or product-runtime resource is added. NASM remains an ephemeral build-time dependency. | pass |
| concurrency-and-ordering | one ordered `build-windows` step list; other jobs unchanged | `GITHUB_PATH` is written before the separate pwsh and cmd consumers; no parallel reader or cross-job mutable state is introduced. | pass |
| interface-and-compatibility | GitHub `GITHUB_PATH`; unchanged `shell: cmd` and `run: build_win.bat` | Later shells continue consuming the ordinary `nasm` command contract; the batch entrypoint and installer filename contract are unchanged. | pass |
| security-and-capacity | exact Chocolatey version; literal local path; forbidden workaround checks | The correction adds no credential, direct mutable download, recursive filesystem scan, or prebuilt crypto object. Work and PATH growth remain bounded to one install and one directory entry. | pass |
| test-adequacy | inspected seven mutation tests and fresh five-command task artifact | Tests expose both hosted-proven wrong strategies and preserve build/package consumers. Only a pushed hosted run can close F-001, and the report keeps that boundary explicit. | pass |

## Document Consistency
| Document | Source | Implementation Consistency | Finding | Status |
|----------|--------|----------------------------|---------|--------|
| design | `pipeline/plan.md` | D-1 now binds the package-defined Program Files path, leaf-file validation, directory publication, unchanged consumers, and hosted evidence boundary implemented by the workflow. | The design no longer contains the disproved Machine/User PATH assumption and directly incorporates hosted run 31716578412. | pass |
| testing | `testplan.yaml` | Unit mutations, local compatibility checks, and manual hosted integration correspond to the corrected workflow and current artifact. | The plan names both earlier hosted failures and requires a future hosted rerun without overstating local evidence. | pass |

## Consistency Summary
- Proposal authority check: the launch-confirmed proposal remains unchanged and still governs the exact pinned-tool outcome and non-goals.
- Proposal vs design: P-001 maps to `CHG-provision-windows-nasm`, deterministic package-path discovery, and unchanged consumers.
- Design vs implementation: path, failure flow, directory publication, ordering, and non-goals agree.
- Design vs testing: exact source contract and local native compatibility are automated; hosted runner lifecycle remains manual.
- Test implementation vs results: the current workflow, test, and testplan are inputs to successful artifact `20260813T160607Z`.
- Bugfix red-green evidence: hosted job 94502776485 is red for the removed PATH-refresh strategy; focused mutations reject reintroducing that strategy, and the corrected current source contract is green locally.
- Implementation correctness audit completeness: all ten required defect-discovery categories were inspected before selecting the conclusion.

## Validation Evidence
- Acceptance entry eligibility: `python3 harness/scripts/lifecycle-check.py --task docs/versions/v0.1/modules/bucky-vpn/019-install-nasm-for-windows-action/task.yaml --require-prior acceptance` passed; this is workflow eligibility only.
- Fresh task artifact: `.harness/test-results/test-runs/20260813T160607Z-bucky-vpn+019-install-nasm-for-windows-action-all.json`, overall exit code 0, five successful commands, one change ID, and current workflow/test/testplan inputs.
- Focused contracts: seven tests passed, including wrong Program Files location and forbidden Machine/User PATH discovery regressions.
- Local Windows evidence: NASM 2.16.01, unchanged `build_win.bat`, Inno Setup 6.5.4, and Cargo-versioned installer verification all succeeded. This is compatibility evidence, not exact hosted-package proof.
- Hosted evidence: run 31716578412 / job 94502776485 installed NASM 2.16.3 but failed the superseded lookup. No hosted run has executed the corrected uncommitted workflow.
- Quality gates: not run because neither the user nor a matching custom rule requested them.

## Automated Test Exception
- Applies: no.
- Reason: fresh task-scoped automation covers every locally executable contract and local Windows packaging handoff.
- Owner: not applicable because no automated-test exception is used.
- Risk: exact hosted Chocolatey installation and GitHub runner step handoff await the next pushed workflow run.
- Acceptance impact: non-blocking under the proposal's explicit hosted-unavailable clause, but the platform must not be called green until F-001 is closed externally.
- Alternative evidence: two hosted red reproductions, structured mutation tests, affected Rust compile closure, and a real local Windows native/package build.

## Conclusion
- Accepted / rejected / needs changes: accepted
- Reason: the corrected workflow replaces the hosted-disproved environment PATH assumption with the Chocolatey package's deterministic Program Files location, validates the executable before publication, preserves all approved boundaries, and has fresh proportional regression and native packaging evidence.
- Supporting task-relevant test evidence: `.harness/test-results/test-runs/20260813T160607Z-bucky-vpn+019-install-nasm-for-windows-action-all.json`, exit code 0, five successful commands covering `CHG-provision-windows-nasm`.
- Residual risk: a new hosted `windows-2022` run has not executed the corrected workflow; exact NASM 2.16.3 output, `aws-lc-sys` compilation, and installer verification remain the explicit F-001 manual gate.

## Follow-Up Tasks
- Requirement task: none.
- User decision required for proposal issue: none.
- Design task: none; D-1 incorporated the hosted-proven package-path contract.
- Implementation task: none.
- Testing task: commit/push and observe the next hosted `build-windows` job; no additional local test is required before handoff.
- Return routing: none; no blocking design, implementation, or testing finding remains.
- Iteration count: 2 for `HOSTED-NASM-PATH-001`; the second return corrected the disproved PATH-refresh design.
- Stop reason if more than 5 unsuccessful iterations: not applicable.
