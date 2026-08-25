---
task_manifest: task.yaml
status: approved
---

# Expand PN Control Command Limit Proposal

Risk profile: not-created (created only after high-risk confirmation)

## Workflow Tier Judgment
- Proposed tier: high-risk
- Final tier: standard
- Tier rationale / triggered boundaries: The requested length type changes the PN-control wire framing from a two-byte to a three-byte command length and crosses the shared `vpn-frame` contract plus both PN/control-plane runtime endpoints in `vpn-server`. Mixed old/new endpoints cannot parse each other's command frames, so this is a protocol-compatibility and coordinated-deployment change and is high-risk by default.
- Proposal and tier confirmation: confirmed with the user-selected standard tier by “确认，按标准任务完成就好” on 2026-07-21. The user-selected lower tier retains the documented wire-compatibility, coordinated-deployment, and default-load residual risks.

## Background and Goal
`VpnControlClient` currently binds its command transport to the general `VpnCmdPkgLen = U16`, limiting serialized PN traffic reports to the two-byte command-size range. The goal is to give the dedicated PN-node-to-control-plane channel its own `U24<{ 10 * 1024 * 1024 }>` command-length contract so traffic-statistics commands can carry bodies up to 10 MiB.

Read-only inspection found that changing only `VpnControlClient` would be invalid: `PnControlServer` and the concrete command service must decode the same three-byte header. The server also independently rejects traffic command bodies above 1 MiB, which would defeat the requested 10 MiB transport limit. The bounded solution therefore migrates both endpoints and aligns that server-side body validation.

## Scope
### In scope
- Define a dedicated, exported PN-control command-length type equivalent to `sfo_cmd_server::U24<{ 10 * 1024 * 1024 }>` in `vpn_frame::server`.
- Bind `VpnControlClient` and its concrete `vpn-server` command-client aliases to the dedicated type.
- Bind `PnControlServer` and the concrete proxy-control command service to the same dedicated type, including the matching command-header type used for version validation.
- Raise the PN-control traffic-command body validation limit from 1 MiB to 10 MiB so it agrees with the framing limit.
- Raise `MAX_TRAFFIC_RECORDS_PER_COMMAND` from 256 to 25,000. With 32-byte node IDs, the larger `ProxyTrafficReport` encoding is approximately `124 + report_id_len` bytes and at the accepted 256-byte report-ID ceiling is 380 bytes; `25,000 * 380 + 8` is 9,500,008 bytes (about 9.06 MiB), leaving about 9.4% of a 10 MiB frame for variance.
- Raise the default `pn.traffic_upload.records_per_command` from 128 to 25,000 and update the example configuration. This is a chunk-size ceiling, not a minimum fill threshold, so smaller collected batches remain immediately eligible for upload.
- Add focused contract/boundary tests and compile verification for both affected crates.

### Out of scope
- Changing the ordinary VPN client/server command framing, which remains `VpnCmdPkgLen = U16`.
- Changing command codes, serialized request/response payload shapes, retry behavior, timeout behavior, sequence generation, or accounting semantics.
- Adding compression, chunking, streaming, runtime negotiation, or backward-compatible dual framing.
- Changing Flutter UI, persistence schema, public HTTP APIs, or configuration.

### Boundary with neighboring modules
`vpn-frame::server` owns the dedicated PN-control length/header contract and applies it symmetrically to `VpnControlClient` and `PnControlServer`. `bucky-vpn-server` owns the concrete PN command client and control-plane command service instantiations and must select the same contract on both sides. General VPN commands outside the proxy-control tunnel keep the existing `VpnCmdPkgLen` contract.

## Requirement Review
The requested larger command length is reasonable for batched traffic reporting, but a client-only generic change cannot work because the number of length bytes is part of the wire frame. The server's current 1 MiB validation is a second effective cap, so it must be aligned with the requested 10 MiB ceiling.

The chosen direction uses a purpose-specific type instead of enlarging `VpnCmdPkgLen` globally. This contains the larger allocation/read allowance to the authenticated PN control tunnel and avoids silently changing ordinary VPN-client compatibility. The tradeoff is deployment coupling: PN nodes and control-plane nodes must be upgraded together because no framing negotiation exists.

The proposed record ceiling is 25,000, derived conservatively from the larger of the two traffic record shapes. Under the runtime's 32-byte node-ID assumption, a `NodeTrafficReport` is `83 + report_id_len` bytes (up to 339 bytes), while a `ProxyTrafficReport` is `124 + report_id_len` bytes (up to 380 bytes). A 25,000-record worst-case proxy request is about 9.06 MiB after the request sequence and vector-length prefix, preserving roughly 0.94 MiB below the frame cap. Actual serialized byte length remains authoritative, so unusual larger node IDs or future record growth fail at 10 MiB instead of bypassing the transport limit.

The uploader default will also become 25,000 as requested. `records_per_command` only caps chunk size: a smaller completed collection batch is sent without waiting to fill the command. Large deployments can therefore consolidate up to 25,000 records per command, while small deployments retain their existing prompt-send behavior. The tradeoff is that the default concurrency of four commands can admit roughly 36.2 MiB of serialized request bodies in flight at the worst-case estimate, before decoded-object and database-processing overhead.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-pn-control-command-length-contract | Introduce a dedicated 10 MiB `U24` PN-control length/header contract and use it on both the shared control client and control server; align server traffic-body validation to 10 MiB and raise the estimated safe record ceiling to 25,000. | Limited to PN control types/handlers and the shared traffic-record ceiling in `vpn_frame`; general `VpnCmdPkgLen` stays `U16`. | Enables larger reports but makes the new PN-control framing incompatible with old endpoints and permits reads/decoded batches up to the bounded 10 MiB/25,000-record limits. | Type/compile assertions and boundary tests demonstrate the three-byte contract, 10 MiB byte boundary, 25,000 record-count boundary, rejection above either boundary, and unchanged general VPN length type. | No payload, command-code, negotiation, compression, or streaming redesign. |
| P-002 | CHG-pn-control-command-length-integration | Instantiate both the PN-side classified command client and the control-plane proxy command service with the dedicated PN-control length type, and set the PN traffic uploader's default/example chunk size to 25,000. | `vpn-server` PN-control assembly and traffic-upload configuration only; the main SN/VPN command service remains unchanged. | Requires coordinated endpoint deployment; large deployments use much larger commands by default, increasing peak in-flight and server-side processing work. | Source ownership scan plus focused `vpn-server` config/compile/tests prove both endpoint aliases use the same type, default/example configuration is 25,000, smaller batches are not delayed, and ordinary command services still use `VpnCmdPkgLen`. | No unrelated server lifecycle, storage, HTTP API, or other configuration change. |

## Success Criteria
- PN traffic-statistics requests can be framed and accepted with serialized bodies up to and including 10 MiB, while bodies above 10 MiB are rejected by the command-length/body boundary.
- Traffic commands accept up to 25,000 records and reject 25,001; tests document the `NodeTrafficReport`/`ProxyTrafficReport` size estimate used to choose that ceiling.
- `VpnControlClient`, `PnControlServer`, the classified PN command client, and the proxy-control command service all use the same dedicated three-byte length contract.
- General VPN command clients/servers continue to use `VpnCmdPkgLen = U16`.
- The uploader's runtime default and example configuration are 25,000 records per command, configuration validation permits values through 25,000, and smaller completed batches are still sent without waiting to fill a chunk.
- Focused boundary tests, affected-crate compile/tests, protocol-type scans, and post-implementation acceptance all pass.
- Deployment documentation in the task evidence records that old and new PN-control endpoints cannot be mixed.

## Risks
- The framing width changes on the wire; staggered deployment can make PN-control connections unreadable until both endpoints run matching code.
- Raising the accepted body size, record count, and uploader default increases peak per-command memory, CPU, and database work. The fixed 10 MiB type limit, server body-size check, authenticated control tunnel, and 25,000-record ceiling bound that exposure.
- With four concurrent commands at the unchanged default, worst-case serialized request bodies alone can total about 36.2 MiB; decoded records, responses, and database operations add further transient cost.
- The 380-byte estimate assumes the runtime's normal 32-byte node IDs and the already-enforced 256-byte report-ID maximum. The 10 MiB serialized-byte check remains authoritative if those assumptions change.
- A missed concrete alias on either endpoint would compile in isolation but fail at runtime, so testing must cover the concrete `vpn-server` instantiations as well as shared generic types.
- Response bodies use the same command framing and therefore also gain the 10 MiB ceiling; acceptance must verify request and response boundary behavior without broadening ordinary VPN commands.
- The working tree contains pre-existing user changes; implementation and evidence must preserve unrelated modifications.
