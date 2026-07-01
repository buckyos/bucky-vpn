---
module: bucky-vpn
version: v0.1
status: approved
approved_by: user-request
approved_at: 2026-07-01T17:44:43+08:00
approved_content_sha256: 0f5d444f2927d987b2d78a46d11541301eb80ab7452adf2163b37bc7d93da382
---

# bucky-vpn Testing

## Test Document Index
| document | topic | scope |
| --- | --- | --- |
| `testing.md` | Client PN proxy route resolver and local API configuration validation | full change |
| `testplan.yaml` | Machine-readable harness entries | full module |

## Unified Test Entry
- Machine-readable plan: `docs/versions/v0.1/modules/bucky-vpn/testplan.yaml`
- Unified runner: `harness/scripts/test-run.py`
- Unit: `python3 ./harness/scripts/test-run.py bucky-vpn unit`
- DV: `python3 ./harness/scripts/test-run.py bucky-vpn dv`
- Integration: `python3 ./harness/scripts/test-run.py bucky-vpn integration`

## Submodule Tests
| submodule | responsibility | detailed_testing_doc | required_behavior | boundary_failure_cases | test_type | test_file |
| --- | --- | --- | --- | --- | --- | --- |
| `pn-proxy-route-resolver` | Maintain target-to-PN-relay route cache and implement p2p-frame resolver trait | none | Routes from `NodeVpnInfo` are visible to p2p-frame resolver callback | missing target route and invalid PN server id | unit gap / DV compile | `vpn-client/src/p2p_vpn.rs` |
| `p2p-vpn-runtime` | Wire resolver into P2P stack and tunnel factory refresh path | none | Stack config receives resolver and factory updates it on VPN info refresh | p2p-frame API drift or route refresh failure | DV | `harness/scripts/test-run.py` |
| `client-local-api-config` | Resolve daemon bind address and CLI target from settings/environment/defaults | none | Default address remains `127.0.0.1:4536`; file and env config override it | invalid or missing config falls back to default where typed parsing cannot produce a valid port | unit/integration | `vpn-client/src/cli.rs`; `harness/scripts/bucky-vpn-process-integration.py` |
| `client-entry` | Create client manager with existing join/settings/API route shape unchanged | none | Client crate builds and crate tests execute through harness | workspace neighbor failure | unit/DV/integration | `harness/scripts/test-run.py` |
| `client-node-id-text` | Keep client NodeId display/logging aligned with base36 canonical text | none | Client NodeId call sites emit base36 and do not format NodeId through base58. | stale server base58 data is a server migration concern | DV | `vpn-client/src/p2p_vpn.rs` |

## Module-Level Tests
| test_item | coverage_boundary | entry | expected_result | test_type | test_file_or_script |
| --- | --- | --- | --- | --- | --- |
| bucky-vpn unit | Client crate test target | `python3 ./harness/scripts/test-run.py bucky-vpn unit` | crate tests pass | automated | `test-results/test-runs/20260626T080527Z-bucky-vpn-unit.json` |
| bucky-vpn dv | Client crate build with resolver wiring | `python3 ./harness/scripts/test-run.py bucky-vpn dv` | client binary builds | automated | `test-results/test-runs/20260626T032458Z-bucky-vpn-dv.json` |
| bucky-vpn integration | Multipass-isolated multi-node PN proxy topology startup, underlay isolation, join, approval, member assignment, proxy approval, refresh, data-plane traffic, and PN traffic reporting | `python3 ./harness/scripts/test-run.py bucky-vpn integration` | requested vpn-client/vpn-server topologies start with one Multipass instance per node, client underlay IP paths are blocked and verified unreachable, clients join through each instance IPv4 and local API port, proxy nodes and joined nodes are approved, network member IPs are assigned, restarted clients recreate expected TUN devices from persisted VPN info, non-PN control paths are blocked where separate from PN, client virtual IP ping succeeds across every shared network, and the control API reports non-zero per-member plus user traffic stats after the ping traffic | automated | `harness/scripts/bucky-vpn-process-integration.py` |

## External Interface Tests
| interface | responsibility | success_case | failure_boundary_case | test_type | test_doc_or_file |
| --- | --- | --- | --- | --- | --- |
| `p2p_frame::stack::PnProxyRouteResolver` implementation | p2p-frame proxy client asks for relay id by target id | client crate builds against current p2p-frame trait | compile failure exposes trait/API drift | DV | `test-results/test-runs/20260626T031819Z-bucky-vpn-dv.json` |
| `P2pVpnTunnelFactory::on_vpn_info_received` | Refresh resolver routes from VPN info | compile proves callback signature and route update call are wired | invalid server id returns existing `VpnResult` error path | unit gap / DV | `vpn-client/src/p2p_vpn.rs` |
| PN proxy resolver runtime path | Ensure real vpn-client/vpn-server processes can drive PN proxy topology setup, VPN info refresh, client data-plane communication, and PN traffic reporting without client underlay reachability | combined control/PN server, separate control plus PN server, and three-client/two-PN pairwise networks run through the Python Multipass harness with one instance per node, client-to-client underlay IPs blocked and verified unreachable, proxy approval, join approval, member IP assignment, client restart, member registration checks, client TUN runtime readiness checks, non-PN control path blocking, virtual IP ping between every client pair sharing a network, and non-zero traffic stats on `/get_network_member` plus `/get_user_traffic_stats` from the control API | Multipass availability, instance launch, iptables isolation, direct-underlay negative ping, process startup, login, proxy node approval, network creation, selected PN server absence, client join, member assignment, client restart, VPN info refresh, expected TUN creation, virtual IP ping, PN traffic stats timeout, or PN setup log failure returns non-zero integration artifact | integration | `harness/scripts/bucky-vpn-process-integration.py` |
| Local API address configuration | Let daemon bind and CLI/test harness target a configured local API address | unit tests cover default, setting file and env override; Multipass integration starts each client daemon in a separate instance and reaches its API through the instance IPv4 and configured bind address | fixed, unreachable, or mismatched daemon/API address causes readiness or join failure | unit/integration | `test-results/test-runs/20260626T080527Z-bucky-vpn-unit.json`; `test-results/test-runs/20260626T080632Z-bucky-vpn-integration.json` |
| Client NodeId base36 text | Client-owned NodeId operations use base36 canonical text | client crate builds after base58 NodeId formatting is removed | stale base58-only NodeId call sites fail review/build checks | DV | `vpn-client/src/p2p_vpn.rs` |

## Direct Change Coverage
| change_id | design_source | validation_id | testplan_level | testplan_step_id | gap | gap_manual_reason |
| --- | --- | --- | --- | --- | --- | --- |
| CHG-client-pn-proxy-route-resolver | design.md Directly Mapped Change Items | VAL-pn-proxy-route-integration | integration | bucky-vpn-integration | no | none |
| CHG-client-configurable-local-api-address | design.md Directly Mapped Change Items | VAL-client-local-api-config | integration | bucky-vpn-integration | no | none |
| CHG-client-node-id-base36 | design.md Directly Mapped Change Items | VAL-client-node-id-base36 | dv | bucky-vpn-dv | no | none |

## Case-Type Coverage
| change_id | case_type | required | validation_id | level | status | gap_manual_reason |
| --- | --- | --- | --- | --- | --- | --- |
| CHG-client-pn-proxy-route-resolver | normal | yes | VAL-pn-proxy-route-dv | dv | covered | none |
| CHG-client-pn-proxy-route-resolver | boundary | yes | VAL-pn-proxy-route-unit | unit | covered | none |
| CHG-client-pn-proxy-route-resolver | negative | yes | VAL-pn-proxy-route-unit | unit | covered | none |
| CHG-client-pn-proxy-route-resolver | error | yes | VAL-pn-proxy-route-unit | unit | covered | none |
| CHG-client-pn-proxy-route-resolver | compatibility | yes | VAL-pn-proxy-route-dv | dv | covered | none |
| CHG-client-pn-proxy-route-resolver | lifecycle | yes | VAL-pn-proxy-route-dv | dv | covered | none |
| CHG-client-pn-proxy-route-resolver | cross-module | yes | VAL-pn-proxy-route-integration | integration | covered | none |
| CHG-client-configurable-local-api-address | normal | yes | VAL-client-local-api-config-unit | unit | covered | none |
| CHG-client-configurable-local-api-address | boundary | yes | VAL-client-local-api-config-unit | unit | covered | none |
| CHG-client-configurable-local-api-address | negative | yes | VAL-client-local-api-config-unit | unit | covered | none |
| CHG-client-configurable-local-api-address | error | yes | VAL-client-local-api-config-unit | unit | covered | none |
| CHG-client-configurable-local-api-address | compatibility | yes | VAL-client-local-api-config-unit | unit | covered | none |
| CHG-client-configurable-local-api-address | lifecycle | yes | VAL-client-local-api-config-integration | integration | covered | none |
| CHG-client-configurable-local-api-address | cross-module | yes | VAL-client-local-api-config-integration | integration | covered | none |
| CHG-client-node-id-base36 | normal | yes | VAL-client-node-id-base36 | dv | covered | none |
| CHG-client-node-id-base36 | boundary | no | VAL-client-node-id-base36 | dv | not-applicable | No new client-side parameter boundary is introduced beyond shared NodeId parsing. |
| CHG-client-node-id-base36 | negative | no | VAL-client-node-id-base36 | dv | not-applicable | Malformed server NodeId strings are owned by server/API validation. |
| CHG-client-node-id-base36 | error | no | VAL-client-node-id-base36 | dv | not-applicable | No new client error branch is introduced by replacing output format. |
| CHG-client-node-id-base36 | compatibility | yes | VAL-client-node-id-base36 | integration | manual | Old base58 server data is a server migration concern; client emits only base36 for NodeId. |
| CHG-client-node-id-base36 | lifecycle | no | VAL-client-node-id-base36 | dv | not-applicable | Log/display format has no runtime lifecycle. |
| CHG-client-node-id-base36 | cross-module | yes | VAL-client-node-id-base36-integration | integration | covered | none |

## Design Element Coverage
| element_type | design_source | derived_cases | level | status | gap_manual_reason |
| --- | --- | --- | --- | --- | --- |
| parameter-domain | design.md Interfaces and Dependencies | target P2P id, PN relay P2P id, missing target route | unit | covered | none |
| state-transition | design.md Data and State | empty route cache -> populated on VPN info -> replaced on refresh -> missing for no PN server | unit | covered | none |
| failure-path | design.md Key Call Flows | missing target route returns p2p error; invalid PN server id returns `VpnResult` error | unit | covered | none |
| error-handling | design.md Key Call Flows | p2p-frame trait error and VPN info parsing error categories | unit | covered | none |
| invariant | design.md Invariants to Preserve | settings, join, HTTP API untouched; PN id remains parsed from `PnServerInfo.id` | dv | covered | none |
| concurrency | design.md Data and State | resolver route cache protected by mutex and shared across stack/factory | unit | covered | none |
| parameter-domain | design.md Interfaces and Dependencies | default `127.0.0.1:4536`, file configured `api.ip`/`api.port`, env override values | unit | covered | none |
| state-transition | design.md Data and State | absent local API config -> setting file config -> env override config | unit | covered | none |
| invariant | design.md Invariants to Preserve | default daemon and CLI address remains `127.0.0.1:4536` | unit | covered | none |
| parameter-domain | design.md Interfaces and Dependencies | NodeId output is base36; P2P id strings remain separate | dv | covered | none |

## Validation Rationale
The lowest complete test level for the resolver cache is unit tests around `update_routes` and `resolve_pn_server`. Focused unit tests now cover route population, no-PN skip, stale-route replacement, missing target route, and invalid PN server id. The DV build remains valuable because it proves the client-owned resolver implements the current p2p-frame trait, `P2pStackConfig::set_proxy_route_resolver` exists for the locked dependency, and `P2pVpnTunnelFactory` wiring compiles.

The local API configuration is verified at unit level because defaulting and precedence are pure client behavior. The integration entry now runs `harness/scripts/bucky-vpn-process-integration.py`; it builds the `bucky-vpn` and `bucky-vpn-server` binaries, defines the three requested server/client scenarios, creates one Multipass instance for every logical server and client node, copies binaries and configs into each instance, blocks and negatively verifies direct client-to-client underlay IP reachability, logs into server APIs through instance-local HTTP calls, approves live remote proxy nodes for split control/proxy topologies, creates networks, starts each client daemon in its own instance, drives client joins through that instance's local API bind, approves joined nodes, assigns network member IPs, restarts clients from persisted `joined_networks` inside the same instance, verifies registered server members, waits for restarted clients to create expected TUN devices from refreshed VPN info, blocks non-PN control underlay paths for split-control scenarios, sends ICMP traffic between every pair of client virtual IPs that share a network, and polls the control API until `/get_network_member` and `/get_user_traffic_stats` show non-zero reported traffic. The harness reduces VM startup time by default: it prepares or reuses a stopped Multipass base instance, clones nodes from that instance unless `--no-use-base-image` or `BUCKY_VPN_INTEGRATION_USE_BASE_IMAGE=0` is set, and creates nodes concurrently with default `--parallel-instances 2`. The harness intentionally does not use `/get_network_member.online` as its pass condition because that API also requires non-empty SN peer WAN IPs, which are not stable in isolated Multipass process tests.

## Unit Tests
| function_or_unit | branch_or_condition | covered_behavior | test_file | status | gap_manual_reason |
| --- | --- | --- | --- | --- | --- |
| `P2pVpnPnProxyRouteResolver::update_routes` | VPN info with PN server and members | route cache maps member ids to PN server id | `vpn-client/src/p2p_vpn.rs` | covered | none |
| `P2pVpnPnProxyRouteResolver::update_routes` | VPN info without PN server | route cache skips the network | `vpn-client/src/p2p_vpn.rs` | covered | none |
| `P2pVpnPnProxyRouteResolver::update_routes` | invalid `PnServerInfo.id` | update returns an error | `vpn-client/src/p2p_vpn.rs` | covered | none |
| `P2pVpnPnProxyRouteResolver::resolve_pn_server` | target route exists | returns selected relay P2P id | `vpn-client/src/p2p_vpn.rs` | covered | none |
| `P2pVpnPnProxyRouteResolver::resolve_pn_server` | target route missing | returns p2p error | `vpn-client/src/p2p_vpn.rs` | covered | none |
| `LocalApiConfig::from_sources` | no config | returns default `127.0.0.1:4536` | `vpn-client/src/cli.rs` | covered | none |
| `LocalApiConfig::from_sources` | `setting.toml` has `[api]` table | uses configured `api.ip` and `api.port` | `vpn-client/src/cli.rs` | covered | none |
| `LocalApiConfig::from_sources` | env config and file config both exist | env config wins | `vpn-client/src/cli.rs` | covered | none |
| client NodeId output | NodeId logs and route refresh call sites | emits base36 canonical text and avoids base58 formatting | `vpn-client/src/p2p_vpn.rs` | covered | none |

## DV Tests
| workflow | kind | entry | expected_result | test_file_or_script | status | gap_manual_reason |
| --- | --- | --- | --- | --- | --- | --- |
| client crate build | main | `python3 ./harness/scripts/test-run.py bucky-vpn dv` | `cargo build -p bucky-vpn` exits 0 | `test-results/test-runs/20260626T032458Z-bucky-vpn-dv.json` | covered | none |
| client crate test lifecycle | lifecycle | `python3 ./harness/scripts/test-run.py bucky-vpn unit` | `cargo test -p bucky-vpn` exits 0 | `test-results/test-runs/20260626T080527Z-bucky-vpn-unit.json` | covered | none |
| resolver failure workflow | failure | `python3 ./harness/scripts/test-run.py bucky-vpn unit` | missing route and invalid PN id errors are asserted | `test-results/test-runs/20260626T080527Z-bucky-vpn-unit.json` | covered | none |
| local API config workflow | config | `python3 ./harness/scripts/test-run.py bucky-vpn unit` | default, file config and env override cases pass | `test-results/test-runs/20260626T080527Z-bucky-vpn-unit.json` | covered | none |
| client NodeId base36 workflow | main | `python3 ./harness/scripts/test-run.py bucky-vpn dv` | client crate builds after removing NodeId base58 output | `harness/scripts/test-run.py` | covered | none |

## Integration Tests
| contract_or_flow | modules_involved | success_case | failure_case | test_file | status | gap_manual_reason |
| --- | --- | --- | --- | --- | --- | --- |
| p2p-frame resolver contract | `bucky-vpn`, `p2p-frame` | client crate builds with `set_proxy_route_resolver` and trait implementation | compile failure catches trait drift | `test-results/test-runs/20260626T032458Z-bucky-vpn-dv.json` | covered | none |
| requested PN proxy topologies | `bucky-vpn`, `bucky-vpn-server`, `vpn-frame`, `p2p-frame` | two clients with combined control/PN server, two clients with separate control and PN server, and three clients with two PN servers each run with one Multipass instance per node, block and negatively verify direct client underlay reachability, approve proxy nodes where needed, join, get approved, receive member IPs, restart, register members, create expected TUN devices, block non-PN control underlay paths where applicable, pass virtual IP ping between every client pair sharing a network, and observe non-zero per-member plus user traffic stats on the control API | Multipass availability, instance launch, iptables isolation, direct-underlay negative ping, binary transfer, server startup, account login, proxy approval, PN server selection, network creation, local API readiness, client join, approval, member assignment, restart recovery, VPN info refresh, expected TUN creation, virtual IP ping, PN traffic stats timeout, or PN setup log failure fails the script | `harness/scripts/bucky-vpn-process-integration.py` | covered | none |
| client NodeId base36 consumer compatibility | `bucky-vpn`, `vpn-frame`, `bucky-vpn-server` | client consumes canonical base36 NodeId strings from server-facing flows | stale base58-only client NodeId operation fails build/review | `harness/scripts/test-run.py` | covered | none |

## Definition of Done
- [x] Testing metadata maps `CHG-client-pn-proxy-route-resolver` to unit, DV, and integration validation.
- [x] Unified test entrypoint produced bucky-vpn unit and DV run artifacts after resolver unit tests were added.
- [x] PN proxy multi-topology Multipass integration script is reachable through the unified integration entry.
- [x] Direct resolver branch coverage is covered by unit tests.
- [x] Configurable local API address is covered by unit tests and Multipass integration.
- [x] Full Multipass-isolated PN proxy topology startup, proxy approval, joined-node approval, member assignment, restart refresh, member registration, client TUN runtime readiness, direct-client-underlay negative checks, client virtual IP data-plane validation, and PN traffic reporting validation are reachable through the integration entry.

## Approval Record
- approver: user-request
- approval_date: 2026-07-01T17:44:43+08:00
- user_statement: "确认，自动处理后续步骤"
