# PN Observed Address Recovery Acceptance Report

## Findings
| ID | Severity | Kind | Evidence | Problem | Blocking |
|----|----------|------|----------|---------|----------|
| F-001 | low | testing-consistency | `testplan.yaml`; successful task artifact `.harness/test-results/test-runs/20260817T091635Z-bucky-vpn-server+026-recover-pn-observed-address-all.json` | Deterministic manager and command-listener coverage proves the state transitions, but this workspace did not run a deployed PN/SN/VPN-client replay with cmd158 paused beyond TTL and resumed on the same transport session. | no |

## Result Summary
- Overall result: accepted
- Outcome: an address-suppressed standalone PN no longer permanently loses its SN-observed public address after one heartbeat offline/online cycle; the same authenticated control session can resume cmd158 and become selectable again, while a final disconnect clears only the matching session observation.
- What was verified: independent heartbeat and observation lifetimes, one-shot offline transition logging, mapped QUIC/TCP endpoint reconstruction, final-disconnect cleanup, changed-address reconnect, fast reconnect versus delayed disconnect ordering, client selection filtering, protocol compatibility, and affected-target compilation.
- Evidence used: approved proposal, automatic design plan, risk profile, current manager/control-server implementation, seven focused manager tests, one command-listener test, task-scoped compile evidence, dependency peer-manager source, and red-green regression history.
- Blocking issues: none; the disconnect/reconnect generation race found during the first falsification pass was returned to implementation, fixed, and regression tested before this final review.
- Next action: close the Harness task; optionally run the documented live PN/SN/client replay in a deployed environment as operational confirmation.

## Object and Scope
- Task manifest: `task.yaml`
- Module: `bucky-vpn-server`
- Version: `v0.1`
- Task name: `026-recover-pn-observed-address`
- change_id values reviewed: `CHG-recover-pn-observed-address`
- Review date: 2026-08-17
- In scope: `vpn-server/src/pn_server_manager.rs`, `vpn-server/src/pn_control_server.rs`, focused task tests, cmd158/PN-list compatibility, and task-local Harness evidence.
- Out of scope: unrelated dirty worktree files, SQLite traffic-accounting behavior, deployment mutation, and a live external PN/SN/VPN-client topology.
- Task-relevant acceptance scope: recovery and cleanup of an SN-observed PN address when `report_local_address: false`, including timeout, same-session recovery, final disconnect, and reconnect races.

## Requirement Review
| Requirement or Boundary | Source | Implementation Evidence | Finding | Status |
|-------------------------|--------|-------------------------|---------|--------|
| Preserve the control-session observation across heartbeat TTL and reuse it when cmd158 resumes. | `proposal.md` P-001; `pipeline/plan.md` state ownership | `RemotePnServerState` retains `observed` while heartbeat liveness expires; `report_heartbeat` refreshes `last_heartbeat` and selection reuses the merged endpoint. | Two repeated timeout/recovery cycles restore `47.113.93.155` with mapped QUIC/TCP port `3625` without another accept event. | pass |
| Do not return a stale or addressless PN. | proposal success and security boundaries | `remote_state_is_usable`, `resolve`, and `select` require a live heartbeat and connectable endpoint; final disconnect removes the matching observation. | An addressless report remains unselectable after disconnect until a new connection supplies a new observation. | pass |
| Clear the observation only after the final control connection ends. | proposal non-goal and lifecycle boundary | `CmdServerEventListener`; dependency `PeerManager::remove_peer_connection`; `get_peer_tunnels` recheck | The dependency emits peer disconnect only when its connection list becomes empty, and the listener rechecks absence before cleanup. | pass |
| A delayed old disconnect must not erase a new reconnect observation. | `pipeline/plan.md` reconnect failure flow | command tunnel registration precedes observation; observations receive monotonic IDs; cleanup compares the captured ID | The stale-generation regression keeps the newer `47.113.93.156` observation after cleanup for the older generation. | pass |
| Preserve shared protocol and public API behavior. | proposal compatibility boundary; `risk-profile.yaml` | existing `PnServerSelector`, cmd158 payload, PN-list structures, and crate exports are unchanged; only crate-local methods were added | All affected targets compile with the real command service and vpn-frame consumers. | pass |

## Implementation Review
| Area | Evidence | Finding | Status |
|------|----------|---------|--------|
| requirement-and-behavior | proposal P-001 mapped to manager pruning, heartbeat recovery, command listener, and selection | The delivered behavior addresses the observed failure mode without requiring PN restart or a changed cmd158 payload. | pass |
| logic-and-control-flow | `mark_remote_state_offline_if_needed`, update paths, disconnect path, and selection/resolve flow | Timeout marks liveness offline but retains a session-backed observation; recovery and cleanup take distinct paths and converge on a recomputed current state. | pass |
| boundary-and-input | unspecified `0.0.0.0` reports, mapped ports, unknown peer, observed-only state, expired state | Unspecified reported addresses never become client-visible without an observation; unknown cleanup is a no-op and expired/observed-only cleanup removes state safely. | pass |
| state-and-data-integrity | separate `reported`, `observed`, `observation_id`, `last_heartbeat`, and `offline_logged` fields | Heartbeat and transport evidence have independent lifetimes, current endpoints are recomputed after updates, and stale generations cannot mutate newer state. | pass |
| error-handling-and-recovery | observed-address persistence errors, command tunnel registration errors, timeout/recovery tests | Command registration failure stops the accept path; observation persistence failure is logged and a vanished peer is rechecked so unsupported addresses are not retained. | pass |
| resource-lifetime-and-cleanup | final-peer callback, weak service reference, state removal branches | The listener does not retain the command service strongly; final disconnect clears observation-backed state while multiple active tunnels keep it alive. | pass |
| concurrency-and-ordering | command tunnel is registered before observation; peer registry recheck; atomic generation; stale-generation regression | The old-disconnect/new-reconnect race found during review is closed across runtime-worker ordering without a global async lock or public contract change. | pass |
| interface-and-compatibility | crate-local manager methods; unchanged cmd158, `PnServerSelector`, client response and Cargo dependencies | Existing consumers compile and no migration, wire field, feature, or crate-root export is introduced. | pass |
| security-and-capacity | authenticated peer lifetime, final-disconnect cleanup, one retained state entry per PN, monotonic `u64` generation | Public address trust is bounded by a command peer; stale observations are cleared, and retained timeout state remains bounded by known PN identities rather than accumulating per heartbeat. | pass |
| test-adequacy | seven manager tests, one listener test, all-target check, red failure before fix, refreshed task artifact | Automated tests cover normal, boundary, negative, error, compatibility, lifecycle, and the discovered concurrency regression; the live multi-process replay remains a documented low residual gap. | pass |

## Document Consistency
| Document | Source | Implementation Consistency | Finding | Status |
|----------|--------|----------------------------|---------|--------|
| design | `pipeline/plan.md` | Manager owns state lifetimes and generation; control server owns peer registration/disconnect events exactly as mapped. | State, interfaces, failure flows, ordering guard, and implementation paths match the automatic design. | pass |
| testing | `testplan.yaml` | Registered steps execute both focused test modules and the affected all-target compile closure; runtime state records all applicable case types and the manual live gap. | Test implementation and latest task artifact match the declared test plan and current evidence inputs. | pass |

## Validation Evidence
- Pre-fix red evidence: `cargo test -p bucky-vpn-server observed_address_survives_timeout_and_same_connection_heartbeat_recovers -- --nocapture` failed because timeout pruning deleted the only observed address and the resumed heartbeat still left selection empty.
- Task-scoped green evidence: `.harness/test-results/test-runs/20260817T091635Z-bucky-vpn-server+026-recover-pn-observed-address-all.json`; seven manager tests passed, one listener test passed, and `cargo check -p bucky-vpn-server --all-targets --locked` passed.
- Broader crate evidence: `cargo test -p bucky-vpn-server` reached 64 passes and one unrelated pre-existing failure in `sqlite_store_factory::node_traffic_record_rolls_back_and_retries_idempotently`; rerunning that exact test alone reproduced the same traffic-expiry assertion and it is outside this task's paths and state contract.
- Dependency evidence: `sfo-cmd-server` 0.4 removes a tunnel from the peer connection list and invokes `on_peer_disconnected` only after that peer's list becomes empty; the task additionally rechecks `get_peer_tunnels` before cleanup.
- Diff hygiene: task-owned Rust diffs pass `git diff --check`; whole-worktree warnings are limited to unrelated pre-existing `vpn_web` CRLF/trailing-whitespace changes.
- Live integration gap: suppress cmd158 longer than TTL while keeping the authenticated control session open, resume it, and confirm `pn_proxy_nodes` plus the next client PN update again contain `47.113.93.155:3625` without restarting any process.

## Conclusion
- Accepted / rejected / needs changes: accepted
- Reason: the approved recovery behavior is implemented with explicit transport and heartbeat lifetimes, the independently discovered reconnect race is closed by registration ordering and observation generations, client selection remains fail-closed, and refreshed task-scoped tests and compilation pass with no blocking finding.
- Supporting task-relevant test evidence: `.harness/test-results/test-runs/20260817T091635Z-bucky-vpn-server+026-recover-pn-observed-address-all.json`
- Residual risk: the deployed multi-process replay is not available in this workspace; deterministic transition coverage and real dependency compilation reduce but do not replace that operational confirmation.
