---
module: bucky-vpn
version: v0.1
status: approved
approved_by: auto-pipeline
approved_at: 2026-07-06T16:09:15+08:00
approved_content_sha256: 3613060c1ad38cb395146c6551171a1519549ed8378642ea54f3e96a5578f4c0
---

# bucky-vpn Design

## Design Scope
This design implements `CHG-client-pn-proxy-route-resolver` for the client P2P VPN assembly. The client provides its own PN proxy route resolver implementation and wires it into the existing `p2p_frame::stack::PnProxyRouteResolver` extension point so pntunnel creation resolves each target node to the PN server selected by VPN info.

This design also implements `CHG-client-configurable-local-api-address` for the local daemon API binding used by process-level integration tests. The daemon and CLI resolve a shared local API address from configuration, defaulting to `127.0.0.1:4536`.

This design also implements `CHG-client-node-id-base36`: client-owned NodeId display/logging and server-facing NodeId string handling use base36. P2P ids that are not `NodeId` keep their existing P2P string contract.

This design also implements `CHG-client-sn-quic-tcp-priority`: client SN stack configuration provides both QUIC and TCP endpoints for the same SN address, with QUIC listed before TCP. The client P2P environment also listens on both QUIC and TCP so p2p-frame has a matching local transport for TCP SN endpoints. Endpoint selection and connection retry behavior remain owned by p2p-frame.

This design also implements `CHG-client-join-server-name-for-sn`: the `join` command and local `/join` API accept an optional `server_name` for the SN remote name passed to `P2pSn::new`. If the user omits it, a domain `server` value becomes the default `server_name`; an IP `server` value defaults to `server_id`.

This design also implements `CHG-client-pn-proxy-reported-name`: when the server-provided `PnServerInfo` contains a non-empty `name`, the client uses that name as the PN proxy remote name when connecting to the proxy node. Missing names preserve the existing fallback to the PN proxy P2P id.

## Overall Approach
Keep the shared `vpn-frame` tunnel manager contract unchanged. `vpn-frame` already stores each network route's `PnServerInfo` and uses it in the tunnel pool key. The missing client behavior is that the P2P stack proxy client currently has no route resolver configured, so p2p-frame falls back to a single configured relay or fails when none is configured.

`vpn-client/src/p2p_vpn.rs` will add a client-owned resolver object that maps target `NodeId` / P2P id to the selected PN server P2P id. `P2pVpnTunnelFactory::on_vpn_info_received` updates this resolver from every `NodeVpnInfo` member and the network `pn_server`. `P2pVpnClientFactory::create_client` passes the same resolver to `P2pStackConfig::set_proxy_route_resolver` before creating the stack. When p2p-frame opens a proxy tunnel, it calls the resolver with the target P2P id and receives the PN relay P2P id.

For PN proxy connection naming, `P2pVpnTunnelFactory::connect_pn_server` keeps deriving `remote_id` from `PnServerInfo.id` and endpoints from `ip/port/addresses`, but computes `remote_name` from `PnServerInfo.name` when the value is present and not blank. If `name` is absent or blank, the client uses `remote_id.to_string()` so old control nodes and old proxy nodes remain connectable.

For local API configuration, `vpn-client/src/cli.rs` owns a small `LocalApiConfig` helper because the CLI is the consumer that needs a base URL and the daemon only needs the bind address. The helper reads `VPN_*` environment values and the existing `setting.toml` from the resolved `data.dir`, then applies defaults `api.ip = "127.0.0.1"` and `api.port = 4536`. `run_daemon` uses the bind address when creating `HttpServerConfig`; `Cli::join` and `Cli::get_state` use the helper's base URL for `HttpClient`.

For SN transport setup, `run_daemon` will construct local P2P listen endpoints for both QUIC and TCP using the configured `p2p.port`, and `P2pVpnClientFactory::create_client` will construct a small ordered remote SN endpoint list from the resolved SN socket address: first `Protocol::Quic`, then `Protocol::Tcp`, both using the same IP and port. The remote list is passed unchanged to `P2pSn::new`; p2p-frame remains responsible for deciding which endpoint it uses to establish the connection.

For SN server name setup, `main.rs` adds a `--server_name` option to `join`, `cli.rs` forwards it through the local API, and `api.rs` persists it on `JoinRecord`. `p2p_vpn.rs` uses a structured client key carrying `server_id`, `server`, `server_port`, and optional `server_name` instead of parsing only `sn_id_host:port`; old string keys remain parseable as a compatibility fallback. `P2pVpnClientFactory::create_client` computes the effective SN name from the explicit non-empty `server_name`, otherwise from domain `server`, otherwise from `server_id`, and passes that value to `P2pSn::new`.

## Simplicity Check
| topic | decision | reason |
| --- | --- | --- |
| new shared trait | Do not add one | `p2p_frame` already exposes `PnProxyRouteResolver`; duplicating a same-name trait in this repo would add adapter noise without new behavior. |
| resolver location | Keep implementation in `vpn-client/src/p2p_vpn.rs` | The resolver is client assembly state derived from `NodeVpnInfo`, not shared protocol logic. |
| route strategy | Use the server-provided network `PnServerInfo` for every target member in that network | This satisfies the approved requirement without adding scoring, health checks, or multi-PN policy. |
| API address config shape | Reuse existing settings/environment mechanisms with `api.ip` and `api.port` | A new config subsystem or random port allocation would add behavior not needed by users; explicit config keeps defaults stable. |
| SN transport selection | Provide local QUIC/TCP P2P endpoints and ordered remote QUIC/TCP SN endpoints to p2p-frame | Reimplementing endpoint selection in the client would duplicate p2p-frame behavior and expand this change beyond the requested scope. |
| SN server name source | Add `server_name` to join input instead of global config or reusing `--name` | SN name is per joined server and distinct from network member display name; a global setting or overloaded flag would be ambiguous. |
| PN proxy remote name | Use `PnServerInfo.name` with id fallback | This reuses the server-reported metadata and avoids adding a client-side proxy name registry. |

## Current Structure
| path | current_responsibility | change |
| --- | --- | --- |
| `vpn-client/src/p2p_vpn.rs` | Builds P2P stack, tunnel factory, tunnel listener, and VPN client manager glue | Add resolver state, update it from VPN info, and pass it into stack config. |
| `vpn-client/src/p2p_vpn.rs` | Logs and routes client NodeId/P2P identities | Render `NodeId` values as base36 and parse server-provided NodeId strings through the shared base36-compatible contract. |
| `vpn-client/src/main.rs` | Resolves data dir, builds P2P env, loads settings, starts local HTTP API | Use configurable local API bind address when starting the daemon HTTP server; build local P2P endpoints as QUIC and TCP on `p2p.port`. |
| `vpn-client/src/cli.rs` | Implements CLI commands by posting to the local daemon API | Resolve the local daemon API target from the same config/defaults instead of hardcoding `127.0.0.1:4536`. |
| `vpn-client/src/p2p_vpn.rs` | Creates the client P2P stack and registers SN information with p2p-frame | Build the SN endpoint list as QUIC then TCP for the resolved SN address. |
| `vpn-client/src/p2p_vpn.rs` | Connects to PN proxy nodes from server-provided `PnServerInfo` | Use reported `name` as `TtpTarget.remote_name` when present; keep `id` as `remote_id`. |
| `vpn-client/src/main.rs`, `vpn-client/src/cli.rs`, `vpn-client/src/api.rs`, `vpn-client/src/p2p_vpn.rs` | Carry join input into the daemon-managed client factory | Add optional `server_name`, persist it with joined networks, and use it as the SN remote name during stack creation. |
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
| SN join key and identity directory naming stay unchanged | The client still parses `sn_id_host:port` and stores identity under the same `sn_id_ip_port` directory. |
| Client does not own SN endpoint selection | The client only supplies ordered endpoints to `P2pSn`; p2p-frame owns connection choice. |
| Default P2P port semantics stay unchanged | QUIC and TCP both use the existing `p2p.port` value, so no new config key or identity directory dimension is introduced. |
| Existing `join --name` remains the network member name | SN remote name uses the new `--server_name`; `--name` continues to be passed only to `vpn_client.join`. |
| Existing joined records remain readable | Missing `server_name` in old `joined_networks` records defaults to `None` and then follows the domain/IP fallback rule. |
| PN proxy identity remains id-based | `PnServerInfo.name` affects only the P2P remote name parameter; route resolver keys, remote id parsing, and endpoints still use `id/ip/port/addresses`. |

## Submodules
| submodule | type | responsibility | depends_on |
| --- | --- | --- | --- |
| `p2p-vpn-runtime` | business | Create P2P stack, tunnel factory, listener, and client manager | `pn-proxy-route-resolver` |
| `pn-proxy-route-resolver` | technical | Maintain target-node to PN-relay-id route state and implement p2p-frame resolver trait | none |
| `sn-client-transport` | technical | Construct local P2P listen endpoints and ordered remote SN endpoint lists for p2p-frame stack configuration | none |
| `sn-server-name` | technical | Resolve optional join `server_name` into the SN remote name passed to `P2pSn::new` | none |
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
| SN transport endpoint list | technical | Provide p2p-frame with QUIC and TCP local P2P endpoints plus remote SN endpoints for the resolved SN address | Existing `P2pConfig` endpoint vector and `P2pSn` endpoint vector | Construct `[QUIC, TCP]` locally and for SN stack setup, and do not implement client-side endpoint selection. |
| SN server name | technical | Provide p2p-frame with the intended SN remote name for command tunnel establishment | `P2pSn::new` name parameter and join input | Add a dedicated `server_name` join field; default from domain server or `server_id` when omitted. |
| Local API address resolution | technical | Let multiple client daemons run on one host without changing API routes | Existing config/environment loading | Keep resolution inside `vpn-client` CLI/entry code and do not change `vpn-client/src/api.rs`. |
| NodeId client string boundary | technical | Display and consume stable node identity strings | Shared `NodeId` helper contract | Use base36 for NodeId output and avoid base58-only client operations. |
| PN proxy remote name | technical | Provide p2p-frame with the proxy node name reported by the control node | `TtpTarget.remote_name` and `PnServerInfo.name` | Use reported `name` for PN proxy connect while keeping `remote_id` from `id`. |

## Dependency Graph
| source | depends_on | reason | cycle_check |
| --- | --- | --- | --- |
| `client-entry` | `p2p-vpn-runtime` | Manager initialization calls runtime factory setup | acyclic |
| `client-entry` | `client-local-api-config` | Daemon startup needs the local HTTP bind address | acyclic |
| `p2p-vpn-runtime` | `pn-proxy-route-resolver` | Stack config and tunnel factory share resolver state | acyclic |
| `client-entry` | `sn-client-transport` | Daemon startup needs local QUIC/TCP P2P endpoints before creating the P2P env | acyclic |
| `p2p-vpn-runtime` | `sn-client-transport` | Stack config needs the ordered remote SN endpoint list before calling `add_sn` | acyclic |
| `p2p-vpn-runtime` | `sn-server-name` | Stack config needs the effective SN remote name before calling `add_sn` | acyclic |
| `p2p-vpn-runtime` | `pn-proxy-route-resolver` | PN proxy connect uses selected `PnServerInfo` and reported name metadata. | acyclic |
| `pn-proxy-route-resolver` | none | Route cache has no dependency on higher-level client assembly | acyclic |
| `sn-client-transport` | none | Endpoint construction depends only on the resolved SN socket address | acyclic |
| `sn-server-name` | none | Name resolution depends only on join input and IP/domain classification | acyclic |
| `client-local-api-config` | none | Configuration parsing depends only on local config sources | acyclic |
| `p2p_frame` | `pn-proxy-route-resolver` | External callback invokes resolver trait object | external consumer |

## Key Call Flows
| flow | caller | callee_submodule_path | purpose | failure_handling |
| --- | --- | --- | --- | --- |
| Client creation | `P2pVpnClientFactory::create_client` | `pn-proxy-route-resolver` | Create one resolver, pass it to stack config, and share it with tunnel factory. | If stack creation fails, existing `VpnResult` error mapping returns failure. |
| Local P2P endpoint registration | `run_daemon` | `sn-client-transport` | Build `[QUIC, TCP]` local listen endpoints on `p2p.port` before calling `create_p2p_env`. | If p2p-frame cannot listen on either endpoint, `create_p2p_env(...).await.unwrap()` preserves the existing daemon startup failure semantics. |
| SN endpoint registration | `P2pVpnClientFactory::create_client` | `sn-client-transport` | Build `[QUIC, TCP]` endpoints for the resolved SN address and pass them to `P2pSn::new`. | Socket address resolution errors keep the existing `VpnResult` failure path; p2p-frame owns later connection choice. |
| SN server name registration | `P2pVpnClientFactory::create_client` | `sn-server-name` | Compute effective SN name from `server_name`, domain server, or `server_id`, then pass it to `P2pSn::new`. | Empty `server_name` is treated as absent; DNS/socket errors remain separate from name selection and keep existing `VpnResult` behavior. |
| VPN info refresh | `VpnClient::run_proc` through `P2pVpnTunnelFactory::on_vpn_info_received` | `pn-proxy-route-resolver` | Refresh target member to selected PN server id mappings. | Invalid PN server id returns `VpnErrorCode::InvalidParam`, preventing a route refresh that could use a wrong relay. |
| Pntunnel creation | p2p-frame proxy client | `pn-proxy-route-resolver` | Resolve target P2P id to PN relay P2P id. | Missing target route returns p2p `InvalidParam`; p2p-frame keeps owning final connect failure semantics. |
| PN proxy connection | `P2pVpnTunnelFactory::connect_pn_server` | `p2p-vpn-runtime` | Connect to selected PN proxy endpoint with `remote_id` from `PnServerInfo.id` and remote name from `PnServerInfo.name` if present. | Invalid id still returns `VpnErrorCode::InvalidParam`; missing name falls back to id; connect failures keep existing address retry behavior. |
| Local API address resolution | `run_daemon`, `Cli::join`, `Cli::get_state` | `client-local-api-config` | Use the same configured daemon address for server bind and CLI target. | Invalid or missing config falls back to defaults where practical; invalid port values are ignored by typed config parsing. |

## Large Module Submodule Decision
| submodule | source_proposal | decision | design_packet | reason |
| --- | --- | --- | --- | --- |
| `pn-proxy-route-resolver` | PROP-client-pn-proxy-route-resolver | existing module-level packet, no direct submodule packet | `docs/versions/v0.1/modules/bucky-vpn/design.md` | The resolver is a small technical component inside existing P2P client assembly, represented in one file. |
| `sn-client-transport` | PROP-client-sn-quic-tcp-priority | existing module-level packet, no direct submodule packet | `docs/versions/v0.1/modules/bucky-vpn/design.md` | The endpoint list construction is a narrow technical change inside existing P2P client assembly. |
| `sn-server-name` | PROP-client-join-server-name-for-sn | existing module-level packet, no direct submodule packet | `docs/versions/v0.1/modules/bucky-vpn/design.md` | The join/API field and P2P stack name selection are narrow additions to existing client assembly paths. |
| `pn-proxy-remote-name` | PROP-client-pn-proxy-reported-name | existing module-level packet, no direct submodule packet | `docs/versions/v0.1/modules/bucky-vpn/design.md` | The change is a narrow use of metadata already carried by `PnServerInfo` inside the existing PN connect function. |

## Trigger Matrix
| trigger_category | applies | evidence | design_coverage | required_checks | deferred_checks_and_reason |
| --- | --- | --- | --- | --- | --- |
| contract/protocol | yes | Client implements an external p2p-frame trait but does not change VPN protocol. | Interfaces section names the external trait consumer and compatibility. | schema-check; admission-check; cargo check or harness equivalent |  |
| contract/protocol | yes | Client now supplies both QUIC and TCP local P2P endpoints and remote SN endpoints to p2p-frame. | Interfaces section names `P2pConfig::new` local endpoint vector and `P2pSn::new` remote endpoint vector as p2p-frame consumer boundaries; client does not own endpoint selection. | schema-check; admission-check; cargo check or harness equivalent |  |
| contract/protocol | yes | Client now supplies a caller-selected or derived SN remote name to p2p-frame. | Interfaces section names `P2pSn::new` name parameter and the compatibility rule for omitted values. | schema-check; admission-check; unit/DV |  |
| data/schema | yes | Local `/join` request and `joined_networks` gain optional `server_name`; no VPN protocol or server DB schema changes. | Data and State records `server_name` ownership, optional persistence, and old-record fallback. | unit tests for defaulting and serde compatibility |  |
| security/privacy/permission | yes | PN proxy choice changes traffic relay selection. | Resolver only uses server-provided `PnServerInfo.id`; logs avoid endpoint-sensitive details beyond existing identifiers. | targeted build/check plus acceptance review |  |
| runtime/integration | yes | Tunnel creation now depends on resolver route state from VPN info refresh. | Key call flows cover client creation, refresh, and pntunnel creation. | `test-run.py bucky-vpn all` or documented narrower Rust check |  |
| runtime/integration | yes | Client online behavior depends on p2p-frame consuming local P2P endpoints and remote SN endpoint list. | Key call flows cover local endpoint list construction before env creation and remote SN endpoint list construction before stack creation. | `cargo check -p bucky-vpn` or harness DV; testing stage records endpoint/listener validation |  |
| runtime/integration | yes | Client online behavior can depend on p2p-frame using the expected SN remote name for tunnel establishment. | Key call flows cover `server_name` resolution before stack creation. | unit tests plus bucky-vpn DV |  |
| contract/protocol | yes | Client consumes `PnServerInfo.name` from the control-node response and passes it to the P2P connect target. | Key call flows keep id as identity and name as remote-name metadata. | schema-check; admission-check; cargo check/test for `bucky-vpn` |  |
| build/dependency/config/deployment | yes | Local API bind/target address is configurable through settings/environment. | Config keys and defaults are listed in data/state and interfaces sections. | process integration script plus targeted client build/test |  |
| ui/datamodel/workflow | yes | CLI `join` and `state` target a configurable local daemon URL; `join` also accepts optional `--server_name`. | Default workflow remains `127.0.0.1:4536`; configured workflow supports multiple daemon instances; `--name` remains network member name. | process integration script exercises configured ports; unit tests cover name defaulting |  |
| contract/protocol | yes | Client consumes NodeId strings received from server APIs and logs local NodeId values. | Base36 is the canonical NodeId text format; P2P ids remain separate. | cargo check/test for `bucky-vpn` | owner: shared server contract; risk: stale base58-only server data may fail earlier in the server boundary |
| harness/process | yes | Auto-pipeline is active for this change. | pipeline plan records this design, implementation, testing, and acceptance flow. | pipeline-plan-check; stage-scope-check |  |

## Directly Mapped Change Items
| change_id | proposal_id | design_coverage | scope_paths |
| --- | --- | --- | --- |
| CHG-client-pn-proxy-route-resolver | PROP-client-pn-proxy-route-resolver | Add a client-owned p2p-frame `PnProxyRouteResolver` implementation, refresh its route cache from `NodeVpnInfo`, and wire it into `P2pStackConfig` before pntunnel creation. | `vpn-client/src/p2p_vpn.rs` |
| CHG-client-configurable-local-api-address | PROP-client-configurable-local-api-address | Add shared local API address resolution for daemon bind and CLI target, preserving default `127.0.0.1:4536` while allowing configured ports for multi-client process tests. | `vpn-client/src/main.rs`, `vpn-client/src/cli.rs` |
| CHG-client-node-id-base36 | PROP-client-node-id-base36 | Update client NodeId logs and NodeId-derived resolver inputs to use base36 canonical NodeId strings, without changing non-NodeId P2P id contracts. | `vpn-client/src/p2p_vpn.rs` |
| CHG-client-sn-quic-tcp-priority | PROP-client-sn-quic-tcp-priority | Build local P2P endpoints as QUIC and TCP on `p2p.port`; build the remote SN endpoint vector as QUIC first and TCP second for the resolved SN socket address, then pass it to `P2pSn::new` without adding client-side endpoint selection. | `vpn-client/src/main.rs`, `vpn-client/src/p2p_vpn.rs` |
| CHG-client-join-server-name-for-sn | PROP-client-join-server-name-for-sn | Add optional `server_name` to join CLI/API and persisted join records; carry it through the client factory key; pass the effective name to `P2pSn::new`, defaulting to domain server or `server_id` for IP servers. | `vpn-client/src/main.rs`, `vpn-client/src/cli.rs`, `vpn-client/src/api.rs`, `vpn-client/src/p2p_vpn.rs` |
| CHG-client-pn-proxy-reported-name | PROP-client-pn-proxy-reported-name | Use non-empty `PnServerInfo.name` as PN proxy `remote_name` during connect, while preserving `id` as `remote_id` and falling back to id when name is absent. | `vpn-client/src/p2p_vpn.rs` |

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
| 8 | Add local and remote SN TCP endpoint registration | approved SN endpoint admission | `P2pConfig` listens on QUIC/TCP and `P2pSn` receives QUIC/TCP remote endpoints in that order | none | yes |
| 9 | Add join server_name support for SN remote name | approved server-name admission | CLI/API/persisted join records carry optional `server_name`; `P2pSn::new` receives explicit or default effective name | none | yes |
| 10 | Use reported PN proxy name during proxy connect | approved reported-name admission | `TtpTarget.remote_name` uses `PnServerInfo.name` when present and id fallback otherwise | none | yes |

## Key Decisions
| decision | chosen | alternatives_considered | rejection_reason |
| --- | --- | --- | --- |
| Resolver API | Implement `p2p_frame::stack::PnProxyRouteResolver` | Add a new repo-local trait named `PnProxyRouteResolver` | The external crate already owns the hook consumed during proxy tunnel creation. |
| Route cache key | Target `P2pId` derived from member `NodeId` | Key by IP address or network id only | p2p-frame asks for target P2P id, so IP/network-only keys cannot answer the callback. |
| Missing route behavior | Return p2p error instead of choosing an arbitrary relay | Fallback to SN or first known PN server | Arbitrary fallback could send traffic through the wrong proxy and violate the requirement. |
| Local API config keys | Use `api.ip` and `api.port` with `VPN_API_IP` / `VPN_API_PORT` overrides | Add a new CLI flag for each command | Config/env works for daemon and automated process tests without changing command syntax. |
| Client NodeId output | base36 canonical | keep base58 log output for debugging | User requested all NodeId operations use base36; dual-format logs would preserve the old output. |
| SN endpoint handling | Provide `[QUIC, TCP]` local and remote endpoints to p2p-frame | Try QUIC then TCP manually in client code | p2p-frame already owns endpoint selection and connection establishment; duplicating it would broaden the client responsibility. |
| SN server name handling | Add separate `server_name` join input and persist it | Reuse `join --name`; add global config; always use `server_id` | `--name` already means network member display name; global config is wrong for multiple joined servers; always using `server_id` fails domain-name certificate/SNI scenarios. |
| PN proxy remote name handling | Use `PnServerInfo.name` with id fallback | Always use `id`; reuse join `server_name` | Always using id ignores the proxy-reported name; join `server_name` names the SN control node, not the selected PN proxy. |

## Data and State
| data_or_state | owner_submodule | access_for_others | state_transitions |
| --- | --- | --- | --- |
| target-to-relay route cache | `pn-proxy-route-resolver` | Read through p2p-frame resolver callback; written only by `on_vpn_info_received` | empty on client creation -> populated on VPN info refresh -> replaced on later refresh -> missing for networks without PN server |
| resolver trait object | `p2p-vpn-runtime` | Passed to P2P stack config and tunnel factory | created with client -> shared for client lifetime -> dropped with client |
| local API config | `client-local-api-config` | `run_daemon` consumes bind `ip:port`; CLI consumes base URL | absent config -> default `127.0.0.1:4536` -> explicit setting/env changes bind and target address |
| client NodeId text | `p2p-vpn-runtime` | Logs and resolver refresh paths read/write NodeId strings | raw NodeId bytes -> base36 output; server-provided NodeId text -> shared parse path |
| local P2P endpoint vector | `sn-client-transport` | Passed once to `P2pConfig::new` during daemon startup | configured `p2p.port` -> local QUIC/TCP listen endpoint list -> p2p-frame listener registration |
| remote SN endpoint vector | `sn-client-transport` | Passed once to `P2pSn::new` during stack creation | resolved SN socket address -> ordered QUIC/TCP endpoint list -> p2p-frame connection selection |
| join server_name | `sn-server-name` | `Cli::join` and `/join` write optional value; `P2pVpnClientFactory::create_client` reads it from the structured client key | absent old/new record -> domain/IP fallback; explicit non-empty value -> passed to `P2pSn::new`; blank value -> treated as absent |
| PN proxy reported name | `p2p-vpn-runtime` | `connect_pn_server` reads from `PnServerInfo` returned by control node | absent/blank -> remote_name id fallback；present -> remote_name uses reported name；id and route cache remain unchanged |

## Testability
| seam | verification |
| --- | --- |
| Route cache update | Unit-style checks can construct resolver and feed synthetic `NodeVpnInfo` if testing stage adds tests. |
| Trait implementation | Rust compile check verifies p2p-frame trait signature and stack config wiring. |
| Missing/invalid PN id handling | Testing stage can document or add targeted coverage around invalid `PnServerInfo.id` if feasible. |
| Runtime integration | Harness client all or cargo check can catch broken client assembly. |
| Configured API address | Process integration script can set `VPN_API_PORT` per client and verify daemon readiness through that port. |
| NodeId base36 output | Compile/check and log call-site review verify client code no longer formats NodeId through base58. |
| local and remote SN endpoint construction | Focused unit-style checks can assert endpoint count and ordering without requiring a live SN connection. |
| SN server name defaulting | Unit tests can assert explicit value, domain default, IP default, blank fallback, and old-key compatibility without live SN. |
| PN proxy remote name fallback | Focused tests can construct `PnServerInfo` with present/blank/missing name and assert the chosen remote name without needing a live proxy. |

## Interfaces and Dependencies
| interface | consumer | compatibility | notes |
| --- | --- | --- | --- |
| `p2p_frame::stack::PnProxyRouteResolver` implementation | p2p-frame proxy client, `CHG-client-pn-proxy-route-resolver` | new | Client provides an implementation; external trait shape is unchanged. |
| `P2pVpnTunnelFactory::on_vpn_info_received` route refresh | `VpnClient::run_proc`, `CHG-client-pn-proxy-route-resolver` | backward-compatible | Existing callback remains; implementation adds resolver cache update before/with PN server connection setup. |
| `P2pStackConfig::set_proxy_route_resolver` usage | `P2pVpnClientFactory::create_client`, `CHG-client-pn-proxy-route-resolver` | backward-compatible | Uses existing p2p-frame builder method; no config file change. |
| Local API address configuration | `run_daemon`, `Cli::join`, `Cli::get_state`, `CHG-client-configurable-local-api-address` | backward-compatible | Default stays `127.0.0.1:4536`; configured `api.ip` / `api.port` or `VPN_API_IP` / `VPN_API_PORT` changes bind/target address. |
| Client NodeId text operations | `p2p-vpn-runtime`, `CHG-client-node-id-base36` | migration-required | NodeId output is base36; P2P id strings are not redefined by this module. |
| `P2pConfig::new` local endpoint vector | p2p-frame env, `CHG-client-sn-quic-tcp-priority` | backward-compatible | Client changes the local P2P endpoint list from one QUIC endpoint to QUIC plus TCP on the same configured `p2p.port`; p2p-frame registers available listeners. |
| `P2pSn::new` endpoint vector | p2p-frame stack, `CHG-client-sn-quic-tcp-priority` | backward-compatible | Client changes the endpoint list from one QUIC endpoint to QUIC plus TCP for the same socket address; p2p-frame chooses the actual connection endpoint. |
| Join `server_name` field and CLI `--server_name` | `CHG-client-join-server-name-for-sn`, local daemon API, CLI users | backward-compatible | The field is optional; omitted old clients and old persisted records default to domain server or `server_id`. |
| `P2pSn::new` name parameter | p2p-frame stack, `CHG-client-join-server-name-for-sn` | backward-compatible | Existing IP-based joins keep using `server_id`; domain joins use the domain by default unless explicitly overridden. |
| `TtpTarget.remote_name` for PN proxy connect | p2p-frame TTP client, `CHG-client-pn-proxy-reported-name` | backward-compatible | Present `PnServerInfo.name` is used as the remote name; absent or blank values use the current id fallback. |

## Document Index
| document | topic | scope |
| --- | --- | --- |
| `design.md` | Client PN proxy route resolver | full change |
| `design.md` | Join server_name for SN remote name | `CHG-client-join-server-name-for-sn` |

## Risks and Rollback
| risk | mitigation | rollback |
| --- | --- | --- |
| Route cache can become stale across VPN info versions | Replace cache contents on each VPN info refresh. | Remove resolver wiring and cache update. |
| Invalid PN server id from server data prevents proxy routing | Log and skip invalid routes so the client does not choose the wrong relay. | Revert resolver implementation or handle invalid data in server-side follow-up. |
| Existing p2p-frame fallback semantics differ by version | Use the external trait exactly as exposed by current lockfile and verify with compile check. | Remove `set_proxy_route_resolver` call and restore current stack config. |
| CLI and daemon could resolve different API addresses | Share one helper for config parsing and base URL generation. | Revert to fixed `127.0.0.1:4536` address. |
| Mixed base58/base36 NodeId logs obscure diagnosis | Emit only base36 NodeId strings from client-owned NodeId call sites. | Re-enable base58 output only with a new approved requirement. |
| SN TCP endpoint may not match the deployed SN server listener | Proposal scopes this as same-address endpoint registration only; deployment mismatch is handled by p2p-frame connection failure semantics. | Revert the endpoint vector to the single QUIC endpoint. |
| Local TCP listener may conflict with an already-used TCP `p2p.port` | Reuse the existing `p2p.port` to avoid new configuration and let p2p-frame surface listener failure at startup. | Revert the local endpoint vector to the single QUIC endpoint. |
| Incorrect `server_name` default can break SN tunnel establishment for domain certificates | Derive domain default from the original `server` host and IP default from `server_id`; allow explicit override. | Remove `--server_name` and restore unconditional `sn_id.to_string()` for `P2pSn::new`. |
| PN proxy name may be absent from older control nodes | Treat missing or blank `PnServerInfo.name` as absent and fall back to id. | Revert `remote_name` selection to id only if p2p-frame rejects reported names. |

## Approval Record
- approver: auto-pipeline
- approval_date: 2026-07-06T16:09:15+08:00
- user_statement: "确认，自动处理后续步骤"
