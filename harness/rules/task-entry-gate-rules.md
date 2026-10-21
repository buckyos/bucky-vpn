# Task Entry Gate Rules

## Goal
Apply after the explicit all-Harness-rules opt-out in `AGENTS.md`, before stage rules. Select the lightest flow supported by task risk.

## GitHub Issue Execution Priority
- Read available issue descriptions, acceptance criteria/checklists, and requirement-defining comments before implementation decisions.
- Issue-described behavior and acceptance conditions are the primary delivery objective. Plans, documents, checkers, existing code, and agent summaries support it; they MUST NOT narrow or replace it.
- Anchor implementation, debugging, tests, and completion review to that outcome. Prefer closing or validating an issue requirement over process-only improvements.
- Issue information alone adds no field, artifact, checker, stage, or tier upgrade. Use the normal risk-based flow.
- Surface unavailable content or material requirement conflicts instead of guessing. System/developer instructions, safety, filesystem permissions, and explicit user resolution remain authoritative.

## Project Custom-Rule Precedence
- Evaluate every matching indexed `harness/custom-rules/` rule before generated rules/checkers. Matching custom rules win repository-owned Harness policy conflicts.
- A replacement mechanical gate must name its command or versioned exception evidence.
- This precedence does not override system/developer instructions, safety, filesystem permissions, explicit current-user instructions, or the all-Harness-rules opt-out.

## Direct Harness Rule Maintenance
- Directly inspect, edit, and verify requests primarily maintaining repository-local `harness/rules/` or `harness/custom-rules/` policy, including their indexes.
- Apply this exception before proposal responsibility: no packet creation/selection, task-index mutation, tier/stage classification, proposal confirmation, downstream artifacts, or lifecycle commands for this scope.
- Mixed product, runtime, build, test, or non-rule tooling changes use normal task workflow for that additional scope. External instructions, explicit user scope, and non-Harness repository constraints still apply.
- Non-rule Harness tooling uses implementation responsibility under [Implementation Rules](implementation-rules.md#harness-tooling-ownership), with testing and independent acceptance retained for high-risk work.

## Mandatory Proposal Responsibility
- Every governed task creates the same proposal packet before execution: allocate `<task-seq>-<task-slug>`, use the canonical single-project or `globals` packet path, create `task.yaml` plus draft `proposal.md`, and register `task.yaml` in the version's unfinished-task index.
- Before confirmation, `task.yaml` uses `workflow_tier: pending`. Draft `proposal.md` records the requested outcome, in-scope behavior, out-of-scope behavior or non-goals, success signal, material assumptions or tradeoffs, proposed tier, and concrete rationale or triggered boundaries.
- Proposal confirmation is mandatory for every tier. Before confirmation, only read-only project inspection and proposal-packet/index maintenance are allowed; do not modify project files or start a tier's execution flow.
- The confirmation request MUST show the proposal path, requested outcome, scope, non-goals, success criteria, proposed tier, rationale/triggered boundaries, and every unresolved question. Explain that confirmation authorizes the entire task; the user may confirm, replace the tier, or revise the proposal.
- On confirmation, write the user-selected tier to `task.yaml` and `proposal.md`, record the confirmation, and set the matching proposal to `status: approved`. For `standard`, also set `baseline_manifest` to `.harness/baselines/<version>/<task-name>-delivery/manifest.json`; leave it blank for high-risk unless the active stage requires selected-file comparison evidence. The selected tier becomes final for generated routing and artifacts, subject only to system/developer instructions, safety requirements, or non-Harness repository constraints. Surface known residual risk when the user selects a lower tier.
- A tier-only reply confirms the displayed proposal and selects that tier only when the confirmation request listed no unresolved proposal question. Otherwise it resolves only the tier; do not infer answers to requirement, scope, tradeoff, or acceptance questions.

## Workflow Tier Classification
- Classify risk before selecting a stage or creating durable artifacts. Use `standard` or `high-risk`.
- `high-risk` is mandatory by default when evidence confirms material impact to contracts/protocols, data/migrations, security/privacy, runtime/concurrency, build/supply-chain behavior, deployment/rollback, UI/accessibility, Harness-process behavior, cross-project integration, architecture, or materially ambiguous scope/acceptance. Explicit requests for full staged artifacts also select high-risk.
- `standard` is the default for bounded single-project feature, bugfix, or refactor work that has no high-risk trigger.
- Documentation-only or configuration-only work remains `standard` by default when it changes no governed intent, runtime behavior, public contract, data, security boundary, dependency/build graph, supply-chain trust, produced artifact, production default/rollout, release/deployment surface, compatibility, or rollback requirement. Classify by consequence, not file type.
- Task size alone never downgrades risk. Before proposal confirmation, start at the lowest tier supported by evidence and automatically revise the proposed tier when investigation reveals a higher-risk condition; the single mandatory proposal confirmation settles the final tier.
- An explicit current-user tier selection wins over the generated default unless it conflicts with system/developer instructions, safety requirements, or a non-Harness repository constraint. Selecting a tier changes the generated rules and mechanical gates that apply; it does not waive gates within the selected tier. Surface known risk when following an explicit downgrade.

## Tier Default Flows
All tiers retain the approved common `task.yaml` and `proposal.md` packet. Issue-driven work follows [GitHub Issue Execution Priority](#github-issue-execution-priority).

For `standard`:
1. Before project mutation, run `lower-tier-check.py --profile pre-edit` to validate approval and capture the task-start working-tree baseline.
2. Create one `.harness/changes/<change>.md` from `.harness/changes/_template.md`, linking the task/proposal and recording approach, compact risk screen, implementation evidence, and verification.
3. Implement and verify continuously using the narrowest useful repository-native check that can expose changed-behavior defects; mark the change record complete.
4. Independently search current delivery for defects in behavior/logic, boundaries/failure paths, and regression/side effects. Do not adopt implementation self-assessment. Record proposal/delivery consistency, coverage, and passing targeted verification in task-local `completion-report.md`.
5. Run `lower-tier-check.py --profile completion`; it generates the canonical changed-path manifest from the baseline. Removal requires the complete bound change record and accepted report.

This proportional close gate adds no risk profile, design/testing documents or receipts, testplan, or full acceptance report. Use `needs changes` for in-scope corrections, `rejected` for user decisions/abandonment/re-scope, and `accepted` only after defect review passes. Tier changes follow [Automatic Upgrade](#automatic-upgrade).

## Automatic Upgrade
- Before initial confirmation, a confirmed higher-risk condition changes the proposed tier immediately. Update `proposal.md` and rerun proposal routing; do not ask for a separate upgrade decision because the mandatory proposal/tier confirmation will settle it.
- After confirmation, the user-selected tier remains authoritative. Newly requested scope or a requirement change returns the task to proposal, sets the proposal to draft, recomputes the recommendation, and requires confirmation again before further project mutation. Newly discovered risk within the confirmed scope is surfaced and recorded; it does not silently replace the user's tier.
- If a reconfirmed tier changes from `standard` to `high-risk`, add the risk profile and missing full-lifecycle artifacts to the existing packet and continue from the earliest responsible stage. Never allocate a second task name or packet for the same confirmed requirement revision.
- A lower-tier `general` route never carries into high-risk. On a user-confirmed high-risk transition, reclassify it to proposal, design, implementation, testing, or acceptance and reroute before further project edits.

## Task Classification
- Use `general` only for standard analysis, diagnosis, explanation, internal documentation, or non-behavioral maintenance that does not fit another responsibility stage. It is a routing fallback, not a high-risk lifecycle stage.
- In high-risk work, a request that changes goals, scope, non-goals, supported/unsupported behavior, acceptance boundaries, or success evidence is proposal work. Lower tiers record the decision in their handoff or single change record.
- A production-code, bugfix, optimization, refactor, runtime, UI-behavior, or build-behavior change is implementation-shaped. High-risk work uses `harness/rules/implementation-rules.md`; lower tiers use their tier default flow.
- Harness governs workflow responsibility and evidence, never repository permissions. Read or write any project file needed for the current user request; no task packet, stage, `Scope Paths`, changed-path manifest, router result, or checker result grants or revokes that access. A failed workflow check blocks completion or routes follow-up, not file access.
- Never ask the user to skip Harness rules to inspect or modify repository files.
- Ambiguity that affects behavior, scope, risk, module, task packet, or `change_id` returns to the owning upstream stage instead of being guessed.

## Task Packet Selection
- All governed tasks use the common proposal entry above and `<task-seq>-<task-slug>` naming; only confirmed high-risk tasks add a risk profile and downstream stage artifacts.
- Every task document lives under the git-ignored `.harness/` tree: packets and `task.yaml`/`proposal.md`/`design.md`/`testing.md`/`testplan.yaml`/`risk-profile.yaml`/`completion-report.md`/`acceptance-report.md`/`lifecycle.json` at `.harness/tasks/<version>/modules/<packet-module>/<task-seq>-<task-slug>/`, and standard change records at `.harness/changes/<change>.md`. They are local and uncommitted, so never require a cloned repository to contain them. `docs/` keeps only project-wide knowledge such as `docs/architecture/` and `docs/modules/`; never create a task document under `docs/`.
- Single-project packets live at `.harness/tasks/<version>/modules/<project>/<task-seq>-<task-slug>/`.
- Cross-project packets live at `.harness/tasks/<version>/modules/globals/<task-seq>-<task-slug>/`; `globals` is a packet-module keyword, never a production target.
- Maintain machine-owned `.harness/tasks/<version>/tasks.json` only through `harness/scripts/task-index.py`: run `init --version <version>` to create it, `add --task <packet>/task.yaml` after packet creation, `list --version <version> --module <module>` for selection, `remove --task <packet>/task.yaml` only after successful completion, and `validate --version <version>` for integrity. Never edit or parse the JSON by hand; other scripts must reuse `task-index.py` helpers.
- For high-risk work, use `task-transition.py return --to proposal` before a requirement revision; it resets approval/tier and invalidates downstream receipts. Reconfirmation and other return paths follow [Task Execution Rules](task-execution-rules.md). Never edit `task.yaml.stage` directly. Use `task-transition.py advance` after each stage and `task-transition.py complete` in acceptance; it records content-bound completion receipts in task-packet `lifecycle.json`. Final `task-index.py remove` fails unless the full receipt chain is valid.
- Select an existing packet only from an explicit current-user reference or a module's Current/Active Task field. Never infer it from sequence, directory order, timestamp, old code, chat history, or a broad module overview.
- A clearly different-module request gets a new packet. If module-filtered `task-index.py list` output contains multiple unfinished packets that could apply and the user did not identify one, ask which packet to use; clearly new work still gets a sibling packet.
- An approved packet document is frozen for the confirmed requirement. Clearly new requirements use a sibling packet; revisions to the current unfinished task return its proposal to draft, record the reason, and require confirmation again.

## Approval Authority
- Proposal confirmation, including tier-only replies, follows [Mandatory Proposal Responsibility](#mandatory-proposal-responsibility). Silence and inferred intent are not approval.
- Downstream high-risk document approval follows [Task Execution Rules](task-execution-rules.md).
- `schema-check.py` checks status values only, not approver identity, conversation provenance, timestamps, or approval records.

## Stage Ownership
- High-risk execution may reuse the parent or stage agent while context capacity permits, with every stage check and independent acceptance retained under [Task Execution Rules](task-execution-rules.md#high-risk-stage-subtasks). Standard tasks combine their responsibilities in one continuous flow, with mandatory fresh-subagent execution when the current session has substantial information under [Standard Task Session Context](task-execution-rules.md#standard-task-session-context).
- A stage names the task's primary responsibility and expected evidence. Completion fails when the task's changed-path manifest contains artifacts owned by another stage.
- If work needs artifacts owned by another stage, return or split the work before completion and record the synchronization so the artifact chain remains understandable.
- Proposal owns requirements, acceptance boundaries, and scripted new-task registration. Design owns implementation shape and Scope Paths. Implementation owns design-bound production changes. Testing owns test design/implementation and task test evidence. Acceptance owns independent defect discovery, requirement/design/implementation/testing findings, secondary document consistency, reporting, and scripted removal of a successfully completed task from `tasks.json`.
- For high-risk tasks, the parent executes shared registration, lifecycle, and closeout commands on behalf of the stage owners under Task Execution Rules.
- Detailed content and completion guidance live in the current stage's authoritative rule files listed in `AGENTS.md`.
- After provisional tier and stage classification, run `harness/scripts/context.py` with both values and the common packet. Proposal-document rules apply to every tier; downstream high-risk stage rules remain excluded from lower-tier routes while matching risk triggers can still inform the pre-confirmation recommendation. Router output is never an exhaustive read list; inspect any additional repository files needed. Loading the quality-gate rule does not execute its gates.

## Path Evidence
- Provide changed-path manifests where the selected tier/stage requires them. `stage-scope-check.py` returns non-zero for paths outside the active stage's artifact group, never solely for paths outside design `Scope Paths` (planned-impact metadata).

## Implementation Transition
- This section applies only to high-risk implementation. Standard implementation begins after the preparation above.
- Record explicit `version`, `packet_module`, `task_name`, implementation targets, and concrete `change_id` values in `task.yaml` when maintaining task traceability.
- Apply `harness/rules/implementation-rules.md` for preparation and design mapping.

## Return Routing
- Missing packet, missing proposal coverage, or a requirement/acceptance-boundary defect: proposal.
- Missing task-test coverage, weak defect-detection coverage, or non-runnable task evidence: testing.
- Implementation defect against adequate governing sources: implementation.
- Proposal ambiguity or contradiction found during acceptance: finish the canonical report with a blocking requirement finding and `rejected`, then stop and ask the user; do not auto-return it.

## Post-Confirmation Execution
- Continue the entire confirmed tier under [Task Execution Rules](task-execution-rules.md), without another mode or stage-confirmation prompt.
- High-risk expands the existing packet with `risk-profile.yaml`, `design.md`, `testing.md`, `testplan.yaml`, `acceptance-report.md`, and `lifecycle.json`.
- A common proposal packet or automatic completion does not select high-risk; material impact and the user-confirmed tier determine the flow.
