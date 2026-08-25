# sfo-cmd-server 0.4 Compile Migration Acceptance Report

## Findings
| ID | Severity | Kind | Evidence | Problem | Blocking |
|----|----------|------|----------|---------|----------|
| F-001 | none | implementation | `vpn-frame`, `vpn-client`, and `vpn-server` task diff; removed-symbol scan; successful locked workspace checks | No requirement, implementation, design-consistency, or testing-consistency defect was found in the delivered package-length type migration. | no |
| F-002 | low | testing-consistency | `testplan.yaml` and `.harness/test-results/test-runs/20260721T083945Z-vpn-frame+007-fix-sfo-cmd-server-0-4-compile-all.json` | Verification proves source compatibility, unit behavior, and workspace compile closure, but does not add a live client/server interoperability scenario specifically for the dependency upgrade. The unchanged two-byte codec and range make this a residual risk rather than an acceptance blocker. | no |

## Requirement Review
| Requirement or Boundary | Source | Implementation Evidence | Finding | Status |
|-------------------------|--------|-------------------------|---------|--------|
| Build the Rust workspace with `sfo-cmd-server 0.4.0`. | `proposal.md` P-001 and Success Criteria | `vpn-frame/Cargo.toml` selects `sfo-cmd-server = "0.4"`; `Cargo.lock` resolves `0.4.0`; the locked all-target workspace test/check steps exited 0. | The dependency update remains in place and every workspace target now compiles against it. | pass |
| Replace every command package-length use rejected by the new `CmdPkgLen` bound. | `proposal.md` P-001 and Scope | `VpnCmdPkgLen = sfo_cmd_server::U16` is defined in `vpn_protocol.rs`; all affected `CmdHeader`, `CmdClient`, `CmdServer`, classified transport, service, and handler generics use it; the removed-symbol scan exited 0. | The migration covers the owning crate and all explicit downstream generic consumers exposed by compile closure. | pass |
| Preserve command framing and VPN control-channel behavior. | `proposal.md` P-001, Requirement Review, and non-goals | `sfo_cmd_server::U16` delegates `RawEncode`, `RawDecode`, and `RawFixedBytes` to `u16`, with an effective maximum of `u16::MAX`; command IDs, versions, payload fields, authentication, and tunnel lifecycle code are unchanged. | The selected wrapper is wire-compatible with the previous two-byte length field and retains its range; ordinary payload/version `u16` values were not mechanically replaced. | pass |
| Keep production edits limited to dependency-affected Rust call sites, with downstream changes only when compilation proves them necessary. | `proposal.md` Scope and Boundary with neighboring modules | Production edits are confined to the dependency declaration and command generic aliases/bounds in `vpn-frame`, `vpn-client`, and `vpn-server`; the downstream edits correspond to compiler-exposed concrete consumers. | The expanded consumer edits are within the proposal's explicit compatibility exception and contain no unrelated refactor, UI, or packaging change. | pass |

## Implementation Review
| Area | Evidence | Finding | Status |
|------|----------|---------|--------|
| Shared protocol owner | `vpn-frame/src/vpn_protocol.rs` | `VpnCmdPkgLen` centralizes the dependency type and `VpnCmdHeader` consumes the alias, preventing client/server type drift while retaining the crate-root re-export path. | pass |
| vpn-frame generic bounds | `control_channel.rs`, client adapters, `pn_control_server.rs`, and `vpn_server.rs` | All package-length bounds use `VpnCmdPkgLen`; the existing header-construction test now supplies the wrapper explicitly without changing the tested version-gating behavior. | pass |
| Downstream consumers | `vpn-client/src/p2p_vpn.rs`, `vpn-server/src/pn_control_client.rs`, `vpn-server/src/pn_control_server.rs`, and `vpn-server/src/sqlite_store_factory.rs` | Concrete classified transports, command services, and delegated handler signatures use the same exported alias. No stale raw `u16` command generic remains in the reviewed crates. | pass |
| Wire compatibility | `sfo-cmd-server 0.4.0` `cmd.rs` implementation and the selected `U16` alias | The wrapper's fixed width, encoding, and decoding are inherited from `u16`; its default limit is capped at 65535. The migration changes the Rust type contract, not serialized bytes or accepted package-length range. | pass |
| Verification | Successful task-scoped run artifact | External public-path compilation, old-symbol closure, locked no-run workspace tests, vpn-frame library tests, vpn-frame all-target checks, and workspace all-target checks all exited 0; no declared level was skipped. | pass |

## Document Consistency
| Document | Source | Implementation Consistency | Finding | Status |
|----------|--------|----------------------------|---------|--------|
| design | `pipeline/plan.md` | The shared alias was introduced first, all planned vpn-frame and downstream consumer paths use it, the dependency remains at 0.4, and no protocol-semantic alternative was introduced. | The implementation follows the automatic design mapping, consumer closure, state ownership, failure-flow handling, and rejected-alternative decisions. | pass |
| testing | `testplan.yaml` | The successful artifact executes all three contract checks plus the declared unit, DV, and integration steps, with `CHG-sfo-cmd-server-0-4-api-migration` bound to every step and no non-executed level. | Testing matches the compile-migration intent. Live cross-version interoperability remains an explicit residual gap, but the approved proposal permits source-level compatibility evidence and compile closure for this mechanical adaptation. | pass |

## Result Summary
- Overall result: accepted
- Outcome: The workspace compiles with `sfo-cmd-server 0.4.0`, and the VPN command package-length integration now uses the supported wire-compatible `U16` wrapper across vpn-frame and its explicit client/server consumers.
- Blocking issues: none in the requirement, implementation, design-consistency, or testing-consistency review.
- Next action: complete the automatic pipeline lifecycle closure.

## Object and Scope
- Task manifest: task.yaml
- Reviewed change: `CHG-sfo-cmd-server-0-4-api-migration`
- In scope: the launch-confirmed proposal, automatic design mapping, dependency and lock resolution, migrated vpn-frame interfaces, downstream vpn-client/vpn-server generic consumers, external fixture, testplan, and successful task-scoped run artifact.
- Out of scope: unrelated dirty-worktree changes, Flutter Web UI, packaging, new VPN behavior, dependency rollback, and unrelated refactoring.

## Conclusion
- Accepted / rejected / needs changes: accepted
- Reason: The implementation satisfies the approved dependency-migration requirement, preserves the existing two-byte command length framing, closes all reviewed command generic consumers, follows the pipeline design and testplan, and passes every declared task-scoped verification step with no blocking finding.
- Residual risk: No new live client/server or cross-version interoperability test was added; behavioral confidence therefore combines the unchanged codec/range implementation, existing vpn-frame unit tests, removed-symbol closure, and full locked workspace compilation.
