# Design Document Rules

## Goal
- Define the minimum useful structure and approval requirements for `design.md` and task-local `design/`.
- Optimize design artifacts for implementation correctness and handoff. Diagrams, prose polish, document length, and per-level file splitting are not completion requirements.

## Scope
- Formal design artifacts and completion requirements in this rule are part of the high-risk workflow. Standard work does not create `design.md` or task-local `design/` artifacts. Proposal discussion uses only the applicability guidance below.
- Single-project design: `.harness/tasks/<version>/modules/<project>/<task-seq>-<task-slug>/design.md` and optional task-local `design/`.
- Cross-project design: `.harness/tasks/<version>/modules/globals/<task-seq>-<task-slug>/design.md`.
- Required long-lived boundary sync in `docs/modules/<module>.md`.
- Project-rule-required updates to global architecture docs under `docs/architecture/`.

## Proposal Applicability
- In any tier, proposal discussion of a candidate design MUST apply the relevant design-quality guardrails below at the depth actually discussed: top-down reasoning, business responsibility boundaries, acyclic dependency direction, shared-module justification, state ownership, interface compatibility, affected consumers, material failure/lifecycle behavior, and preservation of existing invariants.
- Describe relationships at the relevant parent boundary; source-language signatures specify file-level interfaces. Exploratory proposal discussion need not reach file-level detail.
- Keep alternatives and abstractions grounded in the actual requirement and current code. Apply the guardrails against filler, speculative architecture, and unjustified abstractions to candidate solutions too.
- Formal metadata, complete file sequences, migration closure, architecture synchronization, and completion checks remain downstream obligations. Proposal records material compatibility/migration implications without claiming completed evidence.
- The prohibition on test planning below applies to design-stage artifacts. Proposal may discuss testing under [Test Design Rules — Proposal Applicability](test-design-rules.md#proposal-applicability); proposal content and its handoff are governed by [Proposal Document Rules](proposal-doc-rules.md#design-and-testing-discussion).

## Proposal Design Handoff
- When `proposal.md` contains a design solution, the design stage's primary goal is to inspect it for defects and complete it into an implementation-ready design. Start from that solution, retain sound decisions, correct defects, and fill missing detail under the guardrails below. Proposal approval does not prove design correctness.
- Compare the proposed solution with the requirement baseline, current code, affected consumers, dependencies, ownership, interfaces, and failure behavior. Record material findings, corrections, and their reasons in the active design source, referencing the relevant proposal sections or item IDs; do not recreate a valid solution merely to fill a separate stage document.
- Review depth and completion work follow what the proposal already establishes. A partial design needs review plus missing design work; when no solution exists, develop one from the approved requirements. In every case, the final design must satisfy the stage's required coverage and implementation-readiness criteria.

## Required Metadata
- `task_manifest: task.yaml`
- `status`
- Do not repeat `module`, `version`, `task_name`, or `submodule`; canonical identity comes from `task.yaml`.

## Required Content
- Reference `Risk profile: ./risk-profile.yaml`; for every applicable task-level risk, complete its concrete evidence paths and `required_checks` there, and do not copy a Trigger Matrix into `design.md`.
- Design scope, useful context, and the smallest sufficient overall approach.
- A top-down layered design decomposition from the whole affected module to submodules, nested submodules, and file-level modules.
- Split task-local design files only when complexity or context loading benefits; reference any split sources from the root design. No per-level file or fixed index table is required.
- `## Module Dependencies`: one row per affected unit with `level`, `parent`, `node`, and comma-separated `depends_on` (`none` for no dependencies).
- Source-language fenced code blocks for file-level module interfaces when the design reaches implementation files.
- Key boundary-crossing call order and relevant failure outcomes, using concise steps or another useful representation.
- State/data ownership only for persistent data or shared state affected by the task.
- Direct change mapping with stable `change_id` values, a concrete `target_module`, and concrete `Scope Paths`. Refine task-manifest paths during design; proposal does not freeze them. Receipt binding and later path revisions follow [Schema Validation Rules](schema-validation-rules.md#receipt-binding).
- File-level modules to create or modify, ordered by same-level dependency for implementation.
- Implementation ordering constraints, material design notes, risks, and rollback notes.

## Guardrails
- Design implements approved proposal intent without changing scope.
- Approval, packet immutability, and stage ownership follow `harness/rules/task-entry-gate-rules.md`; schema validation checks the document status only.
- Design stays at module shape level: relationships, ownership, interfaces, external dependencies, key flows, state transitions, and implementation order. Avoid low-level implementation detail unless it affects a contract, dependency, state, or control flow.
- Design docs MUST contain only useful content. Delete filler tables, placeholder sections, repeated proposal text, speculative extension points, idealized architecture, test planning, and facts that do not affect implementation, scope binding, or acceptance.
- Design MUST proceed top-down through affected boundaries. Parent responsibilities must be established before lower-level decisions; one document may cover multiple levels.
- Layered design documents belong to the active task's design artifact group. They describe the same task's decomposition and MUST NOT be used as a substitute for creating a sibling task packet for a new requirement or independent task.
- Design-stage artifacts MUST NOT define test cases, test plans, test strategy, validation IDs, test fixtures, testability seams, test implementation, or expected test results. Testing-stage artifacts own test-case design and test implementation after implementation completes.
- Diagrams are optional aids. The dependency record is authoritative for graph checks; flow and state descriptions need no diagram syntax.
- Dependency records describe same-level relationships. Cross-parent collaboration is represented at the nearest common parent level.
- Module and submodule dependency graphs MUST be acyclic. Circular dependencies return to design.
- Split by business responsibility first. Shared implementation logic used by multiple business submodules belongs in a shared submodule; single-consumer logic stays with its consumer.
- Technical areas such as HTTP interfaces, persistence, adapters, codecs, schedulers, or storage become technical submodules only when they have clear responsibility boundaries; otherwise keep them inside the owning business submodule and record the reason in `## Design Notes`.
- Dependency direction is business -> shared/technical. Shared and technical submodules MUST NOT depend on business submodules, and nothing may depend on an assembly submodule.
- A shared submodule needs at least 2 visible consumers and one nameable responsibility. Grab-bag `common`, `utils`, `misc`, or `helpers` designs are defects.
- File-level modules are one source file each. When file-level design is needed, the interface MUST be described with the current project's implementation language in fenced code blocks, for example Rust traits/struct impl signatures, TypeScript interfaces/classes, Python classes/protocols, Go interfaces, or comparable language-native signatures.
- File-level interface descriptions MUST name the concrete consumer or mapped `change_id` and compatibility decision (`new` / `backward-compatible` / `migration-required` / `breaking`). Breaking or migration-required changes MUST list affected callers and migration path.
- Designs that touch exported interfaces MUST include `## API and Build Surface Impact` with concrete `Public API impact`, `Crate-root export change`, `Build-surface change`, and `Documentation examples affected` values.
- A `breaking` or `migration-required` public API, crate-root export change, or build-surface change MUST include `## Consumer Migration Closure` rows mapping old symbol -> new path -> `change_id` -> concrete repository-relative consumer file -> consumer kind -> migration status.
- Consumer paths MUST be file-level paths. Directory-only paths, glob-only paths, `all callers`, and prose summaries do not count as consumer discovery evidence.
- Allowed migration statuses are `migrated`, `allowed-negative-fixture`, `allowed-compatibility-shim`, and `verified-none`. `allowed-compatibility-shim` is valid only for migration-required, never breaking, changes. `verified-none` requires a later machine-run removed-symbol scan.
- Unowned global functions as the primary file-level design are defects unless a repo custom rule or language constraint records an explicit exception in `## Design Notes`.
- Every persistent datum or shared state has exactly one owning submodule; other submodules access it through the owner's interface.
- Lifecycles and cross-boundary call flows include failure, timeout, retry, idempotency, and partial-completion behavior only where those behaviors affect this change.
- Boundary, technical-choice, and cross-module collaboration decisions record a serious rejected alternative only when a real alternative was considered; otherwise omit the filler and record no fake decision.
- Do not record testability seams in design documents. If a replaceable dependency or boundary hook is required for production behavior, describe it as an implementation interface with its real consumer and compatibility decision, not as a testing aid.
- Changes to existing modules list externally observable behavior and invariants to preserve when those invariants constrain the implementation; greenfield modules can record `not-applicable: new module`.
- Small submodules may map to one file. Larger submodules with internal responsibilities should be directories; keep detailed internal layout out of `design.md` unless required for the change.
- The final design MUST identify every file-level module that will be created or modified. `## File-Level Implementation Sequence` MUST order those files by dependency so implementation child tasks can be created and executed in that order.
- Existing-code designs describe current structure only where it constrains the change.
- Large module roots with 3 or more distinct externally visible directories/files MUST model new independent features as direct submodules unless `## Design Notes` explains otherwise.
- Do not add idealized architecture, speculative features, extension points, configuration, or abstractions beyond the approved proposal.
- New abstractions MUST match local patterns or remove real duplicated complexity, with the reason recorded in `## Design Notes`.
- Cross-module work MUST identify every affected module.
- Broad change buckets are not valid implementation scope bindings.
- Every `## Directly Mapped Change Items` row MUST name `target_module`. Single-project packets use their project module; `globals` packets use one row per affected project-level target and never use `globals` as the target.
- `Scope Paths` in `## Directly Mapped Change Items` are planned-impact hints only. They may be broad, may change during implementation, and never authorize or restrict repository reads or writes.
- Design MUST update `docs/architecture/` only when repo-local project rules require global architecture documentation changes. Default generated rules do not require mirrored implementation directories or proposal/design docs under `docs/architecture/`.
- Proposal ambiguity routes back to proposal.
- Downstream testing or acceptance follow-up is recorded unless cross-stage synchronization is explicitly requested.
- Return the design artifacts and evidence to the parent. The parent runs `UV_CACHE_DIR=.harness/uv-cache uv run --active python ./harness/scripts/task-transition.py --task <packet>/task.yaml advance`, which executes completion checks, approves the design, records its receipt, and advances. Do not run a separate completion profile before this transition; follow `harness/rules/task-execution-rules.md`.

Proposal confirmation authorizes downstream execution under `harness/rules/task-execution-rules.md`. Design/testing documents become approved after their stage checks pass; no intermediate user approval is required.
