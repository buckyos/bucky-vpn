#!/usr/bin/env python3
"""Validate post-implementation testing coverage metadata for change_ids."""

from __future__ import annotations

import argparse
import ast
import json
import re
import subprocess
import sys
from pathlib import Path

from task_manifest import cached_parse, read_only_validation, read_text as shared_read_text


TABLE_SEPARATOR_RE = re.compile(r"^\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?\s*$")
CASE_TYPES = {"normal", "boundary", "negative", "error", "compatibility", "lifecycle", "cross-module"}
TEST_LEVELS = {"unit", "dv", "integration"}
EMPTY_VALUES = {"", "-", "n/a", "na", "none", "tbd", "todo", "pending"}
PUBLIC_API_IMPACTS = {"none", "backward-compatible", "migration-required", "breaking"}
CONTRACT_ASSERTIONS = {
    "external-positive": "new-path-compiles",
    "external-negative": "old-path-rejected-for-removed-symbol",
    "removed-symbol-scan": "no-unallowlisted-old-symbol-references",
    "repository-compile-closure": "repository-consumers-compile",
    "documentation-examples": "documentation-examples-compile",
}
MIGRATION_STATUSES = {"migrated", "allowed-negative-fixture", "allowed-compatibility-shim", "verified-none"}


def fail(message: str) -> None:
    print(f"testing-coverage-check: {message}", file=sys.stderr)
    raise SystemExit(1)


def read_text(path: Path) -> str:
    if not path.exists():
        fail(f"missing required file: {path}")
    return shared_read_text(path)


def normalize_column(value: str) -> str:
    return re.sub(r"[^a-z0-9]+", "_", value.strip().lower()).strip("_")


def split_table_row(line: str) -> list[str]:
    parts = [part.strip() for part in line.strip().split("|")]
    if parts and parts[0] == "":
        parts = parts[1:]
    if parts and parts[-1] == "":
        parts = parts[:-1]
    return parts


@cached_parse
def table_rows_after_heading(text: str, heading: str, path: Path) -> list[dict[str, str]]:
    match = re.search(rf"(?m)^##\s+{re.escape(heading)}\s*$", text)
    if not match:
        fail(f"{path} missing required section: ## {heading}")
    lines = text[match.end() :].splitlines()
    table_start = None
    for index, line in enumerate(lines):
        if re.match(r"^##\s+", line):
            break
        if "|" in line and index + 1 < len(lines) and TABLE_SEPARATOR_RE.match(lines[index + 1]):
            table_start = index
            break
    if table_start is None:
        fail(f"{path} section ## {heading} missing required table")
    headers = [normalize_column(cell) for cell in split_table_row(lines[table_start])]
    rows: list[dict[str, str]] = []
    for line in lines[table_start + 2 :]:
        if not line.strip() or not line.lstrip().startswith("|"):
            break
        values = split_table_row(line)
        rows.append({header: values[pos].strip() if pos < len(values) else "" for pos, header in enumerate(headers)})
    if not rows:
        fail(f"{path} section ## {heading} has no data rows")
    return rows


def require_columns(path: Path, heading: str, rows: list[dict[str, str]], columns: tuple[str, ...]) -> None:
    available = set(rows[0])
    missing = [column for column in columns if column not in available]
    if missing:
        fail(f"{path} ## {heading} missing columns: {', '.join(missing)}")


def packet_path(root: Path, version: str, module: str, submodule: str | None) -> Path:
    packet = root / ".harness" / "tasks" / version / "modules" / module
    if submodule:
        packet = packet / submodule
    return packet


def has_value(value: str) -> bool:
    return value.strip().strip('"').strip("'").lower() not in EMPTY_VALUES


def change_ids_from_docs(packet: Path) -> set[str]:
    proposal = packet / "proposal.md"
    proposal_rows = table_rows_after_heading(read_text(proposal), "Proposal Items", proposal)
    require_columns(proposal, "Proposal Items", proposal_rows, ("proposal_id", "change_id", "requirement", "success_evidence"))
    proposal_ids = {row["change_id"] for row in proposal_rows if row.get("change_id")}
    design = packet / "design.md"
    design_rows = table_rows_after_heading(read_text(design), "Directly Mapped Change Items", design)
    require_columns(design, "Directly Mapped Change Items", design_rows, ("change_id", "target_module", "proposal_id", "design_coverage", "scope_paths"))
    design_ids = {row["change_id"] for row in design_rows if row.get("change_id")}
    missing_design = proposal_ids - design_ids
    if missing_design:
        fail(f"change_ids missing from design mapping: {', '.join(sorted(missing_design))}")
    return proposal_ids & design_ids


def parse_inline_list(value: str) -> list[str]:
    try:
        parsed = ast.literal_eval(value)
    except (SyntaxError, ValueError):
        return []
    if not isinstance(parsed, list):
        return []
    return [item for item in parsed if isinstance(item, str)]


def impact_value(text: str, label: str, path: Path) -> str:
    section = re.search(r"(?ms)^##\s+API and Build Surface Impact\s*$\n(.*?)(?=^##\s+|\Z)", text)
    if not section:
        fail(f"{path} missing required section: ## API and Build Surface Impact")
    match = re.search(rf"(?mi)^\s*-\s*{re.escape(label)}:\s*(.+)$", section.group(1))
    value = match.group(1).strip().lower() if match else ""
    if not value:
        fail(f"{path} API/build impact missing concrete {label}")
    return value


def design_api_contract(packet: Path) -> tuple[dict[str, object], list[dict[str, str]]]:
    source = packet / "design.md"
    text = read_text(source)
    public_api = impact_value(text, "Public API impact", source)
    if public_api not in PUBLIC_API_IMPACTS:
        fail(f"{source} has invalid Public API impact: {public_api}")
    flags: dict[str, bool] = {}
    for label, key in (
        ("Crate-root export change", "crate_root_export_change"),
        ("Build-surface change", "build_surface_change"),
        ("Documentation examples affected", "documentation_examples_affected"),
    ):
        value = impact_value(text, label, source)
        if value not in {"yes", "no"}:
            fail(f"{source} {label} must be yes or no")
        flags[key] = value == "yes"
    compatibilities = {
        value.strip().lower()
        for value in re.findall(r"(?im)^\s*-\s*Compatibility:\s*([^\n]+)$", text)
    }
    breaking = public_api == "breaking" or "breaking" in compatibilities
    migration = public_api == "migration-required" or "migration-required" in compatibilities
    risky = breaking or migration or any(flags.values())
    rows = table_rows_after_heading(text, "Consumer Migration Closure", source) if risky else []
    if risky:
        require_columns(
            source,
            "Consumer Migration Closure",
            rows,
            ("old_symbol", "new_path", "change_id", "consumer_path", "consumer_kind", "migration_status"),
        )
    required: set[str] = set()
    if breaking:
        required.update({"external-positive", "external-negative", "removed-symbol-scan", "repository-compile-closure"})
    elif migration:
        required.update({"external-positive", "removed-symbol-scan", "repository-compile-closure"})
    if flags["crate_root_export_change"]:
        required.update({"external-positive", "repository-compile-closure"})
    if flags["build_surface_change"]:
        required.add("repository-compile-closure")
    if flags["documentation_examples_affected"]:
        required.add("documentation-examples")
    return ({"public_api": public_api, "breaking": breaking, "migration": migration, "required": required, **flags}, rows)


def testplan_api_impact(text: str, path: Path) -> dict[str, object]:
    block = re.search(r"(?ms)^api_impact:\s*$\n(.*?)(?=^[A-Za-z0-9_-]+:\s*|\Z)", text)
    if not block:
        fail(f"{path} missing api_impact")
    public_match = re.search(r"(?m)^  public_api:\s*([A-Za-z0-9_-]+)\s*$", block.group(1))
    result: dict[str, object] = {"public_api": public_match.group(1) if public_match else ""}
    for key in ("crate_root_export_change", "build_surface_change", "documentation_examples_affected"):
        match = re.search(rf"(?m)^  {key}:\s*(true|false)\s*$", block.group(1))
        if not match:
            fail(f"{path} api_impact.{key} must be true or false")
        result[key] = match.group(1) == "true"
    return result


def contract_steps(text: str, path: Path) -> list[dict[str, object]]:
    block = re.search(r"(?ms)^contract_checks:\s*$\n(.*?)(?=^[A-Za-z0-9_-]+:\s*|\Z)", text)
    if not block:
        fail(f"{path} missing contract_checks")
    starts = list(re.finditer(r"(?m)^    - id:\s*([A-Za-z0-9_.-]+)\s*$", block.group(1)))
    rows: list[dict[str, object]] = []
    for index, start in enumerate(starts):
        end = starts[index + 1].start() if index + 1 < len(starts) else len(block.group(1))
        body = block.group(1)[start.end() : end]
        kind = re.search(r"(?m)^      kind:\s*([A-Za-z0-9_-]+)\s*$", body)
        assertion = re.search(r"(?m)^      assertion:\s*([A-Za-z0-9_-]+)\s*$", body)
        changes = re.search(r"(?m)^      change_ids:\s*(\[[^\n]*\])\s*$", body)
        run = re.search(r"(?m)^      run:\s*(\[[^\n]*\])\s*$", body)
        rows.append(
            {
                "id": start.group(1),
                "kind": kind.group(1) if kind else "",
                "assertion": assertion.group(1) if assertion else "",
                "change_ids": parse_inline_list(changes.group(1)) if changes else [],
                "run": parse_inline_list(run.group(1)) if run else [],
            }
        )
    return rows


def path_covered_by_inputs(path: str, inputs: list[str]) -> bool:
    path = re.split(r"[*?\[]", path, maxsplit=1)[0].rstrip("/")
    target = Path(path)
    return any(target == Path(item) or Path(item) in target.parents for item in inputs)


def design_scope_paths(packet: Path, requested: set[str]) -> set[str]:
    source = packet / "design.md"
    heading = "Directly Mapped Change Items"
    rows = table_rows_after_heading(read_text(source), heading, source)
    result: set[str] = set()
    for row in rows:
        if row.get("change_id", "").strip() not in requested:
            continue
        raw = row.get("scope_paths", "")
        for value in re.split(r"\s*(?:,|<br\s*/?>)\s*", raw):
            cleaned = value.strip().strip("`")
            if cleaned:
                result.add(cleaned)
    return result


def concrete_file(root: Path, value: str, label: str) -> str:
    value = value.strip().strip("`").replace("\\", "/")
    path = Path(value)
    if not value or path.is_absolute() or re.search(r"[*?\[]", value) or ".." in path.parts:
        fail(f"{label} must be a concrete repository-relative file: {value}")
    resolved = (root / path).resolve()
    if not resolved.is_relative_to(root.resolve()) or not resolved.is_file():
        fail(f"{label} is not a current repository file: {value}")
    return resolved.relative_to(root.resolve()).as_posix()


def concrete_contract_inputs(
    root: Path, packet: Path, requested: set[str], consumers: list[dict[str, str]],
) -> set[str]:
    result: set[str] = set()
    for index, row in enumerate(consumers, start=1):
        status = row.get("migration_status", "").strip().lower()
        if status not in MIGRATION_STATUSES:
            fail(f"consumer migration row {index} has invalid or incomplete status: {status}")
        if row.get("change_id", "").strip() not in requested:
            fail(f"consumer migration row uses change_id outside requested coverage: {row.get('change_id', '')}")
        if status != "verified-none":
            result.add(concrete_file(root, row.get("consumer_path", ""), "consumer migration path"))
    source = packet / "design.md"
    text = read_text(source)
    for heading, column in (("File-Level Implementation Sequence", "file_level_module"),
                            ("API Closure Evidence Inputs", "path")):
        if not re.search(rf"(?m)^##\s+{re.escape(heading)}\s*$", text):
            continue
        rows = table_rows_after_heading(text, heading, source)
        require_columns(source, heading, rows, ("change_id", column))
        for row in rows:
            changes = {item.strip().strip("`") for item in row["change_id"].split(",")}
            if not changes & requested:
                continue
            if heading == "File-Level Implementation Sequence" and row.get("action", "").strip().lower() in {"retired", "delete", "deleted", "remove", "removed"}:
                continue
            result.add(concrete_file(root, row[column], heading))
    return result


def contract_language(root: Path, impact: dict[str, object], concrete_inputs: set[str]) -> str:
    languages: set[str] = set()
    passive = {".md", ".json", ".yaml", ".yml", ".bat", ".sh"}
    for value in sorted(concrete_inputs):
        path = Path(value)
        if path.suffix == ".py":
            languages.add("python")
        elif path.suffix == ".rs" or path.name in {"Cargo.toml", "Cargo.lock"}:
            languages.add("rust")
        elif path.suffix not in passive:
            fail(f"unsupported API/build closure source: {value}; explicit bounded closure required")
    if impact.get("crate_root_export_change"):
        languages.add("rust")
    if len(languages) != 1:
        fail(f"API/build closure has mixed or unknown language impact: {', '.join(sorted(languages)) or 'unknown'}")
    return next(iter(languages))


def check_language_command(
    root: Path, kind: str, command: list[str], language: str,
    concrete_inputs: set[str], evidence_inputs: list[str],
) -> None:
    if kind not in {"repository-compile-closure", "documentation-examples"}:
        return
    if language == "rust":
        if kind == "repository-compile-closure":
            if command[:2] != ["cargo", "test"] or "--no-run" not in command or "--all-targets" not in command:
                fail("Rust repository compile closure must use cargo test --no-run --all-targets")
        elif command[:2] != ["cargo", "test"] or "--doc" not in command or "--no-run" in command:
            fail("Rust documentation example closure must use cargo test --doc")
        return
    executable = Path(command[0]).name.lower()
    if not re.fullmatch(r"python(?:\d+(?:\.\d+)*)?(?:\.exe)?", executable):
        fail("Python closure must invoke a Python interpreter")
    if kind == "repository-compile-closure":
        if command[1:3] != ["-m", "compileall"]:
            fail("Python repository compile closure must use python -m compileall")
        # Only bounded compileall options are accepted; exclusions/limits can omit affected files.
        targets = [item for item in command[3:] if item not in {"-q", "-qq", "-f"}]
        if not targets or any(item.startswith("-") for item in targets):
            fail("Python compile closure requires concrete targets without exclusions or depth limits")
        sources = {value for value in concrete_inputs if Path(value).suffix == ".py"}
        covered: set[str] = set()
        for value in targets:
            path = Path(value)
            resolved = (root / path).resolve()
            if path.is_absolute() or ".." in path.parts or not resolved.is_relative_to(root.resolve()) or not resolved.exists():
                fail(f"Python compile target must be a current repository path: {value}")
            matches = {source for source in sources if (root / source).resolve() == resolved or
                       (resolved.is_dir() and (root / source).resolve().is_relative_to(resolved))}
            if not matches:
                fail(f"Python compile target does not cover affected sources: {value}")
            covered.update(matches)
        if covered != sources:
            fail(f"Python compile closure omits affected sources: {', '.join(sorted(sources - covered))}")
        return
    if len(command) < 3 or "--documentation-examples" not in command[2:]:
        fail("Python documentation closure requires a local wrapper with --documentation-examples")
    wrapper = concrete_file(root, command[1], "documentation example wrapper")
    if Path(wrapper).suffix != ".py" or not path_covered_by_inputs(wrapper, evidence_inputs):
        fail(f"Python documentation wrapper must be a fingerprinted Python file: {wrapper}")
    try:
        tree = ast.parse(read_text(root / wrapper))
    except SyntaxError:
        fail(f"Python documentation wrapper cannot be parsed: {wrapper}")
    modes = {node.value for node in ast.walk(tree) if isinstance(node, ast.Constant) and isinstance(node.value, str)}
    if "--documentation-examples" not in modes:
        fail(f"Python documentation wrapper does not implement --documentation-examples: {wrapper}")
    modules = {alias.asname or alias.name for node in ast.walk(tree) if isinstance(node, ast.Import)
               for alias in node.names if alias.name == "subprocess"}
    functions = {alias.asname or alias.name: alias.name for node in ast.walk(tree)
                 if isinstance(node, ast.ImportFrom) and node.module == "subprocess" for alias in node.names}
    verifying_calls = []
    for node in ast.walk(tree):
        if not isinstance(node, ast.Call):
            continue
        name = ""
        if isinstance(node.func, ast.Attribute) and isinstance(node.func.value, ast.Name) and node.func.value.id in modules:
            name = node.func.attr
        elif isinstance(node.func, ast.Name):
            name = functions.get(node.func.id, "")
        if name in {"check_call", "check_output"} or (name == "run" and any(
            keyword.arg == "check" and isinstance(keyword.value, ast.Constant) and keyword.value.value is True
            for keyword in node.keywords
        )):
            verifying_calls.append(node)
    if not verifying_calls:
        fail(f"Python documentation wrapper must execute CLI examples and propagate failures: {wrapper}")


def check_api_contract_closure(root: Path, packet: Path, requested: set[str]) -> None:
    impact, consumers = design_api_contract(packet)
    required = set(impact["required"])
    testplan = packet / "testplan.yaml"
    if not testplan.is_file():
        if required:
            fail(
                f"{testplan} is required for API/build-surface contract checks: "
                + ", ".join(sorted(required))
            )
        return
    text = read_text(testplan)
    recorded = testplan_api_impact(text, testplan)
    for key in ("public_api", "crate_root_export_change", "build_surface_change", "documentation_examples_affected"):
        if recorded.get(key) != impact.get(key):
            fail(f"{testplan} api_impact.{key} does not match design evidence")
    if not required:
        return
    inputs_match = re.search(r"(?m)^evidence_inputs:\s*(\[[^\n]*\])\s*$", text)
    inputs = parse_inline_list(inputs_match.group(1)) if inputs_match else []
    if not inputs:
        fail(f"{testplan} risk-triggered contract checks require evidence_inputs")
    for value in inputs:
        path = Path(value)
        resolved = (root / path).resolve()
        if path.is_absolute() or ".." in path.parts or re.search(r"[*?\[]", value) or not resolved.is_relative_to(root.resolve()) or not resolved.exists():
            fail(f"contract evidence input must be a current repository file or source directory: {value}")
        relative = resolved.relative_to(root.resolve()).as_posix()
        if (resolved.is_dir() and (relative == "." or relative == ".harness" or relative.startswith(".harness/"))) or (
            relative.startswith(".harness/") and path.name in {"task.yaml", "lifecycle.json", "tasks.json"}
        ) or relative.startswith(".harness/test-results/"):
            fail(f"contract evidence input cannot bind mutable task/runtime trees: {value}")
    concrete_inputs = concrete_contract_inputs(root, packet, requested, consumers)
    language = contract_language(root, impact, concrete_inputs)
    for value in sorted(concrete_inputs):
        if not path_covered_by_inputs(value, inputs):
            fail(f"current API/build closure input is not bound by testplan evidence_inputs: {value}")
    for index, row in enumerate(consumers, start=1):
        status = row.get("migration_status", "").strip().lower()
        if status not in MIGRATION_STATUSES:
            fail(f"consumer migration row {index} has invalid or incomplete status: {status}")
        if impact["breaking"] and status == "allowed-compatibility-shim":
            fail("breaking API consumer closure cannot retain an allowed-compatibility-shim")
        consumer = row.get("consumer_path", "").strip().strip("`")
        if status != "verified-none" and not path_covered_by_inputs(consumer, inputs):
            fail(f"consumer migration path is not bound by testplan evidence_inputs: {consumer}")
        change_id = row.get("change_id", "").strip()
        if change_id not in requested:
            fail(f"consumer migration row uses change_id outside requested coverage: {change_id}")
    steps = contract_steps(text, testplan)
    by_kind = {str(row["kind"]): row for row in steps}
    missing = required - set(by_kind)
    if missing:
        fail(f"{testplan} missing required contract check kinds: {', '.join(sorted(missing))}")
    for kind in sorted(required):
        row = by_kind[kind]
        if row["assertion"] != CONTRACT_ASSERTIONS[kind]:
            fail(f"{testplan} contract check {kind} has mismatched assertion")
        if not requested <= set(row["change_ids"]):
            fail(f"{testplan} contract check {kind} does not cover every requested change_id")
        command = row["run"]
        if not isinstance(command, list) or not command or command[0] in {"true", "echo", "rg"}:
            fail(f"{testplan} contract check {kind} must use a real verifying wrapper/command")
        if kind == "removed-symbol-scan" and not any(
            str(item).replace("\\", "/").endswith("harness/scripts/consumer-closure-check.py")
            for item in command
        ):
            fail("removed-symbol-scan must invoke harness/scripts/consumer-closure-check.py")
        check_language_command(root, kind, command, language, concrete_inputs, inputs)


@cached_parse
def extract_level_blocks(text: str) -> dict[str, str]:
    match = re.search(r"(?m)^levels:\s*$", text)
    if not match:
        return {}
    levels_text = text[match.end() :]
    next_section = re.search(r"(?m)^[A-Za-z0-9_-]+:", levels_text)
    if next_section:
        levels_text = levels_text[:next_section.start()]
    starts = list(re.finditer(r"(?m)^  ([A-Za-z0-9_-]+):\s*$", levels_text))
    blocks: dict[str, str] = {}
    for index, start in enumerate(starts):
        level = start.group(1)
        end = starts[index + 1].start() if index + 1 < len(starts) else len(levels_text)
        blocks[level] = levels_text[start.end() : end]
    return blocks


@cached_parse
def extract_steps(level_block: str) -> dict[str, str]:
    starts = list(re.finditer(r"(?m)^      - id:\s*([A-Za-z0-9_.-]+)\s*$", level_block))
    steps: dict[str, str] = {}
    for index, start in enumerate(starts):
        step_id = start.group(1)
        end = starts[index + 1].start() if index + 1 < len(starts) else len(level_block)
        steps[step_id] = level_block[start.start() : end]
    return steps


def yaml_list_contains(block: str, key: str, value: str) -> bool:
    inline = re.search(rf"(?m)^\s*{re.escape(key)}:\s*(\[[^\]]*\])\s*$", block)
    if inline:
        return value in parse_inline_list(inline.group(1))
    multiline = re.search(rf"(?ms)^\s*{re.escape(key)}:\s*\n((?:\s+-\s*[^\n]+\n?)+)", block)
    if multiline:
        values = [line.split("-", 1)[1].strip().strip("\"'") for line in multiline.group(1).splitlines()]
        return value in values
    return False


DESIGN_ELEMENT_TYPES = {"parameter-domain", "state-transition", "failure-path", "error-handling", "invariant", "concurrency"}
COVERAGE_STATUSES = {"covered", "gap", "manual", "disabled", "not-applicable"}
LIST_FIELDS = {"change_ids", "case_types", "element_types"}
COVERAGE_FIELDS = LIST_FIELDS | {"id", "design_source", "target", "level", "status", "test", "step_id", "assertion", "reason"}


def coverage_rows(packet: Path, *, allow_missing_testplan: bool = False) -> tuple[Path, list[dict[str, object]]]:
    """Read one authoritative coverage list; Markdown is only an explicit plan exception."""
    plan = packet / "testplan.yaml"
    if not plan.is_file():
        if not allow_missing_testplan:
            fail(f"missing required post-implementation test metadata: {plan}")
        source = packet / "testing.md"
        rows = table_rows_after_heading(read_text(source), "Coverage", source)
        converted: list[dict[str, object]] = []
        for row in rows:
            converted.append({key: coverage_list(value, source) if key in LIST_FIELDS else value for key, value in row.items()})
        return source, converted
    text = read_text(plan)
    blocks = list(re.finditer(r"(?ms)^coverage:\s*\n(.*?)(?=^[A-Za-z_][A-Za-z0-9_-]*:|\Z)", text))
    if len(blocks) != 1:
        fail(f"{plan} requires exactly one coverage list")
    rows: list[dict[str, object]] = []
    current: dict[str, object] | None = None
    for line in blocks[0].group(1).splitlines():
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        item = re.fullmatch(r"  - id:\s*(.+)", line)
        if item:
            current = {"id": coverage_scalar(item.group(1), plan)}
            rows.append(current)
            continue
        field = re.fullmatch(r"    ([a-z_]+):\s*(.*?)\s*", line)
        if current is None or not field:
            fail(f"{plan} malformed coverage entry: {line.strip()}")
        key, value = field.groups()
        if key not in COVERAGE_FIELDS or key in current:
            fail(f"{plan} unknown or duplicate coverage field: {key}")
        if key in LIST_FIELDS:
            current[key] = coverage_list(value, plan)
        else:
            current[key] = coverage_scalar(value, plan)
    if not rows:
        fail(f"{plan} coverage must contain cases or explicit gaps")
    return plan, rows


def coverage_list(value: str, path: Path) -> list[str]:
    try:
        parsed = ast.literal_eval(value)
    except (SyntaxError, ValueError):
        fail(f"{path} coverage lists require inline quoted strings")
    if not isinstance(parsed, list) or any(not isinstance(item, str) or not item.strip() for item in parsed):
        fail(f"{path} coverage lists require non-empty strings")
    return parsed


def coverage_scalar(value: str, path: Path) -> str:
    if value.startswith(('"', "'")):
        try:
            parsed = ast.literal_eval(value)
        except (SyntaxError, ValueError):
            fail(f"{path} invalid coverage scalar: {value}")
        if not isinstance(parsed, str):
            fail(f"{path} coverage scalar must be a string")
        return parsed
    return value


def check_coverage(
    packet: Path, requested: set[str], *, allow_missing_testplan: bool = False,
    known_changes: set[str] | None = None,
) -> None:
    source, rows = coverage_rows(packet, allow_missing_testplan=allow_missing_testplan)
    plan = packet / "testplan.yaml"
    levels = extract_level_blocks(read_text(plan)) if plan.is_file() else {}
    seen_ids: set[str] = set()
    covered_changes: set[str] = set()
    known_changes = requested if known_changes is None else known_changes
    for row in rows:
        label = str(row.get("id", ""))
        if not has_value(label) or label in seen_ids:
            fail(f"{source} missing or duplicate coverage id: {label}")
        seen_ids.add(label)
        for field in ("design_source", "target", "level", "status"):
            if not has_value(str(row.get(field, ""))):
                fail(f"{source} coverage {label} missing {field}")
        changes = row.get("change_ids", [])
        if not isinstance(changes, list) or not changes or not set(changes) <= known_changes:
            fail(f"{source} coverage {label} must bind known change_ids")
        status, level = str(row["status"]), str(row["level"])
        if status not in COVERAGE_STATUSES or level not in TEST_LEVELS:
            fail(f"{source} coverage {label} invalid status or level")
        for field, allowed in (("case_types", CASE_TYPES), ("element_types", DESIGN_ELEMENT_TYPES)):
            values = row.get(field, [])
            if not isinstance(values, list) or not set(values) <= allowed:
                fail(f"{source} coverage {label} invalid {field}")
        if not row.get("case_types") and not row.get("element_types"):
            fail(f"{source} coverage {label} requires case_types or element_types")
        if status != "not-applicable":
            covered_changes.update(set(changes) & requested)
        if status == "covered":
            for field in ("test", "assertion"):
                if not has_value(str(row.get(field, ""))):
                    fail(f"{source} covered case {label} missing {field}")
            if not plan.is_file():
                fail(f"{source} automated coverage {label} requires testplan.yaml; use manual/disabled/gap under an exception")
            if level not in levels:
                fail(f"{source} coverage {label} references missing testplan level {level}")
            block = levels[level]
            if not re.search(r"(?m)^    mode:\s*enabled\s*$", block):
                fail(f"{source} covered case {label} requires enabled level {level}")
            step_id = str(row.get("step_id", ""))
            step = extract_steps(block).get(step_id)
            if not step:
                fail(f"{source} coverage {label} references missing step {level}/{step_id}")
            if not re.search(r"(?m)^        run:\s*\[.+\]\s*$", step):
                fail(f"{source} coverage {label} step must have a run command")
            if any(not yaml_list_contains(step, "change_ids", change_id) for change_id in changes):
                fail(f"{source} coverage {label} step does not bind its change_ids")
        else:
            reason = str(row.get("reason", ""))
            # Level mode/reason is authoritative for a manual or disabled layer.
            if not has_value(reason) and status in {"manual", "disabled"} and level in levels:
                block = levels[level]
                mode = re.search(r"(?m)^    mode:\s*(\w+)\s*$", block)
                match = re.search(r"(?m)^    reason:\s*(.+)$", block)
                if mode and mode.group(1) == status and match:
                    reason = match.group(1)
            if not has_value(reason):
                fail(f"{source} coverage {label} {status} requires a concrete reason")
    missing_changes = requested - covered_changes
    if missing_changes:
        fail(f"{source} changes need direct cases or explicit gaps: {sorted(missing_changes)}")
    # Tags describe applicable evidence, not a required Cartesian product.
    # Critical-path adequacy is reviewed against design/code in acceptance;
    # runnable bindings, assertions and explicit gap reasons remain mandatory.


def run_test_runner_dry_run(root: Path, module_key: str, task: Path) -> None:
    test_runner = root / "harness" / "scripts" / "test-run.py"
    if not test_runner.exists():
        fail(f"missing unified test runner: {test_runner}")
    completed = subprocess.run(
        [sys.executable, str(test_runner), module_key, "all", "--root", str(root),
         "--task", str(task.resolve()), "--dry-run"],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        fail(f"unified test runner cannot reach {module_key} all: {detail}")


def task_scope_from_testplan(packet: Path, module: str) -> str:
    testplan = packet / "testplan.yaml"
    if not testplan.exists():
        return module
    if not re.search(r"(?m)^task_manifest:\s*task\.yaml\s*$", read_text(testplan)):
        fail(f"testplan must use task_manifest: task.yaml: {testplan}")
    if not (packet / "task.yaml").is_file():
        fail(f"testplan references missing task manifest: {packet / 'task.yaml'}")
    return f"{module}/{packet.name}"


@read_only_validation
def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=".")
    parser.add_argument("--version", required=True)
    parser.add_argument("--module", required=True)
    parser.add_argument("--submodule")
    parser.add_argument("--change-id", action="append", dest="change_ids")
    parser.add_argument(
        "--allow-missing-testplan",
        action="store_true",
        help="allow repositories with an explicit local rule to complete testing without testplan.yaml",
    )
    parser.add_argument("--skip-test-run-check", action="store_true")
    args = parser.parse_args()

    root = Path(args.root)
    packet = packet_path(root, args.version, args.module, args.submodule)
    if args.allow_missing_testplan and not (packet / "testplan.yaml").exists():
        print("testing-coverage-check: warning: testplan.yaml missing by explicit local exception", file=sys.stderr)
    doc_change_ids = change_ids_from_docs(packet)
    requested = set(args.change_ids or doc_change_ids)
    unknown = requested - doc_change_ids
    if unknown:
        fail(f"requested change_ids are not directly mapped by proposal/design: {', '.join(sorted(unknown))}")

    check_coverage(packet, requested, allow_missing_testplan=args.allow_missing_testplan, known_changes=doc_change_ids)
    check_api_contract_closure(root, packet, requested)

    if not args.skip_test_run_check and (packet / "testplan.yaml").is_file():
        module_key = (
            f"{args.module}/{args.submodule}"
            if args.submodule
            else task_scope_from_testplan(packet, args.module)
        )
        run_test_runner_dry_run(root, module_key, packet / "task.yaml")

    print("testing-coverage-check: passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
