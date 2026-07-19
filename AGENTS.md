# Project AI Instructions

## Overview
- This repository contains the Bucky VPN Rust workspace and Flutter Web UI.
- The repository uses a Harness Engineering workflow with versioned task packets, proportional workflow tiers, post-implementation testing for high-risk work, and evidence-based acceptance.

## Working Model
- Humans define intent, constraints, and acceptance boundaries.
- Agents execute within repository-defined rules.

## Workflow Tiers
- `trivial`: after the common proposal is approved, run inspect -> edit -> targeted verification, then write one task-local `completion-report.md` that reviews the current delivery against the approved proposal with the same implementation-review semantics as high-risk acceptance.
- `standard`: after the common proposal is approved, use one `docs/changes/<change>.md` from `docs/changes/_template.md`, then implement, verify proportionately, complete the record, and write the same implementation-reviewed `completion-report.md` before handoff.
- `high-risk`: after the common proposal is approved, add the risk profile and full design, implementation, testing, lifecycle-check, and acceptance flow. File type, path, or documentation/configuration alone never makes work high-risk; classification follows confirmed material consequences.
- `general` is the fallback routing stage only for trivial/standard analysis, diagnosis, explanation, internal documentation, or non-behavioral maintenance that does not fit a high-risk responsibility stage. High-risk work must use proposal, design, implementation, testing, or acceptance.

## Repository File Access
- Harness rules never restrict reading or writing project files. Agents may inspect and modify any repository path needed for the current user request.
- Stage names, `Scope Paths`, router output, manifests, and checker results are traceability or completion evidence, not read allowlists, write allowlists, or edit authorization. Stage-scope violations may block workflow completion without blocking filesystem access.
- Treat router output as recommended starting context rather than an exhaustive read list; use `harness/scripts/schema-check.py` and `harness/scripts/stage-scope-check.py` only through their owning workflow entrypoints unless compatibility diagnostics require them directly.
- System/developer instructions, safety requirements, filesystem permissions, and explicit user scope remain authoritative because they are not Harness file-access rules.

## Harness Rule Activation
- All indexed rules remain authoritative when their route conditions match; default-on enforcement does not mean loading every rule file.
- Evaluate an explicit current-user all-rules opt-out before other Harness rules; never infer it from urgency, silence, or a general request to proceed. It applies only to the named scope/current task and does not override higher-priority constraints.

## Rule Precedence
When rules conflict, lower numbers win:
1. Explicit current-user instruction, including the explicit all-rules opt-out above. Without that opt-out, current-user instructions may select workflow tier, stage, mode, and scope. The selected tier determines which generated rules and mechanical gates apply; tier selection does not waive gates that apply within the selected tier.
2. Matching project policy under `harness/custom-rules/`. A matching custom rule is the highest-priority repository-owned Harness rule and wins conflicts with generated Harness rules or checkers.
3. During an explicitly launched auto-pipeline only, `harness/rules/auto-pipeline-rules.md` controls the first automatic stage and selects manual documents or pipeline/state artifacts per stage.
4. `harness/rules/task-entry-gate-rules.md`.
5. The current stage's generated rule files under `harness/rules/`.
6. Task packets, architecture docs, and long-lived docs.

Custom rules do not override system/developer instructions, safety requirements, or the current explicit user instruction. If a custom rule replaces a generated mechanical gate, it must name the replacement command or versioned exception evidence so the custom decision remains mechanically auditable.

Report any remaining contradiction instead of silently choosing a side.

## Task Decision Flow
1. Apply the explicit all-rules opt-out when present; otherwise continue.
2. Use read-only inspection to identify the owning module, likely paths, risk triggers, and provisional tier. Classification never limits repository access.
3. Before any project mutation, allocate `<task-seq>-<task-slug>`, create and register the common proposal packet at the canonical module or `globals` packet location, set `task.yaml` `workflow_tier: pending`, and write draft `proposal.md` with the requested outcome, scope, non-goals, success signal, proposed tier, and rationale.
4. Route with `--stage proposal`, the provisional tier, and the packet. Discuss and refine the proposal; update the proposed tier when pre-confirmation evidence changes the judgment.
5. Always request user confirmation before execution. Present the proposal path and summary, proposed tier, tier rationale/triggered boundaries, and any unresolved questions together. The user may confirm, revise the proposal, or replace the tier.
6. On confirmation, record the final tier in `task.yaml` and `proposal.md` and set the matching proposal to approved. A tier-only reply also confirms the displayed proposal when no unresolved question was listed; otherwise it resolves only the tier.
7. Only after confirmation, reroute from proposal to the selected execution responsibility and run the final tier flow: one lightweight completion report for trivial, one change record plus that report for standard, or the expanded packet and full staged lifecycle for high-risk.

| Stage | Applies to | Authoritative rule | Primary responsibility | Completion checks |
|-------|------------|--------------------|------------------------|-------------------|
| proposal | all tiers | `harness/rules/proposal-doc-rules.md` | active proposal and scripted new-task registration | explicit user confirmation |
| design | high-risk | `harness/rules/design-doc-rules.md` | active manual design artifacts, or auto-pipeline plan mappings | document/plan structure and stage scope |
| implementation | high-risk staged flow | `harness/rules/implementation-rules.md` | production behavior and traceability | schema/design consistency evidence |
| testing | high-risk | `harness/rules/testing-doc-rules.md`, `harness/rules/test-design-rules.md`, `harness/rules/unified-test-entry-rules.md` | tests, fixtures, task test metadata, and runner registration | testing coverage, task test run, and stage scope |
| acceptance | high-risk | `harness/rules/acceptance-task-rules.md`, `harness/rules/acceptance-review-rules.md` | acceptance report, runtime status, and scripted completed-task removal | prior-stage lifecycle receipts plus acceptance report check |

## Harness Step Agent Mapping
- The Harness determines the agent from the step type; task plans, child-task templates, and runtime state MUST NOT repeat or choose an agent role.
- Parent orchestration, task decomposition, proposal, design synthesis, acceptance ownership, cross-cutting judgment, result integration, and any unmapped step use `default`.
- Codebase exploration, execution-path tracing, pattern search, dependency/consumer discovery, and evidence collection normally use `explorer`.
- Implementation, bugfix, refactor, migration, test design/implementation, documentation updates, and acceptance-return fixes use `worker`.
- Waiting on or repeatedly polling an already-running build, test, CI run, deployment, service, or other long-running task normally uses `monitor`.
- Code and change review steps use `/review` when available. `/review` is a workflow rather than an agent role; the `default` acceptance owner integrates its findings and owns the final judgment.
- Split a step that mixes responsibilities assigned to different agents into dependency-linked steps.
- Smart Approvals `guardian` is an approval-review mechanism, not a step agent; it does not replace user approval, auto-pipeline launch evidence, schema/stage checks, or Harness acceptance.
- Role selection never changes file access, stage scope, dependencies, locks, approval authority, mechanical gates, or the parent's ownership of shared coordination artifacts.

## Rule Ownership
- `harness/rules/index.yaml` owns activation metadata; `harness/scripts/context.py` validates the index and returns recommended starting context, never a read boundary.
- `harness/scripts/harness-check.py` runs one high-risk stage's checks; `task-transition.py` owns legal manual stage changes and durable receipts, while `lifecycle-check.py` owns prior-stage/final completeness. `lower-tier-check.py` owns the required trivial/standard baseline, changed-path manifest, proposal-entry validation, and lightweight implementation review. `stage-scope-check.py` rejects out-of-stage paths; `schema-check.py` remains an internal entrypoint.
- `task-entry-gate-rules.md` exclusively owns task classification, packet selection/creation, approval authority, stage responsibility, and return routing.
- `implementation-rules.md` exclusively owns implementation preparation, design traceability, and implementation guidance; it does not authorize file access.
- Proposal, design, testing, implementation, and acceptance rules own their stage-specific content and evidence.
- `schema-validation-rules.md` exclusively owns approval-content binding and packet schema mechanics.
- `quality-gate-rules.md` exclusively owns quality-gate execution requested by the user or a matching highest-priority custom rule.
- `auto-pipeline-rules.md` exclusively owns explicit launch, scheduling, runtime state, concurrency, and loop behavior for auto-pipeline mode.
- `harness/custom-rules/` contains project-owned additions and is never rewritten by a generated-rule refresh unless explicitly requested.

Read the owning rule for details; this file remains a routing map.
