# PN Transport Modes Acceptance Report

## Findings
| ID | Severity | Kind | Evidence | Problem | Blocking |
|----|----------|------|----------|---------|----------|
| F-001-closed | none | testing-consistency | six async factory tests at `vpn-server/tests/unit/pn_control_client_tests.rs:245-434`; private seam and production adapter at `vpn-server/src/pn_control_client.rs:61-150`; successful artifact `20260825T092449Z` | The first acceptance run lacked executable evidence for fallback, cleanup, classified routing, route replacement, and serialization. The new tests enter the same `ControlCmdTunnelFactory::create_tunnel` path as production and now assert each transition and resulting maintained-target state; the prior blocker is closed. | no |
| G-001 | low | testing-consistency | `testplan.yaml` disabled integration level and runtime-state manual cross-module row | A deployed authenticated PN/SN replay with OS TCP/UDP socket observation and live heartbeat inspection was not available. This remains operational confirmation; source-level listener wiring, payload tests, deterministic factory tests, and affected-target compilation cover the task's deterministic boundaries. | no |

## Result Summary
- Overall result: accepted
- Outcome: Standalone PN now supports exactly `tcp`, `quic`, and `dual`; the selected mode consistently drives listener identity, control candidates, published endpoints, report primary, and mapped ports, while dual performs QUIC-first TCP fallback and preserves one maintained control target.
- What was verified: strict parsing and compatible default; standalone TCP-only, QUIC-only, and dual endpoint/report construction; combined-mode rejection; PN-disabled SN-only preservation; real factory-path connect/open fallback; failed-target and all-fail cleanup; exact classified routing and rejection; successful route replacement; serialized concurrent creation; private production adapter semantics; one control-client pool slot and one heartbeat task creation site.
- Evidence used: approved `proposal.md`, automatic `pipeline/plan.md`, `risk-profile.yaml`, current `testplan.yaml`, production/test/config sources, runtime state, and `.harness/test-results/test-runs/20260825T092449Z-bucky-vpn-server+031-pn-tcp-only-transport-all.json`.
- Blocking issues: none; F-001 is closed.
- Next action: complete pipeline bookkeeping; optionally perform a deployed TCP/QUIC socket and authenticated fallback replay during normal release validation.

## Object and Scope
- Task manifest: `task.yaml`
- Module: `bucky-vpn-server`
- Version: `v0.1`
- Task name: `031-pn-tcp-only-transport`
- change_id values reviewed: `CHG-enable-standalone-pn-transport-modes`
- Review date: 2026-08-25
- In scope: `pn.transport` configuration; standalone identity/listener/control/report endpoints; report primary and mapped ports; combined SN+PN validation; SN-only compatibility; dual fallback, classified recreation, target cleanup and serialization; the private testability seam; documentation; tests and task evidence.
- Out of scope: splitting combined SN and PN listeners, changing shared wire codecs or client endpoint selection, unrelated dirty-worktree files, and deployment/release mutation.

## Requirement Review
| Requirement or Boundary | Source | Implementation Evidence | Finding | Status |
|-------------------------|--------|-------------------------|---------|--------|
| `CHG-enable-standalone-pn-transport-modes` accepts exactly `tcp`, `quic`, and `dual`; omission remains `dual`. | `proposal.md` P-001 and success criteria | `PnTransportMode` and `FromStr` at `vpn-server/src/server_config.rs:52-103`; configured default; parsing tests in `pn_transport_mode_tests.rs:36-108` | Exact values, clear errors, and compatible default match the approved requirement. | pass |
| Standalone mode drives matching local identity/listeners, control candidates, published endpoints, primary, and port mapping. | `proposal.md` scope and success criteria | `PnTransportMode::endpoints` and `filter_port_mapping`; `resolve_service_endpoints`; startup/report wiring at `vpn-server/src/main.rs:295-368`; payload test at `pn_transport_mode_tests.rs:110-188` | One mode-derived endpoint set is supplied to identity and listener creation; the first enabled endpoint is primary and disabled-protocol mappings are removed. | pass |
| Combined SN+PN rejects non-dual; PN-disabled SN-only retains the existing dual endpoint set. | `proposal.md` compatibility boundary | `validate_server_mode` and `resolve_service_endpoints` at `server_config.rs:362-413`; tests at `pn_transport_mode_tests.rs:190-215` | PN-disabled returns before transport restriction, combined non-dual fails before startup, and SN-only remains QUIC plus TCP. | pass |
| Dual performs QUIC-first TCP fallback, exact classified recreation, failure cleanup, route transfer, and one effective control/heartbeat chain. | `proposal.md` dual requirements and `pipeline/plan.md` failure flows | factory at `pn_control_client.rs:152-312`; one pool slot at `329`; single heartbeat startup at `main.rs:522-529`; async tests at `pn_control_client_tests.rs:245-434` | Candidate order and both connect/open failure paths are executed; connected failures are removed; invalid classifications perform no I/O; route switch removes the old target; concurrent creation has maximum one in-flight operation and one maintained target. | pass |
| Existing shared wire/client behavior, authentication, dependencies, and release output remain unchanged. | `proposal.md` non-goals | changed production scope remains inside `bucky-vpn-server`; consumer closure, all-target check, no-run compile, and `vpn-frame` docs pass | The new seam is private and no shared codec, client, dependency, or release interface changed. | pass |

## Implementation Review
| Area | Evidence | Finding | Status |
|------|----------|---------|--------|
| requirement-and-behavior | proposal P-001; current mode, startup, reporting and factory paths; thirteen focused tests | Every approved behavior and boundary is represented in source and executable evidence; no contradictory or expanded requirement was found. | pass |
| logic-and-control-flow | `server_config.rs:362-413`; `pn_control_client.rs:247-312`; `main.rs:312-334` | Standalone filtering is conditional, unclassified creation preserves configured order and stops at first complete stream, and classified creation selects only the exact configured endpoint. | pass |
| boundary-and-input | strict parser and validation; invalid-value, SN-only and combined tests | Unknown, blank, case-variant and non-string values fail; combined non-dual and invalid standalone control configuration fail before runtime listener startup. | pass |
| state-and-data-integrity | `active_target`, `remove_target`, and `commit_target` at `pn_control_client.rs:102-104` and `206-245`; route-switch/all-fail tests | Failed connected candidates leave no maintained target; successful replacement commits the new target and removes the prior different route; same route is deduplicated by remote identity and exact endpoint. | pass |
| error-handling-and-recovery | connect-failure, open-failure, all-fail and classified-rejection tests at `pn_control_client_tests.rs:245-362` | QUIC connect failure proceeds directly to TCP; QUIC stream failure removes QUIC before TCP; both stream failures remove both targets; invalid classified requests cause no connector operation. | pass |
| resource-lifetime-and-cleanup | production adapter direct delegation; `remove_target` and `commit_target`; fake maintenance-state assertions | The seam preserves TTP ownership and return types. Cleanup is called only after successful target registration/open failure or after a different route succeeds; stream handles remain owned by the returned command tunnel. | pass |
| concurrency-and-ordering | `create_lock` at `pn_control_client.rs:103` and `307`; concurrent test at `pn_control_client_tests.rs:402-434`; pool maximum one at `pn_control_client.rs:329` | Four concurrent factory requests produce maximum one in-flight open and one maintained target. The async lock is held across creation, while the synchronous target-state mutex is held only in non-await sections; no new lock cycle was found. | pass |
| interface-and-compatibility | private `ControlTtpClientOps`; `DefaultControlTtpClientOps`; private `new_for_test`; consumer closure and compile artifact | Production construction still accepts `TtpClientRef`; the adapter delegates the same three operations with unchanged parameters, errors, and await boundaries. Trait-object dispatch is private and introduces no exported contract. | pass |
| security-and-capacity | authenticated TTP target identity; two-candidate bound; one-slot pool; serialized factory | No authorization or codec boundary changed. Work remains bounded to at most two configured candidates, and the new trait does not expose the TTP client beyond the module. | pass |
| test-adequacy | thirteen focused tests; factory fake at `pn_control_client_tests.rs:50-224`; successful artifact `20260825T092449Z`; runtime case coverage | Normal, boundary, negative, error, lifecycle, concurrency and compatibility paths are executable. Static cross-module payload/compile evidence is adequate; only a real deployed topology and socket observation remain manual. | pass |

## Document Consistency
| Document | Source | Implementation Consistency | Finding | Status |
|----------|--------|----------------------------|---------|--------|
| design | `pipeline/plan.md` | Current source preserves mode derivation, standalone ownership, ordered fallback, exact classification, failure cleanup, target transfer and one-client construction. | The private adapter is the minimal testability refinement and does not change any planned runtime boundary or public interface. | pass |
| testing | `testplan.yaml` | The revised plan names the six async factory behaviors, thirteen focused tests, remaining live integration boundary, and exact artifact command. | Testplan, test code, runtime evidence and successful result are consistent; the former F-001 gap is now automated. | pass |

## Validation Evidence
- Acceptance entry eligibility: `lifecycle-check.py --require-prior acceptance` passed on 2026-08-25; this is workflow eligibility, not correctness proof.
- Pipeline structure: `pipeline-plan-check.py pipeline/plan.md` passed on 2026-08-25; this is structural evidence only.
- Unified task run: `.harness/test-results/test-runs/20260825T092449Z-bucky-vpn-server+031-pn-tcp-only-transport-all.json`, exit code 0.
- Artifact steps: affected all-target check, removed-symbol consumer closure, all-target no-run compile, `vpn-frame` doc tests, and focused `pn_transport_` tests all exited 0.
- Focused coverage: seven configuration/resource tests plus six async factory tests, for thirteen focused tests total.
- F-001 closure evidence: connect failure fallback; stream-open failure removal then fallback; all-fail removal and empty active state; exact classified success plus mismatch/unconfigured rejection without operations; successful QUIC-to-TCP route replacement with old removal; four concurrent calls with maximum in-flight open equal to one and one maintained endpoint.
- Production seam evidence: `ControlCmdTunnelFactory::new` wraps the original `TtpClientRef` in `DefaultControlTtpClientOps`; each adapter method is a direct call to the corresponding TTP method; `new_for_test` differs only in supplying the trait object to the common constructor.
- Patch hygiene: task-owned tracked diffs passed `git diff --check`; unrelated dirty-worktree content was excluded from the acceptance judgment.

## Conclusion
- Accepted / rejected / needs changes: accepted
- Reason: F-001 is closed by deterministic tests that execute the production factory control flow through every previously missing fallback, cleanup, classification, route-transfer and concurrency transition; the private adapter preserves production semantics, and no blocking finding remains across the required defect categories.
- Supporting task-relevant test evidence: the successful `20260825T092449Z` task artifact contains all enabled testplan commands and thirteen focused assertions, including the six new async factory tests.
- Residual risk: an authenticated deployed PN/SN fallback replay with OS-level TCP/UDP socket observation and live heartbeat inspection remains useful operational confirmation, but it does not invalidate the deterministic factory, startup wiring, payload and compile evidence.

## Follow-Up Tasks
- Requirement task: none.
- User decision required for proposal issue: no.
- Design task: none.
- Implementation task: none; the private seam preserves the existing production adapter contract.
- Testing task: optional deployed TCP-only, QUIC-only and dual fallback/socket replay during normal release validation.
- Testing return reason if coverage is incomplete: none; F-001 is closed.
- Iteration count: two F-001 return records, successfully closed in this acceptance run.
- Stop reason if more than 5 unsuccessful iterations: not applicable.
