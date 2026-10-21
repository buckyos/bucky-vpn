# Test Design Rules

These full post-implementation coverage contracts are the high-risk default. Standard tasks select the lowest-cost targeted verification that can expose the changed behavior; discovered high-risk boundaries follow the tier decision rules in `task-entry-gate-rules.md`. Proposal discussion uses the applicability guidance below without requiring completed coverage.

## Goal
- Define how post-implementation test cases are designed and recorded.
- Make completeness derivable from approved design documents and delivered code.

## Scope
- Post-implementation test case design at `unit`, `dv`, and `integration` levels.
- The single `testplan.yaml` coverage list; optional testing analysis and acceptance audits use its IDs.
- Acceptance audits that cite this rule.

## Proposal Applicability
- In any tier, proposal discussion of validation MUST use the unit/DV/integration responsibility boundaries and lowest-level placement principles below. Identify which behavior the proposed evidence can demonstrate, including whether it needs real module wiring, cross-module interaction, or an external environment; mocks do not establish behavior beyond their boundary.
- Derive representative scenarios from proposal requirements, candidate interfaces, state transitions, failure paths, invariants, and concurrency risks using the relevant `Case Derivation` categories below. An approved design or delivered code is not required to discuss these scenarios; label assumptions and unresolved behavior explicitly.
- Cover material success, failure, and boundary expectations needed to agree on acceptance. For proposed API/build changes, use `API and Repository Consumer Contracts` below to identify the kinds of future evidence needed. Record evidence feasibility, unavailable environments, and known gaps instead of promising unsupported coverage.
- Risk-based coverage accounting, validation IDs, test files, optional `testing.md`, `testplan.yaml`, unified-entrypoint registration, and execution results remain post-implementation obligations according to the confirmed tier. Planned evidence and expected outcomes are not passing results.
- Proposal content and handoff follow [Proposal Document Rules](proposal-doc-rules.md#design-and-testing-discussion). Later high-risk test-case design still derives from the approved design and delivered code under the contracts below.

## Proposal Test Handoff
- When `proposal.md` contains a test solution, the testing stage's primary goal is to inspect it for defects and complete it into implemented, runnable verification. Begin with those scenarios and evidence expectations, retain valid coverage, correct invalid cases or assertions, and fill gaps after inspecting the approved requirements, active design source, and delivered code. Proposal approval does not prove test adequacy.
- Challenge whether the proposed cases can expose the named failures: check expected outcomes, missing success/failure/boundary paths, test-level placement, mock versus real-system boundaries, environment feasibility, and false-positive assertions. Apply the coverage contracts below to the final test suite rather than treating the proposal's case list as exhaustive.
- Record material findings/refinements once in coverage metadata or optional `testing.md`, referring to proposal item IDs. Implement and execute completed cases through the unified entrypoint; reference actual run artifacts and remaining gaps.
- A partial test solution needs review plus missing case design; when no test solution exists, derive it from proposal, design, and delivered code. Requirement or acceptance-obligation defects return to proposal under `task-entry-gate-rules.md`; design and implementation defects return to their owning stages. Do not weaken agreed success evidence or alter expected behavior merely to make tests pass.

## Level Contracts

### Unit
- Object: changed functions, methods, and branches; external dependencies are stubbed or mocked.
- Placement: code inside an existing Rust `#[cfg(test)]` item is test code, but every newly created unit test MUST be implemented in a dedicated test file, test directory, or test-only crate/package rather than as a new inline test body in a production source file.
- File placement is stage evidence rather than filesystem permission: new tests belong in dedicated test files/directories/packages. Testing-stage validation rejects production paths except a mechanically proven edit confined to an existing Rust `#[cfg(test)]` item with selected-file baseline evidence.
- MUST cover affected behavior and relevant success, failure, and boundary outcomes. Inspect changed branches to find risks; touching a function does not require enumerating every unaffected branch.
- Reuse existing tests by reference. A current coverage-tool report bound to the tested revision may replace handwritten branch inventory; record uncovered material risks and concrete gap reasons.
- Not responsible for cross-module behavior, real external I/O, or full workflows.
- Done when affected behavior and risk boundaries have assertions or explicit gaps. Security, data integrity/migration, concurrency/ordering, resource cleanup, and other critical paths retain explicit branch/failure/invariant coverage or specific gap reasons, using tool reports where sufficient.

### DV (single-module runnable verification)
- Object: the module as a whole inside its boundary, with real internal wiring between submodules.
- MUST cover affected main and failure workflows, plus lifecycle, behavior-changing configuration, and persisted-state recovery where this change or its risks affect them. Reuse valid existing workflow tests.
- Not responsible for branch-level internal coverage or neighbor-module contracts.
- Done when affected workflows and risk boundaries have runnable DV steps or recorded gaps; unchanged workflows do not need new inventory rows.

### Integration
- Object: contracts between this module and neighbor modules.
- MUST cover success and failure semantics of affected consumed interfaces and boundary-crossing flows where the change can alter them. Include affected data flow and side-effect ordering; unchanged interfaces may reuse existing evidence without per-interface duplicate rows.
- Not responsible for re-proving logic already covered at unit or DV level.
- Done when affected contracts and risk boundaries have success/failure coverage or recorded gaps. Critical security, data, and concurrency contracts retain explicit coverage.

### API and Repository Consumer Contracts
- These contract checks complement, and do not replace, unit/DV/integration levels.
- Required contract kinds are: breaking APIs use `external-positive`, `external-negative`, `removed-symbol-scan`, and `repository-compile-closure`; migration-required APIs omit only `external-negative`; crate-root export changes use `external-positive` plus compile closure; build-surface-only changes use compile closure; documentation-example impact uses `documentation-examples`.
- The breaking-API negative wrapper succeeds only when compilation fails for the expected removed symbol; raw failing compiler commands are not valid evidence.
- Repository consumers include production references, tests, examples, benches, doctests, README/documentation compile fixtures, and downstream workspace packages identified by design.
- Risk-triggered contracts declare `evidence_inputs` covering production scope, repository consumers, external fixtures, tests, and affected documentation.
- A text scan is discovery/absence evidence, not a substitute for compiler closure. Aliases, re-exports, macros, feature combinations, and generated targets require compiler-backed checks where applicable.
- The canonical removed-symbol absence check is `harness/scripts/consumer-closure-check.py`; ad hoc `rg`, shell-negated searches, or pasted search output are not machine-valid testing evidence.

## Lowest-Level Placement
- Verify behavior at the lowest level that can expose its failure: pure logic at `unit`, single-module runtime behavior at `dv`, cross-module contracts at `integration`.
- Higher-level tests MUST NOT compensate for missing lower-level coverage.
- Duplicate verification across levels MUST record a reason.

## Case Derivation
Derive cases from approved design and affected implementation behavior. Use the categories below to discover relevant risks; record applicable cases and material gaps in the single coverage list, without mandatory rows for irrelevant categories:

| element_type | Derivation source | Required cases |
|--------------|-------------------|----------------|
| parameter-domain | changed interface input domains | equivalence classes plus min, max, empty, just-outside-range, malformed input |
| state-transition | `## State and Ownership` transitions | one case per legal transition plus illegal-transition rejection cases |
| failure-path | design key-flow failure handling | one fault-injection case per recorded failure path |
| error-handling | changed-code error categories | at least one case triggering each category |
| invariant | design state/ownership invariants | one case verifying each invariant |
| concurrency | concurrency, reentrancy, or ordering declarations | race, reentry, ordering cases, or a manual reason |

- Each derived case MUST cite its governing design source. Implementation details may also cite code locations and explain how the expected outcome follows from that design.
- Failure paths or error categories that are implementation details within existing contracts may be tested directly, including fault injection; record the design reference, code detail, and expected behavior without returning to design.
- Return to design only when an observation exposes a missing or contradictory decision about externally visible behavior, interfaces, state transitions/ownership, recovery policy, security, or another material invariant. Requirement or acceptance-boundary decisions return to proposal. Do not bless an implementation defect by changing its expected result.

## Required Recording
- `testplan.yaml` owns one `coverage` list. Each case/gap has `id`, `change_ids`, `design_source`, `target`, `case_types`, `element_types`, `level`, and `status`. Use single-line scalars and inline quoted-string lists for the three plural fields.
- `target` identifies affected behavior, a risk boundary, workflow, or contract. Group related outcomes and reference existing tests or current coverage reports instead of duplicating branch/interface inventories. Critical-path gaps remain explicit; tags alone do not prove adequate coverage.
- Covered records also name `test` (test file and case), `assertion` (observable expected behavior), and `step_id` in the matching enabled level. The step must bind every recorded `change_id`. One case may carry multiple tags and change IDs instead of being copied into multiple tables.
- Each change needs direct behavior/risk coverage or an explicit material gap. Select applicable `case_types`, `element_types`, and levels; no full category matrix per change or not-applicable rows for every absent category is required. Layer modes/reasons still live once in `levels`.
- Critical security, data, concurrency, and recovery paths require concrete failure/boundary/invariant evidence or specific gaps. Acceptance audits their depth against design and code even when a coverage tool supplies the inventory.
- Statuses: `covered`, `gap`, `manual`, `disabled`, `not-applicable`. Non-covered records need a concrete `reason`; `manual`/`disabled` records may inherit the matching level's reason. An all-not-applicable record set cannot satisfy a change's direct coverage obligation.
- Commands and layer reasons live in `levels`; execution results live in machine-written run artifacts. Do not repeat them in coverage records or copy coverage into `testing.md`, checklists, consumer tables, or per-level tables. Consumer discovery remains in design and contract verification in the task plan.
- When a versioned exception permits missing `testplan.yaml`, record these same fields once in `testing.md`'s `## Coverage` table. List-valued cells use quoted inline lists. Such an exception records manual/disabled/gap evidence; runnable automated coverage requires a plan.

## Guardrails
- Acceptance MUST audit per-level depth, not only case-type row presence.
- Unified-entrypoint registration and execution evidence are governed by `unified-test-entry-rules.md`.
