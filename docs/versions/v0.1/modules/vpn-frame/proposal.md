---
module: vpn-frame
version: v0.1
status: approved
approved_by: auto-pipeline
approved_at: 2026-07-07T00:41:31+08:00
approved_content_sha256: 2c0185b64c60ef813641ab7e03e3c123ae08870fa00052a24e23ea818ca0f548
---

# vpn-frame Proposal

## Background and Goal
`vpn-frame` owns the shared VPN protocol structs consumed by both `bucky-vpn` and `bucky-vpn-server`. The selected PN server must be exposed as structured data, and the PN server id must be the P2P node id of the `vpn-server` process, not an endpoint string or any value derived from transport endpoints.

New requirement: all externally visible and persisted `NodeId` string operations should use base36 instead of base58. `NodeId` bytes remain unchanged; this only changes the canonical string representation used by APIs, SQLite keys, logs where they are meant to identify nodes, and direct client/server request parameters.

New requirement: when a proxy node has a configured name, the proxy node reports that name to the control node; the control node includes the reported name in the proxy-node information returned to clients; the client uses that name when connecting to the proxy node.

New requirement: proxy-node reported addresses and server-returned proxy-node addresses should use `Endpoint` values, not split `ip`/`port` fields. This keeps protocol, address, and port together for QUIC/TCP and prevents callers from reconstructing incomplete transport addresses.

New requirement: the control node does not need to persist proxy-node addresses. Address values are live runtime data reported by the proxy node and/or synthesized by the control node from observed connection IP plus port mapping metadata. A proxy node that configures port mapping reports its local listen Endpoint ports unchanged and reports port mapping separately; the control node applies the mapping only when synthesizing client-connectable live Endpoint values. SQLite persistence should keep identity, approval, network membership, and statistics truth, not stale proxy-node transport addresses.

The goal is to make `NodeNetwork.pn_server` carry `id`, Endpoint-shaped proxy-node address data, and optional reported `name` as first-class data. Old endpoint-string PN server data is not compatible with this contract and is not migrated or parsed in this change.

## Scope
### In scope
- The shared `NodeNetwork` and `PnServerInfo` protocol contract in `vpn-frame/src/vpn_protocol.rs`, including Endpoint-shaped proxy-node addresses and optional proxy-node reported port mapping metadata.
- Optional proxy-node reported name on `PnServerInfo`, when supplied by the proxy node through the control path.
- The canonical external string contract for `NodeId`: use base36 for API, persistence, logging, and direct cross-crate string conversion.
- The `PnServerSelector` and `Network` data contracts that carry selected PN server state.
- Direct `vpn-frame`, `bucky-vpn`, and `bucky-vpn-server` consumers that currently read, write, report, or persist PN server values.
- Store-facing PN server state keeps stable identity and policy fields; proxy-node transport addresses are live Endpoint data and are not persisted as control-node address truth.
- Proxy-node reported `port_mapping` metadata is report-time input for the control node to synthesize live Endpoint ports, not a replacement for local listen Endpoint ports.
- HTTP admin API request/response shapes that approve, reject, or display PN server values.

### Out of scope
- Migrating old SQLite rows that stored PN servers as endpoint strings.
- Treating endpoint strings as PN server ids.
- Reworking the external `bucky-p2p` PN relay implementation.
- Changing account, network group, or authorization semantics.
- Flutter Web UI implementation changes.
- Changing non-`NodeId` base58 uses, such as password hash encoding.

### Boundary with neighboring modules
- `bucky-vpn-server` is responsible for constructing `PnServerInfo` from its P2P identity id, live Endpoint-shaped proxy-node address data, and optional reported proxy-node name.
- `bucky-vpn-server` HTTP API exposes PN server address data as Endpoint-shaped JSON, not endpoint-string ids and not address truth persisted in SQLite.
- `bucky-vpn` consumes returned Endpoint values at the final connection boundary; when `PnServerInfo.name` is present, the client uses it as the remote/proxy connection name.
- `vpn-frame` exposes the structured protocol and store-facing value used by both consumers.

## Assumptions and Ambiguities
| item | resolution |
|------|------------|
| PN server id source | Use the P2P node id of the `vpn-server` PN node. |
| Address family and protocol | Use Endpoint-shaped values so protocol, IP family, and port are represented together. |
| Old endpoint-string data | Do not parse or migrate it. Existing old-shape databases are outside this change's compatibility contract. |
| Multiple endpoints for one id | The selected value may carry one or more Endpoint values for the same PN server id. Selector/server policy decides which live endpoints are returned and in what order. |
| Proxy node name | Treat the reported proxy-node name as optional display/connection metadata on `PnServerInfo`; it is not the node id and must not replace approval or authorization keys. |
| NodeId string encoding | Canonicalize `NodeId` external strings to base36; keep bytes and raw protocol encoding unchanged. |
| Legacy base58 NodeId strings | Treat legacy base58 rows/requests as a migration/compatibility design question; new writes and displayed values should be base36. |
| Address persistence | Do not persist proxy-node transport addresses as control-node truth. Persist only stable identity/policy/statistics data; live endpoints must be refreshed by current proxy-node control connections. |
| Port mapping report shape | Proxy nodes report local listen Endpoint ports unchanged and carry port mapping as optional metadata on the shared PN server report value; control nodes use that metadata only when synthesizing returned live Endpoint values. |

## Constraints
- Keep the change focused on the PN server data contract and its direct runtime/storage consumers.
- Do not introduce new dependencies.
- Do not weaken the existing PN selector approval or heartbeat behavior.
- Do not change VPN membership authorization behavior.
- Preserve raw-codec and serde support for the shared protocol type.
- New `NodeId` string writes and API responses must use base36, not base58.
- Proxy-node address fields in shared protocol must be Endpoint-shaped and must carry protocol with address and port.
- Store-facing contracts must not require persisting proxy-node transport addresses as SQLite truth.

## Requirement Challenge
| question | evaluation | risk_or_tradeoff | decision |
|----------|------------|------------------|----------|
| Should `pn_server` stay as `Option<String>` and encode id/ip/port inside the string? | That would preserve old storage but would keep id ambiguous and endpoint-shaped. | Consumers would still be forced to parse a technical transport string. | Replace the protocol and store-facing field with `Option<PnServerInfo>`. |
| Should the PN server id be derived from endpoint/ip/port? | The product requirement says the id is the `vpn-server` P2P node id. | Endpoint-derived ids break identity semantics and can change when address changes. | Use the P2P node id string as `PnServerInfo.id`. |
| Should old endpoint-string database rows be supported? | The requirement explicitly rejects legacy compatibility. | Existing old databases will not read the old PN server field after this change. | No migration or parsing of old endpoint strings. |
| Should the shared PN server address contract keep split `ip` and `port` fields? | No. Split fields lose protocol and force callers to reconstruct an endpoint manually. | Moving to Endpoint affects raw codec, serde, server construction, HTTP projection, and client consumption. | Use Endpoint-shaped address fields in `PnServerInfo` and direct consumers. |
| Should the control node persist proxy-node addresses in SQLite? | No. Proxy-node addresses are live transport observations or reports and can change across NAT, reconnect, or `port_mapping` updates. | Removing address persistence requires selector/network paths to refresh from live state instead of old cached fields. | Persist stable id/approval/statistics truth; do not persist proxy-node transport addresses as address truth. |
| Should proxy nodes replace their local listen Endpoint ports with mapped ports before reporting? | No. The latest requirement says proxy nodes only need to report `port_mapping`; replacing local listen ports at the proxy node mixes local runtime facts with control-node external address synthesis. | The shared report value needs a place for optional port mapping metadata, and control nodes must clear or ignore it after synthesizing client-facing Endpoint values. | Add optional port mapping metadata to `PnServerInfo`; proxy nodes report listen Endpoint ports unchanged, and control nodes apply mapping when building returned live Endpoints. |
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
| contract/protocol | yes | `NodeNetwork.pn_server` and traffic-report payload carry serialized PN server data including Endpoint-shaped addresses and optional reported name; `NodeId` has a canonical string contract consumed by clients and server APIs. | schema-check; admission-check; cargo check/test through vpn-frame harness entry | none |
| data/schema | yes | SQLite should stop treating proxy-node addresses as persisted truth; NodeId key columns need base36 canonical string writes. | testing must record schema and compatibility boundaries | owner: vpn-frame maintainers; risk: existing endpoint-string/base58/address rows need explicit compatibility decision; acceptance impact: acceptance must verify no legacy endpoint/address persistence claim remains |
| security/privacy/permission | no | The change does not alter authorization, approval, or visibility rules. | none | none |
| runtime/integration | yes | Client and server crates both consume the PN server value. | vpn-frame integration level must cover workspace consumers | none |
| build/dependency/config/deployment | no | No dependency, build script, config, or deployment surface changes are intended. | none | none |
| ui/datamodel/workflow | yes | Server-side HTTP API JSON models expose PN server values. | vpn-frame integration harness level | owner: vpn-frame maintainers; risk: Flutter Web client code may need a separate follow-up; acceptance impact: this task verifies server-side API compilation only |
| harness/process | yes | Auto-pipeline is being used for this task. | pipeline-plan-check | none |

## High-Level Outcomes
- `NodeNetwork.pn_server` carries a PN server `id` and Endpoint-shaped address data.
- Proxy-node reports can carry optional `port_mapping` metadata beside local listen Endpoint data; client-facing returned PN server values use already synthesized Endpoint ports.
- `NodeNetwork.pn_server` carries optional reported proxy-node `name` when the proxy node supplies it.
- The id is the `vpn-server` PN node's P2P node id.
- The reported name is connection/display metadata and does not replace the PN server id.
- PN selector and heartbeat use structured live `PnServerInfo` values; persistence keeps stable identity/policy/statistics truth and does not require proxy-node address persistence.
- Old endpoint-string PN server data is not parsed, migrated, or treated as compatible input.
- Client P2P code consumes returned Endpoint values at the final P2P connection boundary.
- `NodeId` string values produced by shared code use base36 as the canonical representation.

## Proposal Items
| proposal_id | change_id | outcome | success_evidence |
|-------------|-----------|---------|------------------|
| PROP-pn-server-info | CHG-pn-server-info-contract | `NodeNetwork.pn_server`, selector state, and heartbeat state carry structured PN server id and Endpoint-shaped address data; `id` is the `vpn-server` P2P node id and old endpoint-string data is unsupported. | `vpn-frame/src/vpn_protocol.rs` defines the structured type and direct runtime/storage consumers compile through the vpn-frame harness test entry. |
| PROP-pn-server-endpoint-address | CHG-pn-server-endpoint-address-contract | Proxy-node reported addresses and server-returned proxy-node addresses use Endpoint-shaped values rather than split ip/port fields; proxy-node reports may also carry optional `port_mapping` metadata without replacing local listen Endpoint ports. | Design maps Endpoint and port_mapping raw codec/serde, server construction, HTTP projection, and client connection consumption; implementation no longer requires reconstructing endpoints from split fields. |
| PROP-pn-server-address-live-state | CHG-pn-server-address-live-state-contract | Control-node persistence does not treat proxy-node transport addresses as SQLite truth; addresses are live values refreshed by current proxy-node control/report paths. | Design maps selector/network/store contracts so identity and approval persist while address values come from live `PnServerInfo`; implementation does not rely on stale persisted address columns. |
| PROP-pn-server-reported-name | CHG-pn-server-reported-name-contract | `PnServerInfo` carries an optional reported proxy-node name that servers can return to clients, and clients can use as the proxy connection name without replacing the stable PN server id. | Design maps the optional name through shared protocol, server reporting/selection response, and client connection call sites; implementation preserves id/ip/port identity semantics. |
| PROP-node-id-base36-contract | CHG-node-id-base36-contract | `NodeId` external string representation is base36 for new API, persistence, and direct string conversion uses. | Design maps shared `NodeId` helpers and all direct consumers; implementation removes base58 from NodeId string-operation paths except explicitly documented legacy read compatibility. |

## Success Criteria
- The shared protocol exposes a concrete `PnServerInfo` type with `id`, Endpoint-shaped address data, optional `name`, and optional report-time `port_mapping`.
- `NodeNetwork.pn_server` uses `Option<PnServerInfo>`.
- Server-returned PN server data preserves the reported name when available.
- Client-side code can use the reported PN server name at the proxy connection boundary without treating it as the identity key.
- `ReportPnTrafficStatsReq.pn_server`, `PnServerSelector`, and store-facing `Network.pn_server` use structured PN server data rather than endpoint strings.
- Proxy-node transport addresses are not required SQLite truth; stable approval and identity data persist separately from live Endpoint address data.
- Server-side code constructs `PnServerInfo.id` from the `vpn-server` P2P node id.
- Proxy-node side construction preserves local listen Endpoint ports and reports `port_mapping` separately; control-node returned values expose synthesized Endpoint ports.
- Client-side code consumes returned Endpoint address data at the connection boundary.
- New `NodeId` strings emitted by shared contracts are base36.
- Consumers that parse user/API/database `NodeId` strings follow the approved design compatibility rule for old base58 values.
- Validation records protocol, data/schema, runtime, and HTTP API data-shape trigger coverage.

## Risks
- This is a shared protocol and persistence contract change and can break both binaries if any consumer still expects `Option<String>`.
- Existing databases with only endpoint-string PN server fields are intentionally unsupported for this change.
- P2P connection code needs a transport endpoint at the final boundary, so the shared protocol should carry Endpoint-shaped data instead of forcing late reconstruction from split fields.
- Persisting proxy-node addresses can produce stale routes after NAT, reconnect, or port mapping changes; address persistence must not be treated as control-node truth.
- If the reported proxy-node name is treated as the stable id, name changes or duplicate names can corrupt approval, liveness, or selection behavior.
- If the name is omitted from shared protocol while server/client code assumes it exists, clients will continue connecting by id and fail name-based certificate/SNI scenarios.
- Switching existing local SQLite keys from base58 to base36 can make old rows invisible unless compatibility reads or migration are designed.

## Downstream Follow-Up
| stage | follow_up |
|-------|-----------|
| design | Map `PnServerInfo` to protocol, selector, heartbeat, client connection, and store-facing identity/policy scope paths without requiring proxy-node address persistence. |
| design | Map `CHG-pn-server-endpoint-address-contract` through Endpoint raw codec/serde, server reporting/selection response, HTTP projection, and client connection consumption. |
| design | Map `CHG-pn-server-address-live-state-contract` through selector/network/store boundaries so proxy-node addresses are live runtime data, not SQLite truth. |
| design | Map `CHG-pn-server-reported-name-contract` through `PnServerInfo`, proxy heartbeat/report payloads, server API responses, selector/storage compatibility, and client proxy connection name usage. |
| design | Map `CHG-node-id-base36-contract` to shared `NodeId` helpers and every direct NodeId string consumer across `vpn-frame`, `bucky-vpn-server`, `bucky-vpn`, and `vpn_web`; decide old base58 read compatibility. |
| implementation | Update `vpn_protocol.rs`, direct PN server consumers, and store-facing PN server identity/policy contracts only after admission passes. |
| implementation | After approved design/admission, replace NodeId base58 string writes/parses with base36 equivalents in admitted scope paths. |
| testing | Record direct coverage for Endpoint protocol shape, no proxy-node address persistence truth, no old endpoint-string compatibility, and workspace compatibility. |
| acceptance | Audit that no endpoint string remains the PN server id or storage format. |

## Approval Record
- approver: auto-pipeline
- approval_date: 2026-07-07T00:41:31+08:00
- user_statement: "确认，自动处理后续步骤"
