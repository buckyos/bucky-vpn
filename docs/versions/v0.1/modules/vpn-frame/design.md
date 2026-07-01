---
module: vpn-frame
version: v0.1
status: approved
approved_by: user-request
approved_at: 2026-07-01T17:44:43+08:00
approved_content_sha256: c69205adcc92a754468d7b5508af182c3deaab117d5a2b255177d284f40b2cfb
---

# vpn-frame Design

## Design Scope
The design implements `CHG-pn-server-info-contract` for the shared PN server protocol value and its direct runtime/storage consumers. It changes PN server identity from endpoint-shaped strings to structured `PnServerInfo` values whose `id` is the `vpn-server` P2P node id. Old endpoint-string PN server storage is not compatible and is not migrated.

This design also implements `CHG-node-id-base36-contract`: external `NodeId` string values exposed by shared runtime contracts, logs, and direct cross-crate consumers use base36 as the canonical representation. Raw `NodeId` bytes and protocol codecs remain unchanged. Existing base58 helpers may remain as legacy compatibility helpers, but new NodeId-facing operations must not choose base58 as their output format.

## Overall Approach
Add or keep `PnServerInfo` in `vpn_protocol.rs` with `id: String`, `ip: IpAddr`, and `port: u16`, but remove endpoint-string parsing and endpoint-string identity helpers. Change `NodeNetwork.pn_server`, `ReportPnTrafficStatsReq.pn_server`, `PnServerSelector`, store-facing `Network.pn_server`, selector state, and SQLite PN server fields to carry `PnServerInfo`. The server constructs `PnServerInfo` from the local or configured P2P node id plus the selected endpoint address. The client derives a P2P `Endpoint` from `ip` and `port` only when calling the P2P connection API.

## Simplicity Check
| topic | decision | reason |
|-------|----------|--------|
| new abstraction | Use one shared `PnServerInfo` value | The same id/ip/port tuple is needed by protocol, selector, heartbeat, and persistence. |
| persistence migration | not included | The requirement rejects old endpoint-string compatibility. |
| dependency change | not included | `std::net` and existing `p2p-frame` usage are sufficient. |

## Current Structure
| path | current_responsibility | change |
|------|------------------------|--------|
| `vpn-frame/src/vpn_protocol.rs` | Shared command and data structs | Define `PnServerInfo` without endpoint-string compatibility helpers; use it in `NodeNetwork` and `ReportPnTrafficStatsReq`. |
| `vpn-frame/src/server/network_store.rs` | Server storage-facing network model | Change `Network.pn_server` to `Option<PnServerInfo>`. |
| `vpn-frame/src/server/node_store.rs` | Shared `NodeId` domain type and string helpers | Keep raw bytes unchanged and make base36 the canonical external string helper for new consumers. |
| `vpn-frame/src/server/network_manager.rs` | Network update orchestration | Pass structured PN server values into the store and log NodeId values as base36. |
| `vpn-frame/src/server/vpn_server.rs` | Applies PN selector when returning network info and receiving heartbeats | Use structured selector values and report heartbeat with `PnServerInfo`; compare/log NodeId identities as base36. |
| `vpn-frame/src/client/vpn_server_client.rs` | Sends traffic stats to the VPN server | Send structured `PnServerInfo` in report commands. |
| `vpn-frame/src/client/tunnel_manager.rs` | Keeps per-network route PN values | Store structured `PnServerInfo` without endpoint identity conversion. |
| `vpn-frame/src/client/vpn_client.rs` | Receives VPN info and reports PN traffic | Pass structured PN server values through client reporting paths. |
| `vpn-client/src/p2p_vpn.rs` | Connects to PN transport endpoints | Derive the final P2P endpoint from `PnServerInfo.ip` and `port`; do not use `id` as endpoint text. |
| `vpn-server/src/server_config.rs` | Builds PN selector candidates and proxy-node state | Construct `PnServerInfo` from P2P node ids and endpoint addresses; selector stores structured values. |
| `vpn-server/src/main.rs` | Wires local identity, PN endpoints, and optional control server | Use the local P2P id for local PN server info and configured control-server id for remote report targets. |
| `vpn-server/src/vpn_control_client.rs` | Reports PN traffic to a control server | Carry structured PN server values in traffic reports. |
| `vpn-server/src/sqlite_store_factory.rs` | Persists networks and proxy-node approvals | Store PN server id/ip/port in separate fields; do not parse old endpoint strings. |
| `vpn-server/src/api.rs` | Exposes admin HTTP API data models | Expose and accept PN server id/ip/port JSON fields for network/proxy-node APIs. |

## Invariants to Preserve
| invariant | preservation |
|-----------|--------------|
| PN selector approval rules remain functionally unchanged | Approval still gates remote PN servers, but the key is the structured PN server identity tuple rather than an endpoint string. |
| VPN membership authorization remains unchanged | No membership or `allow_join` branch is changed by this contract update. |
| Existing P2P connection APIs receive endpoints | Endpoint derivation happens only at the client P2P boundary from ip/port. |
| Raw codec and serde derive support remain available | `PnServerInfo` derives the same protocol traits needed by command structs. |
| NodeId byte identity remains unchanged | Only text rendering/parsing policy changes; hash bytes, raw codec, and stored binary semantics are preserved. |

## Submodules
| submodule | type | responsibility | depends_on |
|-----------|------|----------------|------------|
| protocol | shared | Shared protocol structs for PN server info and command payloads | none |
| server-runtime | business | Build network info responses, apply selector results, and process heartbeats | protocol |
| client-runtime | business | Route traffic, connect to PN transport endpoints, and report PN traffic | protocol |
| persistence | technical | Persist network PN selection and proxy-node approval state | protocol |
| crate-entry | assembly | Re-export shared protocol types | protocol |

## Boundary Rationale
`PnServerInfo` belongs in the shared protocol boundary because both server and client consumers need the same serialized identity/address tuple. SQLite persistence is in scope because storing endpoint strings would contradict the required id semantics and would preserve the old ambiguity.

## Boundary Decision Matrix
| boundary | classification | business_responsibility | shared_logic_or_technical_area | decision |
|----------|----------------|-------------------------|--------------------------------|----------|
| `vpn_protocol.rs` PN server value | shared | Expose selected PN server identity and address to consumers | protocol serialization | Add/use `PnServerInfo` with no endpoint-string identity helpers. |
| server selector state | business | Choose and validate PN server candidates | selector availability and approval | Use `PnServerInfo` and compare identity by structured fields. |
| SQLite PN server storage | technical | Persist selected PN server and proxy-node approval | data schema | Store id/ip/port columns; do not read old endpoint columns. |
| client P2P connect call | business | Connect to selected PN relay | existing endpoint parser/API | Derive an endpoint from `ip` and `port` at the P2P boundary only. |
| HTTP admin API | business | Display and approve/reject PN proxy nodes | JSON data model | Use nested id/ip/port JSON objects instead of endpoint strings. |
| NodeId string boundary | shared | Provide stable node identity strings to direct consumers | `NodeId` helpers and logs | Base36 is canonical for new output; base58 is legacy input compatibility only when a caller explicitly needs migration tolerance. |

## Dependency Graph
| source | depends_on | reason | cycle_check |
|--------|------------|--------|-------------|
| server-runtime | protocol | Server returns and receives PN server values | acyclic |
| client-runtime | protocol | Client reads and reports PN server values | acyclic |
| persistence | protocol | Store-facing data model uses `PnServerInfo` | acyclic |
| crate-entry | protocol | Public exports re-export protocol types | acyclic |
| protocol | none | Shared data structs have no dependency on runtime modules | acyclic |
| bucky-vpn | protocol | Client binary consumes the shared protocol field | external consumer |
| bucky-vpn-server | protocol | Server binary stores/selects and serves PN info | external consumer |

## Key Call Flows
| flow | caller | callee_submodule_path | purpose | failure_handling |
|------|--------|-----------------------|---------|------------------|
| VPN info response | `server-runtime` | `protocol` | Select a structured PN server and send it in `NodeNetwork`. | If no valid PN server is available, persist and send `None`. |
| Client PN connect | `client-runtime` / `bucky-vpn` | `protocol` | Derive a P2P endpoint from `PnServerInfo.ip` and `port` for the existing P2P API. | Existing connect errors still return `VpnResult` failures at the P2P boundary. |
| Network persistence | `bucky-vpn-server` | `persistence` | Store selected PN server id/ip/port in the network row. | Database errors propagate as existing `VpnResult` I/O errors. |
| Proxy-node heartbeat | `bucky-vpn-server` | `persistence` | Store pending/approved remote PN server id/ip/port rows. | Approval query returns false when no row exists; database errors propagate. |

## Large Module Submodule Decision
| submodule | source_proposal | decision | design_packet | reason |
|-----------|-----------------|----------|---------------|--------|
| protocol | PROP-pn-server-info | existing submodule in module-level packet | docs/versions/v0.1/modules/vpn-frame/design.md | This is a focused shared contract change across direct consumers, not a new independent feature packet. |

## Trigger Matrix
| trigger_category | applies | evidence | design_coverage | required_checks | deferred_checks_and_reason |
|------------------|---------|----------|-----------------|-----------------|----------------------------|
| contract/protocol | yes | `NodeNetwork.pn_server` and `ReportPnTrafficStatsReq.pn_server` serialized types change. | `PnServerInfo` derives raw codec and serde traits. | schema-check; admission-check; vpn-frame tests | none |
| data/schema | yes | Persistent PN server fields change from endpoint string to structured id/ip/port. | SQLite schema and store bindings use separate fields. | testing-coverage-check; vpn-frame unit/DV/integration | owner: vpn-frame maintainers; risk: existing endpoint-string rows are unsupported by requirement; acceptance impact: acceptance must verify no legacy endpoint compatibility claim remains |
| security/privacy/permission | no | No authorization rule or membership visibility change. | not-applicable: no permission branch is changed | none | none |
| runtime/integration | yes | Client and server binaries consume the field and report heartbeats. | Scope includes direct client/server consumer paths. | vpn-frame integration harness level | none |
| build/dependency/config/deployment | no | No build, dependency, or deployment change. | not-applicable: no build surface touched | none | none |
| ui/datamodel/workflow | yes | HTTP API JSON data model changes for PN server values. | API structs use id/ip/port fields without endpoint strings. | vpn-frame integration harness level | owner: vpn-frame maintainers; risk: Flutter Web client code may need a separate follow-up; acceptance impact: this task verifies server-side API compilation only |
| contract/protocol | yes | `NodeId` string contract changes from base58 output to base36 output for NodeId-facing operations. | `NodeId` keeps raw bytes unchanged and exposes base36 as canonical external text; consumers are updated in their module packets. | schema-check; admission-check; vpn-frame tests | owner: direct consumers; risk: old base58 persisted text needs explicit compatibility or migration where stores own old data |
| harness/process | yes | Explicit auto-pipeline launch is recorded. | pipeline plan records tasks and change_id. | pipeline-plan-check | none |

## Directly Mapped Change Items
| change_id | proposal_id | design_coverage | scope_paths |
|-----------|-------------|-----------------|-------------|
| CHG-pn-server-info-contract | PROP-pn-server-info | Add/use `PnServerInfo`, change protocol, selector, heartbeat, client connection, HTTP API, and SQLite PN server storage so PN server id is the `vpn-server` P2P node id and no endpoint-string storage compatibility remains. | `vpn-frame/src/vpn_protocol.rs`, `vpn-frame/src/server/network_store.rs`, `vpn-frame/src/server/network_manager.rs`, `vpn-frame/src/server/vpn_server.rs`, `vpn-frame/src/client/vpn_server_client.rs`, `vpn-frame/src/client/tunnel_manager.rs`, `vpn-frame/src/client/vpn_client.rs`, `vpn-client/src/p2p_vpn.rs`, `vpn-server/src/server_config.rs`, `vpn-server/src/main.rs`, `vpn-server/src/vpn_control_client.rs`, `vpn-server/src/sqlite_store_factory.rs`, `vpn-server/src/api.rs` |
| CHG-node-id-base36-contract | PROP-node-id-base36-contract | Define base36 as the canonical external `NodeId` string representation while preserving raw bytes and protocol codecs; legacy base58 parsing is compatibility-only and must not be used for new NodeId output. | `vpn-frame/src/server/node_store.rs`, `vpn-frame/src/server/network_manager.rs`, `vpn-frame/src/server/vpn_server.rs` |

## Implementation Order
| phase | goal | prerequisite | output | dependency | parallel |
|-------|------|--------------|--------|------------|----------|
| 1 | Normalize `PnServerInfo` semantics | approved proposal/design | protocol type has no endpoint-string compatibility helpers | none | no |
| 2 | Update selector, heartbeat, and network-store contracts | phase 1 | server runtime passes structured PN server values | 1 | no |
| 3 | Update SQLite storage shape | phase 2 | selected PN servers and proxy-node approvals store id/ip/port fields | 2 | no |
| 4 | Update server/client binary consumers | phase 2 | local/control PN server info is constructed from P2P ids and endpoint addresses; client derives connect endpoint from ip/port | 2,3 | no |
| 5 | Run targeted validation | implementation complete | test-run artifact or recorded failure | 1,2,3,4 | no |
| 6 | Normalize NodeId string output | approved base36 design/admission | shared runtime logs and selector comparisons use base36 NodeId strings | none | yes |

## Key Decisions
| decision | chosen | alternatives_considered | rejection_reason |
|----------|--------|--------------------------|------------------|
| PN server id source | `vpn-server` P2P node id string | endpoint string or ip:port | Endpoint and ip:port are transport addresses, not stable node identity. |
| Persistence format | id/ip/port fields | single endpoint string column | Endpoint storage preserves the rejected legacy shape. |
| Old endpoint-string data | unsupported | parse and migrate old strings | The requirement explicitly rejects old-data compatibility. |
| NodeId text format | base36 canonical output with optional explicit legacy parser | keep base58 output; silently accept every format everywhere | The user explicitly requested all NodeId operations move to base36; broad silent parsing would blur the new contract. |

## Data and State
| data_or_state | owner_submodule | access_for_others | state_transitions |
|---------------|-----------------|-------------------|-------------------|
| selected PN server info | persistence | Exposed through `Network.pn_server` and `NodeNetwork.pn_server` | none -> selected structured tuple -> reassigned when selector invalidates old value -> none if no valid selector exists |
| proxy-node approval info | persistence | Exposed through server config/API state as structured PN server info plus approval metadata | missing -> pending on heartbeat -> approved/rejected by operator -> pending/approved/rejected updated on later writes |
| protocol PN server info | protocol | Read by client-runtime and external consumers through command payloads | absent -> present when selector returns structured data; present -> absent if selector returns none |
| NodeId external text | shared NodeId type and direct runtime consumers | Read by server/client/API/UI modules as identity strings | raw bytes -> base36 display/store/request string; old base58 text is a legacy read concern only at module-owned migration boundaries |

## Testability
| seam | verification |
|------|--------------|
| `PnServerInfo` value semantics | Unit-level checks can cover constructor/equality and absence of endpoint-string helper behavior through compile-time usage. |
| SQLite structured storage | Unit or DV-level checks can cover schema bindings for network/proxy-node PN server id/ip/port. |
| Server response and heartbeat conversion | DV/build checks catch producer type mismatches; integration catches stale `Option<String>` usage. |
| Client connection derivation | Workspace integration catches compile-time contract drift in `bucky-vpn` and `bucky-vpn-server`. |
| NodeId base36 contract | Unit/build checks cover base36 encode/decode helpers and direct consumer compilation after base58 call sites are removed. |

## Interfaces and Dependencies
| interface | consumer | compatibility | notes |
|-----------|----------|---------------|-------|
| `NodeNetwork.pn_server: Option<PnServerInfo>` | `bucky-vpn`, `bucky-vpn-server`, `CHG-pn-server-info-contract` | breaking | Affected callers that expected `Option<String>` must pass structured values. |
| `ReportPnTrafficStatsReq.pn_server: Option<PnServerInfo>` | `bucky-vpn-server`, `CHG-pn-server-info-contract` | breaking | Heartbeats report the structured PN server tuple. |
| `PnServerSelector` structured methods | `bucky-vpn-server`, `CHG-pn-server-info-contract` | breaking | Selector validates, selects, and reports `PnServerInfo`. |
| SQLite `network` PN server fields | `bucky-vpn-server`, `CHG-pn-server-info-contract` | migration-required | New rows use id/ip/port fields; old endpoint-string rows are unsupported. |
| SQLite `pn_proxy_node` PN server fields | `bucky-vpn-server`, `CHG-pn-server-info-contract` | migration-required | Approval rows are keyed by PN server id and store ip/port separately. |
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
| Endpoint derivation confusion | Keep derivation confined to client P2P connect code and never store it as PN server id. | Restore direct endpoint storage only with a new approved requirement. |
| Old base58 NodeId text in local data | Treat as an explicit migration/compatibility concern in the owning store or UI module. | Re-enable base58 output only with a new approved requirement. |

## Approval Record
- approver: user-request
- approval_date: 2026-07-01T17:44:43+08:00
- user_statement: "确认，自动处理后续步骤"
