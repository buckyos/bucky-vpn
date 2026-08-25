---
task_manifest: task.yaml
status: approved
---

# Separate VPN And PN Command Server API Proposal

Risk profile: not-created (replace with ./risk-profile.yaml only after high-risk confirmation)

## Workflow Tier Judgment
- Proposed tier: high-risk
- Final tier: trivial
- Tier rationale / triggered boundaries: Removing the `P = T` generic default and the public constructors that reuse one command server deliberately breaks the exported `VpnServer` construction API. Although the current repository production assembly already passes distinct U16 VPN and U24 PN-control services, external consumers using `VpnServer<T, S, F>` or `VpnServer::new` must migrate, so the public-contract compatibility trigger selects high-risk.
- Proposal and tier confirmation: confirmed with the user-selected trivial tier by “用简单任务完成就好” on 2026-07-21. The user-selected lower tier retains the documented external source-compatibility risk and uses targeted compile checks instead of the proposed full breaking-contract lifecycle.

## Background and Goal
`VpnServer` handles two command protocols with different framing contracts: ordinary VPN commands use `VpnCmdPkgLen = U16`, while PN-control commands use `VpnControlCmdPkgLen = U24` with a 10 MiB cap. Its generic declaration still defaults `P` to `T`, and legacy constructors reuse one `Arc<T>` for both roles. Those conveniences imply that one concrete service may own both protocols even though the current runtime intentionally instantiates separate command services.

The goal is to make the type and construction API express that separation unambiguously: callers must name the PN-control server type and pass both command-server instances explicitly.

## Scope
### In scope
- Remove the `P = T` default from `VpnServer` and `VpnServerRef`.
- Remove or replace constructors that accept only one `cmd_server` and internally clone it for PN control.
- Retain one explicit constructor path that requires both `Arc<T>` for ordinary VPN commands and `Arc<P>` for PN-control commands, while preserving the optional selector behavior only if it remains necessary and cannot reintroduce shared-server construction.
- Update direct repository consumers and compile-contract fixtures only if inspection after approval identifies call sites affected by the breaking API.
- Verify new-path compile success, old-path compile failure, removed-symbol/source scans, and compile closure for affected workspace consumers.

### Out of scope
- Changing `VpnCmdPkgLen`, `VpnControlCmdPkgLen`, command codes, serialized payloads, or wire framing.
- Changing PN selection, command handling, server startup, or shutdown behavior.
- Adding an adapter that makes one underlying transport decode both length formats.
- Refactoring unrelated `VpnServer`, `PnControlServer`, or storage logic.

### Boundary with neighboring modules
`vpn-frame` owns the exported generic and constructors. `bucky-vpn-server` is a compile consumer and already uses `new_with_pn_control_cmd_server` with distinct `P2pSnCmdServer` and `ProxyControlCmdService` values; it should need no behavioral change unless the final constructor is renamed or reshaped. External Rust consumers using the removed shorthand API will require source migration.

## Requirement Review
The request is reasonable and strengthens the API invariant at compile time. A generic default is not itself an equality constraint, but `P = T` plus the one-server constructors makes the invalid architectural implication easy to consume. Removing both is clearer than merely tightening trait bounds.

The proposed direction retains a single explicit two-server construction path rather than adding compatibility shims. The tradeoff is intentional source incompatibility for callers of `VpnServer<T, S, F>`, `VpnServerRef<T, S, F>`, `VpnServer::new`, or `VpnServer::new_with_pn_server_selector`. Current repository runtime assembly already follows the intended new path, reducing internal migration risk.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-separate-vpn-pn-command-server-api | Require `VpnServer` callers to provide distinct ordinary VPN and PN-control command-server type/value parameters, with no `P = T` default and no constructor that clones one server into both roles. | Limit production behavior changes to the `vpn-frame` construction API; preserve existing command handling and the current explicit production assembly. | Deliberately breaks shorthand source compatibility to prevent misleading or unsafe reuse across U16/U24 framing contracts. | New explicit two-server construction compiles; old default-generic and one-server forms fail to compile; removed API symbols are absent; affected workspace consumers compile. | No wire-format, command behavior, selector-policy, storage, or unrelated refactor change. |

## Success Criteria
- Concrete user-visible or system-visible result: `VpnServer` cannot be instantiated without explicitly naming and supplying the PN-control command server separately from the ordinary VPN command server.
- Required evidence: positive external compile fixture for the retained explicit API, negative external compile fixture or equivalent compiler evidence for the removed shorthand API, removed-symbol scan, and compile closure for `vpn-frame` plus direct workspace consumers.
- Explicit non-goals: protocol changes, dual-framing adapters, runtime behavior changes, and unrelated cleanup.

## Risks
- This is a deliberate breaking Rust API change for external consumers using the default fourth type parameter or the one-server constructors.
- Renaming the existing explicit constructor would create unnecessary churn; the smallest safe implementation should retain it unless design evidence shows a clearer no-churn alternative.
- The working tree contains pre-existing user changes, including the U24 PN-control work that introduced the distinct bounds; implementation must preserve those changes and isolate this task's edits.
