---
module: vpn-frame
version: v0.1
status: approved
approved_by: user-request
approved_at: 2026-07-06T16:09:15+08:00
approved_content_sha256: c60e62f7e8a98d54c32c3319ac7eef9c503b422567732431415edd18a42d3ca5
---

# vpn-frame Proposal

## Background and Goal
`vpn-frame` owns the shared VPN protocol structs consumed by both `bucky-vpn` and `bucky-vpn-server`. The selected PN server must be exposed as structured data, and the PN server id must be the P2P node id of the `vpn-server` process, not an endpoint string or any value derived from ip and port.

New requirement: all externally visible and persisted `NodeId` string operations should use base36 instead of base58. `NodeId` bytes remain unchanged; this only changes the canonical string representation used by APIs, SQLite keys, logs where they are meant to identify nodes, and direct client/server request parameters.

New requirement: when a proxy node has a configured name, the proxy node reports that name to the control node; the control node includes the reported name in the proxy-node information returned to clients; the client uses that name when connecting to the proxy node.

The goal is to make `NodeNetwork.pn_server` carry `id`, `ip`, `port`, and optional reported `name` as first-class data and to keep the corresponding server-side runtime and persistence paths structured. Old endpoint-string PN server data is not compatible with this contract and is not migrated or parsed in this change.

## Scope
### In scope
- The shared `NodeNetwork` and `PnServerInfo` protocol contract in `vpn-frame/src/vpn_protocol.rs`.
- Optional proxy-node reported name on `PnServerInfo`, when supplied by the proxy node through the control path.
- The canonical external string contract for `NodeId`: use base36 for API, persistence, logging, and direct cross-crate string conversion.
- The `PnServerSelector` and `Network` data contracts that carry selected PN server state.
- Direct `vpn-frame`, `bucky-vpn`, and `bucky-vpn-server` consumers that currently read, write, report, or persist PN server values.
- SQLite PN server storage for network selection and proxy-node approval state, using separate id/ip/port fields.
- HTTP admin API request/response shapes that approve, reject, or display PN server values.

### Out of scope
- Migrating old SQLite rows that stored PN servers as endpoint strings.
- Treating endpoint strings as PN server ids.
- Reworking the external `bucky-p2p` PN relay implementation.
- Changing account, network group, or authorization semantics.
- Flutter Web UI implementation changes.
- Changing non-`NodeId` base58 uses, such as password hash encoding.

### Boundary with neighboring modules
- `bucky-vpn-server` is responsible for constructing `PnServerInfo` from its P2P identity id, listening/control endpoint address, and optional reported proxy-node name.
- `bucky-vpn-server` HTTP API exposes PN server data as id/ip/port/name JSON fields, not endpoint strings.
- `bucky-vpn` may still need to construct a P2P endpoint at the final connection boundary, but that derived endpoint is not the stored or protocol PN server id; when `PnServerInfo.name` is present, the client uses it as the remote/proxy connection name.
- `vpn-frame` exposes the structured protocol and store-facing value used by both consumers.

## Assumptions and Ambiguities
| item | resolution |
|------|------------|
| PN server id source | Use the P2P node id of the `vpn-server` PN node. |
| IP family | Support `IpAddr`, so both IPv4 and IPv6 endpoint addresses can be represented. |
| Port type | Use `u16`, matching network port range and endpoint address output. |
| Old endpoint-string data | Do not parse or migrate it. Existing old-shape databases are outside this change's compatibility contract. |
| Multiple endpoints for one id | The selected value carries one id/ip/port tuple. If a server exposes multiple endpoints, selector policy chooses one tuple. |
| Proxy node name | Treat the reported proxy-node name as optional display/connection metadata on `PnServerInfo`; it is not the node id and must not replace approval or authorization keys. |
| NodeId string encoding | Canonicalize `NodeId` external strings to base36; keep bytes and raw protocol encoding unchanged. |
| Legacy base58 NodeId strings | Treat legacy base58 rows/requests as a migration/compatibility design question; new writes and displayed values should be base36. |

## Constraints
- Keep the change focused on the PN server data contract and its direct runtime/storage consumers.
- Do not introduce new dependencies.
- Do not weaken the existing PN selector approval or heartbeat behavior.
- Do not change VPN membership authorization behavior.
- Preserve raw-codec and serde support for the shared protocol type.
- New `NodeId` string writes and API responses must use base36, not base58.

## Requirement Challenge
| question | evaluation | risk_or_tradeoff | decision |
|----------|------------|------------------|----------|
| Should `pn_server` stay as `Option<String>` and encode id/ip/port inside the string? | That would preserve old storage but would keep id ambiguous and endpoint-shaped. | Consumers would still be forced to parse a technical transport string. | Replace the protocol and store-facing field with `Option<PnServerInfo>`. |
| Should the PN server id be derived from endpoint/ip/port? | The product requirement says the id is the `vpn-server` P2P node id. | Endpoint-derived ids break identity semantics and can change when address changes. | Use the P2P node id string as `PnServerInfo.id`. |
| Should old endpoint-string database rows be supported? | The requirement explicitly rejects legacy compatibility. | Existing old databases will not read the old PN server field after this change. | No migration or parsing of old endpoint strings. |
| Should the database keep a single endpoint-like column? | That would contradict the structured id/ip/port contract. | It preserves ambiguity between identity and transport address. | Store PN server id, ip, and port in separate columns. |
| Should the proxy-node name replace `PnServerInfo.id`? | No. The name is operator-controlled metadata and may change; the id is the stable P2P node identity. | Using name as identity would break approvals, liveness, and persistence when names change or collide. | Add optional `name` beside id/ip/port and keep id as the identity key. |
| Should clients ignore the reported name and keep using id as connection name? | No. The user explicitly requires clients to use the returned proxy-node name when connecting. | Older servers or unnamed proxy nodes may not provide the field. | Clients use `PnServerInfo.name` when present and fall back according to approved client design when absent. |
| Should `NodeId` keep base58 as the external string format? | No. The user explicitly asked to change all NodeId operations to base36, and `P2pId` already uses base36 display/parse in the underlying P2P layer. | Existing SQLite rows and old API clients may still carry base58 strings; a hard cut can break old local data. | Base36 becomes the canonical format for new writes/responses/requests; design must decide whether reads accept base58 temporarily for migration. |

## Large Module Submodule Decision
| submodule | new_or_existing | responsibility | proposal_packet | reason |
|-----------|-----------------|----------------|-----------------|--------|
| protocol | existing | Shared protocol type carried by `vpn_protocol.rs` | docs/versions/v0.1/modules/vpn-frame/proposal.md | The request changes an existing shared protocol field and its direct runtime/storage consumers, not a new independent submodule. |

## Trigger Matrix
| trigger_category | applies | evidence | required_checks | deferred_checks_and_reason |
|------------------|---------|----------|-----------------|----------------------------|
| contract/protocol | yes | `NodeNetwork.pn_server` and traffic-report payload carry serialized PN server data including optional reported name; `NodeId` has a canonical string contract consumed by clients and server APIs. | schema-check; admission-check; cargo check/test through vpn-frame harness entry | none |
| data/schema | yes | SQLite network and proxy-node PN server storage changes from endpoint string to structured id/ip/port/name fields; NodeId key columns need base36 canonical string writes. | testing must record schema and compatibility boundaries | owner: vpn-frame maintainers; risk: existing endpoint-string/base58 rows need explicit compatibility decision; acceptance impact: acceptance must verify no legacy endpoint compatibility claim remains |
| security/privacy/permission | no | The change does not alter authorization, approval, or visibility rules. | none | none |
| runtime/integration | yes | Client and server crates both consume the PN server value. | vpn-frame integration level must cover workspace consumers | none |
| build/dependency/config/deployment | no | No dependency, build script, config, or deployment surface changes are intended. | none | none |
| ui/datamodel/workflow | yes | Server-side HTTP API JSON models expose PN server values. | vpn-frame integration harness level | owner: vpn-frame maintainers; risk: Flutter Web client code may need a separate follow-up; acceptance impact: this task verifies server-side API compilation only |
| harness/process | yes | Auto-pipeline is being used for this task. | pipeline-plan-check | none |

## High-Level Outcomes
- `NodeNetwork.pn_server` carries a PN server `id`, `ip`, and `port`.
- `NodeNetwork.pn_server` carries optional reported proxy-node `name` when the proxy node supplies it.
- The id is the `vpn-server` PN node's P2P node id.
- The reported name is connection/display metadata and does not replace the PN server id.
- PN selector, heartbeat, network selection persistence, and proxy-node approval persistence use structured `PnServerInfo` values.
- Old endpoint-string PN server data is not parsed, migrated, or treated as compatible input.
- Client P2P code derives a connection endpoint only at the final P2P connection boundary.
- `NodeId` string values produced by shared code use base36 as the canonical representation.

## Proposal Items
| proposal_id | change_id | outcome | success_evidence |
|-------------|-----------|---------|------------------|
| PROP-pn-server-info | CHG-pn-server-info-contract | `NodeNetwork.pn_server`, selector state, heartbeat state, and SQLite PN server storage carry structured PN server id, ip, and port; `id` is the `vpn-server` P2P node id and old endpoint-string data is unsupported. | `vpn-frame/src/vpn_protocol.rs` defines the structured type and direct runtime/storage consumers compile through the vpn-frame harness test entry. |
| PROP-pn-server-reported-name | CHG-pn-server-reported-name-contract | `PnServerInfo` carries an optional reported proxy-node name that servers can return to clients, and clients can use as the proxy connection name without replacing the stable PN server id. | Design maps the optional name through shared protocol, server reporting/selection response, and client connection call sites; implementation preserves id/ip/port identity semantics. |
| PROP-node-id-base36-contract | CHG-node-id-base36-contract | `NodeId` external string representation is base36 for new API, persistence, and direct string conversion uses. | Design maps shared `NodeId` helpers and all direct consumers; implementation removes base58 from NodeId string-operation paths except explicitly documented legacy read compatibility. |

## Success Criteria
- The shared protocol exposes a concrete `PnServerInfo` type with `id`, `ip`, `port`, and optional `name`.
- `NodeNetwork.pn_server` uses `Option<PnServerInfo>`.
- Server-returned PN server data preserves the reported name when available.
- Client-side code can use the reported PN server name at the proxy connection boundary without treating it as the identity key.
- `ReportPnTrafficStatsReq.pn_server`, `PnServerSelector`, and store-facing `Network.pn_server` use structured PN server data rather than endpoint strings.
- SQLite stores PN server selection and proxy-node approval in id/ip/port fields instead of endpoint-formatted values.
- Server-side code constructs `PnServerInfo.id` from the `vpn-server` P2P node id.
- Client-side code derives any required P2P endpoint from `ip` and `port` only at the connection boundary.
- New `NodeId` strings emitted by shared contracts are base36.
- Consumers that parse user/API/database `NodeId` strings follow the approved design compatibility rule for old base58 values.
- Validation records protocol, data/schema, runtime, and HTTP API data-shape trigger coverage.

## Risks
- This is a shared protocol and persistence contract change and can break both binaries if any consumer still expects `Option<String>`.
- Existing databases with only endpoint-string PN server fields are intentionally unsupported for this change.
- P2P connection code still needs a transport endpoint at the final boundary, so that derivation must not be mistaken for the persisted identity.
- If the reported proxy-node name is treated as the stable id, name changes or duplicate names can corrupt approval, liveness, or selection behavior.
- If the name is omitted from shared protocol while server/client code assumes it exists, clients will continue connecting by id and fail name-based certificate/SNI scenarios.
- Switching existing local SQLite keys from base58 to base36 can make old rows invisible unless compatibility reads or migration are designed.

## Downstream Follow-Up
| stage | follow_up |
|-------|-----------|
| design | Map `PnServerInfo` to protocol, selector, heartbeat, client connection, and SQLite storage scope paths. |
| design | Map `CHG-pn-server-reported-name-contract` through `PnServerInfo`, proxy heartbeat/report payloads, server API responses, selector/storage compatibility, and client proxy connection name usage. |
| design | Map `CHG-node-id-base36-contract` to shared `NodeId` helpers and every direct NodeId string consumer across `vpn-frame`, `bucky-vpn-server`, `bucky-vpn`, and `vpn_web`; decide old base58 read compatibility. |
| implementation | Update `vpn_protocol.rs`, direct PN server consumers, and SQLite PN server storage only after admission passes. |
| implementation | After approved design/admission, replace NodeId base58 string writes/parses with base36 equivalents in admitted scope paths. |
| testing | Record direct coverage for protocol shape, structured persistence, no old endpoint compatibility, and workspace compatibility. |
| acceptance | Audit that no endpoint string remains the PN server id or storage format. |

## Approval Record
- approver: user-request
- approval_date: 2026-07-06T16:09:15+08:00
- user_statement: "确认，自动处理后续步骤"
