---
task_manifest: task.yaml
status: approved
---

# Revert Format-only Uncommitted Changes Proposal

Risk profile: not-created (replace with ./risk-profile.yaml only after high-risk confirmation)

## Workflow Tier Judgment
- Proposed tier: standard
- Final tier: trivial
- Tier rationale / triggered boundaries: The requested result is non-behavioral and narrowly scoped. The dirty tree contains mixed semantic and formatting edits across `vpn-frame`, `vpn-server`, and `vpn_web`, so completion still requires baseline-backed semantic-diff verification, but no runtime behavior, interface, protocol, dependency, or lifecycle boundary changes and no standard-tier change record is needed.
- Proposal and tier confirmation: confirmed by the user's explicit `以简单任务 完成就好` instruction on 2026-08-18; with no unresolved proposal question, this confirms the displayed proposal and selects the trivial tier.

## Background and Goal
The current tracked working tree contains substantial uncommitted changes. Some are required semantic changes, while accidental formatting activity introduced whitespace or layout churn. The goal is to remove only formatting-only differences without losing any semantic edit or touching unrelated untracked files.

Read-only inspection already proves that `vpn_web/README.md` and `vpn_web/lib/base58.dart` have no non-whitespace diff. Other Rust files contain semantic changes and must not be restored wholesale; their formatting-only hunks or line portions require selective cleanup.

## Scope
### In scope
- Identify tracked formatting-only files, hunks, and separable line-level layout changes against `HEAD`.
- Restore the original formatting for those differences while preserving the current non-whitespace/token-level diff.
- Preserve the completed PN re-online observation fix and all unrelated semantic working-tree changes.
- Verify that the semantic diff before and after cleanup is identical, and run `git diff --check` on the resulting tracked diff.

### Out of scope
- Reverting, rewriting, or reviewing semantic changes.
- Removing or modifying untracked files, generated files, databases, lockfiles, task packets, or user-owned artifacts unrelated to formatting cleanup.
- Running broad formatters or applying new style changes.
- Committing, staging, or publishing the result.

### Boundary with neighboring modules
The cleanup is limited to presentation in already-modified tracked files. Runtime behavior, public interfaces, PN/SN protocol behavior, tests, dependencies, and repository policy remain unchanged.

## Requirement Review
The request is reasonable, but a blanket `git restore` or whole-file replacement would destroy semantic work. The safe direction is to snapshot the current semantic diff, selectively restore formatting from `HEAD`, then mechanically prove that the whitespace-insensitive diff is unchanged. Pure-format files can be restored wholesale; mixed files require hunk- or token-aware reconstruction.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-revert-format-only-diff | Remove only uncommitted formatting churn while retaining every semantic diff. | Modified tracked files under `vpn-frame`, `vpn-server`, and `vpn_web`; no untracked-file mutation. | Selective reconstruction is more deliberate than whole-file restore but protects user-owned semantic work. | Before/after whitespace-insensitive semantic diff is identical; pure-format files disappear from status; `git diff --check` passes. | No semantic rollback, formatter run, staging, commit, untracked cleanup, or behavior change. |

## Success Criteria
- Concrete visible result: formatting-only tracked changes are gone from `git diff`, including the two currently proven pure-format `vpn_web` files.
- Required evidence: unchanged whitespace-insensitive semantic patch before versus after cleanup, preserved PN repair files/tests, scoped status comparison, and passing `git diff --check`.
- Explicit non-goals: semantic review or rollback, untracked-file deletion, formatter execution, tests unrelated to preservation, staging, and commit.

## Risks
- A mixed Rust hunk can contain both formatting and behavior changes; hunk-level restore alone may discard required code.
- Whitespace can be semantically meaningful in Markdown or string literals, so only mechanically proven formatting differences will be reverted.
- The dirty tree predates this cleanup; baseline capture is required so completion attribution does not claim or erase existing work.
