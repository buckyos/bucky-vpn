---
module: vpn_web
version: v0.1
status: approved
approved_by: user-request
approved_at: 2026-07-01T17:44:43+08:00
approved_content_sha256: 586063764cd99446899987fa14a53995585c0e6abbfee1f4eb7ec168d147bdb1
---

# vpn_web Design

## Design Scope
### Goals
- Add a third home tab for proxy node management.
- Consume the proxy-node HTTP API and display proxy allow control, id, observed real connection address with `ip:port` fallback, liveness, comment, and row-level comment action.
- Keep the page consistent with the Joined Nodes tab loading, refresh, table, and toast patterns.
- Treat backend-provided `NodeId` strings as canonical base36 values in frontend operations and remove local base58-to-base36 conversion.

### Non-goals
- Do not redesign the backend proxy-node contract.
- Do not introduce unrelated route, theme, or layout changes.
- Do not add new frontend tests unless explicitly requested.

## Overall Approach
`Home` extends its tab controller from two tabs to three and inserts `ProxyNodesPage` as the third tab. `ProxyNodesPage` follows the `JoinedNodesPage` pattern: it loads data during `initState`, exposes a refresh button, shows a loading spinner before the first response, renders a horizontally scrollable `DataTable`, and reports API failures with a top toast.

The page consumes existing backend endpoints:
- `GET /pn_proxy_nodes` to list proxy nodes.
- `POST /approve_pn_proxy_node` to mark a proxy node approved.
- `POST /reject_pn_proxy_node` to mark a proxy node rejected.

The frontend model mirrors the backend `pn_server`, `observed_addr`, `status`, `live`, `updated_at`, and `comment` fields, but the table intentionally hides separate raw `status` and `updated_at` columns. The Address column prefers `observed_addr`, which is the control node's observed real connection address; if that field is missing, it falls back to `pn_server.ip:pn_server.port` so older or offline records remain readable. `status` drives the restored allow checkbox for approve/reject management, while the `Action` column is reserved for adding or editing comments.

For network-member NodeId handling, the frontend no longer decodes a base58 NodeId and re-encodes it as base36 for display/name matching. The backend is responsible for returning canonical base36 `node_id` strings, and `NetworkMembersPage` compares and displays those strings directly.

## Simplicity Check
| topic | decision | reason |
|-------|----------|--------|
| Page implementation | Add a focused `proxy_nodes_page.dart` instead of expanding `home.dart`. | Matches existing page-per-tab structure and keeps `home.dart` tab wiring small. |
| API wrapper | Add typed models and three API methods in `api.dart`. | Existing backend endpoints already exist; typed wrappers match local API style. |
| Shared table abstraction | Do not extract a shared table component from Joined Nodes. | Current duplication is small and a shared abstraction would be speculative. |

## Current Structure
| path | current_responsibility | relevant_existing_behavior |
|------|------------------------|----------------------------|
| `vpn_web/lib/home.dart` | Home dashboard, traffic cards, and tab wiring | `TabController(length: 2)` with Joined Nodes and My Networks tabs. |
| `vpn_web/lib/joined_nodes_page.dart` | Joined node list, allow-join checkbox, comment/delete actions | Provides the loading, refresh, data table, and toast pattern to mirror. |
| `vpn_web/lib/network_members_page.dart` | Network member list and node-name lookup | Currently contains local base58-to-base36 conversion for NodeId matching; remove this once backend emits canonical base36. |
| `vpn_web/lib/api.dart` | Typed API models and wrappers | Has `(HttpResult, Data?)` list wrappers and session bearer header handling. |
| `vpn_web/lib/api.g.dart` | Generated serialization glue | Must be regenerated if `api.dart` annotated models change. |

## Invariants to Preserve
| invariant | source | preservation_decision |
|-----------|--------|-----------------------|
| Existing Joined Nodes and My Networks tabs keep their behavior. | `home.dart`, `joined_nodes_page.dart`, `networks_page.dart` | Only tab count/order and new child are changed; existing page widgets are not rewritten. |
| API wrappers keep the `(HttpResult, Data?)` pattern. | `api.dart` | Proxy-node list returns `(HttpResult, List<ProxyNode>?)`; mutation methods return `HttpResult`. |
| Generated code remains tool-owned. | `api.g.dart` boundary | `api.g.dart` is changed only through build_runner when model annotations change. |
| Non-NodeId base58 encoding remains untouched. | `api.dart` password hash boundary | Login/account hashing can continue using base58 because the base36 change is only for `NodeId`. |

## Submodules
| submodule | type | responsibility | depends_on |
|-----------|------|----------------|------------|
| `home-shell` | assembly | Compose dashboard header, traffic cards, and tab pages. | `page-widgets`, `api-client` |
| `page-widgets` | business | Render Joined Nodes, Proxy Nodes, My Networks, and dialogs. | `api-client` |
| `api-client` | technical | Map backend HTTP endpoints to typed frontend models and result tuples. | none |

## Boundary Rationale
The proxy node management feature belongs in `page-widgets` because it is a user-facing tab workflow. The HTTP endpoint mapping belongs in `api-client` because it is a typed backend-consumption boundary. `home-shell` remains the composition root for tabs and should not own table rendering or API parsing.

## Boundary Decision Matrix
| boundary | classification | business_responsibility | shared_logic_or_technical_area | decision |
|----------|----------------|-------------------------|--------------------------------|----------|
| Proxy node page | business | Let administrators inspect proxy node identity/observed address/comment, update allow status through a dedicated allow control, and edit comments through actions. | Uses existing refresh/toast/table UI pattern. | Add/update `vpn_web/lib/proxy_nodes_page.dart` as a page widget with allow, id, observed-address-preferred address, live, comment, and action columns. |
| Proxy node API wrapper | technical | Provide typed data to the page. | HTTP session header, JSON parsing, result tuple handling. | Add models and wrapper methods in `vpn_web/lib/api.dart`. |
| Generated JSON glue | technical | Keep model serialization aligned with annotations. | build_runner-generated code. | Regenerate `vpn_web/lib/api.g.dart` if models are added. |
| Network member NodeId lookup | business | Show member names and ids consistently with backend canonical ids. | Existing member API models. | Compare `nodeId` strings directly and remove base58 conversion dependency. |

## Dependency Graph
| source | depends_on | reason | cycle_check |
|--------|------------|--------|-------------|
| `home-shell` | `page-widgets` | Tab composition imports page widgets. | acyclic: assembly depends inward only. |
| `page-widgets` | `api-client` | ProxyNodesPage calls typed API methods. | acyclic: business depends on technical boundary. |
| `api-client` | none | API layer has no dependency on page widgets. | acyclic: terminal dependency. |

## Key Call Flows
| flow | caller | callee_submodule_path | purpose | failure_handling |
|------|--------|-----------------------|---------|------------------|
| Load proxy nodes | `ProxyNodesPage.initState` / refresh button | `api-client` `getProxyNodes` | Fetch all proxy nodes returned by `/pn_proxy_nodes`. | Before first success the page shows a spinner; failures keep existing data and show a top toast. |
| Approve proxy node | Allow checkbox in `ProxyNodesPage` | `api-client` `approveProxyNode` | Mark a proxy node status as approved while preserving the current comment. | On success refresh the list; on failure show a top toast and leave the previous list unchanged. |
| Reject proxy node | Allow checkbox in `ProxyNodesPage` | `api-client` `rejectProxyNode` | Mark a proxy node status as rejected while preserving the current comment. | On success refresh the list; on failure show a top toast and leave the previous list unchanged. |
| Comment proxy node | Comment action in `ProxyNodesPage` | `api-client` `approveProxyNode` / `rejectProxyNode` | Persist row comment through the existing approval API while preserving the current approved/rejected state. Pending nodes need allow status set first because no comment-only backend endpoint exists. | On success refresh the list; on failure show a top toast and leave the previous list unchanged. |

## Large Module Submodule Decision
| submodule | source_proposal | decision | design_packet | reason |
|-----------|-----------------|----------|---------------|--------|
| pages and dialogs | P-vpn-web-proxy-node-management | Use existing root packet; no direct submodule packet. | docs/versions/v0.1/modules/vpn_web/design.md | The feature is one page-level tab workflow inside the existing frontend module. |

## Trigger Matrix
| trigger_category | applies | evidence | design_coverage | required_checks | deferred_checks_and_reason |
|------------------|---------|----------|-----------------|-----------------|----------------------------|
| contract/protocol | yes | Frontend consumes the backend `observed_addr` list-field added by `bucky-vpn-server`. | Overall Approach and Interfaces and Dependencies define `observed_addr` consumption and fallback semantics. | Analyze/build after generated code update; backend admission handles API production change. | Live frontend/backend smoke remains manual; owner integration environment, acceptance risk endpoint drift. |
| data/schema | yes | Frontend models mirror proxy-node JSON fields including `observed_addr`. | Boundary Decision Matrix and Interfaces and Dependencies define `ProxyNode`, `observedAddr`, and `PnServerInfo`. | Regenerate generated JSON glue if annotated models change. | No extra schema migration; owner frontend implementer, acceptance risk generated code must match annotations. |
| security/privacy/permission | yes | The UI can approve or reject proxy-node allow status. | Key Call Flows describe approve/reject failure handling. | Verify status mutation calls use the existing bearer session wrapper and do not bypass backend auth. | Permission semantics remain backend-owned; owner backend, acceptance risk UI only reflects API results. |
| runtime/integration | yes | The page loads and mutates backend data at runtime. | Key Call Flows cover load, approve, reject. | Run or attempt `vpn_web` DV/integration validation through harness. | Manual visual verification may remain; owner frontend implementer, acceptance impact UI behavior. |
| build/dependency/config/deployment | no | No dependency or deployment setting changes are planned. | Simplicity Check rejects new dependencies. | not-applicable: no dependency/config change | not-applicable: no deferred check; acceptance impact none. |
| ui/datamodel/workflow | yes | Adds a home tab and management table with revised proxy-node fields. | Overall Approach, Current Structure, and Data and State define allow, id, combined address, live, comment, and comment action rendering while excluding raw status/update columns. | Analyze/build and inspect tab/page rendering paths. | Automated widget coverage deferred by vpn_web no-new-tests rule; owner testing, acceptance risk documented gap. |
| contract/protocol | yes | Backend NodeId strings are now base36 canonical. | `NetworkMembersPage` treats node ids as already canonical base36 and removes local base58 conversion. | Analyze/build | owner backend contract; risk stale server responses will no longer be normalized by the UI. |
| harness/process | yes | The change requires direct proposal/design mapping and admission. | Directly Mapped Change Items defines scope paths. | Run schema-check, admission-check, and stage checks. | Dirty worktree may block scope checks; owner environment, acceptance risk evidence caveat. |

## Directly Mapped Change Items
| change_id | proposal_id | design_coverage | scope_paths |
|-----------|-------------|-----------------|-------------|
| CHG-proxy-node-management-tab | P-vpn-web-proxy-node-management | Add a third home tab wired to `ProxyNodesPage`; add typed proxy-node models and API wrappers for list/approve/reject; render allow control, id, address, liveness, comment, and comment action without separate raw status or updated-time columns. | `vpn_web/lib/home.dart`, `vpn_web/lib/proxy_nodes_page.dart`, `vpn_web/lib/api.dart`, `vpn_web/lib/api.g.dart` |
| CHG-proxy-node-real-address-display | P-vpn-web-proxy-node-real-address | Extend `ProxyNode` with backend `observed_addr` and render the Address column from `observedAddr` when present, falling back to `pnServer.ip:pnServer.port` only when no observed address is available. | `vpn_web/lib/api.dart`, `vpn_web/lib/api.g.dart`, `vpn_web/lib/proxy_nodes_page.dart` |
| CHG-vpn-web-node-id-base36 | P-vpn-web-node-id-base36 | Remove frontend base58-to-base36 NodeId conversion and use backend base36 `node_id` strings directly in member/name lookup paths; keep non-NodeId base58 password hashing unchanged. | `vpn_web/lib/network_members_page.dart` |

## Implementation Order
| phase | goal | prerequisites | output | depends_on | parallel |
|-------|------|---------------|--------|------------|----------|
| 1 | Add API models and wrappers | Approved proposal/design and admission | `ProxyNode`, `PnServerInfo`, list/approve/reject methods | none | no |
| 2 | Add proxy-node page | Phase 1 | `ProxyNodesPage` UI and actions | 1 | no |
| 3 | Wire home tab | Phase 2 | Third tab and `TabController(length: 3)` | 2 | no |
| 4 | Regenerate JSON glue and validate | Phases 1-3 | Updated generated file and validation evidence | 1,2,3 | no |
| 5 | Normalize frontend NodeId handling | approved base36 admission | member lookup uses backend base36 ids directly and removes base58 dependency from that page | none | yes |

## Key Decisions
| decision | chosen | alternatives_considered | rejection_reason |
|----------|--------|--------------------------|------------------|
| Proxy data source | Use `/pn_proxy_nodes` and proxy approval endpoints. | Reuse `/get_joined_nodes` and label joined nodes as proxies. | Joined nodes are not proxy nodes and would hide backend proxy approval state. |
| Approval/status mapping | Do not render raw `status`; use it internally for a restored allow checkbox that calls approve/reject. | Put approve/reject in the Action column. | The latest user correction says Action is for comments, not proxy approval. |
| Action mapping | Use the Action column for comment editing only. | Use Action for approve/reject. | The latest user correction explicitly assigns Action to adding comments. |
| Address mapping | Prefer backend `observed_addr` for the Address column, falling back to `pn_server.ip:pn_server.port` only when the observed address is absent. | Always render `pn_server.ip` and `pn_server.port`; keep separate IP and Port columns. | The latest proposal requires the real address observed by the control node, and local `pn_server` config may be misleading behind NAT or container networking. |
| Page placement | Insert as third tab after My Networks. | Replace an existing tab or nest inside Joined Nodes. | User requested the third tab; nesting would make proxy management less discoverable. |
| NodeId frontend normalization | Trust backend base36 ids directly. | Keep client-side base58 decoding shim. | Keeping the shim would preserve the old operation the user asked to remove. |

## Data and State
| data_or_state | owner_submodule | access_for_others | state_transitions |
|---------------|-----------------|-------------------|-------------------|
| Proxy node list | `api-client` as backend-consumption boundary | `ProxyNodesPage` reads through `getProxyNodes`. | not-loaded -> loaded empty/list; API failure leaves prior state and shows toast. |
| Proxy node observed address | backend `bucky-vpn-server`, represented through `api-client` | `ProxyNodesPage` reads `ProxyNode.observedAddr` for display. | observed address present -> Address column shows it; absent/empty -> fallback to `pnServer.ip:pnServer.port`; API parsing failure follows existing list error path. |
| Proxy node allow state | backend proxy approval service, represented through `api-client` | `ProxyNodesPage` reads status internally and requests approve/reject through the allow checkbox, then refreshes after success. | pending/rejected -> approved via checked allow; approved/pending -> rejected via unchecked allow; failure keeps old UI state. |
| Proxy node comment | backend proxy approval service, represented through `api-client` | `ProxyNodesPage` displays comment and sends comment through the existing approve/reject request body when edited, preserving approved/rejected status. | empty/existing comment -> edited comment through current status-preserving request; pending nodes cannot use comment-only action without a backend endpoint; failure keeps old UI state. |
| Active tab index | `home-shell` | Page widgets do not write it directly. | selected index changes through TabBar tap/listener; invalid index avoided by matching controller length and child count. |
| Network member NodeId text | backend API represented in page state | `NetworkMembersPage` reads `nodeId` directly. | backend base36 id -> direct display/name lookup; stale base58 id -> no UI-side re-encoding. |

## Testability
| seam | verification_path | failure_case |
|------|-------------------|--------------|
| API wrapper methods | `flutter analyze` and web build compile typed method/model usage including `observedAddr`. | Missing fields or generated-code mismatch fail analyzer/build. |
| ProxyNodesPage UI flow | Manual visual verification or future widget tests can instantiate page with API seam if test support is added. | API failure shows toast; allow/comment failures preserve old list; current no-new-tests rule records automated UI gap. |
| Home tab wiring | Analyzer/build verify child count and imports. | TabController length mismatch or missing page import fails runtime/build review; manual verification can check tab navigation. |
| NodeId direct lookup | Analyzer/build verify removal of base58 dependency and direct string comparison in `NetworkMembersPage`. | Stale backend base58 responses can fail name matching, which belongs to backend migration. |

## Interfaces and Dependencies
| interface | consumer | compatibility | notes |
|-----------|----------|---------------|-------|
| `GET /pn_proxy_nodes` frontend wrapper | `CHG-proxy-node-management-tab`, `CHG-proxy-node-real-address-display` | new | Consumes backend API; response maps to `List<ProxyNode>` including optional `observed_addr` for display. |
| `ProxyNode.observedAddr` | `CHG-proxy-node-real-address-display` | new | Maps backend `observed_addr`; nullable so older/missing responses can fall back to `pn_server.ip:port`. |
| `POST /approve_pn_proxy_node` frontend wrapper | `CHG-proxy-node-management-tab` | new | Consumes existing backend API with `pn_server` and optional `comment`. |
| `POST /reject_pn_proxy_node` frontend wrapper | `CHG-proxy-node-management-tab` | new | Consumes existing backend API with `pn_server` and optional `comment`. |
| `ProxyNodesPage` widget | `home-shell` | new | New page widget imported by `home.dart`. |
| `NetworkMembersPage.nodeId` lookup | `CHG-vpn-web-node-id-base36` | migration-required | Backend must return canonical base36 ids; frontend no longer rewrites base58 ids. |

## Document Index
| document | topic | scope |
|----------|-------|-------|
| `design.md` | Proxy node management tab design | full change |

## Risks and Rollback
- If backend field names differ from documented endpoints, generated model parsing or runtime calls may fail; rollback by removing the new tab and proxy-node API wrappers.
- If the working tree remains dirty with unrelated files, stage scope checks may not provide clean evidence; isolate or clean unrelated changes before relying on final scope evidence.
- If Flutter tooling is unavailable, validation may be limited to static review and recorded as a testing/acceptance blocker.
- If a backend still returns base58 NodeId strings, the frontend will no longer compensate; rollback requires a new approved compatibility requirement rather than restoring base58 normalization by default.

## Approval Record
- approver: user-request
- approval_date: 2026-07-01T17:44:43+08:00
- user_statement: "确认，自动处理后续步骤"
