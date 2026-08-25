# PN Re-online Observation Refresh Acceptance Report

## Findings
| ID | Severity | Kind | Evidence | Problem | Blocking |
|----|----------|------|----------|---------|----------|
| F-001/F-002-closed | none | implementation | bounded exact-tunnel lookup in `vpn-server/src/pn_control_server.rs`; two concrete-observer regressions; task artifact `20260818T031441Z` | The prior command-before-registration race and missing regression coverage are closed. No open implementation finding remains. | no |
| G-001 | low | testing-consistency | `testplan.yaml` integration level | A deployed authenticated PN/SN/VPN-client reconnect replay was not available in this workspace; this remains operational confirmation rather than a code blocker. | no |

## Result Summary
- Overall result: accepted
- Outcome: Re-online cmd158 handling now observes the exact carrying tunnel after a bounded registration wait, refreshes the PN public address before heartbeat merge, and leaves heartbeat TTL as the sole online/offline authority.
- What was verified: dependency registration ordering, exact peer and TunnelId matching, bounded retry and permanent fallback, writer-lock lifetime, retained observation semantics, additive interface compatibility, production startup wiring, and task-scoped regression/compile evidence.
- Evidence used: approved `proposal.md`, automatic `pipeline/plan.md`, `testplan.yaml`, current production code, dependency source, focused test output, and the successful unified task artifact.
- Blocking issues: none; returned findings F-001 and F-002 are closed.
- Next action: deploy through the normal release path and optionally replay PN timeout/reconnect with a live SN and VPN client to confirm operations end to end.

## Object and Scope
- Task manifest: `task.yaml`
- Module: `globals`
- Version: `v0.1`
- Task name: `027-refresh-pn-observation-on-reonline`
- change_id values reviewed: `CHG-observe-pn-heartbeat-tunnel`, `CHG-refresh-pn-observation-on-reonline`
- Review date: 2026-08-18
- In scope: cmd158 tunnel context, concrete command-tunnel observation, PN heartbeat/address state, production wiring, compatibility, tests, and task-local evidence.
- Out of scope: unrelated dirty-worktree changes, protocol or configuration changes, traffic accounting, deployment, and mutation of a live PN/SN/client topology.

## Requirement Review
| Requirement or Boundary | Source | Implementation Evidence | Finding | Status |
|-------------------------|--------|-------------------------|---------|--------|
| Forward the exact cmd158 identity and TunnelId through an additive observer contract. | `proposal.md` P-001 | `PnControlTunnelObserver`; cmd158 handler; exact handler regression | The handler forwards the authenticated PN identity and carrying TunnelId without changing command encoding or existing constructor behavior. | pass |
| Re-observe the public address on the first valid re-online cmd158. | `proposal.md` P-002 and success criteria | bounded concrete observer; dependency-order regression; manager merge | The observer tolerates the known transient registration lag, reads only the exact tunnel endpoint, and supplies it before the heartbeat update. | pass |
| Keep heartbeat TTL as the sole PN online/offline authority. | approved non-goal and invariant | no connection event listener; `last_heartbeat`/TTL selection; lifecycle tests | Connection accept/disconnect does not update manager liveness; only cmd158 heartbeat time controls expiry and recovery. | pass |
| Preserve the last valid observation when the exact tunnel cannot be observed. | `pipeline/plan.md` failure flows | bounded `None` fallback; retained-observation manager test | Permanent absence neither synthesizes nor erases an address and does not extend liveness by itself. | pass |

## Implementation Review
| Area | Evidence | Finding | Status |
|------|----------|---------|--------|
| requirement-and-behavior | proposal P-001/P-002; cmd158 call flow; manager tests | The first valid re-online heartbeat can refresh the observed public endpoint while preserving all confirmed boundaries. | pass |
| logic-and-control-flow | observer loop; observation-before-heartbeat ordering | Lookup retries only the exact key, exits immediately on success, and performs manager update after observation completes. | pass |
| boundary-and-input | exact peer/TunnelId; mismatched identity test; unspecified address handling | Authentication identity, tunnel identity, missing metadata, and addressless reports are handled explicitly without arbitrary fallback. | pass |
| state-and-data-integrity | `reported`, `observed`, `current`, `last_heartbeat`, `offline_logged` | Observation and heartbeat liveness remain separate and are merged atomically under the manager mutex. | pass |
| error-handling-and-recovery | 20 by 5ms bounded lookup; `None` fallback; retained prior observation | Transient registration lag recovers; permanent absence completes within a fixed bound and preserves the last valid value. | pass |
| resource-lifetime-and-cleanup | writer guard block; absent disconnect listener; TTL pruning | The writer guard is released after copying `remote()`, no transport event mutates PN state, and retained state stays keyed per PN. | pass |
| concurrency-and-ordering | dependency starts receive loop before peer registration; delayed-availability regression | The concrete race is covered by bounded retry without adding cross-task lock ordering or waiting on manager state. | pass |
| interface-and-compatibility | additive observer and selector defaults; preserved constructors; all-target check | Existing selector implementers, constructors, wire format, configuration, and client structures remain compatible. | pass |
| security-and-capacity | authenticated peer key; exact TunnelId; fixed maximum attempts | No caller-reported IP or arbitrary peer connection is trusted; authenticated heartbeat work is capped at 20 lookups and about 95ms of sleep. | pass |
| test-adequacy | 2 observer, 1 handler, 5 manager regressions; 29 vpn-frame tests; all-target check | Normal, boundary, negative, error, lifecycle, compatibility, and cross-module compile paths have runnable evidence; only live deployment replay is manual. | pass |

## Document Consistency
| Document | Source | Implementation Consistency | Finding | Status |
|----------|--------|----------------------------|---------|--------|
| design | `pipeline/plan.md` | The implementation keeps exact-tunnel ownership, observation-before-merge ordering, bounded failure behavior, TTL-only liveness, and additive interfaces. | Bounded registration waiting refines the concrete observer failure flow while preserving every approved design boundary. | pass |
| testing | `testplan.yaml` | The test plan names the registration-race regression, bounded fallback, manager lifecycle coverage, complete vpn-frame suite, and affected-target compilation. | The successful unified artifact executes every enabled step; the disabled live integration level records its required environment and manual gap. | pass |

## Validation Evidence
- Unified task run: `.harness/test-results/test-runs/20260818T031441Z-globals+027-refresh-pn-observation-on-reonline-all.json`.
- Exact handler regression: 1 passed, forwarding the carrying TunnelId and observer result.
- Concrete observer regressions: 2 passed, covering delayed exact-tunnel availability and bounded permanent absence.
- PN manager lifecycle regressions: 5 passed, covering changed/same address re-online, missing observation, TTL separation, and identity mismatch.
- Compatibility/compile evidence: affected `vpn-frame` and `bucky-vpn-server` all-target check passed; complete `vpn-frame` library suite passed 29 tests.
- Source-order evidence: `sfo-cmd-server` 0.4.0 spawns its receive loop before adding `PeerConnection`; the new delayed-availability regression models and closes this ordering window.
- Patch hygiene: `git diff --check` passed for all task-owned code, tests, task packet, and runtime state.

## Conclusion
- Accepted / rejected / needs changes: accepted
- Reason: the returned registration race now has a bounded exact-tunnel repair and deterministic coverage, all confirmed behavior and compatibility boundaries pass review, and no blocking finding remains.
- Supporting task-relevant test evidence: the successful `20260818T031441Z` unified artifact contains every enabled testplan step and affected-target compilation.
- Residual risk: a deployed PN/SN/VPN-client timeout and changed-NAT reconnect replay remains useful operational confirmation but does not invalidate the deterministic code-path evidence.

## Follow-Up Tasks
- Requirement task: none.
- User decision required: no.
- Design task: none; the implementation remains within the approved automatic design.
- Implementation task: none; F-001 is closed.
- Testing task: optional live deployment replay when an authenticated standalone PN, SN, controlled NAT transition, and VPN client observer are available.
- Iteration count: 1 return, successfully closed.
