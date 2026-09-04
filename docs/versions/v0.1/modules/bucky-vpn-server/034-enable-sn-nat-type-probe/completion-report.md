# Completion Report

## Object and Scope
- Task manifest: task.yaml
- Workflow tier: standard
- Change record: docs/changes/034-enable-sn-nat-type-probe.md

## Delivery Summary
- Outcome: `bucky-vpn-server` can now opt its embedded SN into the locked `p2p-frame` NAT type probe service through `sn.nat_probe_ports` or `VPN_SN_NAT_PROBE_PORTS`.
- Default behavior: absent or empty ports remain disabled; no default UDP reflector ports were added.
- Enabled behavior: 2–8 unique non-zero ports require an enabled SN and a public top-level IPv4. The SN identity advertises mapped public QUIC/TCP endpoints paired with wildcard local listeners, and the validated ports reach `SnServiceConfig::set_nat_probe_ports`.
- Handoff: every configured port must be exposed as UDP through the host firewall, container mapping, or NAT device. Local verification does not establish public reachability; no further task-scope code change is required.
- Preserved scope: NAT protocol/classification/scheduling remains in `p2p-frame`; PN endpoint reporting, VPN protocol, storage, HTTP API, client, and Flutter behavior are unchanged.

## Proposal Consistency
| change_id | requirement_or_boundary | proposal_source | delivery_evidence | finding | status |
|-----------|-------------------------|----------------|-------------------|---------|--------|
| CHG-enable-sn-nat-type-probe | Add validated `sn.nat_probe_ports`, preserve disabled defaults, construct the upstream-required identity endpoints, wire the ports to `SnServiceConfig`, document deployment, and avoid protocol/PN/lockfile expansion | `proposal.md` P-001, Scope, Success Criteria, and Risks | `server_config.rs` parsing/validation/endpoint helper and six focused tests; `main.rs` identity-only endpoint derivation plus `set_nat_probe_ports`; example YAML comments; current diff excludes task-authored `Cargo.lock` changes | Delivery matches the approved opt-in configuration and assembly boundary | pass |

## Implementation Review
| area | evidence | finding | status |
|------|----------|---------|--------|
| Requirement and call chain | `get_sn_server_config` -> `validate_server_mode` -> `resolve_sn_identity_endpoints` -> `P2pIdentity::update_endpoints` -> `SnServiceConfig::set_nat_probe_ports` -> `create_sn_service` | The configured values reach the actual upstream server builder; this is not parameter plumbing without a runtime consumer | pass |
| Default and compatibility | Missing/default and explicit-empty tests; `resolve_sn_identity_endpoints(..., &[])` preservation test | Existing no-config and empty-config deployments do not gain listeners or endpoint rewriting | pass |
| YAML/environment precedence | Combined valid YAML plus `VPN_SN_NAT_PROBE_PORTS` source test | The environment-expanded path is checked first, so the documented environment variable genuinely overrides file configuration | pass |
| Input boundaries | Invalid-set tests cover one, zero, duplicate, more than eight, and non-integer values; mode/address tests cover disabled SN, unspecified, loopback, private, and IPv6 addresses | Invalid configurations fail before upstream runtime construction; two through eight ports and a public IPv4 are accepted structurally | pass |
| Endpoint ordering and side effects | Mapped/local pair test plus separation between `eps` and `sn_identity_endpoints` in `main.rs` | Every mapped endpoint is immediately followed by a compatible wildcard endpoint as required by `NetManager::listen`; identity-only expansion does not add wildcard/duplicate addresses to PN reports | pass |
| Failure and lifecycle ownership | Source trace into `p2p-frame::SnServer::start_nat_probe_reflectors`; proposal boundary review | Bind/start failures continue to fail SN startup through the upstream error path; reflector maintenance, signing limits, task ownership, and shutdown remain upstream-owned | pass |
| Deployment/security boundary | Example YAML and upstream signed reflector implementation | Additional network exposure is explicit and opt-in. The application validates shape/address but does not claim or attempt firewall/NAT control | pass |
| Regression and dirty-worktree isolation | Targeted tests, serial full run, all-target compile, baseline manifest, and diff review | NAT/config tests and compilation pass. The user-owned lockfile remains untouched by task edits; the sole persistent suite failure is an unchanged 15-second-versus-170-second traffic-cache assertion outside this delivery | pass |

## Verification
- Targeted check: `cargo test -p bucky-vpn-server --locked sn_nat_probe -- --test-threads=1`
- Result: passed
- Targeted test detail: 6 passed, 0 failed, 74 filtered out.
- Compile check: `cargo check -p bucky-vpn-server --all-targets --locked` passed with existing warnings only.
- Full serial check: 79 passed, 1 unrelated pre-existing failure in `sqlite_store_factory::tests::node_traffic_record_rolls_back_and_retries_idempotently`.
- Flake isolation: `dedicated_heartbeat_controls_remote_online_state` passed alone and in the serial full run.
- Diff hygiene: `git diff --check` passed.
- Exception reason: no task-scope verification exception. Public NAT reachability is deployment-only evidence; the unchanged traffic-cache assertion is recorded rather than waived or modified.

## Findings
| id | severity | evidence | problem | blocking |
|----|----------|----------|---------|----------|
| F-001 | none | Fresh branch-order, environment precedence, invalid-input, Endpoint pairing, PN side-effect, upstream lifecycle, test, and diff review | No requirement mismatch, startup-order defect, silent environment override loss, mapped-endpoint ordering error, PN reporting regression, or task-scope leak was found | no |
| F-002 | informational | `DEFAULT_NODE_TRAFFIC_SPEED_TTL` is 15 seconds at unchanged `HEAD`, while the unchanged failing assertion requires at least 170 seconds remaining | Existing traffic-cache test contract is internally inconsistent and prevents a completely green full crate suite; it is unrelated to SN NAT probing | no |

## Conclusion
- Accepted / rejected / needs changes: accepted
- Reason: The approved opt-in configuration reaches the upstream NAT probe runtime with strict boundaries and preserved defaults, targeted tests and all-target compilation pass, and independent defect discovery found no blocking issue. The only persistent full-suite failure is an unchanged, source-demonstrably inconsistent traffic-cache assertion outside this task.
