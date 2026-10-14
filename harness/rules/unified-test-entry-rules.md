# Unified Test Entry Rules

## Goal
- Define the canonical runnable test interface for all project validation.
- Ensure every generated or hand-written test can be run through one stable command surface.

## Scope
- High-risk task evidence must use the task-scoped unified entrypoint. Standard tasks may invoke the repository's narrow native check directly and do not register a `testplan.yaml` by default.
- project-root test shortcuts: `test-run.bat` on Windows and `test-run.sh` on Unix-like systems
- `harness/scripts/test-run.py`
- generated test implementation
- optional `testing.md`
- `testplan.yaml` for completed testing work, unless a repo-local versioned exception explicitly allows missing machine-readable test metadata

## Canonical Commands
- Project-root shortcut:
  - Windows: `test-run.bat [<module> <level>]`
  - Unix: `./test-run.sh [<module> <level>]`
- `UV_CACHE_DIR=.harness/uv-cache uv run --active python ./harness/scripts/test-run.py <module> unit`
- `UV_CACHE_DIR=.harness/uv-cache uv run --active python ./harness/scripts/test-run.py <module> dv`
- `UV_CACHE_DIR=.harness/uv-cache uv run --active python ./harness/scripts/test-run.py <module> integration`
- `UV_CACHE_DIR=.harness/uv-cache uv run --active python ./harness/scripts/test-run.py <module> all`
- `UV_CACHE_DIR=.harness/uv-cache uv run --active python ./harness/scripts/test-run.py <module>/<task-name> all`
- `UV_CACHE_DIR=.harness/uv-cache uv run --active python ./harness/scripts/test-run.py all all`

## Consistency Rule
- `harness/scripts/test-run.py` is mandatory in generated repositories.
- What each level (`unit` / `dv` / `integration`) must verify and how deep is defined by `harness/rules/test-design-rules.md`; this rule governs how those tests are invoked.
- A generated repository MUST include both project-root one-click test shortcuts: `test-run.bat` for Windows and `test-run.sh` for Unix-like systems.
- The root shortcut MUST check whether `uv` is installed and print an installation hint when it is missing.
- The root shortcuts MUST create a local `.venv` when it is missing, use `uv` to sync or install dependencies when project metadata exists, activate the project virtual environment, and then invoke `harness/scripts/test-run.py` through `UV_CACHE_DIR=.harness/uv-cache uv run --active python`.
- The root shortcut MUST NOT bypass the unified test entrypoint.
- The unified test interface MUST be able to run every project test that is part of the harness evidence chain.
- Bootstrap and harness refresh work MUST replace the empty example `MODULE_SUITES` mapping with explicit registrations for the target repository. Each registered module MUST define a non-empty canonical `all` suite; scaffold self-check MUST fail when no canonical module suite is registered.
- `<module> all` and `all all` MUST compose every command registered under that module's `unit`, `dv`, `integration`, and explicit `all` suites, de-duplicating identical argv. Adding a level-specific regression therefore cannot leave the canonical `all` entrypoint unaware of it.
- A testing task is not complete until every new or changed test implementation is registered with, or otherwise reachable through, the unified test interface.
- Every task-local `testplan.yaml` MUST reference `task_manifest: task.yaml`; the runner derives its registration as `<module>/<task-name>` from the canonical packet path, and it MUST NOT attach the plan to the bare `<module>` scope.
- Package/module suites are mandatory maintenance commands independent of task plans; they MUST never be selected by single-task execution.
- Risk-triggered task-local contract steps are not module-suite selection. `<module>/<task-name> all` MAY execute a design-required package/workspace compile-only closure while remaining a task-scoped artifact; it MUST NOT execute the broad runtime suite.
- A task-local `repository-compile-closure` MAY run a package/workspace compile-only command such as `cargo test -p <package> --no-run --all-targets`; this does not authorize the package runtime test suite.
- Rust `--all-targets` excludes doctests. When documentation examples are affected, add a separate `documentation-examples` step such as `cargo test -p <package> --doc`, or a repository-defined README/example compile wrapper.
- `testplan.yaml` binds coverage to runnable test steps; optional `testing.md` references the authoritative metadata and run artifacts without repeating commands, coverage, or layer reasons.
- Test scripts MUST be non-interactive and return meaningful exit codes.
- New test execution paths MUST be added to the canonical entrypoint instead of creating unrelated ad hoc commands.

## Execution Contract
- Before real execution, fingerprint every bound testplan, the runner/registration source, governing proposal/design/risk inputs, and expanded `evidence_inputs`. Declare all relevant production, test, fixture, dependency/build and consumer inputs; directory fingerprints include file membership so additions and deletions invalidate reuse. Runtime outputs belong outside those input trees.
- Compare the same inputs after execution; a changed or missing input makes the run unsuccessful even if every command returned zero. Store the starting fingerprints and stability result in a unique artifact that cannot overwrite another run, including runs within the same second.
- Lifecycle validation selects the latest run by recorded start time for the same packet and task scope before testing its success and input consistency. Never fall back to an older success after a newer failure; a newer failed unit/DV/integration run also requires a fresh successful `all` run. Missing legacy fingerprints require a new run. Recheck freshness when creating or verifying a testing receipt.
- Testing completion and acceptance MUST reuse an existing successful `<module>/<task-name> all` artifact when its task/change binding, input fingerprints, and latest-run validity still pass the checks above. Entering acceptance, changing reviewer, or rechecking a receipt MUST NOT by itself trigger another execution of the same tests. Acceptance records the artifact path and the input-validity check as supporting evidence while independently reviewing behavior and test adequacy.
- Missing, failed, stale, or invalidated evidence requires a new task-scoped `all` run in testing. A concrete uncovered behavior or unresolved failure hypothesis may require targeted additional validation through testing; neither condition authorizes module/project-wide runtime suites.
- Unknown modules or test levels MUST exit non-zero.
- Every real run (not `--list` / `--dry-run`) MUST write a machine-readable run artifact to `.harness/test-results/test-runs/` recording requested task scope, requested level, covered `change_ids`, each executed command, its registration sources, exit code, duration, and optional informational git state. `.harness/` is local, uncommitted state outside durable `harness/` tooling and MUST be listed in `.gitignore`.
- `<module>/<task-name> all` MUST run only the task's `unit`, `dv`, and `integration` testplan commands.
- `<module>/<task-name> all` MUST run enabled task-local `contract_checks` before the task's unit/DV/integration commands. Contract sources record `contract_kind` and its exact assertion in the run artifact.
- Package/module and `all all` commands MUST exist only for explicit user-directed maintenance outside a task flow.
- Exact argv duplicates MUST execute once and the run artifact MUST preserve all contributing registration sources.
- The runner MUST NOT infer semantic containment between different argv values. Broad and filtered framework commands remain distinct unless scope isolation prevents them from being selected together.
- Enabled steps MUST execute in declared order.
- Mixed automated/manual task runs also record non-executed levels and their plan-owned reasons in the run artifact.
- "success without executing steps" MUST be reserved for task scopes whose every selected level is `manual` or `disabled` with a concrete reason; the run artifact records those non-executed levels.
- Each enabled step MUST declare stable machine-readable fields `id`, `name`, `change_ids`, and `run`.
- Each contract step MUST additionally declare `kind` and the matching fixed `assertion`; arbitrary prose assertions do not satisfy acceptance.
- Task runs with API/build-surface contract checks MUST record their expanded `evidence_inputs`.
- Each enabled step MUST declare the `change_ids` it validates for testing and task traceability.
- `harness/scripts/schema-check.py` MUST reject unknown levels, duplicate step ids, enabled levels without steps, enabled steps without `change_ids`, and manual or disabled levels without reasons when `testplan.yaml` exists.
