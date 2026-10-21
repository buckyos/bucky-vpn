#!/usr/bin/env python3
"""Run workflow and evidence checks from one canonical task.yaml manifest.

Neither profile grants edit authorization. Stage-scope validation rejects
changed paths outside the active stage artifact group; declared design Scope
Paths remain traceability metadata rather than a second path gate.

The parser intentionally supports only the small dependency-free YAML subset
used by the generated task template: top-level scalars plus a `changes` list
whose list-valued fields use inline YAML/Python syntax.
"""

from __future__ import annotations

import argparse
import contextlib
import fnmatch
import hashlib
import importlib.util
import io
import json
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

from task_manifest import TaskManifestError, parse_task_manifest, test_run_directory
from task_manifest import content_digest, invalidate_validation_content, read_json, validation_content


STAGES = {"proposal", "design", "implementation", "testing", "acceptance"}
PROFILES = {"pre-edit", "completion"}
TASK_NAME_RE = re.compile(r"^\d{3,}-[a-z0-9][a-z0-9_.-]*$")
NAME_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.-]*$")
CHANGE_ID_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9_.-]*$")
TOP_LEVEL_FIELDS = {
    "workflow_tier", "version", "packet_module", "task_name", "stage",
    "proposal", "design", "testing", "testplan", "acceptance_report",
    "completion_report", "change_record",
    "risk_profile", "lifecycle_state", "changed_paths_file",
    "baseline_manifest",
}
CHANGE_SCALARS = {"id", "target_module", "changed_paths_file"}
CHANGE_LISTS = {"scope_paths"}


def fail(message: str) -> None:
    print(f"harness-check: {message}", file=sys.stderr)
    raise SystemExit(1)


def parse_task(path: Path) -> dict[str, object]:
    try:
        task = parse_task_manifest(path)
    except TaskManifestError as error:
        fail(str(error))
    unknown = set(task) - TOP_LEVEL_FIELDS - {"schema_version", "changes"}
    if unknown:
        fail(f"{path} unsupported top-level fields: {', '.join(sorted(unknown))}")
    if task.get("schema_version") != 1:
        fail(f"{path} schema_version must be 1")
    for change in task.get("changes", []):
        if not isinstance(change, dict):
            fail(f"{path} contains a malformed change entry")
        unknown_change = set(change) - CHANGE_SCALARS - CHANGE_LISTS
        if unknown_change:
            fail(
                f"{path} unsupported change fields: "
                + ", ".join(sorted(unknown_change))
            )
    validate_task(task, path)
    return task


def safe_repo_path(root: Path, value: str, label: str) -> Path:
    relative = Path(value)
    if relative.is_absolute() or ".." in relative.parts:
        fail(f"unsafe {label}: {value}")
    resolved = (root / relative).resolve()
    try:
        resolved.relative_to(root.resolve())
    except ValueError:
        fail(f"{label} resolves outside repository: {value}")
    return resolved


def validate_task(task: dict[str, object], path: Path) -> None:
    tier = task.get("workflow_tier", "high-risk")
    if tier != "high-risk":
        fail(
            f"{path} workflow_tier must be confirmed high-risk for this lifecycle "
            "entrypoint; every tier uses the common proposal packet, while lower "
            "tiers execute their lighter post-confirmation flows directly"
        )
    task["workflow_tier"] = tier
    required = (
        "schema_version", "version", "packet_module", "task_name", "stage",
        "proposal", "risk_profile",
    )
    missing = [key for key in required if task.get(key) in {None, ""}]
    if missing:
        fail(f"{path} missing required fields: {', '.join(missing)}")
    version = str(task["version"])
    packet_module = str(task["packet_module"])
    task_name = str(task["task_name"])
    if not NAME_RE.fullmatch(version):
        fail(f"invalid version: {version}")
    if not NAME_RE.fullmatch(packet_module):
        fail(f"invalid packet_module: {packet_module}")
    if not TASK_NAME_RE.fullmatch(task_name):
        fail(f"invalid task_name: {task_name}")
    if task["stage"] not in STAGES:
        fail(f"stage must be one of: {', '.join(sorted(STAGES))}")
    canonical_artifacts = {
        "proposal": "proposal.md",
        "design": "design.md",
        "testing": "testing.md",
        "testplan": "testplan.yaml",
        "acceptance_report": "acceptance-report.md",
        "risk_profile": "risk-profile.yaml",
        "lifecycle_state": "lifecycle.json",
    }
    for field, expected in canonical_artifacts.items():
        if task.get(field) not in {None, "", expected}:
            fail(f"{field} must use canonical task-packet path {expected}")
    changes = task["changes"]
    if not isinstance(changes, list) or not changes:
        fail(f"{path} changes must contain at least one entry")
    ids: set[str] = set()
    for change in changes:
        assert isinstance(change, dict)
        change_id = str(change.get("id") or "")
        target = str(change.get("target_module") or "")
        if not CHANGE_ID_RE.fullmatch(change_id):
            fail(f"invalid change id: {change_id!r}")
        if change_id in ids:
            fail(f"duplicate change id: {change_id}")
        ids.add(change_id)
        if not NAME_RE.fullmatch(target) or target == "globals":
            fail(f"change {change_id} requires a concrete target_module")
        if packet_module != "globals" and target != packet_module:
            fail(f"change {change_id} target_module must equal packet_module outside globals packets")
        for list_field in CHANGE_LISTS:
            if list_field not in change or not isinstance(change[list_field], list):
                fail(f"change {change_id} missing {list_field} inline list")


def task_packet(root: Path, task: dict[str, object]) -> Path:
    return root / ".harness" / "tasks" / str(task["version"]) / "modules" / str(task["packet_module"]) / str(task["task_name"])


def risk_profile_triggers(path: Path) -> list[str]:
    """Derive router trigger ids from the task's one risk profile."""
    mapping = {
        "contract": "contract-protocol",
        "data": "data-schema",
        "security": "security",
        "runtime": "runtime-integration",
        "build": "build-config-deployment",
        "ui": "ui-workflow",
        "harness": "harness-process",
    }
    if not path.is_file():
        return []
    active: list[str] = []
    in_risks = False
    category: str | None = None
    for raw in path.read_text(encoding="utf-8").splitlines():
        indent = len(raw) - len(raw.lstrip())
        stripped = raw.strip()
        if indent == 0:
            in_risks = stripped == "risks:"
            category = None
        elif in_risks and indent == 2 and stripped.endswith(":"):
            category = stripped[:-1]
        elif in_risks and category in mapping and indent == 4 and stripped == "applies: true":
            active.append(mapping[category])
    return active


def artifact_path(root: Path, manifest: Path, task: dict[str, object], field: str, *, required: bool = False) -> Path | None:
    value = task.get(field)
    if value in {None, ""}:
        if required:
            fail(f"task manifest requires {field} for stage {task['stage']}")
        return None
    text = str(value)
    if text.startswith(".harness/"):
        return safe_repo_path(root, text, field)
    relative = Path(text)
    if relative.is_absolute() or ".." in relative.parts:
        fail(f"unsafe task-packet artifact {field}: {text}")
    result = (manifest.parent / relative).resolve()
    try:
        result.relative_to(root.resolve())
    except ValueError:
        fail(f"task-packet artifact {field} resolves outside repository: {text}")
    return result


def hybrid_baseline_relative(task: dict[str, object]) -> str:
    return (
        Path(".harness") / "baselines"
        / str(task["version"])
        / f"{task['task_name']}-{task['stage']}" / "manifest.json"
    ).as_posix()


def testing_baseline_relative(task: dict[str, object]) -> str:
    explicit = task.get("baseline_manifest")
    return str(explicit) if explicit not in {None, ""} else hybrid_baseline_relative(task)


def baseline_copy_paths(root: Path, task: dict[str, object]) -> list[str]:
    if task["stage"] != "testing":
        return []
    result: list[str] = []
    for value in context_changed_paths(task):
        if any(token in value for token in ("*", "?", "[", "]")) or not value.endswith(".rs"):
            continue
        if safe_repo_path(root, value, "testing baseline source").is_file():
            result.append(value)
    return list(dict.fromkeys(result))


def baseline_pre_edit_command(root: Path, manifest: Path, task: dict[str, object]) -> list[str]:
    baseline = hybrid_baseline_relative(task)
    script = root / "harness" / "scripts" / "baseline-snapshot.py"
    task_relative = manifest.relative_to(root).as_posix()
    if safe_repo_path(root, baseline, "repository baseline manifest").is_file():
        return [
            sys.executable, str(script), "verify", "--root", str(root),
            "--manifest", baseline, "--task", task_relative,
        ]
    command = [
        sys.executable, str(script), "capture", "--root", str(root),
        "--task-id", f"{task['version']}/{task['task_name']}-{task['stage']}",
        "--task", task_relative,
    ]
    for path in baseline_copy_paths(root, task):
        command += ["--copy-path", path]
    return command


def baseline_completion_command(root: Path, manifest: Path, task: dict[str, object]) -> list[str]:
    output = task.get("changed_paths_file")
    if output in {None, ""}:
        fail(f"{task['stage']} completion requires top-level changed_paths_file")
    return [
        sys.executable, str(root / "harness" / "scripts" / "baseline-snapshot.py"),
        "diff", "--root", str(root),
        "--manifest", hybrid_baseline_relative(task),
        "--task", manifest.relative_to(root).as_posix(),
        "--output", str(output),
    ]


def command_prefix(root: Path, script: str) -> list[str]:
    return [sys.executable, str(root / "harness" / "scripts" / script), "--root", str(root)]


def run(command: list[str], *, dry_run: bool) -> bool:
    print("harness-check: " + " ".join(command))
    return True if dry_run else subprocess.run(command).returncode == 0


def checker_cache_key(
    root: Path, manifest: Path, task: dict[str, object], command: list[str], *,
    checker_versions: dict[Path, dict[str, str]] | None = None,
) -> str | None:
    """Fingerprint only read-only checks with a known, complete input contract.

    Lifecycle/test evidence is deliberately excluded: its latest-run checks must
    execute on every invocation. Missing/invalid dependencies disable reuse and
    leave diagnostics to the checker itself.
    """
    script = Path(command[1])
    name = script.name.removesuffix(".template.py").removesuffix(".py")
    if name not in {"schema-check", "risk-profile-check", "stage-scope-check"}:
        return None
    try:
        paths = {manifest}
        resolved_paths: dict[str, str] = {}
        stage = str(task["stage"])
        if name == "schema-check":
            paths.add(manifest.parent / "proposal.md")
            if stage != "proposal":
                paths.add(manifest.parent / "design.md")
            if stage in {"testing", "acceptance"}:
                paths.update(manifest.parent / value for value in ("testing.md", "testplan.yaml"))
            index = root / ".harness" / "tasks" / str(task["version"]) / "tasks.json"
            paths.add(index)
            data = read_json(index)
            for entry in data["tasks"]:
                indexed = safe_repo_path(root, entry["task_manifest"], "indexed task")
                paths.update((indexed, indexed.parent / "proposal.md"))
        elif name == "risk-profile-check":
            fields = ["proposal", "risk_profile"]
            if stage != "proposal":
                fields.append("design")
            if stage in {"testing", "acceptance"}:
                fields.extend(("testing", "testplan"))
            if stage == "acceptance":
                fields.append("acceptance_report")
            defaults = {"testing": "testing.md", "testplan": "testplan.yaml", "acceptance_report": "acceptance-report.md"}
            paths.update(manifest.parent / str(task.get(field) or defaults.get(field, "")) for field in fields)
        else:
            if "--from-git" in command:
                return None
            # This legacy task's pinned exception reads additional migration
            # journals and authorization bytes. Revalidate it rather than cache
            # a scope result whose generic key does not bind those inputs.
            if task.get("task_name") == "039-harness-full-upgrade":
                return None
            spec = importlib.util.spec_from_file_location("cached_stage_scope", script)
            if spec is None or spec.loader is None:
                return None
            scope = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(scope)
            options = {
                argument: command[index + 1]
                for index, argument in enumerate(command[:-1])
                if argument in {"--stage", "--version", "--module", "--submodule"}
            }
            for index, argument in enumerate(command):
                if argument == "--changed-paths-file":
                    changed = safe_repo_path(root, command[index + 1], "changed paths")
                    paths.update((changed, changed.with_name(changed.name + ".meta.json")))
                    for value in scope.changed_paths_from_file(changed):
                        # Resolve every listed path just as the checker does.
                        relative = scope.canonical_repo_path(root, value)
                        resolved_paths[value] = relative
                        # Ordinary scope checks depend on the canonical path,
                        # not file contents. Only mixed Rust validation reads it.
                        if (
                            options.get("--stage") == "testing"
                            and relative.endswith(".rs")
                            and not scope.allowed_for_stage(
                                relative, "testing", options.get("--version"),
                                options.get("--module"), options.get("--submodule"),
                            )
                        ):
                            paths.add(root / relative)
                elif argument == "--baseline-manifest":
                    baseline = safe_repo_path(root, command[index + 1], "baseline")
                    paths.add(baseline)
                    data = read_json(baseline)
                    for record in data["files"]:
                        paths.add(baseline.parent / record["snapshot"])

        def fingerprint(path: Path) -> tuple[str, str, str, str | None]:
            resolved = path.resolve()
            relative = resolved.relative_to(root).as_posix()
            digest = content_digest(path) if path.is_file() else None
            kind = "file" if path.is_file() else "directory" if path.is_dir() else "other" if path.exists() else "missing"
            return str(path), relative, kind, digest

        # Include imported helper versions and membership, not only the driver.
        # This also binds reuse to this cache implementation and Python runtime.
        versions = checker_versions if checker_versions is not None else {}
        if script.parent not in versions:
            versions[script.parent] = {
                str(path): content_digest(path)
                for path in sorted({*script.parent.glob("*.py"), Path(__file__)})
            }
        payload = {
            "schema": 1,
            "command": command,
            "python": sys.version,
            "inputs": [fingerprint(path) for path in sorted(paths)],
            "resolved_paths": resolved_paths,
            "checkers": versions[script.parent],
        }
        return hashlib.sha256(json.dumps(payload, sort_keys=True).encode("utf-8")).hexdigest()
    except (OSError, ValueError, KeyError, TypeError, SystemExit):
        return None


def run_cached(
    root: Path, manifest: Path, task: dict[str, object], command: list[str], *, dry_run: bool,
    checker_versions: dict[Path, dict[str, str]] | None = None,
    timings: list[dict[str, object]] | None = None,
) -> bool:
    event: dict[str, object] = {
        "command": command, "kind": "routing" if Path(command[1]).name in {"context.py", "context.template.py"} else "checker",
        "cache": "dry-run" if dry_run else "bypass", "success": False,
        "fingerprint_s": 0.0, "cache_lookup_s": 0.0, "execution_s": 0.0,
    }
    started = time.perf_counter()
    try:
        with validation_content():
            success = _run_cached(root, manifest, task, command, dry_run=dry_run,
                                  checker_versions=checker_versions, event=event)
        event["success"] = success
        return success
    finally:
        event["duration_s"] = time.perf_counter() - started
        if timings is not None:
            timings.append(event)


def _run_cached(
    root: Path, manifest: Path, task: dict[str, object], command: list[str], *, dry_run: bool,
    checker_versions: dict[Path, dict[str, str]] | None,
    event: dict[str, object],
) -> bool:
    if dry_run:
        return run(command, dry_run=True)

    def key() -> str | None:
        started = time.perf_counter()
        try:
            with contextlib.redirect_stderr(io.StringIO()):
                return checker_cache_key(root, manifest, task, command, checker_versions=checker_versions)
        finally:
            event["fingerprint_s"] += time.perf_counter() - started

    def execute() -> bool:
        started = time.perf_counter()
        try:
            return run(command, dry_run=False)
        finally:
            event["execution_s"] += time.perf_counter() - started
            # Share versions only across read-only fingerprinting/cache hits.
            # Any executed command is a boundary requiring fresh content hashes.
            if checker_versions is not None:
                checker_versions.clear()
            invalidate_validation_content()

    before = key()
    if before is None:
        return execute()
    command_key = hashlib.sha256(json.dumps(command).encode("utf-8")).hexdigest()
    cache = root / ".harness" / "checker-cache" / f"{command_key}.json"
    event["cache"] = "miss"
    lookup_started = time.perf_counter()
    try:
        if json.loads(cache.read_text(encoding="utf-8")) == {"schema": 1, "success_key": before}:
            event["cache"] = "hit"
            print("harness-check: reused successful check: " + " ".join(command))
            return True
    except (OSError, ValueError):
        pass
    finally:
        event["cache_lookup_s"] += time.perf_counter() - lookup_started
    # Never keep a prior success when a real rerun fails or changes its inputs.
    try:
        cache.unlink(missing_ok=True)
    except OSError:
        pass
    success = execute()
    if success and key() == before:
        temporary = None
        try:
            cache.parent.mkdir(parents=True, exist_ok=True)
            with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=cache.parent, delete=False) as stream:
                temporary = Path(stream.name)
                json.dump({"schema": 1, "success_key": before}, stream)
            temporary.replace(cache)
        except OSError:
            pass  # Reconstructable cache storage must not block real checks.
        finally:
            if temporary is not None:
                with contextlib.suppress(OSError):
                    temporary.unlink(missing_ok=True)
    return success


def identity_args(task: dict[str, object]) -> list[str]:
    return [
        "--version", str(task["version"]),
        "--module", str(task["packet_module"]),
        "--submodule", str(task["task_name"]),
    ]


def target_groups(task: dict[str, object]) -> dict[str, list[dict[str, object]]]:
    groups: dict[str, list[dict[str, object]]] = {}
    for change in task["changes"]:
        assert isinstance(change, dict)
        groups.setdefault(str(change["target_module"]), []).append(change)
    return groups


def context_changed_paths(task: dict[str, object]) -> list[str]:
    """Return declared concrete/glob scopes used by path-routed rules."""
    values: list[str] = []
    for change in task["changes"]:
        assert isinstance(change, dict)
        values.extend(str(value) for value in change.get("scope_paths", []))
    return list(dict.fromkeys(values))


def context_changed_path_manifests(root: Path, task: dict[str, object]) -> list[str]:
    """Return existing changed-path manifests; pre-edit manifests may not exist yet."""
    values: list[str] = []
    top_level = task.get("changed_paths_file")
    if top_level not in {None, ""}:
        values.append(str(top_level))
    for change in task["changes"]:
        assert isinstance(change, dict)
        value = change.get("changed_paths_file")
        if value not in {None, ""}:
            values.append(str(value))
    result: list[str] = []
    for value in dict.fromkeys(values):
        if safe_repo_path(root, value, "changed_paths_file").is_file():
            result.append(value)
    return result


def changed_manifest_contains(root: Path, task: dict[str, object], relative: str) -> bool:
    value = task.get("changed_paths_file")
    if value in {None, ""}:
        return False
    path = safe_repo_path(root, str(value), "changed_paths_file")
    if not path.is_file():
        return False
    expected = Path(relative).as_posix()
    for raw in path.read_text(encoding="utf-8").splitlines():
        candidate = raw.strip()
        if candidate and not candidate.startswith("#") and Path(candidate).as_posix() == expected:
            return True
    return False


def context_evidence_paths(root: Path, task: dict[str, object]) -> list[str]:
    """Return existing task-owned runtime evidence for router output."""
    candidates: list[str] = []
    for manifest in context_changed_path_manifests(root, task):
        candidates.append(manifest)
        sidecar = manifest + ".meta.json"
        if safe_repo_path(root, sidecar, "stage-scope sidecar").is_file():
            candidates.append(sidecar)
    baseline = task.get("baseline_manifest")
    if baseline not in {None, ""}:
        candidates.append(str(baseline))
    hybrid_baseline = hybrid_baseline_relative(task)
    if safe_repo_path(root, hybrid_baseline, "repository baseline").is_file():
        candidates.append(hybrid_baseline)

    scope = f"{task['packet_module']}/{task['task_name']}"
    testplan = (task_packet(root, task) / str(task.get("testplan") or "testplan.yaml")).relative_to(root).as_posix()
    run_dir = test_run_directory(root, scope, [testplan])
    if not run_dir.is_dir():
        run_dir = root / ".harness" / "test-results" / "test-runs"
    if run_dir.is_dir():
        for artifact in sorted(run_dir.glob("*.json")):
            try:
                payload = json.loads(artifact.read_text(encoding="utf-8"))
            except (json.JSONDecodeError, UnicodeDecodeError, OSError):
                continue
            requested = payload.get("requested_module") if isinstance(payload, dict) else None
            if requested == scope and testplan in (payload.get("testplans") or []):
                candidates.append(artifact.relative_to(root).as_posix())

    result: list[str] = []
    for value in dict.fromkeys(candidates):
        path = safe_repo_path(root, value, "task evidence")
        if path.is_file():
            result.append(path.relative_to(root.resolve()).as_posix())
    return result


def normalize_column(value: str) -> str:
    return re.sub(r"[^a-z0-9]+", "_", value.strip().lower()).strip("_")


def table_cells(line: str) -> list[str]:
    cells = [cell.strip() for cell in line.strip().split("|")]
    if cells and not cells[0]:
        cells = cells[1:]
    if cells and not cells[-1]:
        cells = cells[:-1]
    return cells


def scope_entries(value: str) -> list[str]:
    entries = [match.group(1) for match in re.finditer(r"`([^`]+)`", value)]
    if not entries:
        entries = value.split(",")
    return sorted({entry.strip().strip("`").replace("\\", "/").rstrip("/") for entry in entries if entry.strip()})


def binding_rows(path: Path, section: str) -> list[dict[str, str]]:
    if not path.is_file():
        fail(f"missing scope binding source: {path}")
    text = path.read_text(encoding="utf-8")
    heading = re.search(rf"(?m)^##\s+{re.escape(section)}\s*$", text)
    if not heading:
        fail(f"{path} missing required section: ## {section}")
    lines = text[heading.end():].splitlines()
    table_index = next((i for i, line in enumerate(lines) if line.lstrip().startswith("|") and i + 1 < len(lines) and re.search(r"-{3,}", lines[i + 1])), None)
    if table_index is None:
        fail(f"{path} ## {section} missing table")
    headers = [normalize_column(cell) for cell in table_cells(lines[table_index])]
    rows: list[dict[str, str]] = []
    for line in lines[table_index + 2:]:
        if not line.strip() or not line.lstrip().startswith("|"):
            break
        values = table_cells(line)
        rows.append({header: values[index] if index < len(values) else "" for index, header in enumerate(headers)})
    return rows


def validate_scope_bindings(manifest: Path, task: dict[str, object]) -> None:
    value = task.get("design")
    if not value:
        fail("task requires design")
    source = manifest.parent / str(value)
    section = "Directly Mapped Change Items"
    rows = binding_rows(source, section)
    for change in task["changes"]:
        assert isinstance(change, dict)
        matches = [
            row for row in rows
            if row.get("change_id") == change["id"]
            and row.get("target_module") == change["target_module"]
        ]
        if len(matches) != 1:
            fail(
                f"task change {change['id']} target {change['target_module']} must have exactly "
                f"one matching row in {source} ## {section}"
            )
        # Scope Paths are descriptive traceability metadata, not project-file
        # permissions. Their textual drift does not block lifecycle checks.


def change_args(changes: list[dict[str, object]]) -> list[str]:
    result: list[str] = []
    for change in changes:
        result.extend(["--change-id", str(change["id"])])
    return result


def common_target_value(task: dict[str, object], changes: list[dict[str, object]], field: str) -> str | None:
    values = {str(change[field]) for change in changes if change.get(field) not in {None, ""}}
    if len(values) > 1:
        fail(f"changes for one target must use one {field}: {sorted(values)}")
    if values:
        return values.pop()
    value = task.get(field)
    return None if value in {None, ""} else str(value)


def write_sidecar(root: Path, manifest_value: str, payload: dict[str, object]) -> None:
    manifest = safe_repo_path(root, manifest_value, "changed_paths_file")
    if not manifest.is_file():
        fail(f"changed_paths_file does not exist: {manifest}")
    sidecar = manifest.with_name(manifest.name + ".meta.json")
    if manifest_value == ".harness/evidence/v0.1/stage-scope/039-harness-full-upgrade-implementation.paths" and (payload.get("stage"), payload.get("version"), payload.get("module"), payload.get("submodule")) == ("implementation", "v0.1", "repo-governance", "039-harness-full-upgrade"):
        payload = {
            **payload,
            "classification_amendment": "D-039-001",
            "authorization_record": ".harness/upgrade-039/governance-implementation-authorization.json",
            "original_inventory": ".harness/upgrade-039/originals-manifest.json",
            "migration_mapping": ".harness/upgrade-039/migration-journal.json",
            "classification_reason": "Pinned complete manifest; governance scripts, production templates/instructions, AGENTS and navigation, and hash-verified legacy removals.",
        }
    sidecar.parent.mkdir(parents=True, exist_ok=True)
    sidecar.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def path_in_declared_scope(path: str, scopes: list[str]) -> bool:
    for scope in scopes:
        normalized = scope.replace("\\", "/").rstrip("/")
        if any(token in normalized for token in ("*", "?", "[", "]")):
            if fnmatch.fnmatchcase(path, normalized):
                return True
        elif path == normalized or path.startswith(normalized + "/"):
            return True
    return False


def write_paths(root: Path, value: str, paths: list[str]) -> None:
    destination = safe_repo_path(root, value, "changed_paths_file")
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text("".join(f"{path}\n" for path in sorted(set(paths))), encoding="utf-8")


def partition_implementation_paths(root: Path, task: dict[str, object]) -> None:
    full_value = task.get("changed_paths_file")
    if full_value in {None, ""}:
        fail("implementation completion requires top-level changed_paths_file")
    full_path = safe_repo_path(root, str(full_value), "changed_paths_file")
    if not full_path.is_file():
        fail(f"changed_paths_file does not exist: {full_path}")
    changed = [
        line.strip().replace("\\", "/")
        for line in full_path.read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    ]
    groups = target_groups(task)
    outputs: dict[str, str] = {}
    scopes_by_target: dict[str, list[str]] = {}
    for target, changes in groups.items():
        output = common_target_value(task, changes, "changed_paths_file")
        if not output:
            fail(f"implementation completion requires changed_paths_file for target {target}")
        if output in outputs and outputs[output] != target:
            fail("cross-target implementation requires one changed_paths_file per target")
        outputs[output] = target
        scopes_by_target[target] = [
            str(scope)
            for change in changes
            for scope in change.get("scope_paths", [])
        ]

    unassigned = [
        path for path in changed
        if not any(path_in_declared_scope(path, scopes) for scopes in scopes_by_target.values())
    ]
    for output, target in outputs.items():
        if output == str(full_value) and len(outputs) == 1:
            continue
        selected = [
            path for path in changed
            if path_in_declared_scope(path, scopes_by_target[target])
        ]
        # Out-of-scope facts are copied to every target manifest so at least one
        # mechanical scope check reports them instead of silently dropping them.
        write_paths(root, output, selected + unassigned)


def materialize_stage_scope_sidecars(root: Path, task: dict[str, object]) -> None:
    if task["stage"] == "implementation":
        partition_implementation_paths(root, task)
    baseline = None
    if task["stage"] == "testing":
        candidate = testing_baseline_relative(task)
        if safe_repo_path(root, candidate, "testing baseline manifest").is_file():
            baseline = candidate
    base = {
        "schema": 1,
        "stage": task["stage"],
        "version": task["version"],
        "module": task["packet_module"],
        "submodule": task["task_name"],
        "baseline_manifest": baseline,
    }
    if task["stage"] == "implementation":
        for target, changes in target_groups(task).items():
            paths = common_target_value(task, changes, "changed_paths_file")
            if not paths:
                fail(f"implementation completion requires changed_paths_file for target {target}")
            write_sidecar(root, paths, {
                **base,
                "target_module": target,
                "change_ids": sorted(str(change["id"]) for change in changes),
            })
    else:
        paths = task.get("changed_paths_file")
        if not paths:
            fail(f"{task['stage']} completion requires changed_paths_file")
        write_sidecar(root, str(paths), {
            **base,
            "target_module": task["packet_module"],
            "change_ids": [],
        })


def stage_scope_commands(root: Path, task: dict[str, object]) -> list[list[str]]:
    base = command_prefix(root, "stage-scope-check.py") + [
        "--stage", str(task["stage"]),
        *identity_args(task),
    ]
    if task["stage"] == "testing":
        baseline = testing_baseline_relative(task)
        if safe_repo_path(root, baseline, "testing baseline manifest").is_file():
            base += ["--baseline-manifest", baseline]
    if task["stage"] != "implementation":
        value = task.get("changed_paths_file")
        if value in {None, ""}:
            fail(f"{task['stage']} completion requires changed_paths_file")
        return [base + ["--changed-paths-file", str(value)]]

    commands: list[list[str]] = []
    for target, changes in target_groups(task).items():
        value = common_target_value(task, changes, "changed_paths_file")
        if not value:
            fail(f"implementation completion requires changed_paths_file for target {target}")
        commands.append(
            base
            + ["--target-module", target, "--changed-paths-file", value]
            + change_args(changes)
        )
    return commands


def build_commands(root: Path, manifest: Path, task: dict[str, object], profile: str) -> list[list[str]]:
    # Every tier has a canonical proposal packet, but only confirmed high-risk
    # work reaches this full lifecycle entrypoint.
    stage = str(task["stage"])
    identity = identity_args(task)
    commands: list[list[str]] = []
    schema = command_prefix(root, "schema-check.py") + identity
    approved_schema = schema + ["--require-approved"]
    approved_design_schema = approved_schema
    doc_structure = command_prefix(root, "doc-structure-check.py") + identity + ["--docs", stage]

    if profile == "pre-edit":
        packet_rel = manifest.parent.relative_to(root).as_posix()
        context = command_prefix(root, "context.py") + [
            "--workflow-tier", str(task["workflow_tier"]),
            "--stage", stage,
            "--packet", packet_rel,
            "--module", str(task["packet_module"]),
        ]
        profile = artifact_path(root, manifest, task, "risk_profile")
        triggers = sorted(risk_profile_triggers(profile)) if profile is not None else []
        for trigger in triggers:
            context += ["--trigger", trigger]
        for changed_path in context_changed_paths(task):
            context += ["--changed-path", changed_path]
            context += ["--scope-path", changed_path]
        for changed_paths_file in context_changed_path_manifests(root, task):
            context += ["--changed-paths-file", changed_paths_file]
        for evidence_path in context_evidence_paths(root, task):
            context += ["--evidence-path", evidence_path]
        commands.append(context)
        if stage in {"implementation", "testing"}:
            commands.append(command_prefix(root, "risk-profile-check.py") + ["--task", str(manifest)])
        if stage == "testing":
            commands.append(approved_design_schema)
        if stage == "implementation":
            commands.append(approved_design_schema)
        return commands

    if stage != "acceptance":
        commands.append(command_prefix(root, "risk-profile-check.py") + ["--task", str(manifest)])
    if stage == "proposal":
        proposal_schema = approved_schema
        commands.extend([proposal_schema, doc_structure])
    elif stage == "design":
        commands.append(schema)
        commands.append(doc_structure)
    elif stage == "implementation":
        commands.append(approved_design_schema)
    elif stage == "testing":
        commands.append(approved_design_schema)
        testing_doc = artifact_path(root, manifest, task, "testing")
        if (
            testing_doc is not None
            and testing_doc.is_file()
        ):
            commands.append(doc_structure)
        command = command_prefix(root, "testing-coverage-check.py") + identity
        command += change_args([change for changes in target_groups(task).values() for change in changes])
        commands.append(command)
    elif stage == "acceptance":
        commands.append([
            sys.executable,
            str(root / "harness" / "scripts" / "lifecycle-check.py"),
            "--root", str(root),
            "--task", str(manifest),
            "--require-prior", "acceptance",
        ])
        report = artifact_path(root, manifest, task, "acceptance_report", required=True)
        assert report is not None
        commands.append([sys.executable, str(root / "harness" / "scripts" / "acceptance-report-check.py"), str(report), "--root", str(root)])

    commands.extend(stage_scope_commands(root, task))
    return commands


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=".")
    parser.add_argument("--task", required=True, help="task packet task.yaml")
    parser.add_argument("--profile", required=True, choices=sorted(PROFILES))
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--timings-json", help="optional timing report path relative to --root")
    args = parser.parse_args()

    started = time.perf_counter()
    timings: list[dict[str, object]] = []
    result = 1
    try:
        result = run_checks(args, timings)
        return result
    finally:
        summary = {
            "total_s": time.perf_counter() - started,
            "routing_s": sum(event["execution_s"] for event in timings if event["kind"] == "routing"),
            "checkers_s": sum(event["execution_s"] for event in timings if event["kind"] == "checker"),
            "fingerprint_s": sum(event["fingerprint_s"] for event in timings),
            "cache_lookup_s": sum(event["cache_lookup_s"] for event in timings),
            # Hit cost includes dependency fingerprinting, not just JSON lookup.
            "cache_hit_s": sum(event["duration_s"] for event in timings if event["cache"] == "hit"),
            "cache_hits": sum(event["cache"] == "hit" for event in timings),
            "cache_misses": sum(event["cache"] == "miss" for event in timings),
        }
        print("harness-check: timings " + json.dumps(summary, sort_keys=True), file=sys.stderr)
        if args.timings_json:
            report = {
                "schema": 1, "task": args.task, "profile": args.profile,
                "dry_run": args.dry_run, "exit_code": result,
                "summary": summary, "commands": timings,
            }
            destination = Path(args.timings_json)
            if not destination.is_absolute():
                destination = Path(args.root) / destination
            temporary = None
            try:
                destination.parent.mkdir(parents=True, exist_ok=True)
                with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=destination.parent, delete=False) as stream:
                    temporary = Path(stream.name)
                    json.dump(report, stream, indent=2)
                    stream.write("\n")
                temporary.replace(destination)
            except OSError as error:
                print(f"harness-check: cannot write timings: {error}", file=sys.stderr)
            finally:
                if temporary is not None:
                    with contextlib.suppress(OSError):
                        temporary.unlink(missing_ok=True)


def run_checks(args: argparse.Namespace, timings: list[dict[str, object]]) -> int:
    root = Path(args.root).resolve()
    manifest = Path(args.task)
    if not manifest.is_absolute():
        manifest = root / manifest
    manifest = manifest.resolve()
    task = parse_task(manifest)
    expected = (task_packet(root, task) / "task.yaml").resolve()
    if manifest != expected:
        fail(f"task manifest path must match its identity: expected {expected}, got {manifest}")
    if args.profile == "completion" and task["stage"] in {"design", "implementation", "testing"}:
        validate_scope_bindings(manifest, task)
    if args.profile == "pre-edit" and task["stage"] == "implementation":
        validate_scope_bindings(manifest, task)
    if args.profile == "completion" and not args.dry_run:
        materialize_stage_scope_sidecars(root, task)
    commands = build_commands(root, manifest, task, args.profile)
    checker_versions: dict[Path, dict[str, str]] = {}
    with validation_content():
        failures = sum(
            not run_cached(root, manifest, task, command, dry_run=args.dry_run,
                           checker_versions=checker_versions, timings=timings)
            for command in commands
        )
    if failures:
        print(f"harness-check: FAILED ({failures} command(s) failed)", file=sys.stderr)
        return 1
    command_count = len(commands)
    print(f"harness-check: passed ({args.profile}, {command_count} command(s))")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
