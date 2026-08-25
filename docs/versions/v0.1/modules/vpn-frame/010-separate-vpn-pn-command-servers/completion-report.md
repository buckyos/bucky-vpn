# Completion Report

## Object and Scope
- Task manifest: task.yaml
- Workflow tier: trivial
- Change record: not-applicable

## Delivery Summary
- Outcome: `VpnServer` and `VpnServerRef` now require an explicit PN-control command-server type, and the single-server constructors that cloned one ordinary server instance into both the U16 VPN and U24 PN-control roles have been removed.
- Handoff: Callers must use `VpnServer::new_with_pn_control_cmd_server` and supply separate `cmd_server` and `pn_cmd_server` values; the existing `bucky-vpn-server` runtime already follows this construction path.

## Proposal Consistency
| change_id | requirement_or_boundary | proposal_source | delivery_evidence | finding | status |
|-----------|-------------------------|-----------------|-------------------|---------|--------|
| CHG-separate-vpn-pn-command-server-api | Require explicit ordinary VPN and PN-control server types/values, with no `P = T` default or constructor that reuses one service | proposal.md P-001, Scope, and Success Criteria | `VpnServer`/`VpnServerRef` require four type parameters; only `new_with_pn_control_cmd_server` remains public; direct consumer compile and negative old-API probe | Delivery implements the approved API separation without changing command framing, handlers, selector behavior, or runtime assembly | pass |

## Implementation Review
| area | evidence | finding | status |
|------|----------|---------|--------|
| Generic contract | The `P` command-server parameter has no default in `VpnServer`; the `VpnServerRef` alias also requires an explicit fourth parameter | The PN-control service type can no longer be omitted or silently inferred as `T` | pass |
| Construction contract | `new`, `new_with_pn_server_selector`, and `new_with_optional_pn_server_selector` are absent; `new_with_pn_control_cmd_server` accepts distinct ordinary and PN-control server instances | No public construction path clones one server value into both protocol roles | pass |
| Runtime preservation | `vpn-server/src/main.rs` already calls `new_with_pn_control_cmd_server` with `P2pSnCmdServer` and `ProxyControlCmdService` | Existing runtime assembly remains explicit and needs no behavioral migration | pass |
| Protocol boundary | `VpnCmdServer` remains U16 and `P` remains bound to the dedicated U24 PN-control length type | No wire format, command code, body codec, or handler behavior changed | pass |
| Scope discipline | Baseline comparison contains only the requested generic/default and legacy-constructor removals in `vpn_server.rs` | Pre-existing U24 PN-control and unrelated working-tree changes were preserved | pass |

## Verification
- Targeted check: `rustfmt --edition 2024 --check vpn-frame/src/server/vpn_server.rs`; focused removed-symbol scan; `git diff --check -- vpn-frame/src/server/vpn_server.rs`; `cargo check -p vpn-frame -p bucky-vpn-server`; `cargo build -p vpn-frame`; external `rustc` old-API probe against the built `vpn_frame` rlib
- Result: passed
- Exception reason: The user selected the trivial tier for this deliberate breaking API removal, so verification uses focused positive compile closure plus negative compiler evidence instead of a full staged compatibility lifecycle; the old three-parameter type fails with E0107 and the old `VpnServer::new` reference fails with E0599.

## Findings
| id | severity | evidence | problem | blocking |
|----|----------|----------|---------|----------|
| F-001 | none | Focused formatting/source checks, successful two-crate compile closure, and expected negative compiler diagnostics | No implementation defect was found in the requested repository scope | no |
| F-002 | low | E0107 for the old three-parameter `VpnServer` form and E0599 for `VpnServer::new` | External Rust consumers using the removed shorthand API require source migration to the explicit four-parameter/two-server API | no |

## Conclusion
- Accepted / rejected / needs changes: accepted
- Reason: The API now enforces explicit separation of the U16 VPN and U24 PN-control services, the repository runtime consumer compiles unchanged on the retained constructor, and focused positive and negative compile evidence matches the approved requirement.
