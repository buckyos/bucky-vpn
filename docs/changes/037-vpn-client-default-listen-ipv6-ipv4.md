# Vpn-client 默认同时监听 IPv4 与 IPv6

- Status: complete
- Task manifest: docs/versions/v0.1/modules/bucky-vpn/037-vpn-client-default-listen-ipv6-ipv4/task.yaml
- Approved proposal: docs/versions/v0.1/modules/bucky-vpn/037-vpn-client-default-listen-ipv6-ipv4/proposal.md
- Affected paths: vpn-client/src/cli.rs, vpn-client/src/main.rs

## Approach
Change `vpn-client`'s default p2p listen addresses from IPv4-only wildcard to a dual-stack wildcard list so the client listens on both IPv4 (`0.0.0.0`) and IPv6 (`::`) out of the box. `DEFAULT_LISTEN_IP` becomes `"0.0.0.0, ::"`; the existing `parse_listen_ips` list parser, per-family wildcard limit, command-line/env/config overrides, and `p2p_listen_endpoints` address-to-EndPoint mapping all remain unchanged. Tests that asserted the old IPv4-only default are updated to assert the dual-stack default.

## Risk Screen
The change is bounded to the `bucky-vpn` vpn-client default listen addresses and the two unit tests that depend on the default. Existing operators with an explicit `p2p.listen_ip` are unaffected because the default only applies when the setting is absent. Residual risk is limited to hosts without IPv6 support, where binding `::` may fail; the existing `ServerRuntime` failure path reports it and operators can still override `p2p.listen_ip`. No schema, protocol, dependency, security boundary, or release surface changes, so the user-confirmed standard tier is appropriate.

## Implementation Evidence
- `vpn-client/src/cli.rs` `DEFAULT_LISTEN_IP` changed from `"0.0.0.0"` to `"0.0.0.0, ::"`.
- `vpn-client/src/cli.rs` test `listen_ip_defaults_to_ipv4_wildcard` renamed/updated to assert the default parses to both IPv4 and IPv6 unspecified addresses.
- `vpn-client/src/main.rs` test `p2p_listen_endpoints_register_quic_and_tcp` updated to assert the default produces four endpoints (Quic+Tcp on IPv4 wildcard, Quic+Tcp on IPv6 wildcard) instead of two.

## Verification
- Targeted check: `cargo test -p vpn-client --locked`.
- Result: pass
- Targeted check detail: unit tests covering the dual-stack default parse and endpoint generation pass, including the pre-existing IPv6/mixed-list handling.
- Diff hygiene: `git diff --check` passed.
- Defect-discovery conclusion: fresh code-path, parser, endpoint-mapping, and regression review found no blocking issue; explicit-config override behavior and `MAX_LISTEN_IPS`/per-family wildcard guards are untouched.
- Residual risk or follow-up: hosts without IPv6 will fail when binding the `::` wildcard endpoint; this is reported by the existing runtime path and overridable via `VPN_P2P_LISTEN_IP`.