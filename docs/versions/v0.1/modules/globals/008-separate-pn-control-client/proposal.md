---
task_manifest: task.yaml
status: approved
---

# Separate PN Control Client Proposal

Risk profile: ./risk-profile.yaml

## Workflow Tier Judgment
- Proposed tier: high-risk
- Final tier: high-risk
- Tier rationale / triggered boundaries: The current blanket implementation makes the general VPN server client own PN-node-to-control-plane commands. Separating that responsibility introduces a dedicated public Rust type in `vpn-frame` and migrates its `vpn-server` consumer, crossing crate and architectural responsibility boundaries. The repository therefore selects high-risk by default.
- Proposal and tier confirmation: explicitly confirmed with automatic downstream completion by the user statement “确认，自动完成任务” on 2026-07-21

## Background and Goal
`VpnControlClientOps` is currently implemented for `VpnServerClient`, even though its four operations are specifically used by a PN node to report to and validate through the control-plane node. This gives the general VPN client an unrelated control-channel responsibility. The goal is to introduce a dedicated control client type for PN-to-control-plane communication and make that type, rather than `VpnServerClient`, implement `VpnControlClientOps`.

## Scope
### In scope
- Add a dedicated `VpnControlClient` Rust struct under the shared `vpn_frame::server` module.
- Give the dedicated type the command client, command version, timeout, and sequence state needed by the four `VpnControlClientOps` commands.
- Implement `VpnControlClientOps` only for the dedicated type and remove the corresponding control-channel responsibility from `VpnServerClient`.
- Update `vpn-server` PN control-client construction, aliases, reporters, and validators to use the dedicated type.
- Preserve the existing command codes, request/response wire shapes, error mapping, tunnel factory, and runtime behavior.
- Add or adjust focused regression/compile coverage during the post-implementation testing stage.

### Out of scope
- Redesigning `VpnControlClientOps` methods or command payloads.
- Changing PN heartbeat, traffic accounting, connection-validation semantics, retry behavior, or tunnel lifecycle.
- Refactoring ordinary VPN client operations such as join, query, or VPN-info retrieval.
- Changing `vpn-client`, Flutter Web UI, persistence, deployment, or configuration behavior.

### Boundary with neighboring modules
`vpn-frame::server` owns and exports the reusable control-plane client type and its `VpnControlClientOps` implementation. `vpn-frame::control_channel` retains the operation trait, trait-object reference, reporter/validator cores, and result conversion helpers, but does not own the concrete client class. `bucky-vpn-server` owns the PN process assembly that creates the concrete command tunnel and injects the dedicated client into reporters and validators. The general `VpnServerClient` remains responsible only for ordinary VPN client-to-server operations.

## Requirement Review
The requested separation is reasonable because the current implementation conflates two communication roles that happen to share command transport mechanics. A dedicated type makes the dependency direction and intended consumer explicit and prevents ordinary VPN clients from implicitly exposing PN control-plane commands. To keep the change bounded, the new type will reuse the existing generic command-client abstractions and preserve wire behavior; it will not introduce a second tunnel stack or speculative generic base class.

The main tradeoff is a small amount of duplicated transport state (`version`, timeout, sequence generator, and phantom generic ownership) between the ordinary server client and the control client. That duplication is preferable to retaining the wrong domain responsibility or introducing a broad transport abstraction without another demonstrated consumer.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-dedicated-pn-control-client | Define and export a dedicated `vpn_frame::server::VpnControlClient` that exclusively implements the existing four-operation `VpnControlClientOps` contract, and remove those operations/that implementation from `VpnServerClient`. | The concrete class lives in `vpn-frame/src/server/vpn_control_client.rs`; `control_channel.rs` retains only the abstract contract and shared reporter/validator logic. Command codes and serialized request/response types remain unchanged. | Accept limited transport-state duplication to keep domain roles explicit and avoid a speculative common base abstraction. | Source inspection shows no `VpnControlClientOps for VpnServerClient`; focused checks prove `vpn_frame::server::VpnControlClient` sends and decodes the same commands; `vpn-frame` compiles. | No protocol, command, retry, or general VPN-client redesign. |
| P-002 | CHG-pn-control-client-integration | Migrate the PN runtime in `vpn-server` to construct, store, and inject the dedicated control client for PN-to-control-plane traffic reporting and connection validation. | PN control assembly and its focused tests only. | Public/local type aliases may change, but runtime behavior and tunnel factory remain intact. | `vpn-server` focused tests and compile checks pass, and all PN control consumers use the dedicated client type. | No unrelated server lifecycle, persistence, API, or configuration changes. |

## Success Criteria
- Concrete user-visible or system-visible result: PN nodes communicate with control-plane nodes through the purpose-specific `vpn_frame::server::VpnControlClient`; the general `VpnServerClient` no longer implements `VpnControlClientOps` or owns PN control commands.
- Required evidence: source-level ownership scan, focused control-channel tests, successful `vpn-frame` and `vpn-server` compilation/test commands, and post-implementation acceptance against this proposal.
- Explicit non-goals: wire-format changes, behavior changes, generalized transport-framework refactoring, or changes outside the two affected Rust crates.

## Risks
- Moving public methods/types can affect downstream compile consumers even when wire behavior is unchanged; direct workspace consumers must be included in testing.
- Exporting a client from `vpn_frame::server` makes the server-side responsibility explicit, but requires checking that the module remains dependency-direction compatible with the control-channel trait and shared protocol types.
- Accidental changes to sequence generation, command version, timeout use, result-code handling, or error conversion could alter runtime behavior and require focused comparison/tests.
- The working tree already contains unrelated tracked and untracked user files; task evidence and edits must remain isolated from them.
