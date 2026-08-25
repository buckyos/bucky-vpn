---
task_manifest: task.yaml
status: approved
---

# Align vpn_web Proxy Node API Proposal

Risk profile: ./risk-profile.yaml

## Workflow Tier Judgment
- Proposed tier: high-risk
- Final tier: high-risk
- Tier rationale / triggered boundaries: The server already returns `JsonPnServerInfo` as `id`, optional `name`, `endpoints`, and optional `port_mapping`, while `vpn_web` still expects and emits the obsolete `id`, `ip`, `port`, and `addresses` shape. Aligning the annotated Web API model changes generated serialization code and is explicitly routed by the repository's Web API model/serialization custom rule through build-runner-backed testing and acceptance review. The server contract itself will not change.
- Proposal and tier confirmation: confirmed by the user on 2026-07-20 with the explicit auto-pipeline launch statement “确认，自动完成任务”

## Background and Goal
`GET /pn_proxy_nodes` and the Web client use the same route, but their nested `pn_server` JSON contracts have drifted. The server's current response contains `id`, `name`, `endpoints`, and `port_mapping`; `vpn_web` deserializes required `ip` and `port` fields that are no longer present. This causes proxy-node acquisition to fail during `ProxyNode.fromJson` instead of producing rows for the page.

The goal is to make `vpn_web` consume the current server response correctly and preserve a matching `pn_server` payload when the same model is sent to the approve/reject endpoints.

## Scope
### In scope
- Replace the obsolete `PnServerInfo` Web fields with the server's current `name`, `endpoints`, and `port_mapping` shape.
- Preserve endpoint display/deduplication behavior for the proxy-node page using the new `endpoints` list.
- Regenerate `vpn_web/lib/api.g.dart` from the annotations in `vpn_web/lib/api.dart` using the repository's Dart build-runner workflow.
- Verify that proxy-node response deserialization and approve/reject request serialization use the same server-defined nested shape.

### Out of scope
- Changing the server route, method, response envelope, or Rust-side JSON contract.
- Redesigning the Proxy Nodes page, approval workflow, authentication, or error handling.
- Adding or modifying `vpn_web` tests, per the repository's no-new-tests rule unless the user separately requests an exception.
- Broad cleanup of existing frontend models or unrelated dirty working-tree files.

### Boundary with neighboring modules
`vpn-server` is the read-only contract source for this correction. Delivery is confined to the `vpn_web` API client model and its tool-generated serialization glue; existing UI code continues consuming `PnServerInfo.allAddresses` and does not require a page-level behavior change.

## Requirement Review
The requested correction is necessary and well bounded: the route itself already agrees, while the nested data model does not. Treating the current server shape as canonical avoids compatibility shims for an obsolete response and also prevents approval requests from sending the old structure. The main tradeoff is that the frontend stops accepting the old `ip`/`port` payload shape; supporting both would add unrequested compatibility logic and could hide future contract drift.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-align-proxy-node-api | Make `vpn_web` deserialize and serialize proxy-node `pn_server` values with `id`, optional `name`, `endpoints`, and optional `port_mapping`, matching `vpn-server/src/api.rs`. | `vpn_web/lib/api.dart` plus build-runner-owned `vpn_web/lib/api.g.dart`; server is read-only evidence. | Drops accidental compatibility with the obsolete `ip`/`port`/`addresses` shape. | Generated mapping reads/writes the current keys; Flutter analysis/build or the narrowest available equivalent passes; implementation review compares both directions with the Rust contract. | No server contract, route, UI layout, or auth change. |

## Success Criteria
- Concrete user-visible or system-visible result: opening or refreshing Proxy Nodes can parse the current `GET /pn_proxy_nodes` response instead of failing on missing `ip`/`port` fields.
- Required evidence: `api.g.dart` is regenerated from annotations; generated `PnServerInfo` mapping uses `id`, `name`, `endpoints`, and `port_mapping`; targeted Flutter analysis/build completes or any environment limitation is explicitly recorded; acceptance confirms approve/reject serialization remains aligned.
- Explicit non-goals: backward-compatible parsing of the obsolete server shape, backend changes, UI redesign, and new frontend tests.

## Risks
- A hand-edited generated file could drift from annotations; regeneration must be performed by build_runner.
- Updating response parsing without request serialization would leave approve/reject calls inconsistent; both directions must be reviewed together.
- Existing unrelated working-tree changes, including `vpn_web/lib/base58.dart`, must be preserved and excluded from this task's implementation scope.
