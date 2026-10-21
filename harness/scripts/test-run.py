#!/usr/bin/env python3
"""Unified test entrypoint for Harness Engineering repositories.

Adapt this template to the target repository's test commands. The contract is
stable: all test implementation must be reachable through this entrypoint.

Every real run (not --list / --dry-run) writes a machine-readable run artifact
to .harness/test-results/test-runs/<task-identity>/<timestamp>-<module>-<level>.json recording
the exact task scope, commands, registration sources, and exit codes.
`.harness/` is local, uncommitted state and must be listed in .gitignore.
Acceptance cites these artifacts instead of pasted command output.
"""

from __future__ import annotations

import argparse
import ast
import datetime
import hashlib
import json
import math
import os
import re
import signal
import subprocess
import sys
import time
import uuid
from pathlib import Path

from task_manifest import (
    TaskManifestError,
    content_digest,
    design_input_paths,
    parse_task_manifest,
    read_only_validation,
    read_text as shared_read_text,
    test_run_directory,
)


LEVELS = ("unit", "dv", "integration")
RUN_ARTIFACT_SCHEMA = 1
DEFAULT_TIMEOUT_SEC = 600.0
CONTRACT_KINDS = {
    "external-positive",
    "external-negative",
    "removed-symbol-scan",
    "repository-compile-closure",
    "documentation-examples",
}
CONTRACT_ASSERTIONS = {
    "external-positive": "new-path-compiles",
    "external-negative": "old-path-rejected-for-removed-symbol",
    "removed-symbol-scan": "no-unallowlisted-old-symbol-references",
    "repository-compile-closure": "repository-consumers-compile",
    "documentation-examples": "documentation-examples-compile",
}

# Canonical module-level regression suites. Task testplan.yaml files are
# discovered separately and are registered only as <module>/<task-name>.
# `module all` and `all all` compose unit, DV, integration, and explicit `all`
# commands, then de-duplicate identical argv. This keeps newly registered
# level-specific regressions reachable even when an older `all` suite exists.
MODULE_SUITES: dict[str, dict[str, list[list[str]]]] = {
    "repo-governance": {
        "unit": [
            ["python3", "./harness/tests/test_tooling_refresh.py", "ToolingUnitTests"],
            ["python3", "./harness/tests/test_tooling_lifecycle.py", "LifecycleBindingRegressionTests", "CanonicalAllSuiteRegressionTests"],
            ["python3", "./harness/scripts/context.py", "--validate-index"],
            ["python3", "./harness/tests/test_schema_check_proposal_approval.py"],
            ["python3", "./harness/tests/test_auto_pipeline_proposal_transition.py"],
            ["python3", "./harness/tests/test_test_file_location_rule.py"],
            ["python3", "./harness/tests/test_harness_upgrade_unit.py", "UpgradeUnitTests"],
            ["python3", "./harness/tests/test_governance_contracts.py"],
        ],
        "dv": [
            ["python3", "./harness/tests/test_tooling_check_reuse.py"],
            ["python3", "./harness/tests/test_tooling_source_isolation.py"],
            ["python3", "./harness/tests/test_tooling_timeouts.py"],
            ["python3", "./harness/tests/test_tooling_lifecycle.py", "UnifiedTaskExecutionTests", "ProposalApprovalRegressionTests"],
            ["python3", "./harness/scripts/harness-self-check.py", "--root", "."],
            ["python3", "./harness/tests/test_harness_upgrade_migration.py", "MigrationTests"],
        ],
        "integration": [
            ["python3", "./harness/tests/test_tooling_refresh.py", "ToolingIntegrationTests"],
            ["python3", "./harness/scripts/check-all.py", "--root", "."],
            ["python3", "./harness/tests/test_harness_upgrade_integration.py", "UpgradeIntegrationTests"],
        ],
        "all": [["python3", "./harness/scripts/check-all.py", "--root", "."]],
    },
    "vpn-frame": {
        "unit": [["cargo", "test", "-p", "vpn-frame"]],
        "dv": [["cargo", "build", "-p", "vpn-frame"]],
        "integration": [["cargo", "test", "--workspace"]],
        "all": [
            ["cargo", "test", "-p", "vpn-frame"],
            ["cargo", "build", "-p", "vpn-frame"],
            ["cargo", "test", "--workspace"],
        ],
    },
    "bucky-vpn": {
        "unit": [["cargo", "test", "-p", "bucky-vpn"]],
        "dv": [["cargo", "build", "-p", "bucky-vpn"]],
        "integration": [[
            "python3",
            "./harness/scripts/bucky-vpn-process-integration.py",
            "--use-base-image",
            "--parallel-instances",
            "2",
        ]],
        "all": [
            ["cargo", "test", "-p", "bucky-vpn"],
            ["cargo", "build", "-p", "bucky-vpn"],
            [
                "python3",
                "./harness/scripts/bucky-vpn-process-integration.py",
                "--use-base-image",
                "--parallel-instances",
                "2",
            ],
        ],
    },
    "bucky-vpn-server": {
        "unit": [["cargo", "test", "-p", "bucky-vpn-server"]],
        "dv": [["cargo", "build", "-p", "bucky-vpn-server"]],
        "integration": [["cargo", "test", "--workspace"]],
        "all": [
            ["cargo", "test", "-p", "bucky-vpn-server"],
            ["cargo", "build", "-p", "bucky-vpn-server"],
            ["cargo", "test", "--workspace"],
        ],
    },
    "vpn_web": {
        "unit": [["flutter", "test"]],
        "dv": [["flutter", "analyze"]],
        "integration": [["flutter", "build", "web"]],
        "all": [
            ["flutter", "test"],
            ["flutter", "analyze"],
            ["flutter", "build", "web"],
        ],
    },
}


def fail(message: str) -> None:
    print(f"test-run: {message}", file=sys.stderr)
    raise SystemExit(1)


def parse_inline_list(value: str) -> list[str]:
    try:
        parsed = ast.literal_eval(value)
    except (SyntaxError, ValueError) as error:
        fail(f"invalid run command list {value!r}: {error}")
    if not isinstance(parsed, list) or not all(isinstance(item, str) for item in parsed):
        fail(f"run command must be a list of strings: {value!r}")
    return parsed


def steps_from_testplan(path: Path, level: str) -> list[tuple[list[str], str, list[str]]]:
    text = shared_read_text(path)
    start = re.search(rf"(?m)^  {re.escape(level)}:\s*$", text)
    if not start:
        return []
    next_level = re.search(r"(?m)^(?:  [A-Za-z0-9_-]+:\s*$|[A-Za-z0-9_-]+:)", text[start.end() :])
    end = start.end() + next_level.start() if next_level else len(text)
    body = text[start.end() : end]
    starts = list(re.finditer(r"(?m)^      - id:\s*(\S+)\s*$", body))
    steps: list[tuple[list[str], str, list[str]]] = []
    for index, match in enumerate(starts):
        block_end = starts[index + 1].start() if index + 1 < len(starts) else len(body)
        block = body[match.end() : block_end]
        change_ids = re.search(r"(?m)^        change_ids:\s*(\[[^\n]*\])\s*$", block)
        run = re.search(r"(?m)^        run:\s*(\[[^\n]*\])\s*$", block)
        if not change_ids or not run:
            fail(f"testplan step {level}/{match.group(1)} must define inline change_ids and run lists")
        steps.append(
            (
                parse_inline_list(run.group(1)),
                match.group(1),
                parse_inline_list(change_ids.group(1)),
            )
        )
    return steps


def contract_steps_from_testplan(path: Path) -> list[tuple[list[str], str, list[str], str, str]]:
    text = shared_read_text(path)
    start = re.search(r"(?m)^contract_checks:\s*$", text)
    if not start:
        return []
    next_top = re.search(r"(?m)^[A-Za-z0-9_-]+:\s*", text[start.end() :])
    end = start.end() + next_top.start() if next_top else len(text)
    body = text[start.end() : end]
    if re.search(r"(?m)^  mode:\s*disabled\s*$", body):
        return []
    starts = list(re.finditer(r"(?m)^    - id:\s*([A-Za-z0-9_.-]+)\s*$", body))
    steps: list[tuple[list[str], str, list[str], str]] = []
    for index, match in enumerate(starts):
        block_end = starts[index + 1].start() if index + 1 < len(starts) else len(body)
        block = body[match.end() : block_end]
        kind_match = re.search(r"(?m)^      kind:\s*([A-Za-z0-9_-]+)\s*$", block)
        assertion_match = re.search(r"(?m)^      assertion:\s*([A-Za-z0-9_-]+)\s*$", block)
        change_ids = re.search(r"(?m)^      change_ids:\s*(\[[^\n]*\])\s*$", block)
        run = re.search(r"(?m)^      run:\s*(\[[^\n]*\])\s*$", block)
        if (
            not kind_match
            or kind_match.group(1) not in CONTRACT_KINDS
            or not assertion_match
            or assertion_match.group(1) != CONTRACT_ASSERTIONS.get(kind_match.group(1))
            or not change_ids
            or not run
        ):
            fail(
                f"testplan contract step {match.group(1)} must define a supported kind "
                "plus inline change_ids and run lists"
            )
        steps.append(
            (
                parse_inline_list(run.group(1)),
                match.group(1),
                parse_inline_list(change_ids.group(1)),
                kind_match.group(1),
                assertion_match.group(1),
            )
        )
    return steps


def evidence_input_paths(root: Path, testplan: Path) -> list[Path]:
    text = shared_read_text(testplan)
    match = re.search(r"(?m)^evidence_inputs:\s*(\[[^\n]*\])\s*$", text)
    configured = parse_inline_list(match.group(1)) if match else []
    candidates = [testplan.resolve()]
    root_resolved = root.resolve()
    for value in configured:
        if value in {"", "."} or any(part == ".." for part in Path(value).parts):
            fail(f"invalid evidence input path in {testplan}: {value!r}")
        configured_path = root / value
        if configured_path.is_symlink():
            fail(f"evidence input must not be a symlink in {testplan}: {value}")
        candidate = configured_path.resolve()
        try:
            candidate.relative_to(root_resolved)
        except ValueError:
            fail(f"evidence input resolves outside repository in {testplan}: {value}")
        if not candidate.exists():
            fail(f"evidence input does not exist in {testplan}: {value}")
        if candidate.is_dir():
            excluded = {".git", ".venv", "target", ".harness", "__pycache__"}
            if any(part in excluded for part in candidate.relative_to(root_resolved).parts):
                continue

            def walk_error(error: OSError) -> None:
                raise error

            for directory, dirs, files in os.walk(candidate, onerror=walk_error):
                parent = Path(directory)
                # Check links at the boundary, but never enter generated trees.
                for name in dirs + files:
                    child = parent / name
                    if child.is_symlink():
                        fail(f"evidence input tree contains a symlink in {testplan}: {child}")
                dirs[:] = sorted(name for name in dirs if name not in excluded)
                candidates.extend(
                    (parent / name).resolve() for name in sorted(files)
                    if name not in excluded and (parent / name).is_file()
                )
        elif candidate.is_file():
            candidates.append(candidate)
    return sorted(set(candidates), key=lambda item: item.relative_to(root_resolved).as_posix())


def evidence_input_roots(root: Path, testplans: list[str]) -> list[str]:
    roots: set[str] = set()
    for relative in testplans:
        text = shared_read_text(root / relative)
        match = re.search(r"(?m)^evidence_inputs:\s*(\[[^\n]*\])\s*$", text)
        if match:
            roots.update(parse_inline_list(match.group(1)))
    return sorted(roots)


def expanded_evidence_inputs(root: Path, testplans: list[str]) -> list[str]:
    if not testplans:
        return []
    root_resolved = root.resolve()
    paths: set[Path] = set()
    for relative in testplans:
        paths.update(evidence_input_paths(root, root / relative))
    return [
        path.relative_to(root_resolved).as_posix()
        for path in sorted(
            paths, key=lambda item: item.relative_to(root_resolved).as_posix()
        )
    ]


@read_only_validation
def run_input_snapshot(
    root: Path, testplans: list[str], *,
    design_sources: dict[tuple[Path, Path], list[Path]] | None = None,
) -> dict[str, object]:
    """Bind declared source/test/build trees (including membership) before execution."""
    paths = set(expanded_evidence_inputs(root, testplans))
    governing: dict[str, str | None] = {}
    for relative in testplans:
        packet = (root / relative).parent
        manifest = packet / "task.yaml"
        task = parse_task_manifest(manifest) if manifest.is_file() else {}
        for field, default in (("proposal", "proposal.md"), ("design", "design.md"), ("risk_profile", "risk-profile.yaml")):
            value = str(task.get(field) or default)
            candidate = (packet / value).resolve()
            try:
                key = candidate.relative_to(root.resolve()).as_posix()
            except ValueError:
                fail(f"governing input resolves outside repository: {value}")
            if field == "design" and candidate.is_file():
                try:
                    sources = design_input_paths(root, candidate, cache=design_sources)
                except TaskManifestError as error:
                    fail(str(error))
                for source in sources:
                    governing[source.relative_to(root.resolve()).as_posix()] = content_digest(source)
            # Parse design sources before hashing so their bytes serve both uses.
            governing[key] = content_digest(candidate) if candidate.is_file() else None
    return {
        "schema": 1,
        "testplans": sorted(set(testplans)),
        "roots": evidence_input_roots(root, testplans),
        "files": {
            relative: content_digest(root / relative)
            for relative in sorted(paths)
        },
        "governing_inputs": governing,
        "runner_sha256": content_digest(Path(__file__)),
    }


def non_executed_levels_from_testplan(path: Path, requested_level: str) -> list[dict[str, str]]:
    text = shared_read_text(path)
    levels = list(LEVELS) if requested_level == "all" else [requested_level]
    records: list[dict[str, str]] = []
    for level in levels:
        start = re.search(rf"(?m)^  {re.escape(level)}:\s*$", text)
        if not start:
            return []
        next_level = re.search(r"(?m)^(?:  [A-Za-z0-9_-]+:\s*$|[A-Za-z0-9_-]+:)", text[start.end() :])
        end = start.end() + next_level.start() if next_level else len(text)
        body = text[start.end() : end]
        mode_match = re.search(r"(?m)^    mode:\s*(manual|disabled)\s*$", body)
        reason_match = re.search(r"(?m)^    reason:\s*(.+)\s*$", body)
        if not mode_match or not reason_match or not reason_match.group(1).strip():
            return []
        records.append(
            {
                "level": level,
                "mode": mode_match.group(1),
                "reason": reason_match.group(1).strip(),
            }
        )
    return records


def task_change_ids_from_testplan(path: Path) -> list[str]:
    """Bind a no-command task run to the canonical task manifest changes."""
    task = path.parent / "task.yaml"
    if not task.is_file():
        fail(f"testplan references missing task manifest: {task}")
    try:
        manifest = parse_task_manifest(task)
    except TaskManifestError as error:
        fail(str(error))
    change_ids = [
        str(change.get("id"))
        for change in manifest.get("changes", [])
        if isinstance(change, dict) and change.get("id") is not None
    ]
    if not change_ids or len(change_ids) != len(set(change_ids)):
        fail(f"task manifest must contain unique change ids: {task}")
    return sorted(change_ids)


def repo_relative_testplan(root: Path, path: Path) -> str:
    try:
        return path.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        fail(f"task testplan resolves outside repository: {path}")


def discover_testplans(root: Path, scope: str | None = None, task: Path | None = None) -> dict[str, Path]:
    """Load an explicit packet directly; legacy discovery only scans its scope."""
    plans: dict[str, Path] = {}
    if task is not None:
        manifest = (root / task).resolve()
        if manifest.name != "task.yaml" or not manifest.is_file():
            fail(f"--task must name an existing canonical task.yaml: {manifest}")
        candidates = [manifest.parent / "testplan.yaml"]
        if not candidates[0].is_file():
            fail(f"missing task testplan: {candidates[0]}")
    elif scope is not None:
        parts = scope.split("/")
        if len(parts) != 2 or any(
            part in {"", ".", ".."} or any(char in part for char in "*?[]\\")
            for part in parts
        ):
            fail(f"invalid task scope: {scope}")
        candidates = root.glob(f".harness/tasks/*/modules/{scope}/testplan.yaml")
    else:
        candidates = root.glob(".harness/tasks/*/modules/*/*/testplan.yaml")
    for path in candidates:
        path = path.resolve()
        try:
            relative = path.relative_to(root / ".harness" / "tasks")
        except ValueError:
            fail(f"task testplan resolves outside repository task packets: {path}")
        if any(part.startswith("_") for part in relative.parts):
            if task is not None:
                fail(f"cannot select a template task packet: {path}")
            continue
        if len(relative.parts) != 5 or relative.parts[1] != "modules":
            fail(f"manifest-bound testplan must live in a canonical task packet: {path}")
        text = shared_read_text(path)
        if not re.search(r"(?m)^task_manifest:\s*task\.yaml\s*$", text):
            fail(f"testplan must use task_manifest: task.yaml: {path}")
        if re.search(r"(?m)^(module|version|task_name|submodule):\s*\S+", text):
            fail(f"testplan duplicates canonical task identity from task.yaml: {path}")
        if not (path.parent / "task.yaml").is_file():
            fail(f"manifest-bound testplan must live in a task packet with task.yaml: {path}")
        module, task_name = relative.parts[2:4]
        key = f"{module}/{task_name}"
        if task is not None and scope != key:
            fail(f"--task selects {key}, which does not match requested scope {scope}")
        if key in plans:
            fail(f"ambiguous task scope {key}: {plans[key]} and {path}; select --task <packet>/task.yaml")
        plans[key] = path
    return plans


def validate_module_suites() -> None:
    allowed_levels = {*LEVELS, "all"}
    for module, suite in MODULE_SUITES.items():
        if not module or "/" in module:
            fail(f"invalid canonical module name: {module!r}")
        unknown = set(suite) - allowed_levels
        if unknown:
            fail(f"module {module} has unknown canonical levels: {', '.join(sorted(unknown))}")
        missing = allowed_levels - set(suite)
        if missing:
            fail(f"module {module} is missing canonical levels: {', '.join(sorted(missing))}")
        if not suite.get("all"):
            fail(f"module {module} must define a non-empty canonical all suite")
        for level, commands in suite.items():
            if not isinstance(commands, list):
                fail(f"module {module} canonical level {level} must be a list of argv lists")
            for command in commands:
                if not command or not isinstance(command, list) or not all(
                    isinstance(item, str) and item for item in command
                ):
                    fail(f"module {module} canonical level {level} has invalid argv: {command!r}")


def command_sources_for(
    root: Path,
    scope: str,
    requested_level: str,
    task_plans: dict[str, Path],
) -> list[tuple[list[str], dict[str, object]]]:
    entries: list[tuple[list[str], dict[str, object]]] = []
    if scope in MODULE_SUITES:
        suite = MODULE_SUITES[scope]
        if requested_level == "all":
            if "all" not in suite or not suite["all"]:
                fail(f"module {scope} must define a non-empty canonical all suite")
            levels = [*LEVELS, "all"]
        else:
            levels = [requested_level]
        for level in levels:
            for command_index, command in enumerate(suite.get(level, [])):
                entries.append(
                    (
                        command,
                        {
                            "scope": scope,
                            "kind": "module-suite",
                            "level": level,
                            "registration_index": str(command_index),
                        },
                    )
                )
        return entries

    plan = task_plans.get(scope)
    if plan is None:
        fail(f"unknown module or task scope: {scope}")
    timeout_sec = testplan_timeout(plan)
    levels = list(LEVELS) if requested_level == "all" else [requested_level]
    try:
        plan_rel = plan.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        plan_rel = plan.as_posix()
    for level in levels:
        for command_index, (command, step_id, change_ids) in enumerate(steps_from_testplan(plan, level)):
            entries.append(
                (
                    command,
                    {
                        "scope": scope,
                        "kind": "task-testplan",
                        "level": level,
                        "testplan": plan_rel,
                        "timeout_sec": timeout_sec,
                        "step_id": step_id,
                        "change_ids": change_ids,
                        "registration_index": str(command_index),
                    },
                )
            )
    if requested_level == "all":
        contract_entries: list[tuple[list[str], dict[str, object]]] = []
        for command_index, (command, step_id, change_ids, contract_kind, assertion) in enumerate(
            contract_steps_from_testplan(plan)
        ):
            contract_entries.append(
                (
                    command,
                    {
                        "scope": scope,
                        "kind": "task-testplan-contract",
                        "level": "contract",
                        "contract_kind": contract_kind,
                        "assertion": assertion,
                        "testplan": plan_rel,
                        "timeout_sec": timeout_sec,
                        "step_id": step_id,
                        "change_ids": change_ids,
                        "registration_index": str(command_index),
                    },
                )
            )
        entries = contract_entries + entries
    return entries


def selected_scopes(requested_module: str, task_plans: dict[str, Path]) -> list[str]:
    if requested_module == "all":
        modules = sorted(MODULE_SUITES)
        if not modules:
            fail(
                "no canonical module suites registered in MODULE_SUITES; "
                "adapt harness/scripts/test-run.py to register each project module's "
                "explicit all suite before using 'all all'"
            )
        return modules
    if requested_module in MODULE_SUITES or requested_module in task_plans:
        return [requested_module]
    fail(f"unknown module or task scope: {requested_module}")


def deduplicated_execution_plan(
    root: Path,
    scopes: list[str],
    requested_level: str,
    task_plans: dict[str, Path],
) -> list[dict[str, object]]:
    by_argv: dict[tuple[str, ...], dict[str, object]] = {}
    for scope in scopes:
        for command, source in command_sources_for(root, scope, requested_level, task_plans):
            key = tuple(command)
            entry = by_argv.setdefault(key, {"command": command, "sources": []})
            sources = entry["sources"]
            if isinstance(sources, list) and source not in sources:
                sources.append(source)
    return list(by_argv.values())


def command_workdir(root: Path, sources: list[dict[str, object]]) -> Path:
    if sources and all(
        source.get("kind") == "module-suite" and source.get("scope") == "vpn_web"
        for source in sources
    ):
        return root / "vpn_web"
    return root


def testplan_timeout(path: Path) -> float:
    text = shared_read_text(path)
    defaults = re.search(r"(?ms)^defaults:[ \t]*\n(.*?)(?=^[A-Za-z0-9_-]+:|\Z)", text)
    values = re.findall(r"(?m)^  timeout_sec:[ \t]*([^\n]*)", defaults.group(1)) if defaults else []
    if not values:
        return DEFAULT_TIMEOUT_SEC
    try:
        timeout = float(values[0].split("#", 1)[0].strip())
    except ValueError:
        fail(f"{path} defaults.timeout_sec must be a finite positive number")
    if len(values) != 1 or not math.isfinite(timeout) or timeout <= 0:
        fail(f"{path} defaults.timeout_sec must be a finite positive number")
    return timeout


def terminate_process_tree(process: subprocess.Popen) -> None:
    cleanup_error: str | None = None
    if os.name == "nt":
        # Killing only the driver leaves test workers and services running.
        try:
            result = subprocess.run(
                ["taskkill", "/PID", str(process.pid), "/T", "/F"],
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=5,
            )
            if result.returncode:
                cleanup_error = f"taskkill exited with {result.returncode} for PID {process.pid}"
        except (OSError, subprocess.TimeoutExpired) as error:
            cleanup_error = f"taskkill failed: {error}"
        if process.poll() is None:
            process.kill()
    else:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    process.wait(timeout=5)
    if cleanup_error:
        raise OSError(cleanup_error)


def run_command(
    command: list[str], root: Path, dry_run: bool, *,
    timeout_sec: float = DEFAULT_TIMEOUT_SEC, outcome: dict[str, object] | None = None,
) -> int:
    print("+ " + " ".join(command))
    if dry_run:
        return 0
    try:
        process = subprocess.Popen(
            command, cwd=root, start_new_session=os.name != "nt",
            creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if os.name == "nt" else 0,
        )
    except OSError as error:
        print(f"test-run: command could not start: {error}", file=sys.stderr)
        return 127
    try:
        return process.wait(timeout=timeout_sec)
    except (subprocess.TimeoutExpired, KeyboardInterrupt) as error:
        cancelled = isinstance(error, KeyboardInterrupt)
        if outcome is not None:
            outcome["cancelled" if cancelled else "timed_out"] = True
        try:
            terminate_process_tree(process)
        except (OSError, subprocess.SubprocessError, KeyboardInterrupt) as cleanup_error:
            message = f"{type(cleanup_error).__name__}: {cleanup_error}"
            if outcome is not None:
                outcome["cleanup_error"] = message
            print(f"test-run: process tree cleanup failed: {message}", file=sys.stderr)
        message = "command cancelled" if cancelled else f"command timed out after {timeout_sec:g}s"
        print(f"test-run: {message}", file=sys.stderr)
        return 130 if cancelled else 124


def git_state(root: Path) -> tuple[str | None, bool | None]:
    try:
        head = subprocess.run(
            ["git", "rev-parse", "HEAD"], cwd=root, capture_output=True, text=True
        )
        status = subprocess.run(
            ["git", "status", "--porcelain"], cwd=root, capture_output=True, text=True
        )
    except OSError:
        return None, None
    if head.returncode != 0 or status.returncode != 0:
        return None, None
    return head.stdout.strip(), bool(status.stdout.strip())


def write_run_artifact(
    root: Path,
    requested_module: str,
    requested_level: str,
    started_at: str,
    steps: list[dict[str, object]],
    exit_code: int,
    non_executed_levels: list[dict[str, str]] | None = None,
    bound_testplans: list[str] | None = None,
    bound_change_ids: list[str] | None = None,
    input_snapshot: dict[str, object] | None = None,
    inputs_unchanged: bool = False,
) -> None:
    testplans = sorted(
        set(bound_testplans or []) | {
            source["testplan"]
            for step in steps
            for source in step.get("sources", [])
            if isinstance(source, dict) and isinstance(source.get("testplan"), str)
        }
    )
    artifact_dir = test_run_directory(root, requested_module, testplans)
    try:
        artifact_dir.mkdir(parents=True, exist_ok=True)
        timestamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")
        slug_module = requested_module.replace("/", "+")
        artifact_path = artifact_dir / f"{timestamp}-{slug_module}-{requested_level}-{uuid.uuid4().hex}.json"
        head, dirty = git_state(root)
        artifact = {
            "schema": RUN_ARTIFACT_SCHEMA,
            "requested_module": requested_module,
            "requested_level": requested_level,
            "started_at": started_at,
            "finished_at": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
            "git_head": head,
            "worktree_dirty": dirty,
            "testplans": testplans,
            "change_ids": sorted(
                set(bound_change_ids or []) | {
                    change_id
                    for step in steps
                    for source in step.get("sources", [])
                    if isinstance(source, dict)
                    for change_id in source.get("change_ids", [])
                    if isinstance(change_id, str)
                }
            ),
            "input_snapshot": input_snapshot,
            "inputs_unchanged": inputs_unchanged,
            "evidence_inputs": list((input_snapshot or {}).get("files", {})),
            "evidence_input_roots": (input_snapshot or {}).get("roots", []),
            "steps": steps,
            "non_executed_levels": non_executed_levels or [],
            "exit_code": exit_code,
        }
        artifact_path.write_text(json.dumps(artifact, indent=2) + "\n", encoding="utf-8")
        print(f"test-run: run artifact written: {artifact_path}")
    except OSError as error:
        print(f"test-run: warning: failed to write run artifact: {error}", file=sys.stderr)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("module", help="module name, module/task-name, or all")
    parser.add_argument("level", choices=[*LEVELS, "all"], help="test level or all")
    parser.add_argument("--root", default=".")
    parser.add_argument("--task", type=Path, help="canonical task.yaml path (relative to --root or absolute); load only this packet")
    parser.add_argument("--list", action="store_true", help="list known modules and exit")
    parser.add_argument("--dry-run", action="store_true", help="print commands without running them")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    validate_module_suites()
    if args.task is not None:
        task_plans = discover_testplans(root, args.module, args.task)
    elif args.list:
        task_plans = discover_testplans(root)
    elif "/" in args.module:
        task_plans = discover_testplans(root, args.module)
    else:
        # Canonical module/project suites never depend on historical task plans.
        task_plans = {}
    modules = sorted({*MODULE_SUITES, *task_plans})
    if args.list:
        for module in modules:
            print(module)
        return 0

    scopes = selected_scopes(args.module, task_plans)
    bound_testplans = sorted({
        repo_relative_testplan(root, task_plans[scope])
        for scope in scopes if scope in task_plans
    })
    bound_change_ids = sorted({
        change_id for scope in scopes if scope in task_plans
        for change_id in task_change_ids_from_testplan(task_plans[scope])
    })
    started_at = datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="microseconds")
    input_snapshot = run_input_snapshot(root, bound_testplans) if not args.dry_run else None
    plan = deduplicated_execution_plan(root, scopes, args.level, task_plans)
    if not plan:
        non_executed_levels = (
            non_executed_levels_from_testplan(task_plans[args.module], args.level)
            if len(scopes) == 1 and args.module in task_plans
            else []
        )
        if not non_executed_levels:
            fail("no test commands matched the requested module/task scope and level")
        print(
            "test-run: passed without automated commands; selected levels are manual/disabled: "
            + ", ".join(
                f"{item['level']}={item['mode']} ({item['reason']})"
                for item in non_executed_levels
            )
        )
        if not args.dry_run:
            task_plan = task_plans[args.module]
            try:
                inputs_unchanged = input_snapshot == run_input_snapshot(root, bound_testplans)
            except (OSError, SystemExit, TaskManifestError):
                inputs_unchanged = False
            write_run_artifact(
                root,
                args.module,
                args.level,
                started_at,
                [],
                0 if inputs_unchanged else 1,
                non_executed_levels,
                bound_testplans=[repo_relative_testplan(root, task_plan)],
                bound_change_ids=task_change_ids_from_testplan(task_plan),
                input_snapshot=input_snapshot,
                inputs_unchanged=inputs_unchanged,
            )
            return 0 if inputs_unchanged else 1
        return 0
    # Preserve manual/disabled reasons in mixed automated/manual runs as well.
    non_executed_levels = [
        item
        for scope in scopes if scope in task_plans
        for level in (LEVELS if args.level == "all" else [args.level])
        for item in non_executed_levels_from_testplan(task_plans[scope], level)
    ]
    source_count = sum(len(entry["sources"]) for entry in plan if isinstance(entry.get("sources"), list))
    if source_count > len(plan):
        print(
            f"test-run: exact argv deduplication merged {source_count} registrations "
            f"into {len(plan)} commands"
        )

    steps: list[dict[str, object]] = []
    exit_code = 0
    for entry in plan:
        command = entry["command"]
        sources = entry["sources"]
        if not isinstance(command, list) or not all(isinstance(item, str) for item in command):
            fail(f"invalid registered command: {command!r}")
        if not isinstance(sources, list) or not sources:
            fail(f"registered command has no sources: {command!r}")
        first_source = sources[0]
        # Exact argv may have several registrations; enforce the tightest limit.
        timeout_sec = min(float(source.get("timeout_sec", DEFAULT_TIMEOUT_SEC)) for source in sources)
        outcome: dict[str, object] = {"timed_out": False, "cancelled": False}
        step_start = time.monotonic()
        code = run_command(command, command_workdir(root, sources), args.dry_run, timeout_sec=timeout_sec, outcome=outcome)
        steps.append(
            {
                "module": first_source.get("scope") if isinstance(first_source, dict) else None,
                "level": first_source.get("level") if isinstance(first_source, dict) else None,
                "command": command,
                "sources": sources,
                "exit_code": code,
                "timeout_sec": timeout_sec,
                "timed_out": outcome["timed_out"],
                "cancelled": outcome["cancelled"],
                **({"cleanup_error": outcome["cleanup_error"]} if "cleanup_error" in outcome else {}),
                "duration_s": round(time.monotonic() - step_start, 3),
            }
        )
        if code != 0:
            exit_code = code
            break

    if not args.dry_run:
        try:
            inputs_unchanged = input_snapshot == run_input_snapshot(root, bound_testplans)
        except (OSError, SystemExit, TaskManifestError):
            inputs_unchanged = False
        if not inputs_unchanged:
            print("test-run: inputs changed during execution; rerun with stable inputs", file=sys.stderr)
            exit_code = exit_code or 1
        write_run_artifact(
            root, args.module, args.level, started_at, steps, exit_code,
            non_executed_levels=non_executed_levels,
            bound_testplans=bound_testplans, bound_change_ids=bound_change_ids,
            input_snapshot=input_snapshot, inputs_unchanged=inputs_unchanged,
        )
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
