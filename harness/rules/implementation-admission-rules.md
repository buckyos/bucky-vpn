# Implementation Admission Rules

Applies to high-risk implementation and bugfix preparation. Standard tasks use [Task Entry Gate Rules](task-entry-gate-rules.md#tier-default-flows).

## Preparation Responsibility
- [Implementation Rules](implementation-rules.md#required-inputs) owns preparation, approved-source review, direct `change_id`/target-module mappings, and implementation guidance. This compatibility entry adds no separate admission gate or mandatory evidence artifact.
- Select the canonical packet under [Task Entry Gate Rules](task-entry-gate-rules.md#task-packet-selection): `.harness/tasks/<version>/modules/<packet-module>/<task-seq>-<task-slug>/`. Cross-project intent uses `globals` and binds each affected target module independently.
- Follow [Implementation Entry Checks](implementation-rules.md#entry-checks) for the canonical `harness-check.py --task <packet>/task.yaml --profile pre-edit` interface. Document status checks follow [Approval Authority](task-entry-gate-rules.md#approval-authority).

## File Access and Evidence
- Harness never restricts repository reads or writes. `Scope Paths`, stage names, manifests, and checker results describe workflow evidence rather than file access authorization.
- The retired edit guard and admission stamps are not prerequisites for editing project files. Optional retained admission evidence stays under `.harness/evidence/<version>/admission/`; it does not replace current task preparation or lifecycle checks.
- Approval status alone does not establish requirement coverage. Review current governing sources; historical packets, module overviews, and chat context do not replace direct task coverage.

## Return and Completion
- Missing governing sources, direct coverage, task bindings, or an upstream contradiction follow [Implementation Return Routing](implementation-rules.md#return-routing).
- Stage ownership checks may block completion or require a return; they do not grant or revoke filesystem access.
- Follow [Task Execution Rules](task-execution-rules.md#high-risk-stage-subtasks): the stage subagent returns artifacts and evidence; the parent runs the lifecycle transition that owns completion checks and receipts.
