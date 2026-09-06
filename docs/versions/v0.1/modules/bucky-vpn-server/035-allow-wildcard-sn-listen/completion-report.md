# Completion Report

## Object and Scope
- Task manifest: task.yaml
- Workflow tier: standard
- Change record: docs/changes/035-allow-wildcard-sn-listen.md

## Delivery Summary
- Outcome: `bucky-vpn-server` now accepts a wildcard SN listen address (`ip: "0.0.0.0"`) while `sn.nat_probe_ports` is enabled. The obsolete public-IPv4 requirement and mapped/public plus wildcard/local identity endpoint adaptation are gone; the SN identity always tracks the ordinary service endpoints.
- Enabled behavior: 2–8 unique non-zero probe ports require `sn.enabled: true`, are validated by `server_config.rs`, reach `SnServiceConfig::set_nat_probe_ports`, and upstream `p2p-frame` binds its reflectors on `0.0.0.0:<port>` based on the port-only wire.
- Handoff: operators must still open or forward each configured UDP probe port through firewall, container mapping, or NAT device. No further task-scope code change is required.
- Preserved scope: NAT probe protocol, classification, scheduling, signatures, reflector lifecycle, and client reconstruction remain owned by `p2p-frame`; PN endpoint reporting, storage, HTTP API, client, and Flutter behavior are unchanged.

## Proposal Consistency
| change_id | requirement_or_boundary | proposal_source | delivery_evidence | finding | status |
|-----------|-------------------------|----------------|-------------------|---------|--------|
| CHG-allow-wildcard-sn-listen | Allow `0.0.0.0` as the SN listen address with NAT probing enabled, stop requiring a public IPv4 and mapped/local identity pairs, keep ordinary service endpoints as identity endpoints, wire validated ports to `SnServiceConfig`, and update deployment documentation | `proposal.md` P-001, Scope, Success Criteria, and Risks | `server_config.rs` passthrough identity resolver plus wildcard/identity tests; `main.rs` identity resolution and `set_nat_probe_ports` wiring; `config.example.yaml` updated guidance; targeted tests, all-targets compile, full serial suite, and diff hygiene all reviewed | Delivery matches the approved wildcard-listen adaptation and does not leak into upstream or PN behavior | pass |

## Implementation Review
| area | evidence | finding | status |
|------|----------|---------|--------|
| Requirement and call chain | `get_sn_server_config` -> `validate_server_mode` -> `resolve_sn_identity_endpoints(&eps)` -> `P2pIdentity::update_endpoints` -> `SnServiceConfig::set_nat_probe_ports` -> `create_sn_service` | The configured probes still reach the actual upstream builder, and identity endpoints no longer depend on NAT probing or require a static public IPv4 | pass |
| Upstream wire contract alignment | `p2p-frame` 9b7e2d6 source: `NatProbeDirective.ports`, `ReportSnResp.nat_probe_ports`, `start_nat_probe_reflectors` binding wildcard `0.0.0.0` with each configured port | The old mapped/public endpoint pairing is obsolete in this revision; keeping wildcard service endpoints matches how clients rebuild probe endpoints from the authenticated QUIC SN IPv4 plus ports | pass |
| Configuration and deployment docs | `config.example.yaml` comments no longer instruct operators to change top-level `ip` to a public IPv4 | The default wildcard listen address is explicitly valid while reflector ports still must be exposed externally | pass |
| Input boundaries and failure behavior | `sn_nat_probe_ports_reject_invalid_sets`, `sn_nat_probe_requires_enabled_sn_even_when_pn_is_disabled`, and `sn_nat_probe_accepts_wildcard_top_level_ip` | Port cardinality, zero/duplicate/non-port values, and disabled-SN use still fail closed; matching the new contract, address validation no longer rejects `0.0.0.0` | pass |
| Regression and dirty-worktree isolation | Task-start baseline, targeted/server_config/full-suite runs, all-targets compile, `git diff --check`, and removed-helper search | Production change is confined to `server_config.rs`, `main.rs`, and `config.example.yaml`; a user-owned `Cargo.lock` update to p2p-frame `9b7e2d6` is preserved as required context and not claimed as task-channel churn | pass |

## Verification
- Targeted check: `cargo test -p bucky-vpn-server --locked sn_nat_probe -- --test-threads=1`
- Result: passed
- Targeted test detail: 5 passed, 0 failed, 74 filtered out.
- Config regression suite: `cargo test -p bucky-vpn-server --locked server_config -- --test-threads=1` passed with 24 passed, 0 failed, 55 filtered out.
- Compile check: `cargo check -p bucky-vpn-server --all-targets --locked` passed with existing dead-code warnings only.
- Full serial check: 78 passed, 1 failed. The sole failure is the unchanged `sqlite_store_factory::tests::node_traffic_record_rolls_back_and_retries_idempotently`, already recorded by task 034 as a source-inconsistent TTL assertion outside this delivery.
- Diff hygiene: `git diff --check` passed.
- Exception reason: no task-scope verification exception. Public UDP reachability is deployment-only evidence; the unrelated TTL assertion is recorded rather than modified.

## Findings
| id | severity | evidence | problem | blocking |
|----|----------|----------|---------|----------|
| F-001 | none | Fresh config/wire contract review, wildcard tests, removed-helper caller search, all-target compile, full serial suite, and diff check | No requirement mismatch, stale public-IPv4 guard, missing probe-port wiring, identity-endpoint regression, or task-scope leak was found | no |
| F-002 | informational | `DEFAULT_NODE_TRAFFIC_SPEED_TTL` is unchanged at 15 seconds in `sqlite_store_factory.rs` while the persisted full-suite failure requires at least 170 seconds remaining | The existing traffic-cache test assertion is internally inconsistent and unrelated to SN NAT probing; it was also recorded in task 034 | no |

## Conclusion
- Accepted / rejected / needs changes: accepted
- Reason: The approved wildcard-listen adaptation is implemented, the identity endpoint set stays unchanged from service endpoints, probe ports still reach the upstream `SnServiceConfig`, targeted and config regression tests plus all-targets compilation pass, and independent defect discovery found no blocking issue. The only full-suite failure is an unchanged, unrelated TTL assertion outside this task.
