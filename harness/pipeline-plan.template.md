# Pipeline Plan

## Trigger
- Approved proposal:
- User launch confirmed:
- Per-stage user confirmation: required / skipped by explicit user auto-pipeline authorization
- Auto-confirm completed document stages: yes / no
- Version:
- Module(s):
- change_id values:

## Acceptance Baseline
- Final acceptance is judged against:
  - `proposal.md`

## Stage Graph
| Task ID | Stage | Status | Responsibility | Scope | Parent Task | Depends On | Output | Done Condition |
|---------|-------|--------|----------------|-------|-------------|------------|--------|----------------|
| D-1 | design | pending / confirmed | convert approved intent into executable structure | module overview | root | proposal approved | `design.md` | design complete |
| I-1 | implementation | pending / confirmed | deliver production code inside approved boundaries | module overview | root | proposal approved, design approved, schema-check passed, admission-check passed, stage-scope-check passed | production code | implementation complete |
| T-1 | testing | pending / confirmed | design test cases from proposal/design/code, generate test implementation, and wire tests into unified entrypoint | module overview | root | implementation complete | tests + test-run wiring + optional testing metadata | testing implementation reachable through test-run |
| A-1 | acceptance | pending / confirmed | generate acceptance rules and expected results, audit the evidence chain, and judge proposal satisfaction | module final | root | testing implementation complete | acceptance report | acceptance passed |

## Submodule Tasks
| Task ID | Stage | Status | Responsibility | Submodule | Parent Task | Depends On | Output | Done Condition |
|---------|-------|--------|----------------|-----------|-------------|------------|--------|----------------|
| | | pending / confirmed | | | | | | |

## Return Rules
- If acceptance finds proposal ambiguity:
  - return to proposal clarification task
- If acceptance finds design mismatch:
  - return to design task
- If acceptance finds implementation defect:
  - return to implementation task
- If acceptance finds testing implementation gap:
  - return to testing task
- For non-requirement findings:
  - repeat design -> implementation -> testing, then rerun acceptance
- If the same unresolved issue remains after more than 5 unsuccessful iterations:
  - stop and report the issue to the user

## Exit Condition
- [ ] All blocking issues closed
- [ ] Required evidence exists
- [ ] Document-producing stages auto-confirmed by setting front matter to `status: approved`, `approved_by: auto-pipeline`, and `approved_at`
- [ ] Every implemented `change_id` has proposal/design traceability and generated test coverage or an explicit test gap
- [ ] Every single-stage task passed stage-scope-check
- [ ] `uv run --active python ./harness/scripts/pipeline-plan-check.py harness/pipeline-plan.md --require-complete` passed
- [ ] Final acceptance passed against `proposal.md`
