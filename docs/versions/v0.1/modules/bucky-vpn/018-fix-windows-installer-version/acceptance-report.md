# Windows Installer Version and Cross-Platform Packaging Acceptance Report

## Findings
| ID | Severity | Kind | Evidence | Problem | Blocking |
|----|----------|------|----------|---------|----------|
| F-000 | none | implementation | Approved `proposal.md`; current `pipeline/plan.md`, scripts, metadata and tests; fresh successful artifact `.harness/test-results/test-runs/20260813T035450Z-bucky-vpn+018-fix-windows-installer-version-all.json`; direct Windows and Debian artifact probes | Second-round independent falsification found no remaining requirement, design, implementation, or testing defect. First-round F-001 and F-002 are closed by a current-plan run and executable Windows native assertions. | no |

## Result Summary
- Overall result: accepted
- Outcome: `build_win.bat` now derives version `1.0.0` from `vpn-client/Cargo.toml`, produces the matching Inno installer, and fails with explicit messages for absent or empty `AppVersion`; Windows executable metadata and the real Debian package also match the Cargo version.
- What was verified: cmd/PowerShell/Cargo extraction, winres inheritance, ISCC defined/undefined/empty behavior, installer output, Debian metadata and modes, four-platform script contracts, cleanup, error propagation, and test/artifact provenance.
- Evidence used: approved proposal, plan and risk profile, current production/test sources, fresh six-step task artifact, executable Windows negative/resource checks, and Debian package inspection.
- Blocking issues: none.
- Next action: complete task acceptance and close the task packet; retain native macOS package and Flutter/Docker server execution as the explicitly recorded hosted-environment gap.

## Object and Scope
- Task manifest: `task.yaml`
- Module: bucky-vpn.
- Version: v0.1 task packet; product version `1.0.0` from `vpn-client/Cargo.toml`.
- Task name: `018-fix-windows-installer-version`.
- change_id values reviewed: `CHG-fix-windows-installer-version`, `CHG-audit-cross-platform-version-packaging`.
- Review date: 2026-08-13.
- In scope: Windows cmd/PowerShell/Cargo/winres/ISPP version chain; Debian package fixes exposed by native packaging; macOS/server static and fake-tool contracts; tests, testplan and runtime evidence.
- Out of scope: product version changes, installer runtime redesign, GitHub publication behavior, signing/notarization, live macOS packaging, live Flutter/Docker packaging, and unrelated worktree changes.
- Task-relevant acceptance scope: independently falsify the single-version packaging contract and confirm that the regressions are durably tested.
- Out-of-scope checks not run: installer installation/uninstallation, native macOS `lipo`/`pkgbuild`, real server Flutter/Docker build, Release/GHCR mutation, and broad VPN runtime suites.

## Requirement Review
| Requirement or Boundary | Source | Implementation Evidence | Finding | Status |
|-------------------------|--------|-------------------------|---------|--------|
| `CHG-fix-windows-installer-version`: Cargo version must reach PowerShell, winres and ISCC without caret damage or a second literal, and undefined/empty input must fail closed. | proposal P-001 and Success Criteria | `build_win.bat`, `install.iss`, `vpn-client/Cargo.toml`; real `BuckyVPN_1.0.0_amd64_Setup.exe`; native verifier | The valid batch path succeeds; both invalid ISPP inputs fail with the intended messages; EXE FileVersion/ProductVersion both equal Cargo `1.0.0`. | pass |
| `CHG-audit-cross-platform-version-packaging`: audit all four entrypoints, repair only confirmed defects, run real Windows/Debian packaging, and state native macOS/server gaps accurately. | proposal P-002 and Success Criteria | `build_deb.sh`, `vpn_deb/DEBIAN/control`, other build scripts, fake-tool contracts, real `.deb` and Windows installer | Debian permission normalization and Maintainer addition are confirmed real-build repairs; all four contracts pass and no native macOS/server success is claimed. | pass |

## Evidence Coverage
| Documented Item | Source Document | Implementation Evidence | Test / Result Evidence | Status |
|-----------------|-----------------|-------------------------|------------------------|--------|
| Windows version repair | proposal P-001; plan I-1 | normal PowerShell Cargo invocation, quoted `/DAppVersion`, explicit ISPP guards, removed winres version literals | real Windows build and native guard/resource verifier pass in the fresh artifact | implemented |
| Cross-platform audit and Debian repair | proposal P-002; plan I-audit | Cargo-owned POSIX version paths; Debian staged mode normalization and Maintainer metadata | 15 focused contracts, real Debian build, artifact verifier and package inspection pass | implemented |
| Current testing provenance | `testplan.yaml`; plan T-1 | compile closure and unit/DV steps bind both change IDs as declared | fresh artifact records the current bindings and all six steps exit 0 | implemented |

## Test Design Adequacy
| Behavior / Risk / change_id | Required Case Types | Test Design Evidence | Runnable Test Evidence | Status |
|-----------------------------|---------------------|----------------------|------------------------|--------|
| Windows repair / `CHG-fix-windows-installer-version` | normal, boundary, negative, error, compatibility, lifecycle, cross-module | source contracts, real batch build, direct undefined/empty ISPP checks, exact EXE resource comparison, all-target compile closure | all applicable steps exit 0 in `20260813T035450Z`; direct rerun of the native verifier also passes | adequate |
| Packaging audit / `CHG-audit-cross-platform-version-packaging` | normal, boundary, negative, error, compatibility, lifecycle, cross-module | fake-tool executions for all entrypoints, real Debian and Windows builds, output metadata verifier, explicit native integration gap | current compile/unit/DV steps pass; macOS/server native execution remains manual with a concrete environment reason | adequate |

## Implementation Review
| Area | Evidence | Finding | Status |
|------|----------|---------|--------|
| requirement-and-behavior | both proposal rows mapped to current scripts, Cargo metadata, native artifacts and tests | The reported Windows failure is eliminated, all produced local metadata matches Cargo, and only confirmed Debian defects were repaired. | pass |
| logic-and-control-flow | batch `for /f`, PowerShell `$LASTEXITCODE`, unique-package/version validation, `errorlevel`; strict POSIX scripts | Cargo or compiler failures stop packaging; only one stable numeric version reaches downstream tools; no unintended fallthrough found. | pass |
| boundary-and-input | package-count checks, version regex, quoted ISCC argument, executable undefined/empty probes | Missing, empty, ambiguous and prerelease inputs fail before a misleading artifact; valid `1.0.0` reaches every local native boundary. | pass |
| state-and-data-integrity | temporary Debian/macOS roots, staged metadata updates, expected output checks | Repository package templates are not modified during a build, output names are version-bound, and no persistent application state is touched. | pass |
| error-handling-and-recovery | Bash strict mode/traps, batch error propagation, ISCC/tool/output checks | Errors remain nonzero and block completion. The previously observed transient Windows resource error was visible rather than masked, and the fresh full run succeeded. | pass |
| resource-lifetime-and-cleanup | `mktemp` plus EXIT traps; `setlocal`; isolated fake-tool fixtures | Temporary staging is cleaned on success/failure; batch variables are process-local; produced installers are intentional retained outputs. | pass |
| concurrency-and-ordering | sequential entrypoints, per-run temporary roots, no shared staged template | No locks, tasks or background workers are introduced. Concurrent POSIX runs isolate staging; replacement of named `dist` outputs is intentional build behavior. | pass |
| interface-and-compatibility | unchanged entrypoint names/output patterns; Cargo metadata interface; Inno public define; Debian schema | Existing callers remain compatible, winres now inherits `1.0.0`, Debian fields/modes are valid, and macOS/server native compatibility is accurately left for hosted execution. | pass |
| security-and-capacity | numeric-only version validation, quoted compiler argument, temporary bounded package trees | No credential or untrusted runtime boundary changes; version text cannot become a shell command, and packaging work remains bounded by repository inputs. | pass |
| test-adequacy | inspected all 15 unit tests, four DV steps, compile closure, artifact timestamps/provenance and direct native reruns | First-round stale-artifact and weak-native-assertion findings are closed. Tests now expose the reported shell defect, invalid ISPP branches, resource drift, Debian metadata/mode defects and cross-platform contract regressions. | pass |

## Document Consistency
| Document | Source | Implementation Consistency | Finding | Status |
|----------|--------|----------------------------|---------|--------|
| design | `pipeline/plan.md` | Both implementation bindings, failure flows, version ownership and native/manual evidence boundaries match the delivered files. | No design/implementation contradiction or unimplemented mapped outcome remains. | pass |
| testing | `testplan.yaml` | Current test source implements every declared command; the fresh artifact records the current two-ID compile closure and all unit/DV steps. | First-round F-001/F-002 are closed; no stale or missing runnable evidence remains. | pass |

## Consistency Summary
- Proposal authority check: the approved proposal remains the baseline; P-002 specifically permits repair of defects confirmed during the audit.
- Proposal vs design: both change IDs, the single version owner, real Windows/Debian evidence, and truthful native gaps are preserved.
- Design vs testing implementation: every declared automated step exists and the macOS/server integration level gives a concrete manual reason.
- Design vs implementation: current Windows and Debian behavior follows the plan; no return is required.
- Test implementation vs test code vs results: current sources predate the fresh successful artifact, whose provenance records all current commands and change-ID bindings.
- Test design adequacy: adequate for locally executable risks; native macOS/server execution remains an approved, explicit residual gap.
- change_id traceability: complete for both IDs across proposal, plan, testplan, runtime state, fresh artifact and this report.
- Cross-module admission: Cargo, cmd/PowerShell, winres, ISPP, dpkg and fake macOS/server handoffs were inspected.
- Implementation correctness audit completeness and routing: exactly ten required defect-discovery categories were independently reviewed and all pass.
- Bugfix red-green regression evidence: the test rejects the old caret-bearing PowerShell form; the real batch succeeds; invalid ISPP branches and Windows resources are now executed and asserted.

## Validation Evidence
- Existing lifecycle entry check: `lifecycle-check.py --require-prior acceptance` passed before the second review; it is eligibility evidence only.
- Task-relevant test artifact: `.harness/test-results/test-runs/20260813T035450Z-bucky-vpn+018-fix-windows-installer-version-all.json`, exit code 0, six steps, current testplan and both change IDs.
- Windows direct verification: `python3 ./tests/github_actions_build_contract.py --verify-windows-native-behavior` passes, proving both custom ISPP failures and EXE FileVersion/ProductVersion `1.0.0`.
- Native artifact verification: `python3 ./tests/github_actions_build_contract.py --verify-native-artifacts` passes.
- Debian artifact inspection: package `bucky-vpn`, Version `1.0.0`, Architecture `amd64`, required Maintainer; directories 0755, service 0644 and binary 0755.
- Existing coverage/checker evidence: parent reports testing-coverage, pipeline-plan and prior-acceptance lifecycle checks passed on current inputs; acceptance independently inspected their primary task sources rather than treating checker success as correctness proof.
- Native environment gaps: macOS `lipo`/`pkgbuild` and server Flutter/Docker were not executed; isolated orchestration/version contracts passed and the gap is not represented as native success.
- Quality gates: not run because the user did not request a repository-wide quality gate.
- Acceptance report check: passed after this report was written; this validates report structure only.

## Automated Test Exception
- Applies: no
- Reason: a fresh successful automated task run covers every locally executable contract and both change IDs.
- Owner: not applicable because no automated-test exception is used.
- Risk: native macOS and server toolchains remain environment-dependent.
- Acceptance impact: non-blocking under the approved explicit manual integration boundary.
- Alternative evidence: isolated fake-tool contracts validate macOS/server version propagation and cleanup without claiming native artifacts.

## Conclusion
- Accepted / rejected / needs changes: accepted
- Reason: the fresh current-plan run proves the fixed Windows shell/ISPP/winres chain, real Debian packaging and all focused contracts; both first-round findings are closed and no blocking defect remains.
- Supporting task-relevant test evidence: `.harness/test-results/test-runs/20260813T035450Z-bucky-vpn+018-fix-windows-installer-version-all.json`, exit code 0, six successful steps covering both change IDs.
- Residual risk: native macOS universal package creation and real Flutter/Docker server image construction remain unexecuted until their supported hosted environments run; pre-existing `nginx:latest` reproducibility is unchanged.

## Follow-Up Tasks
- Requirement task: none.
- User decision required for proposal issue: none.
- Design task: none.
- Implementation task: none.
- Testing task: none; native macOS/server checks remain the approved hosted-environment follow-up.
- Testing return reason if coverage is incomplete: none.
- Iteration count: 2; first-round F-001/F-002 were returned to and closed by testing.
- Stop reason if more than 5 unsuccessful iterations: not applicable.
