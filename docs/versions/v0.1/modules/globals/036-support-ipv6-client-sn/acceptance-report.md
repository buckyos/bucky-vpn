# Acceptance Report

## Object and Scope

- Task manifest: task.yaml
- Version: v0.1
- Packet / target modules: globals packet delivering `bucky-vpn` and `bucky-vpn-server` changes
- Task: 036-support-ipv6-client-sn
- change_id values: CHG-client-ipv6-listen, CHG-server-ipv6-listen, CHG-client-sn-ipv6-endpoint, CHG-sn-comms-ipv6-surfaces, CHG-ipv6-capability-boundaries
- Confirmed baseline: `proposal.md`, confirmed by the user on 2026-09-15 with “确认，自动完成” and executed as an auto-pipeline (design → acceptance)
- Reviewed artifacts: `proposal.md`, `pipeline/plan.md`, `testplan.yaml`, `.harness/pipelines/v0.1/globals/036-support-ipv6-client-sn/state.json`, the delivered client and server changes, and the upstream `p2p-frame` commit recorded in `.harness/evidence/v0.1/036-support-ipv6-client-sn/upstream-p2p-frame-ipv6-bind.md`
- Out of scope by the confirmed proposal: NAT probe and rendezvous IPv6 support, `ReportSn`/`SnQuery` wire changes, `vpn-frame` VPN virtual-address semantics, and public-internet IPv6 reachability claims

## Conclusion

- Accepted / rejected / needs changes: accepted
- Reason: every proposal item is implemented and verified within the confirmed boundary, the task test run artifact is green, the independent defect search found only non-blocking findings, and the one operational prerequisite (publishing the upstream `p2p-frame` commit and updating the dependency revision) is recorded explicitly rather than hidden.

## Findings

| ID | Severity | Kind | Evidence | Problem | Blocking |
|----|----------|------|----------|---------|----------|
| F-001 | low | testing-consistency | `vpn-server/src/sqlite_store_factory.rs:38` (`DEFAULT_NODE_TRAFFIC_SPEED_TTL = 15s`) vs `:2745` (asserts remaining lifetime ≥ 170s); file unmodified by this task (`git diff HEAD -- vpn-server/src/sqlite_store_factory.rs` is empty) | The `bucky-vpn-server` crate-wide unit suite fails on a pre-existing, unrelated traffic-TTL assertion, so module-wide suites cannot be used as this task's evidence; the task testplan therefore runs focused commands, and the module-wide gap is not caused by this delivery | no |
| F-002 | medium | implementation | Upstream commit `98ef0242bd500f9fec727ad6feaec950d5b68102` in `/mnt/f/work/p2p`; `Cargo.lock` still pins `p2p-frame` `a9875f08d515c76dbcd2ed7b23d93d622a2d83d5` | The IPv6-only bind semantics that make an `ip: "0.0.0.0, ::"` configuration safe are not yet consumed by this repository. Until the upstream commit is published and the dependency revision updated, IPv4 traffic on that dual-wildcard configuration can be delivered to the IPv6 listener as a v4-mapped peer, which would break the SN's IPv4 WAN classification and rendezvous eligibility for those peers | no |
| F-003 | low | implementation | `vpn-client/src/main.rs` reads `p2p.listen_ip` from `load_env_config()` (VPN_ prefix), the same source as the existing `p2p.port` | A user who only edits `setting.toml` cannot configure the listen family; this matches the existing `p2p.port` behavior and is not a regression, but it is a discoverability limitation worth documenting | no |
| F-004 | low | design-consistency | `vpn-client/src/cli.rs` and `vpn-server/src/server_config.rs` both implement the 4-address cap and one-wildcard-per-family rule independently | Duplicated validation policy can drift between the two binaries; the rule is currently identical and covered by tests on both sides | no |

## Requirement Review

| Requirement or boundary | Source | Implementation evidence | Finding | Status |
|-------------------------|--------|-------------------------|---------|--------|
| P-001 client listens on IPv6 with a config-controlled family selection | `proposal.md` Proposal Items P-001 / CHG-client-ipv6-listen | `vpn-client/src/cli.rs` (`DEFAULT_LISTEN_IP`, `parse_listen_ips`, validation) and `vpn-client/src/main.rs` (`p2p_listen_endpoints(listen_ips, port)`, `p2p.listen_ip` read, diagnosable failure) | none | pass |
| P-002 server accepts IPv6 listen configuration and drives identity/listener/primary selection from it | `proposal.md` P-002 / CHG-server-ipv6-listen | `vpn-server/src/server_config.rs` (`parse_listen_ips`, `resolve_service_endpoints(listen_ips, port, ...)`) and `vpn-server/src/main.rs` (validated listen set replaces the `Ipv4Addr` parse) | none | pass |
| P-003 client resolves the SN target into a family-aware endpoint list | `proposal.md` P-003 / CHG-client-sn-ipv6-endpoint | `vpn-client/src/p2p_vpn.rs` (`resolve_sn_addrs`, `order_sn_addrs`, `sn_endpoints(&[SocketAddr])`, IPv4-before-IPv6 ordering) | none | pass |
| P-004 remaining SN communication surfaces use the same family-aware configuration | `proposal.md` P-004 / CHG-sn-comms-ipv6-surfaces | `vpn-server/src/server_config.rs` (`normalize_http_bind_ip`), `vpn-server/config/config.example.yaml` (documented family contract and IPv6 HTTP form); PN transport endpoints and inter-SN/TTP paths were reviewed and contain no IPv4-only assumptions | none | pass |
| P-005 capability boundary recorded and observable, upstream change mapped | `proposal.md` P-005 / CHG-ipv6-capability-boundaries | `vpn-client/src/p2p_vpn.rs` (`sn_address_family_scope`, `sn_nat_probe_unavailable`, bounded warning), `.harness/evidence/v0.1/036-support-ipv6-client-sn/upstream-p2p-frame-ipv6-bind.md` (commit + red/green evidence) | F-002 | pass |
| Accepted boundary: IPv6 SN endpoints keep NAT probe fail-closed and rendezvous unchanged | `proposal.md` Decisions 3 / Out of scope | No protocol change; the client logs the boundary and p2p-frame still rejects non-IPv4 active SN endpoints (`ActiveEndpointIpv4`); SN reported-endpoint sanitization and rendezvous eligibility are untouched | none | pass |
| Requirement to keep default IPv4 behavior and startup semantics | `proposal.md` Decisions 2 / Success Criteria | `parse_listen_ips(DEFAULT_LISTEN_IP)` reproduces the previous `0.0.0.0:<port>` QUIC/TCP pair; the server keeps its configured IPv4 default; invalid input fails startup with the offending value and reason | none | pass |

## Implementation Review

| Area | Evidence | Finding | Status |
|------|----------|---------|--------|
| Client listen configuration and endpoint construction | `vpn-client/src/cli.rs`, `vpn-client/src/main.rs`, unit tests `listen_ip_*`, `p2p_listen_endpoints_*` | F-003 | pass |
| Client SN endpoint family selection | `vpn-client/src/p2p_vpn.rs`, unit tests `sn_endpoints_cover_both_families_quic_then_tcp_per_address`, `order_sn_addrs_places_ipv4_first_and_removes_duplicates`, `sn_address_family_scope_reports_single_family_and_dual_lists`, `sn_nat_probe_is_unavailable_only_for_ipv6_only_endpoint_lists` | none | pass |
| Server listen configuration and startup wiring | `vpn-server/src/server_config.rs`, `vpn-server/src/main.rs`, tests `listen_ips_accept_single_family_and_mixed_lists`, `listen_ips_reject_ambiguous_or_malformed_values`, `mixed_listen_ips_resolve_quic_then_tcp_per_address`, `tcp_listener_binds*` regression in `pn_transport_mode_tests.rs` | F-004 | pass |
| Server HTTP API and operator documentation | `normalize_http_bind_ip`, `http_bind_address_is_bracketed_for_ipv6`, `server_config_example_documents_ipv6_listen_contract`, `vpn-server/config/config.example.yaml` | none | pass |
| Cross-repository IPv6-only bind behavior | Upstream `p2p-frame` commit `98ef024` with 5 passing focused tests and a red control that fails without the policy | F-002 | pass |
| Task test evidence | `.harness/test-results/test-runs/20260915T110512Z-globals+036-support-ipv6-client-sn-all.json` (10 commands, exit 0, unit + dv + contract checks) | F-001 | pass |

## Document Consistency

| Document | Source | Implementation consistency | Finding | Status |
|----------|--------|---------------------------|---------|--------|
| design | `pipeline/plan.md` | Delivered files, helper names, endpoint ordering, HTTP bracketing, wildcard validation, and the external upstream dependency all match the plan; `cli.rs` was moved to match the planned file-level boundary | none | pass |
| testing | `testplan.yaml` | Every change id has unit-level coverage or a recorded manual gap; contract checks cover compile, symbol scan, compile closure, and documentation examples; the recorded run artifact matches the testplan steps | none | pass |

## Result Summary

- Overall result: accepted
- Outcome: the client can listen on IPv4, IPv6, or both through `p2p.listen_ip`; the server accepts IPv4/IPv6 listen sets through `ip` and derives service, identity, PN primary, and HTTP bind values from them; the client orders SN endpoints deterministically across families; the accepted NAT-probe boundary is preserved and observable; the required upstream `p2p-frame` fix exists as commit `98ef024` with red/green evidence.
- Blocking issues: none
- Next action: before deploying any IPv6 listen configuration, publish upstream commit `98ef0242bd500f9fec727ad6feaec950d5b68102` to `github.com/buckyos/bucky-p2p` and update this repository's `p2p-frame` dependency revision, then optionally run the manual dual-stack check recorded in `testplan.yaml` (integration level). Also note the pre-existing `bucky-vpn-server` traffic-TTL test failure (F-001) remains open and unrelated to this task.
