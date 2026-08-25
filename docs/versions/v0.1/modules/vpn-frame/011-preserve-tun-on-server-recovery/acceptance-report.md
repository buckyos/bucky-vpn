# Preserve TUN On VPN Server Recovery Acceptance Report

## Findings
| ID | Severity | Kind | Evidence | Problem | Blocking |
|----|----------|------|----------|---------|----------|
| F-003 | low | testing-consistency | `testplan.yaml` integration level; pipeline state cross-module cases | Live Windows/Wintun adapter recovery, server restart, and deterministic adapter-create failure were not executed in this Linux workspace. The limitation is explicitly documented and does not invalidate the verified client-side lifecycle and retry ordering. | no |

The prior blocking findings are closed. F-001 is resolved because `is_first` is cleared only after successful device reconciliation and both applied-version stores. F-002 is resolved by the focused zero-version first-response contract test and the refreshed successful task artifact.

## Requirement Review
| Requirement or Boundary | Source | Implementation Evidence | Finding | Status |
|-------------------------|--------|-------------------------|---------|--------|
| PN/name/member-only refresh must not recreate or drop an unchanged TUN. | `proposal.md` P-001 and `pipeline/plan.md` Change Classification | `VpnDevice::tun_effective_changed` includes only `id`, `ip`, `mask`, `ipv6`, and `ipv6_mask`; `reconcile` leaves `dev` untouched for name/PN/member-only updates. | The field boundary is explicit and control/routing metadata no longer drives OS TUN replacement. | pass |
| `group_id` changes refresh receive dispatch without replacing the TUN. | `pipeline/plan.md` Change Classification and Invariants | `VpnDevice::reconcile` detects `dispatch_changed` and restarts only the receive task when TUN-effective fields are unchanged. | Receive-context ownership matches the approved design. | pass |
| A real TUN transition failure must remain managed, return the underlying error, and be retried without committing applied state. | `proposal.md` P-002 and Success Criteria | `VpnDevice` saves desired network/receiver state before creation; `VpnClient` reinserts the device before propagating an error; version and first-sync stores occur only after `reconcile_result?`. | Failed transitions remain visible and retryable, including the first `(0,0)` response boundary. | pass |
| Multi-network failure must restore all entries, retain stale entries, and leave both versions uncommitted. | `proposal.md` Risks and `pipeline/plan.md` Failure Flows | `run_proc` restores each processed entry before error propagation, restores the complete map unconditionally, and performs stale retention and version commits only after full-loop success. | The transaction preserves processed, failed, and unprocessed device state for deterministic retry. | pass |
| Public and wire contracts remain compatible. | `proposal.md` Boundary and non-goals | Existing public `VpnDevice::start` and `update_device` signatures remain; `GetVpnInfo` request/response, command codes, server handler, and vpn-client factory contracts are unchanged. | No public API, wire, persistence, authorization, or build-surface change was introduced. | pass |

## Implementation Review
| Area | Evidence | Finding | Status |
|------|----------|---------|--------|
| TUN lifecycle classification | `vpn-frame/src/client/vpn_device.rs:212-251` | PN/name changes retain the TUN; `group_id` restarts only the reader; TUN-effective changes recreate. | pass |
| Failed create/recreate state | `vpn-frame/src/client/vpn_device.rs:174-187,212-242` | Desired state and receive context are retained before create; a missing device is retried even when its snapshot already matches; errors are logged and returned. | pass |
| Device-map transaction | `vpn-frame/src/client/vpn_client.rs:275-343` | Every removed/new device is inserted before propagation; errors skip stale removal; the complete local map is restored before return. | pass |
| Router and version ordering | `vpn-frame/src/client/vpn_client.rs:262-347` | PN routes are prepared before reconciliation, member routes follow each successful device, and both applied versions commit only after complete success. | pass |
| First-sync retry state | `vpn-frame/src/client/vpn_client.rs:239-260,339-348`; `vpn-frame/src/server/vpn_server.rs:501-508`; `vpn-frame/src/server/node_pn_manager.rs:29-34` | The initial request remains `None/None` until reconciliation and both version stores succeed; the sole `is_first=false` store is last, so a legitimate failed `(0,0)` response cannot be mistaken for synchronized state. | pass |
| Cross-module compatibility | `vpn-frame/src/vpn_protocol.rs`, `vpn-frame/src/client/vpn_server_client.rs`, `vpn-frame/src/server/vpn_server.rs`, `vpn-client/src/p2p_vpn.rs` | Client/server version comparison and payload contracts remain unchanged; the corrected client retry marker preserves the server's full-response retry mechanism. | pass |

## Testing Review
| Evidence | Result | Finding | Status |
|----------|--------|---------|--------|
| `vpn-frame/tests/tun_recovery_contract.rs` | Six focused tests cover public compatibility, TUN-effective classification, dispatch-only refresh, map retention/error ordering, full-response commit ordering, and zero-version first-response retry. | The added test binds `None/None`, zero-initialized versions, `reconcile_result?`, both version stores, and the sole first-sync store into one mechanically checked success boundary. | pass |
| `.harness/test-results/test-runs/20260805T080613Z-vpn-frame+011-preserve-tun-on-server-recovery-all.json` | Focused `cargo test` and `cargo check -p vpn-frame --all-targets --locked` both exited 0. | The refreshed artifact covers both change IDs and the current post-return sources/testplan. | pass |
| `testplan.yaml` integration boundary | Windows/Wintun live restart and deterministic create-failure scenario remains manual. | The gap is concrete, platform-specific, and retained as low residual risk rather than represented as an automated pass. | pass |

## Document Consistency
| Document | Source | Implementation Consistency | Finding | Status |
|----------|--------|----------------------------|---------|--------|
| design | `pipeline/plan.md` | Field classification, map restoration, stale removal, version ordering, and first-sync retry behavior match the approved state and failure invariants. | No unresolved design-to-implementation conflict remains. | pass |
| testing | `testplan.yaml` | The declared six focused checks and all-target compile check match the successful artifact; the platform limitation is explicitly recorded. | Required automated evidence exists and the remaining manual boundary is honestly scoped. | pass |

## Result Summary
- Overall result: accepted
- Outcome: PN/control-only recovery no longer destroys an unchanged TUN, real adapter failures remain managed and retryable, and polling state commits only after the complete response succeeds.
- Blocking issues: none. Prior F-001 and F-002 are closed.
- Residual risk: live Windows/Wintun server-restart recovery and deterministic adapter-create failure have not been executed in the current Linux environment.
- Next action: complete the accepted task's Harness lifecycle and task-index closeout; execute the documented Windows/Wintun live recovery scenario when that platform environment is available.

## Object and Scope
- Task manifest: task.yaml
- Reviewed changes: `CHG-preserve-tun-on-control-refresh`, `CHG-retry-failed-tun-update`
- In scope: launch-confirmed proposal, automatic pipeline plan, corrected production/test diff, refreshed testplan and successful artifact, vpn-frame boundary, and adjacent vpn-client/vpn-server retry contracts.
- Out of scope: unrelated dirty-worktree changes, SN/PN authorization redesign, privileged live deployment mutation, and Flutter UI.

## Conclusion
- Accepted / rejected / needs changes: accepted
- Reason: The delivered code closes the destructive TUN-refresh path and the prior zero-version first-sync retry defect; all required portable focused tests and all-target compilation passed with no blocking finding.
- Residual risk: Windows/Wintun live recovery remains a documented low-severity manual evidence gap.
