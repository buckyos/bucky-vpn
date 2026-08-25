---
task_manifest: task.yaml
status: approved
---

# Auto-pipeline Proposal Transition Gate Proposal

Risk profile: not-created (replace with ./risk-profile.yaml only after high-risk confirmation)

## Workflow Tier Judgment
- Proposed tier: standard
- Final tier: standard
- Tier rationale / triggered boundaries: The defect affects Harness process behavior, so the repository default is high-risk. The user explicitly selected `standard` on 2026-07-20. Residual risk is controlled with a focused regression, module test-entry registration, documentation synchronization, and direct verification against the blocked real transition; the standard tier remains a user-authorized downgrade.
- Proposal and tier confirmation: confirmed by the user on 2026-07-20 after the required validation-registration and governance-document scope was displayed

## Background and Goal
The confirmed `002-align-proxy-node-api` pipeline launches from proposal with design as its first automatic stage. Its proposal completion is currently impossible: `harness-check.py` always invokes `schema-check.py --require-approved` for proposal completion, while `schema-check.py` correctly reserves that flag for manual proposal/design documents when automatic design is active. `task-transition.py` consequently cannot record the proposal receipt or advance the task.

The goal is to make the canonical proposal transition select schema validation consistent with the documented auto-pipeline approval model, without weakening manual proposal approval enforcement.

## Scope
### In scope
- Change proposal-stage command construction in `harness-check.py` so a proposal-launched pipeline with automatic design uses ordinary schema validation plus its existing explicit-launch binding checks.
- Keep `--require-approved` for manual-flow proposal completion.
- Add a dedicated regression test proving both command-selection branches and the automatic-design case that currently fails.
- Register the dedicated regression in the existing `repo-governance unit` module suite while preserving all pre-existing dirty `test-run.py` content.
- Synchronize `docs/architecture/repository-workflow.md` with the corrected proposal-to-automatic-design transition behavior.
- Run the focused governance regression through the repository test entrypoint.

### Out of scope
- Changing `schema-check.py`, `task-transition.py`, lifecycle receipt binding, pipeline-plan validation, or user approval policy.
- Modifying the existing untracked `harness/tests/test_schema_check_proposal_approval.py`, `schema-check.py`, or other files owned by the earlier governance task; the already-dirty `test-run.py` is touched only to append this task's independent regression entry after a task baseline captures its pre-existing content.
- Broad Harness refactoring or changes to non-proposal stages.
- Modifying `vpn_web`; the blocked `002` task resumes only after this prerequisite completes.

### Boundary with neighboring modules
The repair is confined to `repo-governance`. `vpn_web` is only the blocked consumer demonstrating the defect and remains unchanged until its own already-approved pipeline resumes.

## Requirement Review
The scope expansion requested by the user is reasonable, but folding it directly into the existing single-module `vpn_web` packet would violate canonical packet/target-module bindings. A separate prerequisite governance task keeps ownership and lifecycle evidence valid. The narrowest correct behavior change belongs in `harness-check.py`: it already knows whether design is automatic and can choose the ordinary schema command only for the launch-confirmed automatic-design branch. Removing the rejection from `schema-check.py` would blur its documented manual-approval contract. The project custom rule additionally requires the small test-entry and workflow-document synchronization included here.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-auto-pipeline-proposal-transition | Make proposal completion choose ordinary schema validation for launch-confirmed automatic-design pipelines while retaining approval-enforcing schema validation for manual proposal flows. | `harness-check.py`, one dedicated regression, its `repo-governance unit` registration, and the required workflow-document synchronization. | Adds a small conditional and two traceability updates; explicit launch binding, proposal structure, plan, and scope checks remain mandatory. | Regression proves automatic-design proposal commands omit `--require-approved`, manual proposal commands retain it, the module entry runs it, and the blocked `002` transition succeeds after the fix. | No schema policy weakening, lifecycle redesign, or unrelated Harness cleanup. |

## Success Criteria
- Concrete user-visible or system-visible result: `task-transition.py` can legally advance `002-align-proxy-node-api` from proposal to its first automatic design stage.
- Required evidence: focused regression passes through `repo-governance unit`; manual proposal completion still constructs `schema-check.py --require-approved`; automatic-design proposal completion constructs schema validation without that flag; the real blocked transition completes; workflow documentation describes the boundary.
- Explicit non-goals: changing schema approval semantics, bypassing pipeline launch evidence, and modifying existing unrelated dirty files.

## Risks
- An overly broad conditional could weaken manual proposal approval; regression coverage must assert the manual branch explicitly.
- Treating every auto-pipeline proposal identically without confirming automatic design could affect pipelines launched at later stages; the condition must use the existing design-stage policy helper.
- Existing dirty governance files belong to another task and must remain semantically untouched; the standard-flow baseline must separate this task's appended test-run entry from prior content.
