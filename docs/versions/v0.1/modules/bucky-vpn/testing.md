---
module: bucky-vpn
version: v0.1
status: approved
approved_by: auto-pipeline
approved_at: 2026-07-07T00:22:12+08:00
approved_content_sha256: 9a5c30fd825fed22af3957efb55f170965c8cc385e4fabb23c6b2d06fe4ecd4c
---

# bucky-vpn Testing

## Test Document Index
| document | topic | scope |
| --- | --- | --- |
| `testing.md` | Client PN proxy route resolver, local API configuration, NodeId text, SN endpoint list, and join server_name validation | full change |
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
| `sn-client-transport` | Construct local P2P endpoint list and ordered SN endpoint list for p2p-frame | none | Local P2P endpoints contain QUIC/TCP on `p2p.port`; SN endpoints contain QUIC then TCP for the resolved SN socket address | p2p-frame endpoint selection is outside client ownership | unit/DV | `vpn-client/src/main.rs`; `vpn-client/src/p2p_vpn.rs` |
| `sn-server-name` | Resolve optional join `server_name` into the SN remote name used by p2p-frame | none | Explicit `server_name` wins; omitted domain server defaults to domain; omitted IP server defaults to `server_id`; blank value is treated as absent; old manager keys still parse | malformed structured manager key returns existing invalid-parameter path | unit/DV | `vpn-client/src/main.rs`; `vpn-client/src/cli.rs`; `vpn-client/src/api.rs`; `vpn-client/src/p2p_vpn.rs` |

## Module-Level Tests
| test_item | coverage_boundary | entry | expected_result | test_type | test_file_or_script |
| --- | --- | --- | --- | --- | --- |
| bucky-vpn unit | Client crate test target | `python3 ./harness/scripts/test-run.py bucky-vpn unit` | crate tests pass | automated | `test-results/test-runs/20260706T073344Z-bucky-vpn-unit.json` |
| bucky-vpn dv | Client crate build with resolver wiring | `python3 ./harness/scripts/test-run.py bucky-vpn dv` | client binary builds | automated | `test-results/test-runs/20260706T073427Z-bucky-vpn-dv.json` |
| bucky-vpn integration | Multipass-isolated multi-node PN proxy topology startup, underlay isolation, join, approval, member assignment, proxy approval, refresh, data-plane traffic, and PN traffic reporting | `python3 ./harness/scripts/test-run.py bucky-vpn integration` | requested vpn-client/vpn-server topologies start with one Multipass instance per node, client underlay IP paths are blocked and verified unreachable, clients join through each instance IPv4 and local API port, proxy nodes and joined nodes are approved, network member IPs are assigned, restarted clients recreate expected TUN devices from persisted VPN info, non-PN control paths are blocked where separate from PN, client virtual IP ping succeeds across every shared network, and the control API reports non-zero per-member plus user traffic stats after the ping traffic | automated | `harness/scripts/bucky-vpn-process-integration.py` |

## External Interface Tests
| interface | responsibility | success_case | failure_boundary_case | test_type | test_doc_or_file |
| --- | --- | --- | --- | --- | --- |
| `p2p_frame::stack::PnProxyRouteResolver` implementation | p2p-frame proxy client asks for relay id by target id | client crate builds against current p2p-frame trait | compile failure exposes trait/API drift | DV | `test-results/test-runs/20260704T161424Z-bucky-vpn-dv.json` |
| `P2pVpnTunnelFactory::on_vpn_info_received` | Refresh resolver routes from VPN info | compile proves callback signature and route update call are wired | invalid server id returns existing `VpnResult` error path | unit gap / DV | `vpn-client/src/p2p_vpn.rs` |
| PN proxy resolver runtime path | Ensure real vpn-client/vpn-server processes can drive PN proxy topology setup, VPN info refresh, client data-plane communication, and PN traffic reporting without client underlay reachability | combined control/PN server, separate control plus PN server, and three-client/two-PN pairwise networks run through the Python Multipass harness with one instance per node, client-to-client underlay IPs blocked and verified unreachable, proxy approval, join approval, member IP assignment, client restart, member registration checks, client TUN runtime readiness checks, non-PN control path blocking, virtual IP ping between every client pair sharing a network, and non-zero traffic stats on `/get_network_member` plus `/get_user_traffic_stats` from the control API | Multipass availability, instance launch, iptables isolation, direct-underlay negative ping, process startup, login, proxy node approval, network creation, selected PN server absence, client join, member assignment, client restart, VPN info refresh, expected TUN creation, virtual IP ping, PN traffic stats timeout, or PN setup log failure returns non-zero integration artifact | integration | `harness/scripts/bucky-vpn-process-integration.py` |
| Local API address configuration | Let daemon bind and CLI/test harness target a configured local API address | unit tests cover default, setting file and env override; Multipass integration starts each client daemon in a separate instance and reaches its API through the instance IPv4 and configured bind address | fixed, unreachable, or mismatched daemon/API address causes readiness or join failure | unit/integration | `test-results/test-runs/20260626T080527Z-bucky-vpn-unit.json`; `test-results/test-runs/20260626T080632Z-bucky-vpn-integration.json` |
| Client NodeId base36 text | Client-owned NodeId operations use base36 canonical text | client crate builds after base58 NodeId formatting is removed | stale base58-only NodeId call sites fail review/build checks | DV | `vpn-client/src/p2p_vpn.rs` |
| `P2pConfig::new` local endpoint vector and `P2pSn::new` remote endpoint vector | Client supplies p2p-frame with local and remote SN-capable endpoints | unit tests assert local QUIC/TCP endpoints and remote QUIC/TCP SN endpoints are present in order; DV build proves both vectors match p2p-frame types | live endpoint choice remains owned by p2p-frame and is not duplicated in client tests | unit/DV | `vpn-client/src/main.rs`; `vpn-client/src/p2p_vpn.rs` |
| Join `server_name` and `P2pSn::new` name parameter | Client supplies p2p-frame with explicit or derived SN remote name | unit tests assert explicit value, domain default, IP default, blank fallback, structured key roundtrip, and legacy key compatibility; DV build proves CLI/API/factory wiring compiles | invalid JSON manager key returns existing invalid-parameter path; live certificate/SNI validation remains p2p-frame behavior | unit/DV | `vpn-client/src/main.rs`; `vpn-client/src/cli.rs`; `vpn-client/src/api.rs`; `vpn-client/src/p2p_vpn.rs` |
| PN proxy reported remote name | Client supplies p2p-frame proxy connection with reported proxy name when present | shared `PnServerInfo::remote_name` unit test covers name/fallback semantics; client crate tests compile the `connect_pn_server` wiring against p2p-frame | live certificate mismatch behavior remains owned by p2p-frame | unit/DV | `vpn-frame/src/vpn_protocol.rs`; `vpn-client/src/p2p_vpn.rs` |

## Direct Change Coverage
| change_id | design_source | validation_id | testplan_level | testplan_step_id | gap | gap_manual_reason |
| --- | --- | --- | --- | --- | --- | --- |
| CHG-client-pn-proxy-route-resolver | design.md Directly Mapped Change Items | VAL-pn-proxy-route-integration | integration | bucky-vpn-integration | no | none |
| CHG-client-configurable-local-api-address | design.md Directly Mapped Change Items | VAL-client-local-api-config | integration | bucky-vpn-integration | no | none |
| CHG-client-node-id-base36 | design.md Directly Mapped Change Items | VAL-client-node-id-base36 | dv | bucky-vpn-dv | no | none |
| CHG-client-sn-quic-tcp-priority | design.md Directly Mapped Change Items | VAL-client-sn-endpoint-list | unit | bucky-vpn-unit | no | none |
| CHG-client-join-server-name-for-sn | design.md Directly Mapped Change Items | VAL-client-join-server-name | unit | bucky-vpn-unit | no | none |
| CHG-client-pn-proxy-reported-name | design.md Directly Mapped Change Items | VAL-client-pn-proxy-reported-name | unit | bucky-vpn-unit | no | Shared `PnServerInfo::remote_name` unit coverage plus client crate tests cover the connection target wiring. |
| CHG-client-pn-proxy-endpoint-address | design.md Directly Mapped Change Items | VAL-client-pn-endpoint-address | unit | bucky-vpn-unit | no | Unit tests cover Endpoint ordering, deduplication, and protocol conversion; DV build covers `connect_pn_server` p2p-frame target wiring. |

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
| CHG-client-sn-quic-tcp-priority | normal | yes | VAL-client-sn-endpoint-list | unit | covered | none |
| CHG-client-sn-quic-tcp-priority | boundary | yes | VAL-client-sn-endpoint-list | unit | covered | none |
| CHG-client-sn-quic-tcp-priority | negative | no | VAL-client-sn-endpoint-list | unit | not-applicable | The client adds no endpoint-selection failure branch; p2p-frame owns unusable endpoint handling. |
| CHG-client-sn-quic-tcp-priority | error | no | VAL-client-sn-endpoint-list | unit | not-applicable | The changed code constructs endpoints from an already resolved `SocketAddr` and introduces no new `VpnResult` error category. |
| CHG-client-sn-quic-tcp-priority | compatibility | yes | VAL-client-sn-endpoint-list-dv | dv | covered | none |
| CHG-client-sn-quic-tcp-priority | lifecycle | no | VAL-client-sn-endpoint-list | unit | not-applicable | The endpoint list is constructed once during client stack creation and has no persisted lifecycle state. |
| CHG-client-sn-quic-tcp-priority | cross-module | yes | VAL-client-sn-endpoint-list-dv | dv | covered | none |
| CHG-client-join-server-name-for-sn | normal | yes | VAL-client-join-server-name | unit | covered | none |
| CHG-client-join-server-name-for-sn | boundary | yes | VAL-client-join-server-name | unit | covered | none |
| CHG-client-join-server-name-for-sn | negative | yes | VAL-client-join-server-name | unit | covered | none |
| CHG-client-join-server-name-for-sn | error | yes | VAL-client-join-server-name | unit | covered | none |
| CHG-client-join-server-name-for-sn | compatibility | yes | VAL-client-join-server-name-dv | dv | covered | none |
| CHG-client-join-server-name-for-sn | lifecycle | yes | VAL-client-join-server-name | unit | covered | none |
| CHG-client-join-server-name-for-sn | cross-module | yes | VAL-client-join-server-name-dv | dv | covered | none |
| CHG-client-pn-proxy-reported-name | normal | yes | VAL-client-pn-proxy-reported-name | unit | covered | none |
| CHG-client-pn-proxy-reported-name | boundary | yes | VAL-client-pn-proxy-reported-name | unit | covered | none |
| CHG-client-pn-proxy-reported-name | negative | no | VAL-client-pn-proxy-reported-name | unit | not-applicable | Missing name falls back to id and does not add a client reject path. |
| CHG-client-pn-proxy-reported-name | error | no | VAL-client-pn-proxy-reported-name | unit | not-applicable | Live certificate/SNI errors are p2p-frame behavior. |
| CHG-client-pn-proxy-reported-name | compatibility | yes | VAL-client-pn-proxy-reported-name-dv | dv | covered | none |
| CHG-client-pn-proxy-reported-name | lifecycle | no | VAL-client-pn-proxy-reported-name | unit | not-applicable | Remote name is computed for a single connection attempt. |
| CHG-client-pn-proxy-reported-name | cross-module | yes | VAL-client-pn-proxy-reported-name-integration | integration | covered | none |
| CHG-client-pn-proxy-endpoint-address | normal | yes | VAL-client-pn-endpoint-address | unit | covered | none |
| CHG-client-pn-proxy-endpoint-address | boundary | yes | VAL-client-pn-endpoint-address | unit | covered | none |
| CHG-client-pn-proxy-endpoint-address | negative | yes | VAL-client-pn-endpoint-address | unit | covered | none |
| CHG-client-pn-proxy-endpoint-address | error | yes | VAL-client-pn-endpoint-address | unit | covered | none |
| CHG-client-pn-proxy-endpoint-address | compatibility | yes | VAL-client-pn-endpoint-address-dv | dv | covered | none |
| CHG-client-pn-proxy-endpoint-address | lifecycle | no | VAL-client-pn-endpoint-address | unit | not-applicable | Endpoint selection happens per connection attempt and has no persisted client lifecycle state. |
| CHG-client-pn-proxy-endpoint-address | cross-module | yes | VAL-client-pn-endpoint-address-integration | integration | covered | none |

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
| parameter-domain | design.md Interfaces and Dependencies | local P2P endpoint vector contains QUIC/TCP on the same configured port; remote SN endpoint vector contains exactly QUIC then TCP for the same socket address | unit | covered | none |
| invariant | design.md Invariants to Preserve | join key and identity directory naming remain unchanged because only the endpoint vector changes after address resolution | unit | covered | none |
| failure-path | design.md Key Call Flows | DNS/socket address resolution failure remains the existing pre-endpoint `VpnResult` path; p2p-frame owns endpoint connection failure | unit | covered | none |
| parameter-domain | design.md Interfaces and Dependencies | explicit `server_name`, domain default, IP default, blank fallback, and legacy key compatibility | unit | covered | none |
| state-transition | design.md Data and State | absent old/new record -> domain/IP fallback; explicit non-empty value -> `P2pSn::new` name; blank value -> absent fallback | unit | covered | none |
| failure-path | design.md Key Call Flows | malformed structured manager key maps to invalid-parameter parse failure; DNS/socket failure remains separate | unit | covered | none |
| invariant | design.md Invariants to Preserve | `join --name` remains network member name and old `joined_networks` records remain readable | unit | covered | none |
| parameter-domain | design.md Interfaces and Dependencies | PN server reported name present, blank/absent name fallback, and id remains the P2P identity source | dv | covered | none |

## Validation Rationale
The lowest complete test level for the resolver cache is unit tests around `update_routes` and `resolve_pn_server`. Focused unit tests now cover route population, no-PN skip, stale-route replacement, missing target route, and invalid PN server id. The DV build remains valuable because it proves the client-owned resolver implements the current p2p-frame trait, `P2pStackConfig::set_proxy_route_resolver` exists for the locked dependency, and `P2pVpnTunnelFactory` wiring compiles.

The local API configuration is verified at unit level because defaulting and precedence are pure client behavior. The integration entry now runs `harness/scripts/bucky-vpn-process-integration.py`; it builds the `bucky-vpn` and `bucky-vpn-server` binaries, defines the three requested server/client scenarios, creates one Multipass instance for every logical server and client node, copies binaries and configs into each instance, blocks and negatively verifies direct client-to-client underlay IP reachability, logs into server APIs through instance-local HTTP calls, approves live remote proxy nodes for split control/proxy topologies, creates networks, starts each client daemon in its own instance, drives client joins through that instance's local API bind, approves joined nodes, assigns network member IPs, restarts clients from persisted `joined_networks` inside the same instance, verifies registered server members, waits for restarted clients to create expected TUN devices from refreshed VPN info, blocks non-PN control underlay paths for split-control scenarios, sends ICMP traffic between every pair of client virtual IPs that share a network, and polls the control API until `/get_network_member` and `/get_user_traffic_stats` show non-zero reported traffic. The harness reduces VM startup time by default: it prepares or reuses a stopped Multipass base instance, clones nodes from that instance unless `--no-use-base-image` or `BUCKY_VPN_INTEGRATION_USE_BASE_IMAGE=0` is set, and creates nodes concurrently with default `--parallel-instances 2`. The harness intentionally does not use `/get_network_member.online` as its pass condition because that API also requires non-empty SN peer WAN IPs, which are not stable in isolated Multipass process tests.

The SN endpoint list change is verified at unit level because the client-owned behavior is pure endpoint construction from an already resolved `SocketAddr` and a configured `p2p.port`. A DV build/check verifies compatibility with p2p-frame's `Endpoint`, `Protocol`, `P2pConfig::new`, and `P2pSn::new` contracts. A live QUIC-vs-TCP selection test is intentionally not added here because the approved design leaves endpoint choice and retry behavior to p2p-frame rather than reimplementing that behavior in `bucky-vpn`.

The join `server_name` change is verified at unit level because the client-owned behavior is pure input/default/key handling before p2p-frame performs live SN tunnel validation. Unit tests cover explicit name priority, domain default, IP default, blank fallback, structured manager-key roundtrip, and legacy manager-key parsing. A DV build verifies the new CLI argument, local API request model, persisted `JoinRecord`, and `P2pSn::new` name wiring compile together.

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
| `p2p_listen_endpoints` | configured `p2p.port` | returns exactly two local listen endpoints: QUIC and TCP on `0.0.0.0:<p2p.port>` | `vpn-client/src/main.rs` | covered | none |
| `sn_endpoints` | resolved SN socket address | returns exactly two endpoints: QUIC first and TCP second, both using the same socket address | `vpn-client/src/p2p_vpn.rs` | covered | none |
| `effective_server_name` | explicit `server_name` | returns explicit SN remote name even when server is an IP | `vpn-client/src/p2p_vpn.rs` | covered | none |
| `effective_server_name` | omitted domain server | returns domain server as SN remote name | `vpn-client/src/p2p_vpn.rs` | covered | none |
| `effective_server_name` | omitted IP server | returns `server_id` as SN remote name | `vpn-client/src/p2p_vpn.rs` | covered | none |
| `effective_server_name` | blank `server_name` | treats blank as absent and applies default rule | `vpn-client/src/p2p_vpn.rs` | covered | none |
| `P2pVpnClientKey` | structured key with `server_name` | round-trips server id, server, port, and normalized server_name | `vpn-client/src/p2p_vpn.rs` | covered | none |
| `P2pVpnClientKey` | legacy `server_id_server:port` key | parses old key with no `server_name` for compatibility | `vpn-client/src/p2p_vpn.rs` | covered | none |
| `connect_pn_server` target construction | PN server has optional reported name | uses shared `PnServerInfo::remote_name()` for p2p-frame remote name and keeps `PnServerInfo.id` as remote id | `vpn-client/src/p2p_vpn.rs`; `vpn-frame/src/vpn_protocol.rs` | covered | none |
| `pn_server_endpoints` | repeated QUIC endpoint and TCP fallback endpoint | orders QUIC Endpoint values before non-QUIC values and deduplicates without reconstructing from split fields | `vpn-client/src/p2p_vpn.rs` | covered | none |
| `pn_endpoint_to_p2p_endpoint` | QUIC and TCP Endpoint protocols | converts server-returned Endpoint values into p2p-frame `Endpoint` targets while preserving protocol and port | `vpn-client/src/p2p_vpn.rs` | covered | none |
| `pn_endpoint_to_p2p_endpoint` | unknown protocol string | returns `VpnErrorCode::InvalidParam` instead of silently connecting to the wrong transport | `vpn-client/src/p2p_vpn.rs` | covered | none |

## DV Tests
| workflow | kind | entry | expected_result | test_file_or_script | status | gap_manual_reason |
| --- | --- | --- | --- | --- | --- | --- |
| client crate build | main | `python3 ./harness/scripts/test-run.py bucky-vpn dv` | `cargo build -p bucky-vpn` exits 0 | `test-results/test-runs/20260706T073427Z-bucky-vpn-dv.json` | covered | none |
| client crate test lifecycle | lifecycle | `python3 ./harness/scripts/test-run.py bucky-vpn unit` | `cargo test -p bucky-vpn` exits 0 | `test-results/test-runs/20260706T073344Z-bucky-vpn-unit.json` | covered | none |
| resolver failure workflow | failure | `python3 ./harness/scripts/test-run.py bucky-vpn unit` | missing route and invalid PN id errors are asserted | `test-results/test-runs/20260706T073344Z-bucky-vpn-unit.json` | covered | none |
| local API config workflow | config | `python3 ./harness/scripts/test-run.py bucky-vpn unit` | default, file config and env override cases pass | `test-results/test-runs/20260706T073344Z-bucky-vpn-unit.json` | covered | none |
| client NodeId base36 workflow | main | `python3 ./harness/scripts/test-run.py bucky-vpn dv` | client crate builds after removing NodeId base58 output | `test-results/test-runs/20260706T073427Z-bucky-vpn-dv.json` | covered | none |
| SN endpoint vector contract | main | `python3 ./harness/scripts/test-run.py bucky-vpn dv` | client crate builds with QUIC/TCP local endpoint vector passed to `P2pConfig::new` and QUIC/TCP remote endpoint vector passed to `P2pSn::new` | `test-results/test-runs/20260706T073427Z-bucky-vpn-dv.json` | covered | none |
| join server_name workflow | config | `python3 ./harness/scripts/test-run.py bucky-vpn unit` | explicit name, domain default, IP default, blank fallback, structured key, and legacy key cases pass | `test-results/test-runs/20260706T073344Z-bucky-vpn-unit.json` | covered | none |

## Integration Tests
| contract_or_flow | modules_involved | success_case | failure_case | test_file | status | gap_manual_reason |
| --- | --- | --- | --- | --- | --- | --- |
| p2p-frame resolver contract | `bucky-vpn`, `p2p-frame` | client crate builds with `set_proxy_route_resolver` and trait implementation | compile failure catches trait drift | `test-results/test-runs/20260626T032458Z-bucky-vpn-dv.json` | covered | none |
| requested PN proxy topologies | `bucky-vpn`, `bucky-vpn-server`, `vpn-frame`, `p2p-frame` | two clients with combined control/PN server, two clients with separate control and PN server, and three clients with two PN servers each run with one Multipass instance per node, block and negatively verify direct client underlay reachability, approve proxy nodes where needed, join, get approved, receive member IPs, restart, register members, create expected TUN devices, block non-PN control underlay paths where applicable, pass virtual IP ping between every client pair sharing a network, and observe non-zero per-member plus user traffic stats on the control API | Multipass availability, instance launch, iptables isolation, direct-underlay negative ping, binary transfer, server startup, account login, proxy approval, PN server selection, network creation, local API readiness, client join, approval, member assignment, restart recovery, VPN info refresh, expected TUN creation, virtual IP ping, PN traffic stats timeout, or PN setup log failure fails the script | `harness/scripts/bucky-vpn-process-integration.py` | covered | none |
| client NodeId base36 consumer compatibility | `bucky-vpn`, `vpn-frame`, `bucky-vpn-server` | client consumes canonical base36 NodeId strings from server-facing flows | stale base58-only client NodeId operation fails build/review | `harness/scripts/test-run.py` | covered | none |
| SN endpoint selection boundary | `bucky-vpn`, `p2p-frame` | client supplies local QUIC/TCP transport endpoints and ordered remote QUIC/TCP SN endpoints to p2p-frame | actual endpoint selection failure remains p2p-frame-owned and is not reimplemented in the client | `vpn-client/src/main.rs`; `vpn-client/src/p2p_vpn.rs` | manual | Integration-level live QUIC/TCP selection would validate p2p-frame behavior, not the client-owned endpoint list construction. |
| SN server name boundary | `bucky-vpn`, `p2p-frame` | client supplies explicit or derived SN remote name to `P2pSn::new`; p2p-frame owns live certificate/SNI validation | actual remote certificate mismatch remains p2p-frame-owned and is not reimplemented in the client | `vpn-client/src/main.rs`; `vpn-client/src/cli.rs`; `vpn-client/src/api.rs`; `vpn-client/src/p2p_vpn.rs` | covered | none |

## Definition of Done
- [x] Testing metadata maps `CHG-client-pn-proxy-route-resolver` to unit, DV, and integration validation.
- [x] Unified test entrypoint produced bucky-vpn unit and DV run artifacts after resolver unit tests were added.
- [x] PN proxy multi-topology Multipass integration script is reachable through the unified integration entry.
- [x] Direct resolver branch coverage is covered by unit tests.
- [x] Configurable local API address is covered by unit tests and Multipass integration.
- [x] Full Multipass-isolated PN proxy topology startup, proxy approval, joined-node approval, member assignment, restart refresh, member registration, client TUN runtime readiness, direct-client-underlay negative checks, client virtual IP data-plane validation, and PN traffic reporting validation are reachable through the integration entry.
- [x] Local P2P endpoint and remote SN endpoint list construction are covered by unit tests and p2p-frame type compatibility is covered by DV.
- [x] Join `server_name` explicit/default/compatibility behavior is covered by unit tests and CLI/API/P2pSn wiring is covered by DV.

## Approval Record
- approver: auto-pipeline
- approval_date: 2026-07-07T00:22:12+08:00
- user_statement: "确认，自动处理后续步骤"
