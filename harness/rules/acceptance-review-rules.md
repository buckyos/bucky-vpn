# Acceptance Review Gate

This standalone review contract applies to high-risk task packets. Trivial and standard tasks do not generate an acceptance report unless the user explicitly requests one.

## Goal
- Decide whether the requirement is sound and satisfied by the delivered implementation.
- Find implementation defects that prevent the requested behavior from being accepted.
- When design or testing documents exist, verify that the implementation is consistent with them.

## Required Review
- `proposal.md` is the mandatory requirement baseline; an acceptance report MUST NOT conclude `accepted` when it is missing.
- Requirement review: inspect `proposal.md` for ambiguity, contradiction, incorrect boundaries, and missing implemented behavior.
- Implementation review: inspect the delivered production implementation for defects relevant to the requested behavior.
- Design consistency, only when `design.md` or auto-pipeline `pipeline/plan.md` exists: compare implementation with the documented solution shape.
- Testing-document consistency, only when `testing.md` or `testplan.yaml` exists: compare implementation with the documented testing intent.

## Excluded By Default
- Do not generate acceptance rules or expected-result documents.
- Do not require `acceptance.md`.
- Do not require schema, risk-profile, stage-scope, pipeline-plan, test-run, quality-gate, or other command artifacts merely to complete acceptance.
- Do not require a fixed design-quality checklist, test-design-adequacy table, or category-by-category correctness table.
- Test results may be consulted as implementation evidence, but passing or missing tests do not replace review of the requirement and implementation and are not a mandatory acceptance-report section.

## Decision And Routing
- A requirement ambiguity, contradiction, or incorrect boundary stops acceptance and asks the user to decide.
- Missing required behavior or an implementation defect returns to implementation unless the requirement itself is the problem.
- If implementation satisfies the requirement but conflicts with an existing design or testing document, report the inconsistency and return the stale or incorrect document to its owning stage.
- Acceptance does not repair requirements, design/testing documents, or implementation in the same task.

## Report
- Put findings first and classify each as `requirement`, `implementation`, `design-consistency`, or `testing-consistency`.
- Record requirement review, implementation review, conditional document consistency, a plain-language result, and the next action.
- `acceptance-report-check.py` validates only this minimal structure and conclusion consistency.
