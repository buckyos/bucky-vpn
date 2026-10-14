# Proposal Document Rules

## Goal
- Keep `proposal.md` as a human-readable requirement baseline.
- Proposal answers why, what, the boundary, the tradeoff, the success criteria, and approval state.

## Scope
- This rule governs the mandatory formal `proposal.md` created for every tier before user confirmation.
- Single-project proposal: `.harness/tasks/<version>/modules/<project>/<task-seq>-<task-slug>/proposal.md`.
- Cross-project proposal: `.harness/tasks/<version>/modules/globals/<task-seq>-<task-slug>/proposal.md`.

## Required Metadata
- `task_manifest: task.yaml`
- `status`
- Do not repeat `module`, `version`, `task_name`, or `submodule`; canonical identity comes from `task.yaml`.

## Required Content
- `## Workflow Tier Judgment` recording the proposed tier, final user-confirmed tier (`pending` before confirmation), concrete rationale or triggered boundaries, and the proposal/tier confirmation statement.
- `## Background and Goal` describing the problem and intended outcome.
- `## Scope` describing in-scope behavior, out-of-scope behavior, and neighboring boundaries.
- `## Requirement Review` recording whether the request is reasonable, material risks/tradeoffs, and the chosen direction.
- `## Proposal Items` with stable `proposal_id` and `change_id` values for each implementation-ready requirement.
- `## Success Criteria` describing the visible result, required evidence, and explicit non-goals.
- `## Risks` for material requirement, boundary, security, migration, or shared-contract risk.

## Design and Testing Discussion
- When discussing design, read and apply [Design Document Rules — Proposal Applicability](design-doc-rules.md#proposal-applicability). When discussing testing, read and apply [Test Design Rules — Proposal Applicability](test-design-rules.md#proposal-applicability). These topic-specific references apply even when the stage router does not select the downstream rule file.
- Keep the relevant discussion in `## Requirement Review`, `## Proposal Items`, `## Success Criteria`, or `## Risks`; add a focused subsection only when useful. Distinguish candidate approaches and unresolved assumptions from chosen requirement constraints and acceptance obligations.
- Discussion depth follows the decision being made. It does not require a complete design, exhaustive cases, downstream artifacts, test implementation, or execution evidence before proposal approval, and does not by itself change the workflow tier or stage.
- After approval, downstream work carries forward agreed constraints and success evidence; tentative approaches remain revisable within that baseline. Requirement revisions follow `harness/rules/task-entry-gate-rules.md`.
- When the proposal already contains a solution, downstream work starts by reviewing and completing it: follow [Proposal Design Handoff](design-doc-rules.md#proposal-design-handoff) for design and [Proposal Test Handoff](test-design-rules.md#proposal-test-handoff) for testing.

## Guardrails
- Proposal is the requirement baseline for later work.
- Proposal-stage work MUST discuss the problem with the user, evaluate whether requirements are reasonable, surface risks/tradeoffs, and propose a better approach when it better satisfies the goal.
- Packet creation/selection, unfinished-index mutation, confirmation requests, tier selection, approval transitions, and requirement revisions are owned by `harness/rules/task-entry-gate-rules.md`; this rule defines proposal metadata and content only.
- If the final tier is `high-risk`, create `risk-profile.yaml`, replace the proposal's risk-profile placeholder with `Risk profile: ./risk-profile.yaml`, and complete stable `change_id` plus risk evidence before downstream design. Lower tiers do not create a risk profile, but retain stable proposal-item `change_id` values so lightweight completion can compare delivery with the approved requirements.
- Every implementation-ready requirement must have a stable `change_id`; high-risk carries it through design/testing lifecycle mappings, while lower tiers carry it directly into `completion-report.md`.
- After user confirmation and completion of the high-risk proposal artifacts, return the artifacts and evidence to the parent. The parent runs `UV_CACHE_DIR=.harness/uv-cache uv run --active python ./harness/scripts/task-transition.py --task <packet>/task.yaml advance`, which executes completion checks once, records the proposal receipt, and advances to design. Do not run a separate completion profile before this transition; follow `harness/rules/task-execution-rules.md`. Lower tiers proceed directly to their confirmed default flow.
