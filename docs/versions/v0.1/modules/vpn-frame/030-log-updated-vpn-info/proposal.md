---
task_manifest: task.yaml
status: approved
---

# Log Updated VPN Information Proposal

Risk profile: not-created (replace with ./risk-profile.yaml only after high-risk confirmation)

## Workflow Tier Judgment
- Proposed tier: trivial
- Final tier: trivial
- Tier rationale / triggered boundaries: The change adds bounded observability to one existing `vpn-frame` client polling path. It does not change the `GetVpnInfo` wire contract, version comparison, reconciliation, persistence, security boundary, connection behavior, dependencies, or release output. A focused source-contract regression and package test provide a targeted verification signal.
- Proposal and tier confirmation: approved by the user with “确认” on 2026-08-25.

## Background and Goal
The client currently logs only the versions after a changed `GetVpnInfo` response has been fully applied. It does not show the information returned by SN, which makes PN endpoint and membership updates difficult to diagnose. The goal is to log the received VPN information whenever the response is non-empty or its version values differ from the client's committed versions.

## Scope
### In scope
- Emit an `info` summary for a changed/non-empty `GetVpnInfo` response, including `info_version`, `pn_info_version`, and network count.
- Emit an `info` detail entry for every returned network, including network identity and addresses, PN identity/name/endpoints, and returned members.
- Emit the received-information logs after the unchanged-empty fast path and before applying the response, so diagnostics remain available when reconciliation later fails.
- Extend the focused source-contract regression to bind the logging condition and placement.

### Out of scope
- Changing `GetVpnInfo` request/response serialization, version semantics, polling interval, caching, retries, reconciliation, PN selection, or tunnel behavior.
- Logging authentication material, raw command bodies, or opaque protocol bytes.
- Adding SN-side response logging or changing global log levels/sinks.

### Boundary with neighboring modules
The implementation remains in `vpn-frame`'s client runtime. `bucky-vpn` continues to consume the existing logging facade and configuration without application-level changes; `bucky-vpn-server` and the shared wire structures remain unchanged.

## Requirement Review
The requested observability is reasonable because the current post-commit version log cannot identify which network, PN endpoints, or members were returned. Logging only after the unchanged-empty fast path avoids periodic no-change noise. Using `info` makes the diagnostic visible under the normal client log level; this increases log volume only when the server returns actionable information. The member and endpoint data are operational VPN topology data, so the implementation will avoid unrelated credentials and raw payloads.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-log-updated-vpn-info | Log summary and per-network details whenever `GetVpnInfo` returns a non-empty response or changed versions. | Client receive path only, after unchanged detection and before response application. | Changed responses produce additional `info` log entries containing operational network topology. | Focused contract regression proves the condition, fields, and placement; `vpn-frame` targeted tests pass. | No protocol, polling, state, PN selection, or logging-backend changes. |

## Success Criteria
- A non-empty response or version change produces an `info` summary with both versions and network count.
- Every returned network produces an `info` detail containing its network fields, PN metadata/endpoints, and member information.
- An unchanged response with identical versions and an empty VPN list produces no new received-information log.
- Logging happens before response application, while the existing post-commit version log remains after successful reconciliation.
- Focused regression and relevant `vpn-frame` tests pass without modifying unrelated working-tree content.

## Risks
- The additional log entries expose operational VPN topology to principals already able to read the client's runtime logs; no credentials, request bodies, or opaque payload bytes are included.
- Large changed responses create one detail line per returned network, but unchanged 30-second polls stay silent.
