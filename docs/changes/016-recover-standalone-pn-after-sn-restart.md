# Recover Standalone PN After SN Restart

- Status: in progress
- Task manifest: docs/versions/v0.1/modules/globals/016-recover-standalone-pn-after-sn-restart/task.yaml
- Approved proposal: docs/versions/v0.1/modules/globals/016-recover-standalone-pn-after-sn-restart/proposal.md
- Affected paths: vpn-frame/src/proxy_node.rs, vpn-frame/src/server/node_pn_manager.rs, vpn-frame/src/server/pn_control_server.rs, vpn-frame/src/server/vpn_server.rs, vpn-frame/src/vpn_protocol.rs, vpn-frame/src/client/vpn_server_client.rs, vpn-frame/src/client/vpn_client.rs, vpn-frame/tests/unit/node_pn_manager_tests.rs, vpn-frame/tests/unit/vpn_client_restart_tests.rs, vpn-frame/tests/pn_version_protocol_contract.rs, vpn-frame/tests/tun_recovery_contract.rs, tests/fixtures/pn_info_version_consumer, tests/pn_info_version_u16_negative_contract.py, tests/integration/pn_sn_restart_process.py

## Approach
Replace the unfinished `u64` millisecond PN version with a `u32` Unix-second contract. Initialize vpn-client's PN cache to zero, capture one server default when SN starts, use that startup value for newly created per-client PN states, and use the current Unix second only when canonical PN assignment content actually changes. Keep network `Node.info_version` and its `u16` persistence/wire path unchanged, retain equality-only comparison and commit-after-success ordering, and identify the final wire contract through the VPN command protocol version.

## Risk Screen
The user selected the standard tier despite a breaking command payload change and cross-process restart behavior. Old `u16` counter binaries and the unfinished `u64` millisecond build cannot be mixed with the final `u32` build. Raw seconds intentionally allow identical values for starts or changes within one second, clock rollback can decrease a later value, and Unix seconds exceed `u32` in 2106. Synchronization therefore remains equality-only and deployment must upgrade SN/PN/client peers coherently.

## Verification
- Targeted check: `cargo test -p vpn-frame` (28 unit, 3 protocol, 9 TUN/retry contracts, and 4 control-client tests passed); `cargo check -p vpn-frame -p bucky-vpn -p bucky-vpn-server --all-targets`; positive final-u32 external consumer; negative legacy-u16 and unfinished-u64 consumers; Python integration-script compile; focused source/width/default scan; focused `git diff --check`
- Result: deterministic, contract, consumer, and affected-crate compile checks passed; the required live SN-only restart scenario is blocked before execution because `multipass list --format json` times out after 35 seconds against the externally stuck Windows Multipass daemon.
- Residual risk or follow-up: Do not mark the change record complete or close the task until Multipass is restored and the final u32 build records a successful live chain from the new SN startup-second version through each unchanged client process to recovered PN-forced traffic.
