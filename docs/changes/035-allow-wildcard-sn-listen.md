# Allow Wildcard SN Listen Address for NAT Probing

- Status: complete
- Task manifest: docs/versions/v0.1/modules/bucky-vpn-server/035-allow-wildcard-sn-listen/task.yaml
- Approved proposal: docs/versions/v0.1/modules/bucky-vpn-server/035-allow-wildcard-sn-listen/proposal.md
- Affected paths: vpn-server/src/server_config.rs, vpn-server/src/main.rs, vpn-server/config/config.example.yaml, Cargo.lock

## Approach
The locked `p2p-frame` NAT probe contract now carries only UDP ports on `NatProbeDirective` and `ReportSnResp`, binds every reflector to `0.0.0.0:<port>`, and lets clients rebuild complete probe endpoints from the authenticated QUIC SN IPv4 plus the delivered ports. `bucky-vpn-server` therefore keeps the SN identity endpoints identical to the ordinary service endpoints regardless of `sn.nat_probe_ports`, removes the public-IPv4 guard and the obsolete mapped/public plus wildcard/local pair construction from `resolve_sn_identity_endpoints`, and updates the example configuration so operators can keep top-level `ip: "0.0.0.0"`.

## Risk Screen
The change is local to `bucky-vpn-server` configuration validation and startup assembly, and the default empty `sn.nat_probe_ports` behavior is unchanged. It removes a guard that was required only by the older endpoint-bearing wire. The remaining operational risk is unchanged: configured UDP probe ports must still be opened or forwarded on the deployment, and local tests cannot prove external reachability.

## Implementation Evidence
- `resolve_sn_identity_endpoints` now returns the ordinary service endpoints directly; it no longer requires a concrete public IPv4 and no longer emits `EndpointArea::Mapped`/`Lan` pairs.
- `main` calls `resolve_sn_identity_endpoints(&eps)` and continues forwarding `sn.nat_probe_ports` to `SnServiceConfig::set_nat_probe_ports`; the local identity carries the wildcard listen endpoints when configured.
- `config.example.yaml` documents that top-level `ip` may be `0.0.0.0` and still separately requires firewall/container/NAT forwarding for the reflector ports.
- The user-owned `Cargo.lock` change to `git+https://github.com/buckyos/bucky-p2p.git#9b7e2d627de33e1b7984fdbfc227df904d31b46f` is preserved as the required dependency context.

## Verification
- Targeted check: `cargo test -p bucky-vpn-server --locked sn_nat_probe -- --test-threads=1`.
- Result: pass
- Targeted check detail: 5 passed, 0 failed, 74 filtered out. The added tests prove a wildcard top-level `ip` is accepted while `sn.nat_probe_ports` is set and that identity endpoints remain the ordinary service endpoints.
- Config regression suite: `cargo test -p bucky-vpn-server --locked server_config -- --test-threads=1` passed with 24 passed, 0 failed, 55 filtered out.
- Affected-crate compile: `cargo check -p bucky-vpn-server --all-targets --locked` passed with only existing dead-code warnings; the new `p2p-frame` API closes over `SnServiceConfig::set_nat_probe_ports`.
- Full serial crate check: 78 passed, 1 failed. The sole failure is the unchanged `sqlite_store_factory::tests::node_traffic_record_rolls_back_and_retries_idempotently` TTL assertion already recorded by task 034 (`DEFAULT_NODE_TRAFFIC_SPEED_TTL` is 15 seconds while the test requires at least 170 seconds remaining); it is outside this task and was not modified.
- Diff hygiene: `git diff --check` passed.
- Defect-discovery conclusion: fresh source-path, upstream API, boundary, regression, and full-suite review found no blocking issue; removed-helper search confirms no stale `resolve_sn_identity_endpoints` caller remains outside `server_config.rs`/`main.rs` test closure.
- Residual risk or follow-up: operators must expose the configured UDP ports; local tests prove configuration/identity behavior and upstream API closure, not public firewall reachability.
