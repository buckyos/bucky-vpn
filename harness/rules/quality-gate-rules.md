# Quality Gate Rules

Declares repeatable build, lint, typecheck, and format checks with machine-written evidence.

## Configuration
- Declare gates in `harness/quality-gates.yaml`. Each needs a stable `id` and deterministic, non-interactive `run` command list with meaningful exit codes.
- Missing configuration fails closed. Empty `gates: []` requires a concrete, non-placeholder `empty_reason`.
- Record gate additions/removals as Harness/process decisions; other stages may edit the configuration when needed.

## Execution
- Run gates only on explicit current-user request or a matching highest-priority custom-rule requirement. Task execution, testing, acceptance, `check-all.py`, and changes under `harness/**` or `docs/**` do not trigger them automatically.
- Use `UV_CACHE_DIR=.harness/uv-cache uv run --active python ./harness/scripts/quality-check.py`; ad hoc equivalents are not gate evidence.
- The checker writes commands, exit codes, and durations to git-ignored `.harness/test-results/quality-runs/<timestamp>-quality.json`.
- Reports may cite requested gate artifacts as optional context; `acceptance-report-check.py` neither requires nor validates them.
- Report failing requested gates without weakening or bypassing them. Passing gates supplements tests and does not establish acceptance.
- Keep gates suitable for maintenance runs; long-running behavioral validation belongs in `test-run.py` task levels.
