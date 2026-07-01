---
module: bucky-vpn
version: v0.1
status: approved
approved_by: user-request
approved_at: 2026-07-01T17:44:43+08:00
approved_content_sha256: 18af6c75fc206ca04b440e52230d1e57ce8fa7422273599f943aad7dfca82bb5
---

# bucky-vpn Design

## Design Scope
This design implements `CHG-client-pn-proxy-route-resolver` for the client P2P VPN assembly. The client provides its own PN proxy route resolver implementation and wires it into the existing `p2p_frame::stack::PnProxyRouteResolver` extension point so pntunnel creation resolves each target node to the PN server selected by VPN info.

This design also implements `CHG-client-configurable-local-api-address` for the local daemon API binding used by process-level integration tests. The daemon and CLI resolve a shared local API address from configuration, defaulting to `127.0.0.1:4536`.

This design also implements `CHG-client-node-id-base36`: client-owned NodeId display/logging and server-facing NodeId string handling use base36. P2P ids that are not `NodeId` keep their existing P2P string contract.

## Overall Approach
Keep the shared `vpn-frame` tunnel manager contract unchanged. `vpn-frame` already stores each network route's `PnServerInfo` and uses it in the tunnel pool key. The missing client behavior is that the P2P stack proxy client currently has no route resolver configured, so p2p-frame falls back to a single configured relay or fails when none is configured.

`vpn-client/src/p2p_vpn.rs` will add a client-owned resolver object that maps target `NodeId` / P2P id to the selected PN server P2P id. `P2pVpnTunnelFactory::on_vpn_info_received` updates this resolver from every `NodeVpnInfo` member and the network `pn_server`. `P2pVpnClientFactory::create_client` passes the same resolver to `P2pStackConfig::set_proxy_route_resolver` before creating the stack. When p2p-frame opens a proxy tunnel, it calls the resolver with the target P2P id and receives the PN relay P2P id.

For local API configuration, `vpn-client/src/cli.rs` owns a small `LocalApiConfig` helper because the CLI is the consumer that needs a base URL and the daemon only needs the bind address. The helper reads `VPN_*` environment values and the existing `setting.toml` from the resolved `data.dir`, then applies defaults `api.ip = "127.0.0.1"` and `api.port = 4536`. `run_daemon` uses the bind address when creating `HttpServerConfig`; `Cli::join` and `Cli::get_state` use the helper's base URL for `HttpClient`.

## Simplicity Check
| topic | decision | reason |
| --- | --- | --- |
| new shared trait | Do not add one | `p2p_frame` already exposes `PnProxyRouteResolver`; duplicating a same-name trait in this repo would add adapter noise without new behavior. |
| resolver location | Keep implementation in `vpn-client/src/p2p_vpn.rs` | The resolver is client assembly state derived from `NodeVpnInfo`, not shared protocol logic. |
| route strategy | Use the server-provided network `PnServerInfo` for every target member in that network | This satisfies the approved requirement without adding scoring, health checks, or multi-PN policy. |
| API address config shape | Reuse existing settings/environment mechanisms with `api.ip` and `api.port` | A new config subsystem or random port allocation would add behavior not needed by users; explicit config keeps defaults stable. |

## Current Structure
| path | current_responsibility | change |
| --- | --- | --- |
| `vpn-client/src/p2p_vpn.rs` | Builds P2P stack, tunnel factory, tunnel listener, and VPN client manager glue | Add resolver state, update it from VPN info, and pass it into stack config. |
| `vpn-client/src/p2p_vpn.rs` | Logs and routes client NodeId/P2P identities | Render `NodeId` values as base36 and parse server-provided NodeId strings through the shared base36-compatible contract. |
| `vpn-client/src/main.rs` | Resolves data dir, builds P2P env, loads settings, starts local HTTP API | Use configurable local API bind address when starting the daemon HTTP server. |
| `vpn-client/src/cli.rs` | Implements CLI commands by posting to the local daemon API | Resolve the local daemon API target from the same config/defaults instead of hardcoding `127.0.0.1:4536`. |
| `vpn-frame/src/client/tunnel_manager.rs` | Maintains route table and tunnel pools keyed by target and `PnServerInfo` | No design change; existing PN value remains useful for pool separation. |
| `p2p_frame::stack::PnProxyRouteResolver` | External crate extension point for target-to-relay PN routing | Client implements this trait with local route state. |

## Invariants to Preserve
| invariant | preservation |
| --- | --- |
| Existing direct connection behavior remains available | If a target has no PN route, resolver returns an error and p2p-frame can continue its existing non-proxy tunnel attempts according to stack behavior. |
| Existing VPN info polling and route refresh cadence stays unchanged | Resolver updates happen inside `on_vpn_info_received`, which already runs after `get_vpn_info`. |
| PN server id remains the P2P node id | Resolver parses `PnServerInfo.id` to `P2pId` and does not derive relay identity from endpoint text. |
| Default client settings, join flow, and local HTTP API route shape stay unchanged | With no config, daemon and CLI continue using `127.0.0.1:4536`; only the address becomes configurable. |
| NodeId raw identity remains unchanged | Only text representation in client operations changes to base36; P2P id string behavior remains owned by p2p-frame. |

## Submodules
| submodule | type | responsibility | depends_on |
| --- | --- | --- | --- |
| `p2p-vpn-runtime` | business | Create P2P stack, tunnel factory, listener, and client manager | `pn-proxy-route-resolver` |
| `pn-proxy-route-resolver` | technical | Maintain target-node to PN-relay-id route state and implement p2p-frame resolver trait | none |
| `client-local-api-config` | technical | Resolve daemon bind address and CLI target URL from settings/environment/defaults | none |
| `client-entry` | assembly | Initialize client runtime and expose manager singleton | `p2p-vpn-runtime`, `client-local-api-config` |

## Boundary Rationale
The resolver is a client technical submodule because it adapts VPN info received by the client into the external p2p-frame proxy routing hook. It should not be placed in `vpn-frame`: `vpn-frame` owns shared VPN protocol and generic tunnel management, while this resolver owns client-local p2p stack configuration and route cache state.

## Boundary Decision Matrix
| boundary | classification | business_responsibility | shared_logic_or_technical_area | decision |
| --- | --- | --- | --- | --- |
| Resolver implementation | technical | Pick PN relay id for a target node during client tunnel creation | P2P proxy route adapter | Implement in `vpn-client/src/p2p_vpn.rs` as client-owned route cache. |
| VPN info route source | business | Use network-level selected `PnServerInfo` for member traffic | Existing `NodeVpnInfo` payload | Update resolver from `on_vpn_info_received`; clear stale entries before inserting refreshed routes. |
| Shared `vpn-frame` tunnel contract | shared boundary | Preserve generic tunnel manager behavior | Existing route table and pool key | Do not change `VpnTunnelFactory` or `TunnelManager` for this client-only resolver. |
| p2p-frame proxy hook | external technical area | Route pntunnel through selected PN server | `p2p_frame::stack::PnProxyRouteResolver` | Implement the existing trait instead of creating a competing same-name trait. |
| Local API address resolution | technical | Let multiple client daemons run on one host without changing API routes | Existing config/environment loading | Keep resolution inside `vpn-client` CLI/entry code and do not change `vpn-client/src/api.rs`. |
| NodeId client string boundary | technical | Display and consume stable node identity strings | Shared `NodeId` helper contract | Use base36 for NodeId output and avoid base58-only client operations. |

## Dependency Graph
| source | depends_on | reason | cycle_check |
| --- | --- | --- | --- |
| `client-entry` | `p2p-vpn-runtime` | Manager initialization calls runtime factory setup | acyclic |
| `client-entry` | `client-local-api-config` | Daemon startup needs the local HTTP bind address | acyclic |
| `p2p-vpn-runtime` | `pn-proxy-route-resolver` | Stack config and tunnel factory share resolver state | acyclic |
| `pn-proxy-route-resolver` | none | Route cache has no dependency on higher-level client assembly | acyclic |
| `client-local-api-config` | none | Configuration parsing depends only on local config sources | acyclic |
| `p2p_frame` | `pn-proxy-route-resolver` | External callback invokes resolver trait object | external consumer |

## Key Call Flows
| flow | caller | callee_submodule_path | purpose | failure_handling |
| --- | --- | --- | --- | --- |
| Client creation | `P2pVpnClientFactory::create_client` | `pn-proxy-route-resolver` | Create one resolver, pass it to stack config, and share it with tunnel factory. | If stack creation fails, existing `VpnResult` error mapping returns failure. |
| VPN info refresh | `VpnClient::run_proc` through `P2pVpnTunnelFactory::on_vpn_info_received` | `pn-proxy-route-resolver` | Refresh target member to selected PN server id mappings. | Invalid PN server id returns `VpnErrorCode::InvalidParam`, preventing a route refresh that could use a wrong relay. |
| Pntunnel creation | p2p-frame proxy client | `pn-proxy-route-resolver` | Resolve target P2P id to PN relay P2P id. | Missing target route returns p2p `InvalidParam`; p2p-frame keeps owning final connect failure semantics. |
| Local API address resolution | `run_daemon`, `Cli::join`, `Cli::get_state` | `client-local-api-config` | Use the same configured daemon address for server bind and CLI target. | Invalid or missing config falls back to defaults where practical; invalid port values are ignored by typed config parsing. |

## Large Module Submodule Decision
| submodule | source_proposal | decision | design_packet | reason |
| --- | --- | --- | --- | --- |
| `pn-proxy-route-resolver` | PROP-client-pn-proxy-route-resolver | existing module-level packet, no direct submodule packet | `docs/versions/v0.1/modules/bucky-vpn/design.md` | The resolver is a small technical component inside existing P2P client assembly, represented in one file. |

## Trigger Matrix
| trigger_category | applies | evidence | design_coverage | required_checks | deferred_checks_and_reason |
| --- | --- | --- | --- | --- | --- |
| contract/protocol | yes | Client implements an external p2p-frame trait but does not change VPN protocol. | Interfaces section names the external trait consumer and compatibility. | schema-check; admission-check; cargo check or harness equivalent |  |
| data/schema | no | Uses optional existing `setting.toml` keys but does not require schema migration. | not-applicable: absent settings keep defaults |  |  |
| security/privacy/permission | yes | PN proxy choice changes traffic relay selection. | Resolver only uses server-provided `PnServerInfo.id`; logs avoid endpoint-sensitive details beyond existing identifiers. | targeted build/check plus acceptance review |  |
| runtime/integration | yes | Tunnel creation now depends on resolver route state from VPN info refresh. | Key call flows cover client creation, refresh, and pntunnel creation. | `test-run.py bucky-vpn all` or documented narrower Rust check |  |
| build/dependency/config/deployment | yes | Local API bind/target address is configurable through settings/environment. | Config keys and defaults are listed in data/state and interfaces sections. | process integration script plus targeted client build/test |  |
| ui/datamodel/workflow | yes | CLI `join` and `state` target a configurable local daemon URL. | Default workflow remains `127.0.0.1:4536`; configured workflow supports multiple daemon instances. | process integration script exercises configured ports |  |
| contract/protocol | yes | Client consumes NodeId strings received from server APIs and logs local NodeId values. | Base36 is the canonical NodeId text format; P2P ids remain separate. | cargo check/test for `bucky-vpn` | owner: shared server contract; risk: stale base58-only server data may fail earlier in the server boundary |
| harness/process | yes | Auto-pipeline is active for this change. | pipeline plan records this design, implementation, testing, and acceptance flow. | pipeline-plan-check; stage-scope-check |  |

## Directly Mapped Change Items
| change_id | proposal_id | design_coverage | scope_paths |
| --- | --- | --- | --- |
| CHG-client-pn-proxy-route-resolver | PROP-client-pn-proxy-route-resolver | Add a client-owned p2p-frame `PnProxyRouteResolver` implementation, refresh its route cache from `NodeVpnInfo`, and wire it into `P2pStackConfig` before pntunnel creation. | `vpn-client/src/p2p_vpn.rs` |
| CHG-client-configurable-local-api-address | PROP-client-configurable-local-api-address | Add shared local API address resolution for daemon bind and CLI target, preserving default `127.0.0.1:4536` while allowing configured ports for multi-client process tests. | `vpn-client/src/main.rs`, `vpn-client/src/cli.rs` |
| CHG-client-node-id-base36 | PROP-client-node-id-base36 | Update client NodeId logs and NodeId-derived resolver inputs to use base36 canonical NodeId strings, without changing non-NodeId P2P id contracts. | `vpn-client/src/p2p_vpn.rs` |

## Implementation Order
| phase | goal | prerequisite | output | dependency | parallel |
| --- | --- | --- | --- | --- | --- |
| 1 | Add resolver state type | approved design and admission | route cache maps target P2P id to relay P2P id | none | no |
| 2 | Wire resolver into P2P stack config and tunnel factory | phase 1 | one resolver shared by stack proxy client and factory refresh path | 1 | no |
| 3 | Refresh resolver from VPN info | phase 2 | routes update when server VPN info changes | 2 | no |
| 4 | Add local API address config resolver | approved config-address change and admission | helper returns bind address and CLI base URL with stable defaults | none | no |
| 5 | Use configured API address in daemon and CLI | phase 4 | daemon listen and CLI target can use per-process ports | 4 | no |
| 6 | Validate compile/runtime contract | implementation complete | run artifact or documented blocker | 1,2,3,4,5 | no |
| 7 | Normalize client NodeId text operations | approved base36 admission | client NodeId display/logging no longer emits base58 | none | yes |

## Key Decisions
| decision | chosen | alternatives_considered | rejection_reason |
| --- | --- | --- | --- |
| Resolver API | Implement `p2p_frame::stack::PnProxyRouteResolver` | Add a new repo-local trait named `PnProxyRouteResolver` | The external crate already owns the hook consumed during proxy tunnel creation. |
| Route cache key | Target `P2pId` derived from member `NodeId` | Key by IP address or network id only | p2p-frame asks for target P2P id, so IP/network-only keys cannot answer the callback. |
| Missing route behavior | Return p2p error instead of choosing an arbitrary relay | Fallback to SN or first known PN server | Arbitrary fallback could send traffic through the wrong proxy and violate the requirement. |
| Local API config keys | Use `api.ip` and `api.port` with `VPN_API_IP` / `VPN_API_PORT` overrides | Add a new CLI flag for each command | Config/env works for daemon and automated process tests without changing command syntax. |
| Client NodeId output | base36 canonical | keep base58 log output for debugging | User requested all NodeId operations use base36; dual-format logs would preserve the old output. |

## Data and State
| data_or_state | owner_submodule | access_for_others | state_transitions |
| --- | --- | --- | --- |
| target-to-relay route cache | `pn-proxy-route-resolver` | Read through p2p-frame resolver callback; written only by `on_vpn_info_received` | empty on client creation -> populated on VPN info refresh -> replaced on later refresh -> missing for networks without PN server |
| resolver trait object | `p2p-vpn-runtime` | Passed to P2P stack config and tunnel factory | created with client -> shared for client lifetime -> dropped with client |
| local API config | `client-local-api-config` | `run_daemon` consumes bind `ip:port`; CLI consumes base URL | absent config -> default `127.0.0.1:4536` -> explicit setting/env changes bind and target address |
| client NodeId text | `p2p-vpn-runtime` | Logs and resolver refresh paths read/write NodeId strings | raw NodeId bytes -> base36 output; server-provided NodeId text -> shared parse path |

## Testability
| seam | verification |
| --- | --- |
| Route cache update | Unit-style checks can construct resolver and feed synthetic `NodeVpnInfo` if testing stage adds tests. |
| Trait implementation | Rust compile check verifies p2p-frame trait signature and stack config wiring. |
| Missing/invalid PN id handling | Testing stage can document or add targeted coverage around invalid `PnServerInfo.id` if feasible. |
| Runtime integration | Harness client all or cargo check can catch broken client assembly. |
| Configured API address | Process integration script can set `VPN_API_PORT` per client and verify daemon readiness through that port. |
| NodeId base36 output | Compile/check and log call-site review verify client code no longer formats NodeId through base58. |

## Interfaces and Dependencies
| interface | consumer | compatibility | notes |
| --- | --- | --- | --- |
| `p2p_frame::stack::PnProxyRouteResolver` implementation | p2p-frame proxy client, `CHG-client-pn-proxy-route-resolver` | new | Client provides an implementation; external trait shape is unchanged. |
| `P2pVpnTunnelFactory::on_vpn_info_received` route refresh | `VpnClient::run_proc`, `CHG-client-pn-proxy-route-resolver` | backward-compatible | Existing callback remains; implementation adds resolver cache update before/with PN server connection setup. |
| `P2pStackConfig::set_proxy_route_resolver` usage | `P2pVpnClientFactory::create_client`, `CHG-client-pn-proxy-route-resolver` | backward-compatible | Uses existing p2p-frame builder method; no config file change. |
| Local API address configuration | `run_daemon`, `Cli::join`, `Cli::get_state`, `CHG-client-configurable-local-api-address` | backward-compatible | Default stays `127.0.0.1:4536`; configured `api.ip` / `api.port` or `VPN_API_IP` / `VPN_API_PORT` changes bind/target address. |
| Client NodeId text operations | `p2p-vpn-runtime`, `CHG-client-node-id-base36` | migration-required | NodeId output is base36; P2P id strings are not redefined by this module. |

## Document Index
| document | topic | scope |
| --- | --- | --- |
| `design.md` | Client PN proxy route resolver | full change |

## Risks and Rollback
| risk | mitigation | rollback |
| --- | --- | --- |
| Route cache can become stale across VPN info versions | Replace cache contents on each VPN info refresh. | Remove resolver wiring and cache update. |
| Invalid PN server id from server data prevents proxy routing | Log and skip invalid routes so the client does not choose the wrong relay. | Revert resolver implementation or handle invalid data in server-side follow-up. |
| Existing p2p-frame fallback semantics differ by version | Use the external trait exactly as exposed by current lockfile and verify with compile check. | Remove `set_proxy_route_resolver` call and restore current stack config. |
| CLI and daemon could resolve different API addresses | Share one helper for config parsing and base URL generation. | Revert to fixed `127.0.0.1:4536` address. |
| Mixed base58/base36 NodeId logs obscure diagnosis | Emit only base36 NodeId strings from client-owned NodeId call sites. | Re-enable base58 output only with a new approved requirement. |

## Approval Record
- approver: user-request
- approval_date: 2026-07-01T17:44:43+08:00
- user_statement: "确认，自动处理后续步骤"
