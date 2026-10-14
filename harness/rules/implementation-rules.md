# Implementation Rules

Applies to high-risk implementation, bugfixes, and production-code changes. Standard tasks use [Task Entry Gate Rules](task-entry-gate-rules.md#tier-default-flows).

## Required Inputs
- Select the active packet under [Task Entry Gate Rules](task-entry-gate-rules.md#task-packet-selection).
- Read approved `proposal.md` and `design.md`; each current `change_id` needs direct design, target-module, and Scope Path mappings. Historical packets, module overviews, and chat context do not replace them.
- Keep the active stage, targets, change ids, and Scope Paths in canonical `task.yaml`, with task-level `risk-profile.yaml`.

## Entry Checks
- When preparation evidence is useful, run `UV_CACHE_DIR=.harness/uv-cache uv run --active python ./harness/scripts/harness-check.py --task <packet>/task.yaml --profile pre-edit`.
- This profile checks approved proposal/design, risk profile, and direct task/design mappings. `task-transition.py` records design approval after its checks pass; approved status alone does not prove coverage.
- Resolve missing coverage through the return routes below before coding.

## Execution
- Make the minimum production change satisfying the proposal/design and current request. For issue-driven work, follow [GitHub Issue Execution Priority](task-entry-gate-rules.md#github-issue-execution-priority); recheck the issue after material discoveries or design deviations.
- Follow `## File-Level Implementation Sequence` in dependency order. When child tasks are useful, give each the relevant proposal/design excerpts, child design, `change_id`, interfaces, and likely files; additional file access remains allowed.
- Update traceability when useful if actual impact materially differs from `Scope Paths`.
- Prefer post-implementation test work; combined or supporting edits are allowed when useful.
- Match surrounding style. Avoid unrelated refactoring, formatting, renaming, comment rewrites, features, or cleanup. Remove only artifacts made unused by this change; record unrelated defects as follow-up or residual risk.

## Verification
- A repository may prohibit proactive implementation validation except when the user asks, debugging needs evidence, or task docs/local rules require it.
- Rust: run `cargo fmt` only on explicit user request or repo-local requirement.

## Return Routing
- Missing/draft proposal or direct proposal mapping: proposal.
- Missing module, packet, or `change_id`: task entry or owning upstream stage.
- Failed schema/risk/plan check or upstream contradiction: owning stage.
- Approved documents do not cover the task: prefer a sibling/amendment packet, or update the current packet with an explicit reason and status change under [Task Entry Gate Rules](task-entry-gate-rules.md).
