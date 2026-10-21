# Testing Document Rules

This rule is part of the high-risk workflow. Standard tasks run proportionate targeted verification without creating Harness testing documents or `testplan.yaml` by default.

## Goal
- Define persistent testing artifacts and post-implementation testing responsibilities.

## Scope
- `.harness/tasks/<version>/modules/<project>/<task-seq>-<task-slug>/testing.md`, `testing/`, and `testplan.yaml`.
- Cross-project testing artifacts under `.harness/tasks/<version>/modules/globals/<task-seq>-<task-slug>/`.

## Metadata For Testing Documents
- `task_manifest: task.yaml`
- `status`
- Do not repeat `module`, `version`, `task_name`, or `submodule`; canonical identity comes from `task.yaml`.

## Required Content
- Reference `Risk profile: ./risk-profile.yaml`; implement the applicable profile's `required_checks` through task test metadata/evidence, and do not copy a Trigger Matrix into testing artifacts.
- Test cases reviewed, refined, and completed after implementation from proposal, design, and delivered code, following [Proposal Test Handoff](test-design-rules.md#proposal-test-handoff) when the proposal already contains a test solution.
- One authoritative `coverage` list in `testplan.yaml` records cases and gaps across submodules, module workflows, interfaces, and direct `change_id` values; the schema is owned by `test-design-rules.md`.
- Validation rationale tied to concrete behaviors, risks, and success criteria.
- Each coverage record combines case-type/design-element tags, test level, target, assertion, test location, and runnable step binding or gap reason.
- `testplan.yaml` owns commands and manual/disabled layer reasons; machine-written run artifacts own actual execution results. Reference these sources instead of copying them into Markdown.
- For breaking/migration-required public APIs, crate-root export changes, or build-surface changes: reference the design-owned file-level consumer closure and implement its risk-triggered contract checks; do not copy the consumer inventory into testing Markdown.
- Explicit gap records for missing direct validation.
- `testing.md` is optional analysis for material rationale, corrections, or environment limits not already captured in metadata/evidence. No fixed section set, split index, checklist, or length gate applies.

## Guardrails
- Testing operationalizes approved proposal/design intent against delivered implementation.
- When the task contains GitHub issue information, design and prioritize tests around the issue-described behavior, boundaries, and acceptance conditions. Process-document coverage is secondary and cannot substitute for evidence that the issue outcome works.
- Testing docs use the approval authority in `harness/rules/task-entry-gate-rules.md` and metadata schema in `harness/rules/schema-validation-rules.md`.
- Testing runs after implementation and MUST inspect proposal, design, and delivered code before reviewing, refining, or designing cases.
- By default, the testing-stage owner MUST continuously own case review/design, test implementation, metadata and unified-entrypoint registration, and required test execution. The owner may be the parent or execution agent under Task Execution Rules. Do not create separate subtasks solely for these sequential responsibilities. Further decomposition is justified only for independent test work with a concrete parallelism or scale benefit; the testing-stage owner retains integration and evidence ownership, and delegated steps follow the generated AGENTS step-to-agent mapping.
- Case depth, design-element derivation, lowest-level placement, test-file placement, and design-return conditions are owned by `harness/rules/test-design-rules.md`.
- Baseline capture is project-local generated state. `.harness/` is git-ignored local state and MUST NOT be committed, and testing tasks MUST NOT use `GIT_INDEX_FILE`, `git read-tree`, `git write-tree`, or `git commit-tree` to create synthetic Git baseline objects.
- Every implemented change MUST have direct validation coverage or an explicit gap.
- Completed testing MUST generate/update `testplan.yaml` unless a repo-local versioned exception records reason, owner, risk, and acceptance impact.
- Every implemented `change_id` MUST map to validation ids and generated tests or `testplan.yaml` steps unless the validation path is `manual` or `disabled`.
- Every implemented `change_id` MUST have case-type coverage or concrete gap/not-applicable reasons in the single coverage list.
- Every generated or changed automated test and completed task plan MUST be runnable through the task-scoped interface defined by `harness/rules/unified-test-entry-rules.md`.
- Completion requires valid `<module>/<task-name> all` evidence under [Unified Test Entry Rules](unified-test-entry-rules.md#execution-contract); reuse qualifying artifacts instead of rerunning unchanged tests for completion or acceptance. Module/project-wide runtime suites require explicit user-directed maintenance outside the task flow.
- Bugfix testing MUST show red-green regression evidence for the bugfix `change_id`, or record why pre-fix reproduction is not feasible.
- Validation paths MUST prove named behavior or risk, not unrelated checks.
- Acceptance-returned testing gaps MUST supplement test design, test implementation, metadata when used, and unified-entrypoint runnable evidence before retry.
- Manual/disabled coverage may reference the matching level reason; do not duplicate that reason in `testing.md`.
- Upstream design or implementation problems route to the owning stage instead of silently widening testing scope.
- Downstream acceptance follow-up is recorded unless cross-stage synchronization is explicitly requested.
- Return the testing artifacts and required test-run evidence to the parent. The parent runs `UV_CACHE_DIR=.harness/uv-cache uv run --active python ./harness/scripts/task-transition.py --task <packet>/task.yaml advance`, which executes completion checks (including optional `testing.md` validation), approves the testing document when present, records its receipt, and advances. Do not run a separate completion profile before this transition; follow `harness/rules/task-execution-rules.md`.
- Use `testing-coverage-check.py --allow-missing-testplan` only with a recorded repo-local versioned exception. In that case, the same coverage fields live once in a `## Coverage` table in `testing.md`; automated covered cases still require a runnable plan.

Proposal confirmation authorizes downstream execution under `harness/rules/task-execution-rules.md`. Design/testing documents become approved after their stage checks pass; no intermediate user approval is required.
