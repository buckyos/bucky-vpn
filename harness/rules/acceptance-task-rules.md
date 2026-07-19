# Acceptance Task Rules

This separate acceptance stage is required only for high-risk work by default. Trivial and standard flows finish with an inline diff/result review and concise handoff.

## Goal
- Run the minimal requirement/implementation acceptance review defined by `harness/rules/acceptance-review-rules.md`.

## Primary Outputs
- The canonical acceptance output is the active packet's `acceptance-report.md` and, for auto-pipeline, final/return status under `.harness/pipelines/`.

## Execution
1. Require and read the active packet's `proposal.md` requirement baseline and delivered implementation.
2. Review requirement problems and implementation problems.
3. If a design source exists, compare implementation with it.
4. If a testing document exists, compare implementation with it.
5. Run `lifecycle-check.py --task <packet>/task.yaml --require-prior acceptance`; missing proposal, design, implementation, or testing receipts return to the missing owning stage instead of being waived by the report.
6. Write `acceptance-report.md` and run `task-transition.py --task <packet>/task.yaml complete`; it runs the acceptance completion profile and records the acceptance receipt only after the report check passes.
7. Only after acceptance succeeds, run `harness/scripts/task-index.py remove --task <packet>/task.yaml`; the script rechecks the accepted report and the complete manual receipt chain, or the manual-prefix receipts plus completed auto-pipeline state. Record `tasks.json` in the acceptance changed-path manifest.
