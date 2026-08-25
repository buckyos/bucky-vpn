# Completion Report

## Object and Scope
- Task manifest: task.yaml
- Workflow tier: standard
- Change record: docs/changes/032-pn-control-dual-prefer-quic.md

## Delivery Summary
- Outcome: Restored `ControlCmdTunnelFactory` to the original concrete `TtpClientRef`, single `control_endpoint`, and direct `open_cmd_tunnel` flow. The only transport adaptation in `pn_control_client.rs` is selecting the first configured control endpoint; established ordering yields TCP for `tcp`, QUIC for `quic`, and QUIC for `dual`.
- Handoff: No further code action is required. In dual mode, outbound control remains QUIC-only by design; operators that require outbound TCP select `pn.transport: tcp`.
- Removed scope: The upper-layer fallback loop, active-target tracking, target cleanup/switching, creation lock, injectable TTP adapter, and their dedicated async fake tests are gone.
- Preserved behavior: The three-mode parser, listener endpoint set, PN report endpoints, primary endpoint, and port-mapping filtering from task 031 remain unchanged. The example now states that dual control uses QUIC.

## Proposal Consistency
| change_id | requirement_or_boundary | proposal_source | delivery_evidence | finding | status |
|-----------|-------------------------|----------------|-------------------|---------|--------|
| CHG-simplify-pn-control-transport-selection | Restore the single-endpoint PN control factory; use TCP, QUIC, and QUIC for `tcp`, `quic`, and `dual`, without upper-layer TCP fallback | `proposal.md` P-001, Scope, Boundary, and Success Criteria | Current `pn_control_client.rs` matches the pre-change factory except for checked selection of `control_server.endpoints.first()`; `PnTransportMode::endpoints` orders dual as QUIC then TCP; removed-symbol search is empty | Delivery matches the approved minimal restoration and does not extend the control-client state machine | pass |

## Implementation Review
| area | evidence | finding | status |
|------|----------|---------|--------|
| Requirement and behavior | Current `PnTransportMode::endpoints` and `create_vpn_control_client` call path | `tcp` provides one TCP endpoint, `quic` one QUIC endpoint, and `dual` QUIC then TCP; selecting the first endpoint therefore gives the approved outbound control protocol in every mode | pass |
| Logic and control flow | Comparison of current `ControlCmdTunnelFactory` with `HEAD:vpn-server/src/pn_control_client.rs` | The original one-target `connect_server` then `open_control_stream` flow is restored. No fallback, target-switch, cleanup, or additional synchronization branch remains | pass |
| Boundaries and input | `get_control_server_config_at` always constructs the endpoint vector through `PnTransportMode::endpoints`; `create_vpn_control_client` also rejects an empty vector | Normal configuration cannot produce an empty endpoint set, while a manually constructed invalid config returns `InvalidParam` instead of panicking | pass |
| Failure and recovery | Original `connect_server` and `open_control_stream` error mapping remains intact | Failure behavior is restored to the existing control-client retry mechanism. Dual intentionally does not introduce TCP fallback; operators can select `transport: tcp` when required | pass |
| Listener/report side effects | `resolve_service_endpoints` still consumes the full transport endpoint set; focused tests assert listener, primary, report, and mapping results | Selecting one endpoint is confined to outbound control-client construction and does not narrow dual listener or published endpoints | pass |
| Test adequacy and regression | Six focused `pn_transport_` tests plus all-target compile | Tests exercise all three modes, order/default/invalid boundaries, service/report/mapping effects, example parsing, and combined-mode restrictions; the affected crate and tests compile | pass |
| Scope and dirty-worktree isolation | Task-start baseline and current targeted diff | The restoration touches only the approved production file, test registration file, and example. Existing task-031 changes in `server_config.rs` and `main.rs` remain intact | pass |

## Verification
- Targeted check: `cargo test -p bucky-vpn-server pn_transport_ --locked`
- Result: passed
- Test count: 6 passed, 0 failed, 68 filtered out.
- Targeted compile: `cargo check -p bucky-vpn-server --all-targets --locked` - passed with only existing dead-code warnings.
- Removed-symbol search: no upper-layer fallback, active-target, creation-lock, or TTP adapter symbols remain in the affected control source/test files.
- Diff hygiene: `git diff --check` passed.
- Exception reason: No exception.

## Findings
| id | severity | evidence | problem | blocking |
|----|----------|----------|---------|----------|
| F-001 | none | Direct source-path review, pre-change comparison, focused tests, compile result, removed-symbol search, and task baseline | No requirement mismatch, control-flow defect, listener/report regression, stale runtime fallback claim, or task-scope leak was found | no |

## Conclusion
- Accepted / rejected / needs changes: accepted
- Reason: The control client is restored to the old single-endpoint design, the only retained production adaptation selects QUIC first for dual, the three-mode listener/report behavior remains intact, and the independent defect review found no blocking issue.
