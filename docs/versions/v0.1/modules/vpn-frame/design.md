---
module: vpn-frame
version: v0.1
status: approved
approved_by: auto-pipeline
approved_at: 2026-07-07T00:41:31+08:00
approved_content_sha256: 4e1a992d1dbaf402768da726c3b45992d6c00d6e09f4f960451b6bbd813de8bb
---

# vpn-frame Design

## Design Scope
The design implements `CHG-pn-server-info-contract` for the shared PN server protocol value and its direct runtime/storage consumers. It changes PN server identity from endpoint-shaped strings to structured `PnServerInfo` values whose `id` is the `vpn-server` P2P node id. Old endpoint-string PN server storage is not compatible and is not migrated.

This design also implements `CHG-node-id-base36-contract`: external `NodeId` string values exposed by shared runtime contracts, logs, and direct cross-crate consumers use base36 as the canonical representation. Raw `NodeId` bytes and protocol codecs remain unchanged. Existing base58 helpers may remain as legacy compatibility helpers, but new NodeId-facing operations must not choose base58 as their output format.

This design also implements `CHG-pn-server-reported-name-contract`: `PnServerInfo` carries an optional reported proxy-node `name`. The field is metadata used by control-node responses and client PN connection naming; it is not a node identity, approval key, selector key, or endpoint replacement.

This design also implements `CHG-pn-server-endpoint-address-contract`: `PnServerInfo` carries proxy-node transport addresses as Endpoint-shaped values, preserving protocol, IP, and port together for QUIC/TCP consumers. Split `ip`/`port` fields are no longer the shared PN address contract.

This design also implements `CHG-pn-server-address-live-state-contract`: proxy-node transport addresses are live runtime data from proxy reports and control-node observation, not SQLite truth. Persistence may keep stable identity, approval, network membership, and statistics data, but selector and client-returned address data must be refreshed from live `PnServerInfo`.

This design also covers report-time `port_mapping` metadata on `PnServerInfo`: a proxy node reports local listen Endpoint ports unchanged and may attach port mapping metadata; the control node applies that metadata when synthesizing client-facing live Endpoint ports and does not need to preserve the mapping on returned client values.

## Overall Approach
Add or keep `PnServerInfo` in `vpn_protocol.rs` with `id: String`, Endpoint-shaped address data, optional `name: Option<String>`, optional report-time `port_mapping`, and no endpoint-string identity helpers. The Endpoint-shaped address field is ordered: the first endpoint is the preferred connection candidate, and later endpoints are fallback candidates. Change `NodeNetwork.pn_server`, `ReportPnTrafficStatsReq.pn_server`, `PnServerSelector`, store-facing `Network.pn_server`, selector state, and direct consumers to carry `PnServerInfo`. The proxy-node side constructs `PnServerInfo` from the local or configured P2P node id plus local listen Endpoint values, and when configured/reported, the proxy node name and port mapping metadata. The control node constructs returned live `PnServerInfo` values by applying observed connection IP and mapping-derived external ports to Endpoint values. The client consumes returned Endpoint values when calling the P2P connection API, and may use `name` only as the remote connection name.

`vpn-frame` should not require SQLite implementations to persist PN transport endpoints. Store-facing models can carry a `PnServerInfo` value while it is live, but persistence owners should store only stable identity/policy data unless a module has a separate approved reason to snapshot runtime addresses. Existing persisted endpoint-string or split address fields are compatibility concerns for the owning store module and are not the shared source of truth.

## Simplicity Check
| topic | decision | reason |
|-------|----------|--------|
| new abstraction | Use one shared `PnServerInfo` value | The same id/Endpoint/name/report-time port_mapping value is needed by protocol, selector, heartbeat, and client connection. |
| persistence migration | not included | The requirement rejects old endpoint-string compatibility. |
| dependency change | not included | `std::net` and existing `p2p-frame` usage are sufficient. |
| address persistence | live-only | Persisting proxy-node transport addresses would create stale NAT/LB state; persistence keeps stable identity and policy data. |

## Current Structure
| path | current_responsibility | change |
|------|------------------------|--------|
| `vpn-frame/src/vpn_protocol.rs` | Shared command and data structs | Define `PnServerInfo` with Endpoint-shaped address data and without endpoint-string compatibility helpers; use it in `NodeNetwork` and `ReportPnTrafficStatsReq`; add optional reported name and port_mapping metadata. |
| `vpn-frame/src/server/network_store.rs` | Server storage-facing network model | `Network.pn_server` can carry live `Option<PnServerInfo>` values, but persistence implementations must not treat endpoints as durable address truth. |
| `vpn-frame/src/server/node_store.rs` | Shared `NodeId` domain type and string helpers | Keep raw bytes unchanged and make base36 the canonical external string helper for new consumers. |
| `vpn-frame/src/server/network_manager.rs` | Network update orchestration | Pass structured PN server values into the store and log NodeId values as base36. |
| `vpn-frame/src/server/vpn_server.rs` | Applies PN selector when returning network info and receiving heartbeats | Use structured selector values and report heartbeat with `PnServerInfo`; compare/log NodeId identities as base36. |
| `vpn-frame/src/client/vpn_server_client.rs` | Sends traffic stats to the VPN server | Send structured `PnServerInfo` in report commands. |
| `vpn-frame/src/client/tunnel_manager.rs` | Keeps per-network route PN values | Store structured `PnServerInfo` without endpoint identity conversion. |
| `vpn-frame/src/client/vpn_client.rs` | Receives VPN info and reports PN traffic | Pass structured PN server values through client reporting paths. |
| `vpn-client/src/p2p_vpn.rs` | Connects to PN transport endpoints | Consume `PnServerInfo` Endpoint values directly; use `name` as the PN remote connection name when present, otherwise fall back to `id`. |
| `vpn-server/src/server_config.rs` | Builds PN selector candidates and proxy-node state | Construct report `PnServerInfo` from P2P node ids, local listen Endpoint addresses, reported names, and optional port_mapping; construct selected live values from observed IP and mapping-derived external ports. |
| `vpn-server/src/main.rs` | Wires local identity, PN endpoints, and optional control server | Use the local P2P id for local PN server info and configured control-server id for remote report targets. |
| `vpn-server/src/vpn_control_client.rs` | Reports PN traffic to a control server | Carry structured PN server values in traffic reports. |
| `vpn-server/src/sqlite_store_factory.rs` | Persists networks and proxy-node approvals | Persist stable identity and approval data; stop relying on PN transport endpoint columns as address truth. |
| `vpn-server/src/api.rs` | Exposes admin HTTP API data models | Expose Endpoint-shaped PN server address data for live values; approval identity remains id-based. |

## Invariants to Preserve
| invariant | preservation |
|-----------|--------------|
| PN selector approval rules remain functionally unchanged | Approval still gates remote PN servers, but the key is the structured PN server identity tuple rather than an endpoint string. |
| VPN membership authorization remains unchanged | No membership or `allow_join` branch is changed by this contract update. |
| Existing P2P connection APIs receive endpoints | Endpoint values flow through `PnServerInfo` and are consumed at the client P2P boundary. |
| Raw codec and serde derive support remain available | `PnServerInfo` derives the same protocol traits needed by command structs. |
| NodeId byte identity remains unchanged | Only text rendering/parsing policy changes; hash bytes, raw codec, and stored binary semantics are preserved. |
| PN server identity semantics remain unchanged | `PnServerInfo.name` is optional metadata and must not replace `id` for approval, liveness, selector, or persistence keys. |
| Proxy-node addresses are live | SQLite persistence must not make stale proxy-node endpoints valid after restart without a fresh proxy-node report/control connection. |

## Submodules
| submodule | type | responsibility | depends_on |
|-----------|------|----------------|------------|
| protocol | shared | Shared protocol structs for PN server info, Endpoint-shaped addresses, and command payloads | none |
| server-runtime | business | Build network info responses, apply selector results, and process heartbeats | protocol |
| client-runtime | business | Route traffic, connect to PN transport endpoints, and report PN traffic | protocol |
| persistence | technical | Persist stable network and proxy-node policy state without owning PN transport address truth | protocol |
| crate-entry | assembly | Re-export shared protocol types | protocol |

## Boundary Rationale
`PnServerInfo` belongs in the shared protocol boundary because both server and client consumers need the same serialized identity and Endpoint-shaped address values. Persistence is in scope only to define what it must not own: proxy-node transport endpoints are live runtime data and must not become durable address truth.

## Boundary Decision Matrix
| boundary | classification | business_responsibility | shared_logic_or_technical_area | decision |
|----------|----------------|-------------------------|--------------------------------|----------|
| `vpn_protocol.rs` PN server value | shared | Expose selected PN server identity and Endpoint addresses to consumers | protocol serialization | Add/use `PnServerInfo` with Endpoint-shaped addresses, optional report-time port_mapping, and no endpoint-string identity helpers. |
| `vpn_protocol.rs` PN server reported name | shared | Carry operator/configured proxy-node name across report and response payloads | optional protocol metadata | Add optional `name` to `PnServerInfo`; missing/blank name preserves old behavior. |
| server selector state | business | Choose and validate PN server candidates | selector availability and approval | Use `PnServerInfo` and compare identity by structured fields. |
| SQLite PN server storage | technical | Persist stable identity and approval policy | data schema | Do not persist PN transport endpoints as address truth; old address columns are compatibility-only for owning stores. |
| client P2P connect call | business | Connect to selected PN relay | existing Endpoint API | Consume Endpoint values from `PnServerInfo` at the P2P boundary. |
| HTTP admin API | business | Display and approve/reject PN proxy nodes | JSON data model | Use nested Endpoint-shaped address data for live PN server values; approval requests remain id-based. |
| NodeId string boundary | shared | Provide stable node identity strings to direct consumers | `NodeId` helpers and logs | Base36 is canonical for new output; base58 is legacy input compatibility only when a caller explicitly needs migration tolerance. |

## Dependency Graph
| source | depends_on | reason | cycle_check |
|--------|------------|--------|-------------|
| server-runtime | protocol | Server returns and receives PN server values | acyclic |
| client-runtime | protocol | Client reads and reports PN server values | acyclic |
| persistence | protocol | Store-facing data model can carry `PnServerInfo` but does not own Endpoint truth | acyclic |
| crate-entry | protocol | Public exports re-export protocol types | acyclic |
| protocol | none | Shared data structs have no dependency on runtime modules | acyclic |
| bucky-vpn | protocol | Client binary consumes the shared protocol field | external consumer |
| bucky-vpn-server | protocol | Server binary stores/selects and serves PN info | external consumer |

## Key Call Flows
| flow | caller | callee_submodule_path | purpose | failure_handling |
|------|--------|-----------------------|---------|------------------|
| VPN info response | `server-runtime` | `protocol` | Select a structured PN server and send it in `NodeNetwork`. | If no valid PN server is available, persist and send `None`. |
| Client PN connect | `client-runtime` / `bucky-vpn` | `protocol` | Consume Endpoint values from `PnServerInfo` and pass `PnServerInfo.name` as the remote name when present. | Existing connect errors still return `VpnResult` failures at the P2P boundary; missing name falls back to id. |
| Network persistence | `bucky-vpn-server` | `persistence` | Persist stable network state without treating PN endpoints as durable address truth. | Database errors propagate as existing `VpnResult` I/O errors; missing live PN endpoint means no returned proxy address. |
| Proxy-node heartbeat | `bucky-vpn-server` | `persistence` | Persist pending/approved identity state while live endpoints stay in selector/runtime state. | Approval query returns false when no row exists; database errors propagate. |

## Large Module Submodule Decision
| submodule | source_proposal | decision | design_packet | reason |
|-----------|-----------------|----------|---------------|--------|
| protocol | PROP-pn-server-info | existing submodule in module-level packet | docs/versions/v0.1/modules/vpn-frame/design.md | This is a focused shared contract change across direct consumers, not a new independent feature packet. |
| protocol | PROP-pn-server-reported-name | existing submodule in module-level packet | docs/versions/v0.1/modules/vpn-frame/design.md | This extends the existing PN server protocol value with optional metadata instead of creating a new independent feature packet. |
| protocol | PROP-pn-server-endpoint-address | existing submodule in module-level packet | docs/versions/v0.1/modules/vpn-frame/design.md | Endpoint-shaped addresses are part of the same shared PN server protocol value. |
| persistence | PROP-pn-server-address-live-state | existing submodule in module-level packet | docs/versions/v0.1/modules/vpn-frame/design.md | The change constrains existing store-facing ownership rather than introducing a new persistence feature. |

## Trigger Matrix
| trigger_category | applies | evidence | design_coverage | required_checks | deferred_checks_and_reason |
|------------------|---------|----------|-----------------|-----------------|----------------------------|
| contract/protocol | yes | `NodeNetwork.pn_server` and `ReportPnTrafficStatsReq.pn_server` serialized types change. | `PnServerInfo` derives raw codec and serde traits. | schema-check; admission-check; vpn-frame tests | none |
| contract/protocol | yes | `PnServerInfo.name` is carried from proxy-node reports to control-node responses and client PN connection setup. | Optional field with serde default/backward-compatible absence; identity remains `id`. | schema-check; admission-check; vpn-frame tests | none |
| contract/protocol | yes | PN server address values must be Endpoint-shaped instead of split ip/port, and proxy reports need optional port_mapping metadata without rewriting local listen ports. | `PnServerInfo` carries ordered Endpoint values and optional port_mapping; client/server consumers no longer reconstruct protocol endpoints from split fields, and control-node synthesis applies mapping only to returned live Endpoint values. | schema-check; admission-check; vpn-frame tests | none |
| data/schema | yes | Persistent PN server fields must not become proxy-node address truth. | Persistence contracts keep identity/policy truth and treat endpoint values as live runtime data. | testing-coverage-check; vpn-frame unit/DV/integration | owner: vpn-frame maintainers; risk: existing endpoint/address rows must not be used as live addresses after restart |
| security/privacy/permission | no | No authorization rule or membership visibility change. | not-applicable: no permission branch is changed | none | none |
| runtime/integration | yes | Client and server binaries consume the field and report heartbeats. | Scope includes direct client/server consumer paths. | vpn-frame integration harness level | none |
| build/dependency/config/deployment | no | No build, dependency, or deployment change. | not-applicable: no build surface touched | none | none |
| ui/datamodel/workflow | yes | HTTP API JSON data model changes for PN server values. | API structs use id/ip/port fields without endpoint strings. | vpn-frame integration harness level | owner: vpn-frame maintainers; risk: Flutter Web client code may need a separate follow-up; acceptance impact: this task verifies server-side API compilation only |
| contract/protocol | yes | `NodeId` string contract changes from base58 output to base36 output for NodeId-facing operations. | `NodeId` keeps raw bytes unchanged and exposes base36 as canonical external text; consumers are updated in their module packets. | schema-check; admission-check; vpn-frame tests | owner: direct consumers; risk: old base58 persisted text needs explicit compatibility or migration where stores own old data |
| harness/process | yes | Explicit auto-pipeline launch is recorded. | pipeline plan records tasks and change_id. | pipeline-plan-check | none |

## Directly Mapped Change Items
| change_id | proposal_id | design_coverage | scope_paths |
|-----------|-------------|-----------------|-------------|
| CHG-pn-server-info-contract | PROP-pn-server-info | Add/use `PnServerInfo`, change protocol, selector, heartbeat, client connection, HTTP API, and store-facing contracts so PN server id is the `vpn-server` P2P node id and no endpoint-string identity compatibility remains. | `vpn-frame/src/vpn_protocol.rs`, `vpn-frame/src/server/network_store.rs`, `vpn-frame/src/server/network_manager.rs`, `vpn-frame/src/server/vpn_server.rs`, `vpn-frame/src/client/vpn_server_client.rs`, `vpn-frame/src/client/tunnel_manager.rs`, `vpn-frame/src/client/vpn_client.rs`, `vpn-client/src/p2p_vpn.rs`, `vpn-server/src/server_config.rs`, `vpn-server/src/main.rs`, `vpn-server/src/vpn_control_client.rs`, `vpn-server/src/sqlite_store_factory.rs`, `vpn-server/src/api.rs` |
| CHG-pn-server-endpoint-address-contract | PROP-pn-server-endpoint-address | Replace split PN address fields with Endpoint-shaped address values in `PnServerInfo`; consumers preserve protocol/address/port as a single endpoint when reporting, selecting, returning, and connecting to proxy nodes; proxy reports may carry optional port_mapping metadata while local listen Endpoint ports remain unchanged. | `vpn-frame/src/vpn_protocol.rs`, `vpn-frame/src/server/vpn_server.rs`, `vpn-frame/src/client/vpn_server_client.rs`, `vpn-frame/src/client/tunnel_manager.rs`, `vpn-frame/src/client/vpn_client.rs`, `vpn-client/src/p2p_vpn.rs`, `vpn-server/src/server_config.rs`, `vpn-server/src/main.rs`, `vpn-server/src/vpn_control_client.rs`, `vpn-server/src/api.rs` |
| CHG-pn-server-address-live-state-contract | PROP-pn-server-address-live-state | Define PN transport endpoints as live runtime data, not SQLite truth; store-facing contracts persist stable identity/policy/statistics while selectors and responses use current live `PnServerInfo` endpoints. | `vpn-frame/src/server/network_store.rs`, `vpn-frame/src/server/network_manager.rs`, `vpn-frame/src/server/vpn_server.rs`, `vpn-server/src/sqlite_store_factory.rs`, `vpn-server/src/server_config.rs`, `vpn-server/src/vpn_control_client.rs`, `vpn-server/src/api.rs` |
| CHG-node-id-base36-contract | PROP-node-id-base36-contract | Define base36 as the canonical external `NodeId` string representation while preserving raw bytes and protocol codecs; legacy base58 parsing is compatibility-only and must not be used for new NodeId output. | `vpn-frame/src/server/node_store.rs`, `vpn-frame/src/server/network_manager.rs`, `vpn-frame/src/server/vpn_server.rs` |
| CHG-pn-server-reported-name-contract | PROP-pn-server-reported-name | Add optional reported proxy-node name to `PnServerInfo` and carry it through network info, traffic report, selector state, API JSON, and client PN connect remote-name fallback rules. | `vpn-frame/src/vpn_protocol.rs`, `vpn-frame/src/server/vpn_server.rs`, `vpn-frame/src/client/tunnel_manager.rs`, `vpn-client/src/p2p_vpn.rs`, `vpn-server/src/server_config.rs`, `vpn-server/src/vpn_control_client.rs`, `vpn-server/src/api.rs`, `vpn-server/src/sqlite_store_factory.rs` |

## Implementation Order
| phase | goal | prerequisite | output | dependency | parallel |
|-------|------|--------------|--------|------------|----------|
| 1 | Normalize `PnServerInfo` semantics | approved proposal/design | protocol type has no endpoint-string compatibility helpers and carries Endpoint-shaped address values plus optional report-time port_mapping | none | no |
| 2 | Update selector, heartbeat, and network-store contracts | phase 1 | server runtime passes structured live PN server values | 1 | no |
| 3 | Remove address persistence truth from store contracts | phase 2 | selected PN endpoints are not treated as durable SQLite truth | 2 | no |
| 4 | Update server/client binary consumers | phase 2 | local/control PN server info is constructed from P2P ids and live Endpoint addresses; client consumes Endpoint values | 2,3 | no |
| 5 | Run targeted validation | implementation complete | test-run artifact or recorded failure | 1,2,3,4 | no |
| 6 | Normalize NodeId string output | approved base36 design/admission | shared runtime logs and selector comparisons use base36 NodeId strings | none | yes |

## Key Decisions
| decision | chosen | alternatives_considered | rejection_reason |
|----------|--------|--------------------------|------------------|
| PN server id source | `vpn-server` P2P node id string | endpoint string or ip:port | Endpoint and ip:port are transport addresses, not stable node identity. |
| Address representation | Endpoint-shaped address values | split ip/port fields | Split fields lose protocol and force consumers to reconstruct endpoints. |
| Address persistence | live-only endpoints, stable identity/policy persistence | persist endpoint or split address columns as truth | Runtime endpoints can become stale across NAT, reconnect, or port mapping changes. |
| Old endpoint-string data | unsupported | parse and migrate old strings | The requirement explicitly rejects old-data compatibility. |
| NodeId text format | base36 canonical output with optional explicit legacy parser | keep base58 output; silently accept every format everywhere | The user explicitly requested all NodeId operations move to base36; broad silent parsing would blur the new contract. |
| PN server name semantics | Optional metadata on `PnServerInfo` | Use name as primary selector key; add a separate name registry | The requested name is for certificate/connection naming and display, while `id` remains the stable node identity. |

## Data and State
| data_or_state | owner_submodule | access_for_others | state_transitions |
|---------------|-----------------|-------------------|-------------------|
| selected PN server info | server-runtime | Exposed through `Network.pn_server` and `NodeNetwork.pn_server` as live values | none -> selected live structured tuple -> refreshed when selector live data changes -> none if no valid live selector exists |
| proxy-node approval info | persistence | Exposed through server config/API state as stable id/status metadata | missing -> pending on heartbeat -> approved/rejected by operator -> pending/approved/rejected updated on later writes |
| protocol PN server info | protocol | Read by client-runtime and external consumers through command payloads | absent -> present when selector returns structured data with Endpoint values; present -> absent if selector returns none |
| PN transport endpoints | server-runtime | Carried in `PnServerInfo` for clients and reports; persistence reads stable ids only | no live report -> no endpoint returned; proxy report carries local listen Endpoint plus optional port_mapping -> control observed synthesis creates live Endpoint list; reconnect/port mapping change -> live Endpoint list replaced |
| reported PN server name | protocol | Read by server selector, API projection, and client connection setup through `PnServerInfo` | absent -> fallback to id; configured/reported non-empty name -> propagated unchanged; blank values normalize to absent in owners that parse config |
| NodeId external text | shared NodeId type and direct runtime consumers | Read by server/client/API/UI modules as identity strings | raw bytes -> base36 display/store/request string; old base58 text is a legacy read concern only at module-owned migration boundaries |

## Testability
| seam | verification |
|------|--------------|
| `PnServerInfo` value semantics | Unit-level checks can cover constructor/equality and absence of endpoint-string helper behavior through compile-time usage. |
| Store-facing no-address-truth behavior | Unit or DV-level checks can cover that persisted approval/network state does not make stale endpoints selectable without live data. |
| Server response and heartbeat conversion | DV/build checks catch producer type mismatches; integration catches stale `Option<String>` usage. |
| Client Endpoint consumption | Workspace integration catches compile-time contract drift in `bucky-vpn` and `bucky-vpn-server`. |
| PN server reported name propagation | Unit/DV checks can assert reported `name` survives selector/API/network-info paths and client remote-name fallback. |
| NodeId base36 contract | Unit/build checks cover base36 encode/decode helpers and direct consumer compilation after base58 call sites are removed. |

## Interfaces and Dependencies
| interface | consumer | compatibility | notes |
|-----------|----------|---------------|-------|
| `NodeNetwork.pn_server: Option<PnServerInfo>` | `bucky-vpn`, `bucky-vpn-server`, `CHG-pn-server-info-contract` | breaking | Affected callers that expected `Option<String>` must pass structured values. |
| `ReportPnTrafficStatsReq.pn_server: Option<PnServerInfo>` | `bucky-vpn-server`, `CHG-pn-server-info-contract` | breaking | Heartbeats report the structured PN server tuple. |
| `PnServerSelector` structured methods | `bucky-vpn-server`, `CHG-pn-server-info-contract` | breaking | Selector validates, selects, and reports `PnServerInfo`. |
| `PnServerInfo` Endpoint address values and optional `port_mapping` | `bucky-vpn-server`, `bucky-vpn`, `CHG-pn-server-endpoint-address-contract` | breaking | Address consumers use Endpoint values and no longer depend on split ip/port fields; proxy reports can attach port_mapping metadata without replacing local listen Endpoint ports. |
| `PnServerInfo.name: Option<String>` | `bucky-vpn-server`, `bucky-vpn`, `CHG-pn-server-reported-name-contract` | backward-compatible | Missing name is accepted and treated as absent; non-empty name is used for display/control responses and PN remote-name setup only. |
| Store-facing PN endpoint persistence | `bucky-vpn-server`, `CHG-pn-server-address-live-state-contract` | migration-required | Stable identity/policy may persist; Endpoint address values are live and must not be trusted from old persisted address columns. |
| HTTP API PN server JSON | `bucky-vpn-server`, `CHG-pn-server-info-contract` | breaking | Proxy-node approval/list and network list responses use id/ip/port JSON objects. |
| `NodeId` external string values | `bucky-vpn-server`, `bucky-vpn`, `vpn_web`, `CHG-node-id-base36-contract` | migration-required | New outputs are base36; any old base58 data must be handled only by the module that owns that storage/input boundary. |

## Document Index
| document | topic | scope |
|----------|-------|-------|
| `design.md` | PN server info protocol/runtime/storage contract | full module |
| `design/client-runtime.md` | Existing client runtime notes | background only |
| `design/server-runtime.md` | Existing server runtime notes | background only |

## Risks and Rollback
| risk | mitigation | rollback |
|------|------------|----------|
| Shared protocol breakage | Update direct consumers and run vpn-frame integration validation. | Revert `PnServerInfo` field change and consumers. |
| Existing database incompatibility | Requirement explicitly accepts no old endpoint compatibility; document this in testing/acceptance. | Restore endpoint-string storage only with a new approved requirement. |
| Endpoint field migration confusion | Keep Endpoint values in shared protocol and do not reintroduce split address fields as identity or durable truth. | Restore split fields only with a new approved requirement. |
| Stale persisted proxy address | Treat old address columns as compatibility-only and require live selector data before returning endpoints. | Re-enable address persistence only with a new approved requirement. |
| Old base58 NodeId text in local data | Treat as an explicit migration/compatibility concern in the owning store or UI module. | Re-enable base58 output only with a new approved requirement. |
| Name treated as identity by mistake | Preserve id-based keys and document name as metadata in selector/store/API/client boundaries. | Drop the optional name field with a new approved requirement if it proves incompatible with downstream p2p-frame naming. |

## Approval Record
- approver: auto-pipeline
- approval_date: 2026-07-07T00:41:31+08:00
- user_statement: "确认，自动处理后续步骤"
