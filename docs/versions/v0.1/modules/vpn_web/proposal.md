---
module: vpn_web
version: v0.1
status: approved
approved_by: user-request
approved_at: 2026-07-01T17:44:43+08:00
approved_content_sha256: 4c4285634e969677a39e8cb7b570fdb8eaa38af60a50300e7e7f6c6e1a757076
---

# vpn_web Proposal

## Background and Goal
`vpn_web` is the Flutter Web frontend for the VPN project. This packet records frontend requirements so UI work is admitted through versioned scope instead of chat-only context.

Current user request: adjust `vpn_web/lib/proxy_nodes_page.dart` so proxy node rows combine IP and port into one address field, do not show separate update or raw status fields, keep proxy approval management as its own allow control, make the action field add or edit the proxy node comment shown in the comment field, and display the real address observed when the proxy node connects to the control node rather than the proxy node's local configured address.

New user request: all NodeId string operations should use base36. For `vpn_web`, this means frontend-displayed node ids, request parameters sent to backend NodeId APIs, and any local NodeId conversion helpers should use base36. Non-NodeId base58 usage such as password hashing is out of scope.

## Scope
### In scope
- Flutter application root, route-owned pages, tabs, dialogs, and API consumption under `vpn_web/lib/`.
- API wrapper and generated model source in `vpn_web/lib/api.dart`, with generated code kept tool-owned.
- A proxy node management tab/page that displays connected proxy nodes with a compact address, allow control, comment, and comment action.
- Frontend NodeId display/request handling uses base36.

### Out of scope
- Rust client/server behavior changes.
- Backend API contract redesign beyond consuming existing or already exposed proxy node fields, except that the real connection address requires a paired `bucky-vpn-server` API extension before frontend implementation can be complete.
- Manual edits to generated files such as `vpn_web/lib/api.g.dart`.
- Non-NodeId base58 encodings, including password hash display/transport.

### Boundary with neighboring modules
- `vpn_web` owns frontend UI rendering, navigation, and API wrapper usage.
- Backend modules own the authoritative proxy node data and permission semantics.

## Assumptions and Ambiguities
| question | evaluation | risk_or_tradeoff | decision |
|----------|------------|------------------|----------|
| Should proxy node management be a new direct submodule packet? | The feature belongs to the existing pages/widgets responsibility and reuses the Joined Nodes style. | A separate packet would add process overhead without a distinct business boundary. | Keep this in the root `vpn_web` packet and implement as a page/widget change. |
| Should the UI change backend allow state? | The latest request says action is for comments, not approving proxy nodes, and asks to restore proxy approval logic. | Hiding the raw status keeps the table simpler, but approval still needs a visible control. | Restore the independent allow control for approve/reject and do not display a separate raw status column. |
| Should IP and port remain separate fields? | The latest request explicitly asks to merge IP and port into one field. | A combined address is easier to scan, but sorting/filtering by port is not separately available. | Display a single address field in `ip:port` form. |
| Should update time remain visible? | The latest request explicitly says the update field is not needed. | Removing it reduces operational detail but lowers table density. | Do not show a separate updated/update-time field in the proxy node table. |
| Should new Flutter tests be added? | Repository rule says `vpn_web` should not add tests unless explicitly requested. | Lack of UI tests leaves manual verification burden. | Do not add new tests by default; use analyze/build/manual verification downstream. |

## Constraints
- Use the existing Flutter/Dart dependencies and page/dialog structure.
- Preserve current `(HttpResult, Data?)` API wrapper style.
- Keep fields explicit and aligned with backend keys.
- Do not manually edit generated serialization files; regenerate them if model annotations change.
- Keep `flutter analyze` passing.

## Requirement Challenge
| question | evaluation | risk_or_tradeoff | decision |
|----------|------------|------------------|----------|
| Is adding the entry as the third tab reasonable? | The user named the exact tab location and existing home tab area. | Reordering tabs can affect user muscle memory, but this is the requested navigation location. | Add proxy node management as the third tab in `home.dart`. |
| Should the page mirror Joined Nodes behavior? | Reusing Joined Nodes loading/refresh/toast patterns keeps the UI consistent and lowers implementation risk. | Blind duplication could keep columns the user has now rejected, such as raw status or update time. | Reuse the structural pattern only; table columns follow the latest proxy-node field requirements. |
| Should the page list all connected proxy nodes? | This matches the user's operational need to inspect proxy nodes. | If backend data is paginated or filtered, "all" depends on the existing API response contract. | Display all proxy nodes returned by the existing frontend API source for connected proxy nodes. |
| Is replacing status/update columns with address/comment/action reasonable? | It fits the user's table-shape request when action means comment editing and approval stays in a separate allow control. | Design/testing/implementation documents from the previous approval become stale until updated and re-approved. | Treat this as a proposal-stage revision; downstream stages must be updated before production code can be changed. |
| Can comment be edited without changing approval state? | Existing frontend/backend proxy APIs carry optional comment only through approve/reject requests. | Pending nodes have no status-preserving comment-only API, so a pure comment action could accidentally approve or reject them. | Action edits comments for approved/rejected nodes by preserving the current status; pending nodes must use the allow control first unless a future backend endpoint is added. |
| Which proxy node address should the UI show? | The latest request requires the real address observed through the node's connection into the control node, not the proxy node's local configured/listening address. | Current `/pn_proxy_nodes` frontend model only exposes `pn_server.ip` and `pn_server.port`; using them would keep showing the wrong address. | Frontend address display depends on a backend field for observed/remote connection address and should prefer that field over local `pn_server` config address. |
| Should frontend NodeId helpers keep base58-to-base36 conversion? | No. Once backend canonical output is base36, converting from base58 becomes wrong and can corrupt already-base36 ids. | During transition, mixed backend responses may exist; blindly removing compatibility could affect old servers. | Frontend canonical path uses base36 and avoids base58 conversion for NodeId operations; any temporary compatibility must be explicit in design. |

## Large Module Submodule Decision
| submodule | new_or_existing | responsibility | proposal_packet | reason |
|-----------|-----------------|----------------|-----------------|--------|
| pages and dialogs | existing | Home tabs and page-level frontend workflows | docs/versions/v0.1/modules/vpn_web/proposal.md | Proxy node management is a page-level workflow similar to Joined Nodes and does not need an independent packet. |

## Trigger Matrix
| trigger_category | applies | evidence | required_checks | deferred_checks_and_reason |
|------------------|---------|----------|-----------------|----------------------------|
| contract/protocol | no | No backend protocol redesign is requested. | not-applicable: no contract change | not-applicable: no deferred check; acceptance impact is limited to frontend usage. |
| data/schema | yes | The page consumes proxy-node id, observed real address, status, and comment fields; status is used for the allow control and comment state preservation, but is not shown as a raw column. Frontend NodeId fields become base36 canonical strings. | Backend must expose observed/remote address and base36 NodeId strings; frontend model/helper code must align. | If backend field or encoding is missing, frontend implementation is blocked or must show an explicit fallback; owner backend/frontend implementer, acceptance risk wrong address/id display. |
| security/privacy/permission | yes | The UI restores allow/reject management as a separate allow control and reserves action for comments. | Ensure allow control calls existing backend authorization and comment action does not silently change approval state. | Pending-node comment-only edit remains deferred without a backend endpoint; owner backend/frontend implementer, acceptance risk documented limitation. |
| runtime/integration | yes | The tab depends on backend data loading like Joined Nodes. | Verify loading, empty, error, and returned-node states through frontend analysis/build/manual verification. | Automated UI coverage remains deferred by vpn_web no-new-tests rule; acceptance impact documented in testing. |
| build/dependency/config/deployment | no | No dependency, config, or deployment change is requested. | not-applicable: no build/dependency/config/deployment change | not-applicable: no deferred check; acceptance impact none. |
| ui/datamodel/workflow | yes | Adjusts the proxy-node page workflow and table fields. | Design and implement allow/address/comment/action columns, with address sourced from backend observed/remote address, removing separate update/raw-status display. | Manual visual verification may be needed; owner frontend implementer, acceptance risk UI regression. |
| harness/process | yes | This request changes approved requirements, so downstream docs and admission must be refreshed before production code edits. | Run proposal checks now; after approval, update design/testing and rerun admission for `CHG-proxy-node-management-tab`. | Downstream stages are deferred until this revised proposal is approved; owner user approval, acceptance risk code cannot start. |

## High-Level Outcomes
- Home navigation includes a third tab for proxy node management.
- The proxy-node page lists connected proxy nodes with allow control, id, observed real connection address, comment, and a comment action.
- The page follows the existing Joined Nodes tab interaction and loading/error patterns where applicable.
- Frontend NodeId display and backend request parameters use base36.

## Proposal Items
| proposal_id | change_id | outcome | success_evidence |
|-------------|-----------|---------|------------------|
| P-vpn-web-proxy-node-management | CHG-proxy-node-management-tab | Add and refine a proxy node management tab/page that lists connected proxy nodes with allow control, id, observed real connection address, comment, and a row-level comment action, without separate raw status or update-time columns. | Design maps the change to `vpn_web/lib/home.dart` and the relevant page/API files; implementation renders the third tab and proxy-node list; frontend analyze/build/manual verification is recorded downstream. |
| P-vpn-web-proxy-node-real-address | CHG-proxy-node-real-address-display | Display the real address observed by the control node for each connected proxy node, not the proxy node's local configured `pn_server.ip:port`. | `bucky-vpn-server` design/API exposes an observed address field; `vpn_web` model consumes it; `ProxyNodesPage` Address column prefers the observed address. |
| P-vpn-web-node-id-base36 | CHG-vpn-web-node-id-base36 | Display and send NodeId strings as base36, avoiding base58 conversion for canonical NodeId operations. | `bucky-vpn-server` returns base36 NodeId fields; `vpn_web` uses those ids directly or converts only by explicit compatibility rule in design. |

## Success Criteria
- The third tab entry in `home.dart` opens the proxy node management UI.
- The UI lists every connected proxy node provided by the existing frontend API source.
- Each listed proxy node shows an allow control for approval, node id, the real observed connection address, comment, and an action field for adding or editing the row comment.
- The address field does not use the proxy node's local configured `pn_server.ip:port` when an observed/remote connection address is available.
- The proxy-node table does not show separate raw status or updated/update-time columns.
- NodeId values shown in frontend tables and sent in NodeId API request bodies are base36.
- The UI behavior is consistent with Joined Nodes for loading, empty, and error states where the data source supports those states.
- No new frontend tests are added unless the user explicitly requests the exception.

## Risks
- Existing backend/API models may not expose exactly the required proxy-node fields.
- The current backend response does not expose observed/remote connection address, so this UI requirement depends on a `bucky-vpn-server` contract update.
- Mirroring Joined Nodes too closely may expose labels or actions that belong only to joined nodes.
- Existing `vpn_web` automated tests are limited; UI behavior may require manual verification.
- Mixed base58/base36 backend responses during transition can make frontend matching logic ambiguous unless the backend contract is updated first.

## Downstream Follow-Up
| stage | required_follow_up | reason |
|-------|--------------------|--------|
| design | Refresh `CHG-proxy-node-management-tab` and add `CHG-proxy-node-real-address-display` in `design.md` so the table model uses allow, id, observed real address, comment, and comment action, and excludes separate raw status/update display. | Implementation admission requires direct design mapping to the revised field requirements and the paired backend API field. |
| design | Coordinate with `bucky-vpn-server` design for the `/pn_proxy_nodes` observed/remote address field. | Frontend cannot reliably display the real connection address until the backend contract exposes it. |
| design | Add `CHG-vpn-web-node-id-base36` mapping for frontend NodeId display/request conversion and remove base58-only NodeId helper assumptions. | Implementation admission requires direct design coverage before touching `network_members_page.dart` or API request paths. |
| testing | Refresh direct change coverage for `CHG-proxy-node-management-tab` without adding new tests unless explicitly requested. | Testing docs must reflect the revised field requirements and the no-new-tests rule. |
| implementation | After revised proposal/design/testing coverage is approved and admission passes, update `proxy_nodes_page.dart` and any required API/model glue. | Production code edits are blocked until admission succeeds. |
| acceptance | Review proposal/design/code/testing consistency and cite generated run artifacts where available. | Acceptance must audit the complete evidence chain. |

## Approval Record
- approver: user-request
- approval_date: 2026-07-01
- user_statement: "确认，自动处理后续步骤"
