# Restore Minimal PN Control Client

- Status: complete
- Task manifest: docs/versions/v0.1/modules/bucky-vpn-server/032-pn-control-dual-prefer-quic/task.yaml
- Approved proposal: docs/versions/v0.1/modules/bucky-vpn-server/032-pn-control-dual-prefer-quic/proposal.md
- Affected paths: vpn-server/src/pn_control_client.rs, vpn-server/tests/unit/pn_control_client_tests.rs, vpn-server/config/config.example.yaml

## Approach
Restore `ControlCmdTunnelFactory` to the pre-change concrete `TtpClientRef` and single-endpoint flow. Keep only the transport-required adaptation at client construction: select the first configured control endpoint, whose established ordering is TCP for `tcp`, QUIC for `quic`, and QUIC first for `dual`. Remove the upper-layer fallback, maintained-target state, serialization, test adapter, and tests dedicated only to those removed mechanisms. Preserve all listener, publication, primary, mapping, and configuration behavior delivered by task 031.

## Risk Screen
This is a bounded internal restoration with no public API, wire, dependency, persistence, security, or release change. The main risk is accidentally removing the three-mode configuration behavior or leaving documentation/tests that still claim dual control fallback. The task-start baseline preserves the existing dirty worktree so completion can distinguish this restoration from prior task 031 changes.

## Verification
- Targeted check: `cargo test -p bucky-vpn-server pn_transport_ --locked`.
- Result: passed
- Test detail: 6 tests passed, 0 failed, 68 filtered out. The cases cover exact `tcp`/`quic`/`dual` parsing, the `dual` QUIC-first endpoint order, defaulting, invalid values, listener/report/mapping behavior, and the SN-only/combined-mode boundary.
- Compile check: `cargo check -p bucky-vpn-server --all-targets --locked` passed with only existing dead-code warnings.
- Removal check: no `ControlTtpClientOps`, `DefaultControlTtpClientOps`, `active_target`, `create_lock`, `create_unclassified_tunnel`, `create_classified_tunnel`, or `same_maintained_target` symbol remains in the affected control source/test files.
- Diff hygiene: `git diff --check` passed.
- Residual risk or follow-up: In `dual`, the outbound PN control connection selects QUIC and does not fall back to TCP. Deployments that require a TCP control connection must configure `pn.transport: tcp`; dual TCP remains available for inbound listener/publication behavior.
