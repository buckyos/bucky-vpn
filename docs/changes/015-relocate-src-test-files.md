# Relocate Independent Test Files Outside src

- Status: complete
- Task manifest: docs/versions/v0.1/modules/globals/015-relocate-src-test-files/task.yaml
- Approved proposal: docs/versions/v0.1/modules/globals/015-relocate-src-test-files/proposal.md
- Affected paths: vpn-client/src/p2p_vpn.rs, vpn-client/src/p2p_vpn_pn_registry_tests.rs, vpn-client/tests/unit/p2p_vpn_pn_registry_tests.rs, vpn-frame/src/client/vpn_client.rs, vpn-frame/src/client/vpn_client_restart_tests.rs, vpn-frame/tests/unit/vpn_client_restart_tests.rs, vpn-server/src/pn_control_client.rs, vpn-server/src/pn_control_client_tests.rs, vpn-server/src/pn_traffic_service.rs, vpn-server/src/pn_traffic_service_tests.rs, vpn-server/tests/unit/pn_control_client_tests.rs, vpn-server/tests/unit/pn_traffic_service_tests.rs

## Approach
Move the four independent Rust test files from crate `src` trees into sibling `tests/unit` directories while retaining their existing `#[cfg(test)]` module compilation context through adjusted `include!` and `#[path]` references. The `unit` subdirectory prevents Cargo from independently compiling these private-context files as top-level integration crates. Update the one file-relative `include_str!` source reference affected by the move. Do not alter test behavior, production visibility, crate targets, or historical task records.

## Risk Screen
The change spans three crates but is limited to test-only compilation paths. Preserving the original module context avoids public API, binary/library target, protocol, persistence, runtime, dependency, and release impact. The main failure risk is an incorrect relative path or lost file content, covered by focused tests and final path/content review.

## Verification
- Targeted check: `cargo test -p bucky-vpn p2p_vpn::tests` (17 passed); `cargo test -p vpn-frame client::vpn_client::tests` (5 passed); `cargo test -p bucky-vpn-server pn_control_client_tests` (2 passed); `cargo test -p bucky-vpn-server pn_traffic_service::node_traffic_tests` (16 passed); `rg --files -g '**/src/**/*test*.rs'` returned no paths; focused content comparison and `git diff --check`
- Result: passed
- Residual risk or follow-up: The migrated files remain internal unit-test modules loaded from `tests/unit` rather than independent Cargo integration crates, preserving private access while satisfying the repository directory rule. Existing historical task records retain their original evidence paths by design.
