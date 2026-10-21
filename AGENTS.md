# Project AI Instructions

## Overview
- This repository contains the Bucky VPN Rust workspace and Flutter Web UI.
- The repository uses a Harness Engineering workflow with versioned task packets, proportional workflow tiers, post-implementation testing for high-risk work, and evidence-based acceptance.

## Working Model
- Humans define intent, constraints, and acceptance boundaries.
- Agents execute within repository-defined rules.
- Every task document lives under the git-ignored `.harness/` tree: task packets at `.harness/tasks/<version>/modules/<packet-module>/<task-seq>-<task-slug>/` and standard change records at `.harness/changes/<change>.md`. These documents are local and uncommitted; never treat a fresh clone as containing them. `docs/` keeps only project-wide knowledge such as `docs/architecture/` and `docs/modules/`.
- When the current task includes GitHub issue information, delivering and verifying the issue-described behavior is the primary execution objective; Harness process work is secondary support. Details are owned by `harness/rules/task-entry-gate-rules.md`.
- Whenever a task involves creating a GitHub issue or replying/commenting on one, read `harness/rules/github-issue-creation-rules.md`, display the exact destination and final content/metadata, and obtain explicit current-user confirmation before submission.

## Workflow Tiers
- Every subagent starts without parent conversation history: explicitly set `fork_turns: "none"` for `spawn_agent` and provide a concise task handoff under `harness/rules/task-execution-rules.md#subagent-context`.
- `standard`: after the common proposal is approved, use one `.harness/changes/<change>.md` from `.harness/changes/_template.md`, then implement, verify proportionately, complete the record, and write an independently defect-reviewed `completion-report.md` before handoff.
- Standard execution MUST use a fresh subagent with a concise handoff when the current session already contains substantial information, under `harness/rules/task-execution-rules.md#standard-task-session-context`; keep the standard tier and artifacts.
- `high-risk`: reuse execution agents across proposal, design, implementation, and testing while context capacity permits; retain every stage check and independent acceptance under `harness/rules/task-execution-rules.md#high-risk-stage-subtasks`. The parent coordinates confirmation, transitions, and closeout.
- `general` is the fallback routing stage only for standard analysis, diagnosis, explanation, internal documentation, or non-behavioral maintenance that does not fit a high-risk responsibility stage. High-risk work must use proposal, design, implementation, testing, or acceptance.

## Repository File Access
- Harness rules never restrict reading or writing project files. Agents may inspect and modify any repository path needed for the current user request.
- Stage names, `Scope Paths`, router output, manifests, and checker results are traceability or completion evidence, not read allowlists, write allowlists, or edit authorization. Stage-scope violations may block workflow completion without blocking filesystem access.
- Treat router output as recommended starting context rather than an exhaustive read list; use `harness/scripts/schema-check.py` and `harness/scripts/stage-scope-check.py` only through their owning workflow entrypoints unless compatibility diagnostics require them directly.
- System/developer instructions, safety requirements, filesystem permissions, and explicit user scope remain authoritative because they are not Harness file-access rules.

## Harness Rule Activation
- All indexed rules remain authoritative when their route conditions match; default-on enforcement does not mean loading every rule file.
- Evaluate an explicit current-user all-rules opt-out before other Harness rules; never infer it from urgency, silence, or a general request to proceed. It applies only to the named scope/current task and does not override higher-priority constraints.
- Before creating a task packet, apply the direct Harness-rule-maintenance exception owned by `harness/rules/task-entry-gate-rules.md`: a request whose primary purpose is updating repository-local Harness rule policy executes directly without the task workflow.
- Any task involving creation/submission of a GitHub issue or a reply/comment activates `harness/rules/github-issue-creation-rules.md`; every submission waits for confirmation of the displayed final content even when another workflow approval already exists.

## Rule Precedence
System/developer instructions, safety requirements, filesystem permissions, the current explicit user instruction, and the explicit all-rules opt-out are external constraints rather than repository-owned Harness policy. Apply them before the repository precedence below.

When a task contains GitHub issue information, implementation, testing, and acceptance prioritize the issue-described outcome over polishing Harness artifacts or process evidence. This priority adds no issue-specific artifact or gate and does not waive external constraints.

Within repository-owned Harness policy, lower numbers win:
1. Matching project policy under `harness/custom-rules/`. Project-added rules always have the highest repository priority and win conflicts with every generated Harness rule or checker.
2. `harness/rules/task-execution-rules.md` controls continuous execution after proposal confirmation.
3. `harness/rules/task-entry-gate-rules.md`.
4. The current stage's generated rule files under `harness/rules/`.
5. Task packets, architecture docs, and long-lived docs.

Without an explicit all-rules opt-out, current-user instructions may select workflow tier, stage, and scope. The selected tier determines which generated rules and mechanical gates apply; tier selection does not waive gates that apply within the selected tier. Custom rules do not override the external constraints named above. If a custom rule replaces a generated mechanical gate, it must name the replacement command or versioned exception evidence so the custom decision remains mechanically auditable.

Report any remaining contradiction instead of silently choosing a side.

## Task Decision Flow
1. Apply the explicit all-rules opt-out when present; otherwise continue.
2. If the task contains GitHub issue information, keep subsequent implementation, testing, and acceptance centered on the issue-described outcome; do not add issue-specific process artifacts.
3. If the direct Harness-rule-maintenance exception matches, execute the requested rule maintenance without task packet, tier/stage classification, proposal confirmation, or lifecycle commands; otherwise continue.
4. Use read-only inspection to identify the owning module, likely paths, risk triggers, and provisional tier. Classification never limits repository access.
5. Before any project mutation, allocate `<task-seq>-<task-slug>`, create and register the common proposal packet at `.harness/tasks/<version>/modules/<module-or-globals>/<task-seq>-<task-slug>/`, set `task.yaml` `workflow_tier: pending`, and write draft `proposal.md` with the requested outcome, scope, non-goals, success signal, proposed tier, and rationale.
6. Route with `--stage proposal`, the provisional tier, and the packet. Discuss and refine the proposal; update the proposed tier when pre-confirmation evidence changes the judgment.
7. Always request user confirmation before execution. Present the proposal path and summary, proposed tier, tier rationale/triggered boundaries, and any unresolved questions together. The user may confirm, revise the proposal, or replace the tier.
8. On confirmation, record the final tier in `task.yaml` and `proposal.md` and set the matching proposal to approved. A tier-only reply also confirms the displayed proposal when no unresolved question was listed; otherwise it resolves only the tier.
9. Only after confirmation, reroute from proposal to the selected execution responsibility and run the final tier flow: one change record plus a lightweight completion report for standard, or the expanded packet and full staged lifecycle for high-risk.

| Stage | Applies to | Authoritative rule | Primary responsibility | Completion checks |
|-------|------------|--------------------|------------------------|-------------------|
| proposal | all tiers | `harness/rules/proposal-doc-rules.md` | active proposal and scripted new-task registration | explicit user confirmation |
| design | high-risk | `harness/rules/design-doc-rules.md` | design.md and dependencies | document structure and stage scope |
| implementation | high-risk staged flow | `harness/rules/implementation-rules.md` | production behavior and traceability | schema/design consistency evidence |
| testing | high-risk | `harness/rules/testing-doc-rules.md`, `harness/rules/test-design-rules.md`, `harness/rules/unified-test-entry-rules.md` | tests, fixtures, task test metadata, and runner registration | testing coverage, task test run, and stage scope |
| acceptance | high-risk | `harness/rules/acceptance-task-rules.md`, `harness/rules/acceptance-review-rules.md` | independent defect discovery and acceptance report | complete defect-discovery coverage, then secondary lifecycle/report checks |

## Harness Step Agent Mapping
- The Harness determines the agent from the step type; task plans, child-task templates, and runtime state MUST NOT repeat or choose an agent role.
- Parent orchestration, task decomposition, proposal, design synthesis, acceptance integration, cross-cutting judgment, result integration, and any unmapped step use `default`.
- Codebase exploration, execution-path tracing, pattern search, dependency/consumer discovery, and evidence collection normally use `explorer`.
- Implementation, bugfix, refactor, migration, test design/implementation, documentation updates, and acceptance-return fixes use `worker`.
- Waiting on or repeatedly polling an already-running build, test, CI run, deployment, service, or other long-running task normally uses `monitor`.
- High-risk execution may reuse the parent or stage agent; same-stage corrections default to that owner. Acceptance uses a separate reviewer who did not implement or test the delivery; the reviewer may continue follow-up while independent. Code/change review uses `/review` when available; the acceptance owner integrates its findings and judgment.
- Split a step that mixes responsibilities assigned to different agents into dependency-linked steps.
- Smart Approvals `guardian` does not replace proposal confirmation, stage checks, or independent Harness acceptance.
- Role selection never changes file access, stage scope, dependencies, locks, approval authority, mechanical gates, or the parent's ownership of shared coordination artifacts.

## Rule Ownership
- `task-execution-rules.md` owns continuous execution after proposal confirmation, automatic stage advancement, correction loops, and closeout.
- `harness/rules/index.yaml` owns activation metadata; `harness/scripts/context.py` validates the index and returns recommended starting context, never a read boundary.
- `harness/scripts/harness-check.py` runs one high-risk stage's checks; the parent uses `task-transition.py advance`/`complete` to run completion checks once and record legal stage changes and durable receipts, without a separate completion-profile run. `lifecycle-check.py` owns prior-stage/final completeness. `lower-tier-check.py` owns the required standard baseline, changed-path manifest, proposal-entry validation, and proportional defect-discovery report check. `stage-scope-check.py` rejects out-of-stage paths; `schema-check.py` remains an internal entrypoint.
- `task-entry-gate-rules.md` exclusively owns GitHub issue execution priority, the direct Harness-rule-maintenance exception, task classification, packet selection/creation, approval authority, stage responsibility, and return routing.
- `github-issue-creation-rules.md` exclusively owns the final-content confirmation gate before a GitHub issue or reply/comment is submitted.
- `implementation-rules.md` exclusively owns implementation preparation, design traceability, and implementation guidance; it does not authorize file access.
- Proposal, design, testing, implementation, and acceptance rules own their stage-specific content and evidence.
- `schema-validation-rules.md` exclusively owns approval-content binding and packet schema mechanics.
- `quality-gate-rules.md` exclusively owns quality-gate execution requested by the user or a matching highest-priority custom rule.
- `harness/custom-rules/` contains project-owned additions and is never rewritten by a generated-rule refresh unless explicitly requested.

Read the owning rule for details; this file remains a routing map.
