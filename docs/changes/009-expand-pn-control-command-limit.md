# Expand PN Control Command Limit

- Status: complete
- Task manifest: docs/versions/v0.1/modules/globals/009-expand-pn-control-command-limit/task.yaml
- Approved proposal: docs/versions/v0.1/modules/globals/009-expand-pn-control-command-limit/proposal.md
- Affected paths: vpn-frame/src/proxy_node.rs, vpn-frame/src/server/vpn_control_client.rs, vpn-frame/src/server/pn_control_server.rs, vpn-frame/src/server/vpn_server.rs, vpn-frame/tests/vpn_control_client_tests.rs, vpn-server/src/pn_control_client.rs, vpn-server/src/pn_control_server.rs, vpn-server/src/pn_traffic_service_tests.rs, vpn-server/src/server_config.rs, vpn-server/config/config.example.yaml

## Approach
Give the dedicated PN control channel a shared `U24<{ 10 * 1024 * 1024 }>` length/header contract on both endpoints, split the shared `VpnServer` generic bounds so its ordinary and PN-control command services retain their distinct U16/U24 contracts, align the server's traffic-command byte limit to 10 MiB, raise the validated record ceiling and uploader default/example chunk size to 25,000, and leave the ordinary VPN command channel on `VpnCmdPkgLen = U16`.

## Risk Screen
The user selected the standard tier despite a wire-format compatibility boundary and increased default batch size. PN and control-plane endpoints must be upgraded together. The 10 MiB transport/body checks and 25,000-record ceiling bound individual commands; four default concurrent worst-case commands can still create about 36.2 MiB of serialized request bodies plus decode/database overhead.

## Verification
- Targeted check: `cargo check -p vpn-frame -p bucky-vpn-server`; `cargo test -p vpn-frame`; `cargo test -p bucky-vpn-server server_config::tests::sn_and_pn_default_enabled_without_config_file`; `cargo test -p bucky-vpn-server pn_control_client_tests`; `cargo test -p bucky-vpn-server pn_traffic_service::node_traffic_tests`; focused `rustfmt --check`; focused `git diff --check`; protocol-type source scan
- Result: passed
- Residual risk or follow-up: Mixed old/new PN-control endpoint deployment remains unsupported. The user-selected standard tier omits a full staged compatibility rollout exercise; direct type scans, cross-crate compilation, boundary tests, and concrete client/server tests mitigate but do not remove that deployment requirement. Four maximum-size commands may place roughly 36.2 MiB of serialized requests in flight before decode/database overhead.
