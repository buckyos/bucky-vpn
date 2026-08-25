---
task_manifest: task.yaml
status: approved
---

# Online State Independent of IP Proposal

Risk profile: not-created (replace with ./risk-profile.yaml only after high-risk confirmation)

## Workflow Tier Judgment
- Proposed tier: high-risk
- Final tier: trivial
- Tier rationale / triggered boundaries: the change is a bounded bug fix in `vpn-frame`, but it intentionally changes the control-plane meaning of the public `online` field and the runtime liveness boundary. The confirmed API-contract and lifecycle consequences select the high-risk workflow by repository policy.
- Proposal and tier confirmation: explicitly confirmed by the user with the replacement `trivial` tier on 2026-07-20

## Background and Goal
`VpnServer::get_node_online_state` currently returns offline when the node has a fresh `online_nodes` entry but `get_peer_wan_ip` returns an empty list. This conflates liveness with optional address discovery and causes connected TCP-only or otherwise non-addressable clients to appear offline in the control plane.

The goal is to make node liveness depend only on membership in `online_nodes` and its existing expiration rule. WAN IP discovery remains supplemental metadata and an empty list must not change the online result.

## Scope
### In scope
- Change `VpnServer::get_node_online_state` so a fresh `online_nodes` entry returns `Some((version, ip_list))` even when `ip_list` is empty or WAN-IP lookup fails.
- Preserve the existing missing-node and expired-node behavior as offline.
- Add focused regression coverage for fresh-node/empty-IP and offline boundary behavior.

### Out of scope
- Changing the 30-second client refresh cadence, 120-second expiry, or 65-second offline monitor cadence.
- Adding a dedicated client heartbeat command.
- Changing client identity persistence, reconnect behavior, PN proxy heartbeat behavior, or endpoint discovery.
- Changing HTTP response field names or serialization shapes.

### Boundary with neighboring modules
The implementation remains in `vpn-frame`; `vpn-server` continues to translate `Some` into `online: true` and carries an empty `ip_list` without schema changes. `vpn-client` and `vpn_web` require no production change.

## Requirement Review
The request is reasonable and corrects a state-modeling defect: liveness and discovered addresses are independent facts. Keeping the existing return type minimizes compatibility impact while changing only the erroneous `None` branch. The tradeoff is that consumers can now receive `online: true` with an empty IP list; this is intentional and backward-compatible at the wire-shape level.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-online-state-independent-of-ip | Determine online state only from a fresh `online_nodes` entry and return online with an empty IP list when address discovery produces no usable address. | `VpnServer::get_node_online_state` and focused tests only; preserve the existing response shape and timeout behavior. | Online consumers must tolerate `ip_list: []`, separating optional metadata from liveness. | Regression test proves fresh node plus empty/failed IP lookup returns `Some`; missing or expired node remains `None`; targeted Rust tests pass. | No heartbeat, identity, transport, API-schema, UI, or PN behavior redesign. |

## Success Criteria
- Concrete user-visible or system-visible result: a connected node with a fresh online record is reported as `online: true` even when no WAN IP is available.
- Required evidence: focused automated coverage of empty/failed IP lookup and existing offline boundaries, plus passing targeted `vpn-frame` tests.
- Explicit non-goals: changing heartbeat timing, endpoint discovery, identity handling, or serialized field names.

## Risks
- The user selected `trivial` despite the recommended `high-risk` classification. The delivery therefore uses focused implementation review and targeted verification rather than separate design, testing, and acceptance stages; the semantic API risk remains visible here.
- Some consumers may have implicitly assumed that `online: true` guarantees at least one IP address; the response shape remains compatible, but the semantic distinction must be covered by tests.
- Error handling for WAN-IP lookup must not accidentally turn genuine liveness into offline or mask the missing/expired-node checks.
- The fix must not weaken the existing expiry rule or affect PN proxy liveness, which uses a separate heartbeat path.
