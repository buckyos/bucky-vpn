---
task_manifest: task.yaml
status: approved
---

# Preserve TUN On VPN Server Recovery Proposal

Risk profile: ./risk-profile.yaml

## Workflow Tier Judgment
- Proposed tier: high-risk
- Final tier: high-risk
- Tier rationale / triggered boundaries: The fix changes the lifecycle and recovery semantics of an operating-system TUN resource after distributed control-plane failure. It must distinguish routing-only metadata from interface-effective configuration, preserve or recover usable state across partial failures, and avoid committing refresh versions after an unsuccessful device transition. These confirmed runtime-integration, lifecycle, retry, and cross-platform consequences select high-risk.
- Proposal and tier confirmation: confirmed with automatic downstream completion by the user statement “确认，自动完成任务” on 2026-08-05.

## Background and Goal
After `vpn_server` fails and restarts, `vpn-client` temporarily cannot use the SN/PN control path. When control connectivity recovers, the next `GetVpnInfo` response can change PN metadata. The client currently compares the complete `NodeNetwork`, destroys the existing TUN before rebuilding it, discards rebuild errors, removes the failed device from its device map, and still commits the returned server/PN versions. The observed result is `create tun device` immediately followed by `drop tun device`; later unchanged responses do not retry, so the TUN remains absent even after transport recovery.

The goal is to keep a working TUN across control-plane-only refreshes and make genuine interface updates failure-visible and retryable, so a `vpn_server` restart cannot silently and permanently remove all client TUN devices.

## Scope
### In scope
- Separate TUN-effective configuration changes from control-plane routing metadata changes. A PN server, PN endpoint/name, or peer-membership refresh must update routing state without recreating the operating-system TUN.
- Keep the existing TUN registered and usable when the received network information does not require an interface-level change.
- Stop swallowing TUN create/update errors; include the network ID and underlying error in logs and return the failure through the refresh path.
- Do not commit the new VPN-info or PN-info version when a required TUN transition fails. A later polling cycle must retry the transition instead of treating the failed response as applied.
- Avoid losing the device-map entry on update failure. If an actual interface-effective change requires destructive platform work, retain sufficient managed state for deterministic retry and make any temporary loss explicit in logs.
- Add focused lifecycle regression coverage for PN-only refresh, successful interface-effective refresh, failed refresh/version commit behavior, and subsequent retry.
- Run the applicable `vpn-frame` tests and compile checks, plus document any Windows/Wintun behavior that cannot be executed in the current environment.

### Out of scope
- Changing SN discovery, PN authorization policy, `pn control open rejected` semantics, or VPN server restart ordering.
- Changing VPN command codes, `GetVpnInfo` wire encoding, persistence schema, network allocation, or Flutter UI behavior.
- Broadly redesigning `tun-rs`, replacing Wintun, or introducing a new platform abstraction unrelated to this failure.
- Treating temporary PN tunnel rejection as a reason to remove the local TUN.

### Boundary with neighboring modules
`vpn-frame::client` owns the TUN and configuration-application lifecycle and is the implementation target. `vpn-client` continues to provide P2P/PN routing factories without taking ownership of operating-system device recovery. `vpn-server` continues to publish versioned VPN/PN information; its protocol and persistence behavior remain unchanged. Platform-specific TUN creation remains delegated to `tun-rs`/Wintun.

## Requirement Review
The requested fix is necessary and the proposed direction addresses the destructive boundary rather than masking the server restart. The `no active sn` and `pn control open rejected` errors explain temporary packet delivery failure, but the permanent device loss is caused locally when a later `GetVpnInfo` refresh destroys the TUN and commits the refresh version despite failure.

The preferred approach is to avoid interface work for routing-only changes, propagate real device-transition failures, and make version advancement conditional on successfully applying the response. This is safer than unconditional TUN recreation or merely adding a delay, because a delay would still couple PN metadata to an unrelated operating-system resource and would not guarantee recovery.

The principal tradeoff is that interface-effective changes may remain pending across multiple polling cycles when the platform cannot recreate the adapter immediately. That is preferable to silently declaring the configuration applied while no TUN exists. The design stage must define the exact device-effective field set and atomicity/rollback behavior before implementation.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-preserve-tun-on-control-refresh | Apply PN and peer-routing updates without recreating or dropping an unchanged TUN, while retaining correct router state. | `vpn-frame::client` configuration application and device lifecycle only; no server or wire change. | Requires an explicit distinction between interface-effective and routing-only fields instead of whole-structure equality. | A focused test starts with a live logical device, applies a PN-only change, proves that no device restart/drop path runs, and proves routing receives the new PN data. | No change to PN authorization, discovery, or transport retry policy. |
| P-002 | CHG-retry-failed-tun-update | Make required TUN updates failure-visible and retryable: preserve managed device state, return/log the underlying error, and advance response versions only after the complete device application succeeds. | The 30-second VPN-info refresh remains the retry driver; no new background retry loop or protocol field. | A real interface change may remain pending until a later poll rather than being reported as applied. | Focused failure-injection tests prove the device entry is not silently discarded, versions do not advance on failure, and a later successful poll applies the update. | No guarantee that an old interface configuration remains usable when the OS itself requires destructive replacement; the guarantee is no silent permanent loss and deterministic retry. |

## Success Criteria
- Concrete user-visible or system-visible result: restarting `vpn_server` or changing only PN routing information no longer removes a working client TUN; after temporary control-path rejection, transport recovery resumes without restarting `vpn-client`.
- Required evidence: focused unit/lifecycle tests for change classification and failure retry, `vpn-frame` targeted tests, compile closure for affected Rust crates, a review of the exact `GetVpnInfo -> apply -> version commit` ordering, and explicit current-environment coverage limits for Windows/Wintun.
- Explicit non-goals: eliminating all transient packet loss while the server/PN validator is unavailable, changing server authorization policy, or changing VPN wire contracts.

## Risks
- Incorrectly classifying an IP, mask, network identity, or dispatcher-routing change as metadata-only could retain a stale device or packet receiver.
- Destructive adapter updates behave differently across Windows, Linux, and macOS; tests need an injectable lifecycle boundary rather than relying only on privileged real-TUN creation.
- Version advancement and device-map updates must remain consistent if multiple networks are returned and a later network fails; partial application must not make the next retry skip the failed network.
- Existing unrelated tracked and untracked user changes must be preserved and excluded from this task's implementation evidence.
