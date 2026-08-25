# Restore PN Reassignment After SN Restart Acceptance Report

## Findings
| ID | Severity | Kind | Evidence | Problem | Blocking |
|----|----------|------|----------|---------|----------|
| F-001 | low | testing-consistency | `testplan.yaml` integration level; pipeline state cross-module cases | A live multi-process run with SN restart, an independent PN, and observable P2P `connect_server` activity was not available. The deterministic unit and source-contract coverage proves both client-side decisions, but does not measure deployed recovery time or traffic continuity. | no |

No blocking requirement, implementation, design-consistency, or testing-consistency finding was identified.

## Requirement Review
| Requirement or Boundary | Source | Implementation Evidence | Finding | Status |
|-------------------------|--------|-------------------------|---------|--------|
| A non-empty response remains authoritative when restart-local server and PN versions collide with cached values. | `proposal.md` P-001 | `is_unchanged_vpn_info_response` requires `vpn_infos_empty`; `run_proc` applies every other response through PN cache, tunnel factory, routes, and device reconciliation. | Equal numeric versions no longer discard reconstructed PN assignment content. | pass |
| An empty equal-version incremental response remains a no-op. | `proposal.md` P-001 and Risks | The unchanged predicate requires both equal versions and an empty payload before returning early. | The fix does not reinterpret the protocol's empty unchanged response as an empty network set. | pass |
| Response application failures remain retryable and do not commit versions. | `proposal.md` Scope and P-001 | `on_vpn_info_received(...).await?` and `reconcile_result?` precede both version stores and the final `is_first=false` store. | Existing task 011 commit-after-success behavior remains intact for the newly admitted equal-version payload. | pass |
| Removing a no-longer-desired PN also removes its logical connected entry. | `proposal.md` P-002 | `take_removed_pn_targets` removes stale keys from the taken map before external target removal, and the reduced map is restored after synchronization. | Registry membership no longer falsely represents a removed PN as connected. | pass |
| Identical PN reappearance reaches a fresh connection attempt without restarting vpn-client. | `proposal.md` P-002 | After the prior poll stores a map without the PN, the desired loop sees `!connected.contains_key(&pn_server)` and invokes `connect_pn_server_targets`, which calls TTP `connect_server`. | Same identity and endpoint metadata no longer suppress reconnect. | pass |
| Wire, public API, persistence, heartbeat, and selection policy remain unchanged. | `proposal.md` Out of scope | Changes are private helpers and internal branch/state handling in `vpn-frame` and `bucky-vpn`; both packages compile across all targets. | The correction is backward-compatible and stays inside the approved client boundaries. | pass |

## Implementation Review
| Area | Evidence | Finding | Status |
|------|----------|---------|--------|
| Response fast path | `vpn-frame/src/client/vpn_client.rs:162-175,270-279` | The predicate covers first-sync, both version comparisons, and response emptiness; only the exact unchanged empty case exits early. | pass |
| Response transaction | `vpn-frame/src/client/vpn_client.rs:280-361` | PN connection and device application errors return before applied versions and first-sync state are committed. | pass |
| PN registry removal | `vpn-client/src/p2p_vpn.rs:197-217,318-331` | Stale keys are collected before mutation, removed without holding the mutex across async work, and returned with their targets for external cleanup. | pass |
| PN reappearance | `vpn-client/src/p2p_vpn.rs:333-340` | A removed key is absent on the next desired response, so the existing connection path runs before the new key is recorded. A connection error exits before recording false connected state. | pass |
| Failure and concurrency boundaries | `vpn-client/src/p2p_vpn.rs:309-343` | The standard mutex is held only for take/restore operations; external removal and async connection work happen without the lock. Removal errors remain logged, while logical absence preserves reappearance eligibility. | pass |
| Scope discipline | production diff and task bindings | No server, schema, heartbeat, endpoint ordering, authorization, UI, or public signature change was introduced. Existing task 011 changes and unrelated dirty-worktree files were preserved. | pass |

## Testing Review
| Evidence | Result | Finding | Status |
|----------|--------|---------|--------|
| `vpn-frame/src/client/vpn_client_restart_tests.rs` | Five selected vpn_client tests passed; three are the new restart predicate regressions and two retain PN cache behavior. | Coverage includes empty/non-empty response content, zero and `u16::MAX` equal-version boundaries, first sync, independent version mismatches, and an explicit legacy-predicate red model. | pass |
| `vpn-frame/tests/tun_recovery_contract.rs` | Six focused transaction and recovery contract tests passed. | The newly admitted response still reaches task 011's map restoration, error propagation, version commit, and first-sync ordering guarantees. | pass |
| `vpn-client/src/p2p_vpn_pn_registry_tests.rs` | Seventeen selected p2p_vpn tests passed; four are the new PN registry regressions. | Coverage proves stale removal, desired retention, identical reappearance eligibility, the legacy stale-key failure model, and removal ordering before the fallible TTP cleanup call. | pass |
| `.harness/test-results/test-runs/20260805T091355Z-globals+012-restore-pn-reassignment-after-sn-restart-all.json` | All five task steps exited 0: three focused test commands plus `cargo check --all-targets` for vpn-frame and bucky-vpn. | The task-scoped artifact binds both change IDs to the current testplan and affected source inputs. | pass |
| `testplan.yaml` integration boundary | Live SN/independent-PN restart recovery remains manual. | This is retained as F-001 rather than represented as an automated integration pass. | pass |

## Document Consistency
| Document | Source | Implementation Consistency | Finding | Status |
|----------|--------|----------------------------|---------|--------|
| proposal | `proposal.md` | Both P-001 and P-002 are implemented at their named client ownership boundaries with their non-goals preserved. | No requirement ambiguity or implemented scope drift remains. | pass |
| design | `pipeline/plan.md` | Response emptiness participates in the unchanged decision; stale registry keys are removed; retry and mutex/await ordering match the state and failure-flow mappings. | No design-to-implementation conflict was found. | pass |
| testing | `testplan.yaml` and pipeline state | Unit, DV, red-green model, compatibility, lifecycle, and manual cross-module rows match the successful task artifact and documented live gap. | No stale or overstated automated evidence was found. | pass |

## Result Summary
- Overall result: accepted
- Outcome: an already-running client now consumes a non-empty PN refresh despite restart-local version collision, and an identical PN removed from the P2P registry is eligible for a fresh connection attempt when it reappears.
- Blocking issues: none.
- Residual risk: live SN restart plus independent-PN reassignment was not executed in a deployed multi-process environment, so operational recovery latency and traffic behavior remain manually verifiable.
- Next action: complete the task's Harness lifecycle and remove 012 from the unfinished-task index; run the documented live SN/PN scenario when those services are available.

## Object and Scope
- Task manifest: task.yaml
- Reviewed changes: `CHG-apply-equal-version-pn-refresh`, `CHG-reconnect-reappeared-pn`
- In scope: launch-confirmed proposal, automatic pipeline plan, two client production paths, dedicated regression tests, testplan, task-scoped run artifact, and task 011 commit-after-success compatibility.
- Out of scope: unrelated dirty-worktree changes, server persistence/version redesign, PN authorization/selection/heartbeat changes, push delivery, privileged service deployment, and Flutter UI.

## Conclusion
- Accepted / rejected / needs changes: accepted
- Reason: The delivered internal changes close both client-side causes of post-SN-restart PN loss, preserve the empty incremental fast path and retry transaction, and pass focused regressions plus all-target compilation for both affected packages.
- Residual risk: the live SN/PN multi-process recovery scenario remains a documented low-severity manual evidence gap.
