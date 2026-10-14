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
- Full branch/interface coverage accounting, every derivation-category row, validation IDs, test files, `testing.md`, `testplan.yaml`, unified-entrypoint registration, and execution results remain post-implementation obligations according to the confirmed tier. Planned evidence and expected outcomes are not passing results.
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
- MUST cover every public function touched by implemented `change_id` values and every conditional branch in changed code: if/else arms, match/switch arms, early returns, error returns, loop zero/one/many iterations, and boundary comparisons.
- Uncovered branches MUST have a coverage record naming the branch and a concrete gap reason.
- Not responsible for cross-module behavior, real external I/O, or full workflows.
- Done when every changed-code branch is covered or has a recorded per-branch gap reason.

### DV (single-module runnable verification)
- Object: the module as a whole inside its boundary, with real internal wiring between submodules.
- MUST cover lifecycle where applicable, each main workflow, at least one failure workflow, behavior-changing configuration variants, and persisted-state recovery when the module persists state.
- Not responsible for branch-level internal coverage or neighbor-module contracts.
- Done when each main workflow and at least one failure workflow have runnable DV steps or recorded gaps.

### Integration
- Object: contracts between this module and neighbor modules.
- MUST cover every consumed exported interface with at least one success case and one failure-semantics case, plus cross-module data flow and side-effect ordering for boundary-crossing key call flows.
- Not responsible for re-proving logic already covered at unit or DV level.
- Done when every consumed exported interface has success and failure coverage or recorded gaps.

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
Test cases MUST derive from design artifacts. Each derivation source below gets concrete cases or an explicit gap/not-applicable record in the single coverage list:

| element_type | Derivation source | Required cases |
|--------------|-------------------|----------------|
| parameter-domain | changed interface input domains | equivalence classes plus min, max, empty, just-outside-range, malformed input |
| state-transition | `## State and Ownership` transitions | one case per legal transition plus illegal-transition rejection cases |
| failure-path | design key-flow failure handling | one fault-injection case per recorded failure path |
| error-handling | changed-code error categories | at least one case triggering each category |
| invariant | design state/ownership invariants | one case verifying each invariant |
| concurrency | concurrency, reentrancy, or ordering declarations | race, reentry, ordering cases, or a manual reason |

- Every element type MUST appear in coverage tags at least once; absent design content uses `status: not-applicable` with a concrete design source and reason.
- Each derived case MUST name its design source so acceptance can audit authenticity.
- New failure paths, error categories, or state transitions found in code but missing from design return to design.

## Required Recording
- `testplan.yaml` owns one `coverage` list. Each case/gap has `id`, `change_ids`, `design_source`, `target`, `case_types`, `element_types`, `level`, and `status`. Use single-line scalars and inline quoted-string lists for the three plural fields.
- `target` identifies the changed function/branch, module workflow, or consumed interface. Record each changed branch, each main/lifecycle/failure/config/persistence workflow where applicable, and each interface's success/failure semantics; tags alone do not prove that depth.
- Covered records also name `test` (test file and case), `assertion` (observable expected behavior), and `step_id` in the matching enabled level. The step must bind every recorded `change_id`. One case may carry multiple tags and change IDs instead of being copied into multiple tables.
- Applicable DV coverage accounts for `normal`, `error`, and `lifecycle` with DV records; a stateless lifecycle may use a reasoned not-applicable record. Tags in other levels do not satisfy these DV obligations.
- Every change accounts for `normal`, `boundary`, `negative`, `error`, `compatibility`, `lifecycle`, and `cross-module` case types. The task accounts for every design element type above and all three test levels. Group truly inapplicable categories only when they share a concrete task-specific reason.
- Statuses: `covered`, `gap`, `manual`, `disabled`, `not-applicable`. Non-covered records need a concrete `reason`; `manual`/`disabled` records may inherit the matching level's reason. An all-not-applicable record set cannot satisfy a change's direct coverage obligation.
- Commands and layer reasons live in `levels`; execution results live in machine-written run artifacts. Do not repeat them in coverage records or copy coverage into `testing.md`, checklists, consumer tables, or per-level tables. Consumer discovery remains in design and contract verification in the task plan.
- When a versioned exception permits missing `testplan.yaml`, record these same fields once in `testing.md`'s `## Coverage` table. List-valued cells use quoted inline lists. Such an exception records manual/disabled/gap evidence; runnable automated coverage requires a plan.

## Guardrails
- Acceptance MUST audit per-level depth, not only case-type row presence.
- Unified-entrypoint registration and execution evidence are governed by `unified-test-entry-rules.md`.
