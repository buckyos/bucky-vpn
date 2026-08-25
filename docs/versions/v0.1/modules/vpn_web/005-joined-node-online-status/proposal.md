---
task_manifest: task.yaml
status: approved
---

# Joined Node Online Status Display Proposal

Risk profile: not-created (replace with ./risk-profile.yaml only after high-risk confirmation)

## Workflow Tier Judgment
- Proposed tier: standard
- Final tier: standard
- Tier rationale / triggered boundaries: This is a bounded, single-module UI bugfix with a known cause and a narrow verification signal. It changes user-visible status rendering, so `standard` is more appropriate than `trivial`, but it does not materially change a UI workflow or accessibility boundary, API contract, persistent data, runtime lifecycle, security, build graph, or cross-project behavior, so no high-risk trigger is confirmed.
- Proposal and tier confirmation: confirmed by the user on 2026-07-20 with “确认”

## Background and Goal
The Joined Nodes table renders an online node's IP list with `node.ipList?.join(', ') ?? 'online'`. When the backend correctly returns `online: true` and an empty `ip_list`, `join(', ')` produces an empty string rather than `null`, so the Status cell is blank, as shown in the supplied screenshot.

The goal is to keep online state visible even when no IP address is currently available.

## Scope
### In scope
- Update the Joined Nodes Status cell so an online node with a non-empty IP list displays the addresses.
- Display `online` when the node is online but its IP list is null or empty.
- Preserve the existing `offline` label and status colors.

### Out of scope
- Changing backend online-state semantics or the `online` / `ip_list` API fields.
- Redesigning the table, status vocabulary, refresh behavior, or other Joined Nodes actions.
- Adding or modifying `vpn_web` tests, per the repository's no-new-tests custom rule.
- Modifying unrelated existing dirty working-tree files.

### Boundary with neighboring modules
The backend response is treated as correct: online state and optional address metadata are independent. Delivery is confined to `vpn_web/lib/joined_nodes_page.dart`; `vpn-frame`, `vpn-server`, and the generated Web API model remain unchanged.

## Requirement Review
The requested fix is reasonable. The Status column should never become visually empty for a known online node merely because optional address metadata is empty. A narrow empty-list fallback preserves the useful address display when addresses exist and avoids introducing a compatibility or contract change.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-joined-node-online-status | Render a visible `online` fallback for online joined nodes whose IP list is null or empty, while preserving address display and offline behavior. | `vpn_web/lib/joined_nodes_page.dart` only. | Keeps the current English status vocabulary instead of introducing a new label or badge. | Code inspection covers null, empty, non-empty, and offline branches; targeted Flutter analysis succeeds, or any environment limitation is recorded; manual rendering logic review matches the screenshot case. | No backend/API-model change, table redesign, or new frontend tests. |

## Success Criteria
- Concrete user-visible or system-visible result: the screenshot scenario shows `online` in the Status column instead of a blank cell.
- Required evidence: online plus null/empty IP list resolves to `online`; online plus addresses still displays those addresses; offline remains `offline`; the narrowest relevant Flutter analysis passes or an environment limitation is recorded.
- Explicit non-goals: backend liveness changes, API serialization changes, broader UI cleanup, and frontend test additions.

## Risks
- The existing working tree contains unrelated modifications, including other `vpn_web` files; they must be preserved and excluded from this task's delivery scope.
- Validation will rely on static analysis and targeted/manual branch inspection because the repository custom rule forbids adding frontend tests by default.
