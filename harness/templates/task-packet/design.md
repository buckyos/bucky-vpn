---
task_manifest: task.yaml
status: draft
---

# [Module Name] Design

Risk profile: ./risk-profile.yaml

> Record implementation decisions and constraints. Reuse approved proposal content by reference. Diagrams and document splitting are optional; no presentation or line-count requirement applies.

## Design Scope
### Goals
### Non-goals

## Useful Context
<!-- Only record facts that materially affect the design: existing constraints, selected proposal items, compatibility obligations, runtime limits, or repo-local rules. Do not restate the whole proposal. -->

## Overall Approach
<!-- Short prose summary of the chosen implementation shape and why it is the smallest sufficient approach. -->
<!-- If proposal.md already contains a design solution, follow harness/rules/design-doc-rules.md#proposal-design-handoff: reference it and record material review findings, corrections, and completed detail. -->

## Module Dependencies
<!-- Record affected same-level units and their parent boundary. List comma-separated dependencies or none. Cross-parent collaboration is represented at the nearest common parent. Diagrams may supplement this checkable record. -->
| level | parent | node | depends_on |
|-------|--------|------|------------|
| submodule | example-module | ExampleFeature | ExampleStore |
| submodule | example-module | ExampleStore | none |

## File-Level Interfaces
<!-- When the design reaches source files, describe interfaces in the current project's implementation language. Use fenced code blocks with the real language identifier (`rust`, `typescript`, `python`, etc.) and show only signatures, types, traits/classes, exported functions, errors, and compatibility notes needed by implementers and reviewers. Do not use prose tables for file-level interfaces. If no file-level interface is relevant, record `not-applicable: <reason>`. -->

```python
class ExampleStore:
    def load(self, key: str) -> ExampleRecord | None: ...
    def save(self, record: ExampleRecord) -> None: ...
```

- Consumer: `ExampleFeature` / `CHG-example`
- Compatibility: new / backward-compatible / migration-required / breaking
- Migration path when required:

## API and Build Surface Impact
- Public API impact: none / backward-compatible / migration-required / breaking
- Crate-root export change: no
- Build-surface change: no
- Documentation examples affected: no

## Consumer Migration Closure
<!-- Required for breaking or migration-required APIs, crate-root export changes, and build-surface changes. Use one concrete repository-relative file per consumer row. `verified-none` is allowed only with a removed-symbol-scan contract check. Negative compile fixtures use `allowed-negative-fixture`; migration-required compatibility shims use `allowed-compatibility-shim`; breaking changes forbid compatibility shims; every other old-symbol consumer must be `migrated`. -->
| Old Symbol | New Path | change_id | Consumer Path | Consumer Kind | Migration Status |
|------------|----------|-----------|---------------|---------------|------------------|
| `crate::OldType` | `crate::new::NewType` | CHG-example | `tests/api.rs` | test | migrated |

## Key Flows
<!-- Record relevant call order and failure, timeout, retry, idempotency, and partial-completion behavior using concise steps or another useful representation. If absent, record not-applicable: <reason>. -->

## State and Ownership
<!-- Record affected persistent/shared state, owners, access paths, transitions, and invariants. Stateless tasks use not-applicable: <reason>. -->
- Owner:
- Access path:
- Transitions and invariants:

## Directly Mapped Change Items
| change_id | target_module | proposal_id | Design Coverage | Scope Paths | Interface / Boundary Impact | Notes |
|-----------|---------------|-------------|-----------------|-------------|-----------------------------|-------|
| CHG-example | example-module | P-001 | design decisions or source references | `src/example/**` | none / describe impact | |

## File-Level Implementation Sequence
<!-- List affected files and dependency order once. Include only ordering constraints that matter. These paths support planning and never restrict file access. -->
| sequence | file_level_module | action | depends_on | change_id |
|----------|-------------------|--------|------------|-----------|
| 1 | `src/example/example_store.py` | modify | none | CHG-example |

## Design Notes
<!-- Record only decisions a future implementer/reviewer needs: rejected alternatives, new abstraction justification, rollout/rollback constraints, or large-module submodule decisions. May record verification strategy, observability, constraints, and necessary test interfaces. Concrete cases, validation IDs, executable test plans, fixtures, implementation, and results belong to testing. Use bullets, not mandatory filler tables. -->
- Rejected alternative:
- New abstraction justification:
- Large-module submodule decision:

## Risks and Rollback
<!-- Material implementation, migration, compatibility, rollback, or operational risks only. -->
