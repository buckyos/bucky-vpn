---
task_manifest: task.yaml
status: approved
---

# Second-Level PN Assignment Version Proposal

Risk profile: not-created (created only for high-risk tasks)

## Workflow Tier Judgment
- Proposed tier: high-risk
- Final tier: standard
- Tier rationale / triggered boundaries: The revision changes the PN-assignment protocol field and cache from the previously approved `u64` millisecond value to a `u32` second-level value while deliberately preserving the existing network `Node.info_version`. It remains a breaking VPN command wire change with client/server compatibility, process-local PN state, retry, and SN-restart recovery impact across `vpn-frame`, `bucky-vpn-server`, and `bucky-vpn`.
- Proposal and tier confirmation: confirmed with the user-selected standard tier by “确认，作为standard任务完成就好” on 2026-08-10. The lower tier retains the documented breaking protocol, same-second collision, year-2106, and live-integration residual risks.

## Background and Goal
The synchronization protocol currently carries two versions with different ownership. Network information uses the SQLite-backed `Node.info_version`; PN assignment information uses `NodePnInfoState.version`, an in-memory `u16` incremented when a client's PN assignment changes. The client stores both as `AtomicU16` and treats equality as an unchanged-state signal.

Only the PN version is defective for the reported restart path. It resets when SN restarts, wraps at `u16::MAX`, and can reuse a value cached by a still-running vpn-client. Consequently, PN can return to `live=true` while SN and vpn-client compare equal numeric PN versions that refer to different synchronization occurrences.

The goal is to define `pn_info_version` as a `u32` Unix epoch second value for the latest actual change to that client's canonical PN assignment state. `Node.info_version` and all network-information version behavior remain unchanged.

## Scope
### In scope
- Change `pn_info_version` from the old `u16` counter contract, and replace the current unfinished `u64` millisecond implementation, with `u32` across PN state, VPN request/response payloads, server comparison, and vpn-client caching.
- Store the latest PN assignment modification time in seconds since Unix epoch in `NodePnInfoState`.
- Update the timestamp only when the canonical per-client PN assignment changes, including PN assignment, endpoint, name, availability, addition, or removal changes visible to the client.
- Initialize vpn-client's PN version cache to `0`.
- Capture one `u32` Unix-second PN version when SN starts, and use that startup value as the initial server-side version whenever the new SN process first creates a client's PN assignment state.
- When an existing canonical PN assignment actually changes, assign the current Unix-second value. No numeric ordering or request-cache-based correction is used for synchronization.
- Keep vpn-client commit-after-success semantics: store the received `u32` PN version only after PN target, router, and device reconciliation succeeds.
- Verify the client-zero/server-startup defaults, unchanged polling, real-change transitions, content reversion, same-second behavior, clock rollback behavior, SN restart recovery, protocol compatibility behavior, and removal of PN counter/millisecond semantics.

### Out of scope
- Hashing PN assignment content or introducing a content-digest type.
- Changing `Node.info_version`, `NodeStore::inc_info_version`, SQLite `node.info_version`, or any network-information version generation/comparison behavior.
- Any SQLite schema migration or persistence of `pn_info_version`.
- Persisting live PN endpoint addresses as durable truth.
- Changing PN versions merely because a client polls.
- Changing PN approval, heartbeat, selection, endpoint synthesis, TUN lifecycle, membership, online-state, or UI behavior.
- Guaranteeing uninterrupted traffic while SN is unavailable.
- Providing a post-2106 representation beyond `u32::MAX` Unix seconds.

### Boundary with neighboring modules
`vpn-frame` owns `NodePnInfoState`, PN state comparison, the `pn_info_version: u32` request/response field, server synchronization behavior, and vpn-client cache/application contract. `bucky-vpn-server` continues to supply live selector results without a database migration. `bucky-vpn` consumes refreshed PN assignments through its existing target synchronization callback. The existing network `info_version: u16` wire and persistence path is an explicit untouched boundary.

## Requirement Review
A second-level modification timestamp is a compact replacement for the process-local counter and matches the revised requested type. A newly started client supplies `0`, while a newly started SN uses its captured startup Unix-second value for all newly created per-client PN states. When state changes and later returns to identical content in a different second, the new modification time forces reconciliation. When SN restarts, reconstructing the PN assignment after the PN becomes live uses the new SN process's startup value instead of resetting to a small counter.

The version is a `u32` Unix epoch second value. vpn-client initializes its PN cache to `0`. SN captures its default PN version once at process startup; `NodePnManager` uses that value when a per-client state is first created, while later real canonical changes use the then-current Unix second. State comparison continues to use the existing canonicalized `NodeNetworkPnInfo` equality, and unchanged polling retains the stored value. Server and client use equality, not numeric ordering, as the synchronization decision. There is deliberately no counter or cache-based monotonic correction: two changes or two SN starts in the same second may share a value, and clock rollback may make a later value numerically smaller. At `u32::MAX`, later Unix seconds cannot be represented; this is an explicit year-2106 limitation of the requested type.

Changing the wire field from the old `u16` counter contract to `u32` is not compatible with existing binaries, and the unfinished `u64` millisecond build is also incompatible with this revision. The VPN command protocol version will identify the final `u32` contract and reject incompatible peers with an explicit diagnostic. `info_version` remains `u16`. A mixed-version bridge may be designed only if rolling compatibility is explicitly required; old PN counters or unfinished millisecond values must not be silently reinterpreted.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-define-pn-seconds-version | Define `pn_info_version` as a `u32` Unix epoch-second value; client default is `0`, server default is captured once at SN startup, and later canonical PN changes use the current second. | `vpn-frame` PN state, client cache, and time-generation helper; network `info_version` remains unchanged. | Second resolution is compact but permits same-second collisions and has a year-2106 ceiling. | Tests prove client zero initialization, one stable SN startup default across newly created client states, unchanged content retaining the value, real PN changes using the supplied current second, and rollback remaining valid under equality-only comparison. | No content hash, millisecond value, generic network-version replacement, polling-driven update, or monotonic correction. |
| P-002 | CHG-replace-pn-counter-with-seconds | Replace `NodePnManager` counter/millisecond transitions with the SN startup seconds default and current seconds on actual PN changes. | `NodePnInfoState`, `NodePnManager`, and selector-resolved client PN data; keep live endpoints and timestamps non-persistent. | Two changes or starts in the same second can reuse a value; the design follows the requested raw second-level semantics. | Tests prove initial states use the captured SN startup value, PN offline/online, endpoint/name/assignment changes use current seconds, unchanged polling retains the value, and clock rollback introduces no ordering assumption. | No `Node.info_version`, SQLite, heartbeat, selector-policy, request-cache correction, or counter change. |
| P-003 | CHG-migrate-pn-version-to-u32-wire | Use `u32` for request/response `pn_info_version` and vpn-client's PN cache, identify the final protocol contract, and commit the PN version only after successful application. | `GetVpnInfoReq`, `GetVpnInfoResp`, server/client wrappers, vpn-client reconciliation, and direct consumers; retain network `info_version: u16`. | Old `u16` counter and unfinished `u64` millisecond binaries are wire-incompatible. | Final-protocol round trips, negative mixed-version cases, serialization evidence, retry-after-application-failure coverage, unchanged network-version checks, and consumer compile closure pass. | No modification of network-version storage/type/semantics, implicit truncation, or silent fallback. |
| P-004 | CHG-verify-seconds-pn-restart | Execute the reported live sequence with SN restarted while PN and vpn-client remain running and bind logs to the new SN startup-second PN version. | Repository multi-process integration plus deterministic lower-layer tests. | The scenario is slower, depends on a healthy Multipass service, and must cross a second boundary when asserting distinct old/new raw timestamp values. | Evidence shows unchanged network `info_version`, server initialization from the new SN startup value, client receipt/application, and restored PN address without PN/client restart. | No acceptance based only on source inspection or unit tests. |

## Success Criteria
- Concrete user-visible or system-visible result: `pn_info_version` is a `u32` Unix-second value for the latest PN assignment modification rather than a wrapping counter or `u64` millisecond value; after SN restart, an already-running vpn-client receives and applies the reconstructed live PN address without restarting PN or client, while `Node.info_version` remains unchanged.
- Required evidence: client-zero and server-startup default tests, PN change-time transition tests, same-second and clock-rollback semantics, wire compatibility and serialization checks, retry-ordering tests, explicit unchanged-network-version checks, affected-crate compile closure, and an executed multi-process SN-restart recovery scenario.
- Explicit non-goals: hashing content, changing `Node.info_version` or SQLite, persisting PN runtime versions/endpoints, polling-driven PN version churn, unrelated VPN behavior changes, or zero outage during SN downtime.

## Risks
- An incomplete PN equality comparison can miss a client-visible change and leave the timestamp unchanged; design must enumerate every field participating in `NodeNetworkPnInfo` equality.
- Same-second reconstruction/change can repeat the client's cached raw timestamp; this is an accepted consequence of the requested raw second-level value and live evidence must avoid claiming uniqueness within one second.
- Wall-clock rollback can decrease the numeric value; server/client comparison must remain equality-only and tests must reject ordering assumptions.
- `u32` Unix seconds reaches its representational ceiling in 2106; this is accepted by the requested compact type and must saturate or fail explicitly rather than wrap silently.
- Protocol mismatch can disconnect old clients; compatibility and coordinated rollout behavior must be explicit and tested.
- Accidental edits to `Node.info_version`, its SQLite column, or network counter callers are scope violations and must fail review.
- Unit-only tests can miss the exact restart timing; acceptance must execute the reported live sequence.
