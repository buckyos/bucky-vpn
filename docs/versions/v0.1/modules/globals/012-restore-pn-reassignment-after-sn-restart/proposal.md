---
task_manifest: task.yaml
status: approved
---

# Restore PN Reassignment After SN Restart Proposal

Risk profile: ./risk-profile.yaml

## Workflow Tier Judgment
- Proposed tier: high-risk
- Final tier: high-risk
- Tier rationale / triggered boundaries: The correction changes recovery semantics across the persisted SN control plane, the in-memory PN version protocol, and the vpn-client P2P connection registry. Version-collision handling and remove/re-add lifecycle behavior affect distributed runtime recovery across two Rust project modules, so the work remains high-risk.
- Proposal and tier confirmation: the user confirmed the displayed expansion of task 011 and automatic downstream completion with the statement “确认扩展 011” on 2026-08-05. Because task 011 had already completed and the added scope crosses `vpn-frame` and `bucky-vpn`, repository schema requires this bound cross-module follow-up packet while preserving 011 as accepted history.

## Background and Goal
Task 011 preserved client TUN devices across VPN-server recovery, but subsequent inspection confirmed that an independent PN can return to `online` without becoming usable by existing clients. PN liveness, server assignment, client refresh application, and client-side TTP registration are separate states.

After SN restart, the in-memory per-node PN version counter starts again from zero. The server can return a non-empty PN refresh because its freshly reconstructed state changed while returning a numeric version equal to the client's pre-restart cached version. The client currently discards that non-empty response solely because both version numbers compare equal. Separately, when a PN temporarily disappears, `sync_pn_server_connections` removes its TTP targets but leaves the PN in `connected_pn_targets`; reappearance with identical metadata therefore skips `connect_server`.

The goal is to ensure that a PN which becomes live and eligible after SN restart is applied and reconnected by existing clients without restarting the client process.

## Scope
### In scope
- Treat a non-empty `GetVpnInfo` response as authoritative even when its numeric server and PN versions equal the locally cached values after an SN restart.
- Preserve the empty incremental-response fast path so an unchanged response does not remove networks or recreate client state.
- Remove stale PN entries from the client connection registry when their TTP targets are removed.
- Reconnect a PN that later reappears with the same identity and endpoint metadata.
- Preserve retry semantics: a connection/application failure must leave versions or connection registry state such that a later poll retries.
- Add focused red-green regression coverage for equal-version non-empty refreshes and same-PN remove/re-add recovery.

### Out of scope
- Proactively pushing PN changes from SN to clients; the existing client polling model remains.
- Changing PN approval policy, heartbeat timing, selection order, persistent network schema, command payloads, or public APIs.
- Revisiting task 011's already delivered TUN field classification except where the refresh entry condition must allow its reconciliation path to run.
- Guaranteeing uninterrupted traffic while SN or PN is actually offline.

### Boundary with neighboring modules
`vpn-frame::client::VpnClient` owns whether a received VPN-info payload is applied and when response versions are committed. `bucky-vpn::p2p_vpn::P2pVpnTunnelFactory` owns the set of PN targets actually registered with the P2P TTP client. The server continues to reconstruct PN state and assign an eligible PN during `GetVpnInfo`; no server-side protocol or persistence changes are required.

## Requirement Review
The requested expansion is necessary: PN `online` only proves fresh heartbeat state and does not prove that an existing client consumed the reconstructed assignment or re-established its PN target. The safest correction is to honor response content over colliding restart-local version numbers and make the connection registry reflect actual remove operations.

The principal tradeoff is that a non-empty response with equal versions now performs reconciliation instead of taking the fast path. This is intentional and bounded because an empty list remains the protocol's unchanged-response signal. Removing stale registry entries may cause a later poll to reconnect, which is the required lifecycle behavior and is preferable to retaining false connected state.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-apply-equal-version-pn-refresh | Apply non-empty VPN-info responses even when returned versions equal cached values, while retaining the empty unchanged-response fast path and commit-after-success semantics. | `vpn-frame/src/client/vpn_client.rs`; no wire or server change. | Equal-version non-empty responses perform idempotent reconciliation instead of being skipped. | A regression test reproduces the restart version collision before the fix and proves a non-empty response reaches PN routing/device reconciliation; empty equal-version responses still short-circuit. | No redesign of PN version generation or server persistence. |
| P-002 | CHG-reconnect-reappeared-pn | Make `connected_pn_targets` represent targets still registered with the TTP client so a removed PN with identical metadata is reconnected when desired again. | `vpn-client/src/p2p_vpn.rs`; no P2P protocol or endpoint-selection change. | Removed entries no longer suppress a later reconnect and may incur the expected new connection attempt. | A focused lifecycle regression removes a PN, re-adds the same PN, and proves `connect_server` is attempted again without restarting vpn-client. | No proactive server push, heartbeat change, or broad connection-manager redesign. |

## Success Criteria
- Concrete user-visible or system-visible result: after SN restarts, an already-running client resumes using an independent PN once that PN is live and eligible, including when the reconstructed PN version and metadata equal their pre-restart values.
- Required evidence: red-green focused tests for both change IDs, current `vpn-frame` and `bucky-vpn` compile closure, a task-scoped unified test-run artifact, and independent acceptance review against task 011's recovery outcome plus this proposal.
- Explicit non-goals: zero packet loss during outage, push-based reassignment, server schema changes, or changes to PN authorization.

## Risks
- Removing the version-only short circuit incorrectly could interpret a truly empty incremental response as “no networks” and drop devices; the fix must require non-empty response content before reconciliation.
- Connection registry updates must reflect successful logical removal without holding a standard mutex across asynchronous connection work.
- A failed PN reconnect must remain retryable on the next unchanged-version poll and must not leave false connected state.
- The working tree contains task 011 changes and unrelated user changes; this follow-up must preserve them and bind evidence only to its two production paths and task artifacts.
