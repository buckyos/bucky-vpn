# Retired Auto Pipeline Rules

## Status
- The generated workflow uses [Task Execution Rules](task-execution-rules.md). Proposal/tier confirmation authorizes continuous downstream execution, correction loops, and closeout; no separate auto-pipeline launch or execution-mode choice is required.
- This file remains only as a compatibility link for existing repository documents. It grants no alternate approval authority, artifact source, or lifecycle bypass.

## Existing Legacy Tasks
- Existing `mode: auto-pipeline` packets and pipeline state were produced by older tooling. Preserve their current artifacts and receipts; do not silently reinterpret or rewrite them as current-format tasks.
- Report incompatible tooling or legacy bindings and route any tooling or task-directory migration through the normal proposal flow. Rule-policy maintenance alone does not authorize those migrations.
- Matching project custom rules retain repository-policy priority under `AGENTS.md`; retired pipeline policy cannot override them.

## Current Tasks
- Follow [Task Entry Gate Rules](task-entry-gate-rules.md) for packet location, proposal confirmation, tier selection, and approval authority.
- Follow [Task Execution Rules](task-execution-rules.md) for subagent handoffs, stage transitions, independent acceptance, and completion. Do not create alternate pipeline plans or runtime state for new tasks.
