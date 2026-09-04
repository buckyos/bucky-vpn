# Enable SN NAT Type Probe

- Status: complete
- Task manifest: docs/versions/v0.1/modules/bucky-vpn-server/034-enable-sn-nat-type-probe/task.yaml
- Approved proposal: docs/versions/v0.1/modules/bucky-vpn-server/034-enable-sn-nat-type-probe/proposal.md
- Affected paths: vpn-server/src/server_config.rs, vpn-server/src/main.rs, vpn-server/config/config.example.yaml, vpn-server/tests/unit/pn_transport_mode_tests.rs

## Approach
Add `sn.nat_probe_ports` to `SnServerConfig` and treat an absent or empty list as disabled. Parse YAML arrays and the comma-separated `VPN_SN_NAT_PROBE_PORTS` environment override, then reject invalid cardinality, zero, duplicates, non-port values, and use while `sn.enabled` is false. When enabled, convert the existing QUIC/TCP service endpoints into the mapped-public plus wildcard-local pairs required by `p2p-frame`, without feeding those identity-only pairs into the PN reporting path. Pass the validated ports to `SnServiceConfig::set_nat_probe_ports` and document the public IPv4 plus UDP forwarding requirements.

## Risk Screen
The change is opt-in and local to `bucky-vpn-server`; the default empty port list leaves current startup behavior unchanged. It does not alter the NAT probe wire protocol, scheduling, rate limits, signatures, persistence, authentication, or dependency declarations. The material operational risks are configuration mistakes and exposing additional UDP ports. Those fail closed for invalid local values and are documented, while actual firewall/NAT reachability remains deployment evidence rather than a local-test claim.

## Implementation Evidence
- `get_sn_server_config` parses and validates `sn.nat_probe_ports`; because the repository's environment separator expands underscores into path segments, it gives the environment-expanded `sn.nat.probe.ports` value precedence over the YAML key.
- `validate_server_mode` rejects configured probe ports when the embedded SN is disabled, including the PN-disabled branch.
- `resolve_sn_identity_endpoints` rejects IPv6 and non-public IPv4 addresses, then emits each mapped endpoint immediately followed by a protocol-compatible wildcard local endpoint.
- `main` keeps ordinary `eps` for PN reporting, uses the derived endpoint pairs only for the local identity/SN listener, and forwards the ports with `set_nat_probe_ports`.
- The example configuration keeps the feature commented out and explains the 2–8 port range, public `ip`, and firewall/container/NAT forwarding requirements.

## Verification
- Targeted check: `cargo test -p bucky-vpn-server --locked sn_nat_probe -- --test-threads=1`.
- Result: passed
- Targeted test detail: 6 passed, 0 failed, 74 filtered out.
- Affected-crate compile: `cargo check -p bucky-vpn-server --all-targets --locked` passed with existing dead-code warnings only.
- Full serial crate test: `cargo test -p bucky-vpn-server --locked -- --test-threads=1` completed with 79 passed and 1 failed. Every new NAT probe test and all neighboring configuration/transport tests passed. The sole failure is the unchanged `sqlite_store_factory::tests::node_traffic_record_rolls_back_and_retries_idempotently`: current `HEAD` defines `DEFAULT_NODE_TRAFFIC_SPEED_TTL` as 15 seconds while that test requires at least 170 seconds remaining. This source-level contradiction and test are outside the task diff.
- The initially parallel full run had one additional short-TTL heartbeat failure; `cargo test -p bucky-vpn-server --locked dedicated_heartbeat_controls_remote_online_state -- --test-threads=1` passed on isolated rerun, and the same test passed in the final serial full run.
- Diff hygiene: `git diff --check` passed.
- Runtime evidence limitation: no live public-NAT/firewall test was run; local tests prove parsing, validation, endpoint structure, and compile-time upstream API closure, not external reachability.
- Residual risk or follow-up: Operators must choose and expose 2–8 UDP ports and configure top-level `ip` with the externally reachable IPv4. A wrong but syntactically valid public address cannot be detected locally. The unrelated traffic-cache test inconsistency remains pre-existing and was not changed under this task.
