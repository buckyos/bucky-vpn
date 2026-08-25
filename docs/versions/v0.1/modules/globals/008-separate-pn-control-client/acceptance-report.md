# Separate PN Control Client Acceptance Report

## Findings
| ID | Severity | Kind | Evidence | Problem | Blocking |
|----|----------|------|----------|---------|----------|
| F-001 | none | implementation | task production diff, dedicated behavior tests, external API contracts, consumer scan, and locked workspace compile closure | No requirement, implementation, design-consistency, or testing-consistency defect was found in the delivered client-boundary refactor. | no |
| F-002 | low | testing-consistency | `testplan.yaml` and the successful task-scoped run artifact | The new client is exercised through a deterministic command-client fake and the concrete server assembly is compile-checked, but no live PN-to-control-plane network process pair was started. Existing tunnel construction is unchanged, so this remains residual integration risk rather than an acceptance blocker. | no |

## Requirement Review
| Requirement or Boundary | Source | Implementation Evidence | Finding | Status |
|-------------------------|--------|-------------------------|---------|--------|
| Define the concrete client under `vpn_frame::server`. | `proposal.md` P-001 and the user's confirmed location revision | `vpn-frame/src/server/vpn_control_client.rs` defines `VpnControlClient`; `vpn-frame/src/server/mod.rs` declares and publicly re-exports it. | The delivered public path is exactly `vpn_frame::server::VpnControlClient`, and the abstract `control_channel.rs` no longer owns a concrete client implementation. | pass |
| Make the dedicated client, not `VpnServerClient`, implement `VpnControlClientOps`. | `proposal.md` P-001 | The only production generic implementation is in `server/vpn_control_client.rs`; the former implementation and its command-generic imports were removed from `control_channel.rs`. | Trait ownership now matches the PN-to-control-plane responsibility. | pass |
| Remove PN control-plane commands from the ordinary VPN server client. | `proposal.md` P-001 and non-goals | `vpn_server_client.rs` retains join, VPN-info, and node-query methods while the four report/validate methods and their payload imports are absent; the external negative fixture proves the removed methods no longer compile. | The general client no longer exposes the specialized control surface, without changing its remaining ordinary client behavior. | pass |
| Migrate the PN runtime to the dedicated type without changing tunnel or validation behavior. | `proposal.md` P-002 | `vpn-server/src/pn_control_client.rs` aliases the concrete transport to `vpn_frame::server::VpnControlClient` and constructs it with the existing `ControlCmdClient`, factory, concurrency value, and timeout; reporter and validator injection remain trait-object based. | The concrete cross-crate consumer is migrated and the existing tunnel factory, heartbeat, traffic, and validation wrappers are unchanged. | pass |
| Preserve command codes, payloads, version, timeout, sequence, result, and error semantics. | `proposal.md` Scope and Success Criteria | The four method bodies moved to the dedicated type with the same request/response types, `VPN_CMD_VERSION`, `SequenceGenerator`, timeout, result checks, and error conversions; four focused tests cover success, non-zero results, validation boundaries, transport failure, and decode failure. | The move changes ownership and API location, not wire or runtime semantics. | pass |

## Implementation Review
| Area | Evidence | Finding | Status |
|------|----------|---------|--------|
| Dedicated state owner | `vpn-frame/src/server/vpn_control_client.rs` | The new type exclusively owns the command client, version, timeout, sequence generator, and generic transport lifetime marker described by the plan; no new retry, background task, or shared mutable lifecycle was introduced. | pass |
| Abstract control boundary | `vpn-frame/src/control_channel.rs` | The file retains only `VpnControlClientOps`, its trait-object reference, reporter/validator cores, remote validation helper, adapter, and result conversion; concrete command transport bounds are gone. | pass |
| Ordinary client boundary | `vpn-frame/src/client/vpn_server_client.rs` | The four PN control methods and their request/response dependencies were removed, while existing join/query/info paths are untouched. | pass |
| Server assembly migration | `vpn-server/src/pn_control_client.rs` | The concrete alias and constructor now use the server-module client; every reporter and validator still receives the same `VpnControlClientOpsRef` abstraction. | pass |
| Public API migration closure | external positive/negative fixtures and `consumer-closure-check.py` | The new path compiles externally, all four old type-qualified methods are rejected, no unallowlisted old consumer remains, and every locked workspace target compiles. | pass |
| Focused behavior | `vpn-frame/tests/vpn_control_client_tests.rs` and existing `vpn-server` PN control tests | Four new tests cover all command forwarding and error/result branches; two existing validator tests preserve accepted/rejected cross-module behavior. | pass |

## Document Consistency
| Document | Source | Implementation Consistency | Finding | Status |
|----------|--------|----------------------------|---------|--------|
| design | `pipeline/plan.md` | Files were implemented in the planned add/export/migrate/remove order; the public interface, breaking migration, single state owner, failure propagation, rejected common-base alternative, and two target-module bindings match the delivered code. | The implementation follows the automatic design mapping without adding an unapproved abstraction or changing the stated runtime boundary. | pass |
| testing | `testplan.yaml` | The successful task run executes all four required breaking-API contract kinds plus the declared unit, DV, and integration steps, with direct evidence for both change ids and no disabled automated level. | Testing matches the planned ownership, behavior, compatibility, and cross-module validation intent. | pass |

## Result Summary
- Overall result: accepted
- Outcome: PN nodes now use the purpose-specific `vpn_frame::server::VpnControlClient` for control-plane communication, while `VpnServerClient` is limited to ordinary VPN client operations.
- Blocking issues: none in the requirement, implementation, design-consistency, or testing-consistency review.
- Next action: complete the automatic pipeline state and remove the task from the unfinished-task index.

## Object and Scope
- Task manifest: task.yaml
- Reviewed changes: `CHG-dedicated-pn-control-client`, `CHG-pn-control-client-integration`
- In scope: the launch-confirmed proposal, automatic design mapping, five production files, focused client tests, external compatibility fixtures, task testplan, successful task-scoped run, and cross-crate consumer closure.
- Out of scope: unrelated dirty-worktree files, VPN protocol payload redesign, live multi-process deployment testing, ordinary client refactoring, Flutter Web UI, persistence, configuration, and packaging.

## Conclusion
- Accepted / rejected / needs changes: accepted
- Reason: The implementation places the concrete class in `vpn_frame::server`, transfers all four control operations and their state to it, removes the incorrect `VpnServerClient` ownership, migrates the concrete PN server consumer, preserves command/runtime semantics, and passes every declared task-scoped verification with no blocking finding.
- Residual risk: No live multi-process PN/control-plane network scenario was added; confidence in that layer relies on unchanged tunnel assembly, concrete workspace compilation, preserved existing validator tests, and direct command-client behavior coverage.
