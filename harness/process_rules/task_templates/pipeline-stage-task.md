# Retired Pipeline Stage Task

Execution modes and pipeline plans are retired. Use `harness/process_rules/task-template.md` for a concise stage handoff referencing the canonical `.harness/tasks/<version>/modules/<packet-module>/<task-seq>-<task-slug>/task.yaml`.

User confirmation approves the proposal and tier. For high-risk work, a fresh subagent owns each stage and the parent runs `task-transition.py advance`, `return` or `complete`. Do not create pipeline state, launch approval, automatic approval or admission evidence. Existing artifacts remain historical archives.
