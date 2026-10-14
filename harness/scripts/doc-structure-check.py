#!/usr/bin/env python3
"""Validate Harness Engineering module packet document structure."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

from task_manifest import TaskManifestError, parse_task_manifest


MAX_PROPOSAL_LINES = 1000
TABLE_SEPARATOR_RE = re.compile(r"^\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?\s*$")
EMPTY_VALUES = {"", "-", "n/a", "na", "none", "tbd", "todo"}

REQUIRED_SECTIONS = {
    "proposal.md": (
        "Background and Goal",
        "Scope",
        "Requirement Review",
        "Proposal Items",
        "Success Criteria",
        "Risks",
    ),
    "design.md": (
        "Module Dependencies",
        "File-Level Interfaces",
        "Key Flows",
        "State and Ownership",
        "Directly Mapped Change Items",
        "File-Level Implementation Sequence",
    ),
    "testing.md": (),
}

REQUIRED_TABLE_COLUMNS = {
    ("proposal.md", "Proposal Items"): (
        "proposal_id",
        "change_id",
        "requirement",
        "boundary",
        "tradeoff",
        "success_evidence",
        "non_goal",
    ),
    ("design.md", "Directly Mapped Change Items"): (
        "change_id",
        "target_module",
        "proposal_id",
        "design_coverage",
        "scope_paths",
    ),
    ("design.md", "Module Dependencies"): ("level", "parent", "node", "depends_on"),
    ("design.md", "File-Level Implementation Sequence"): (
        "sequence", "file_level_module", "action", "depends_on", "change_id",
    ),
}

COMPATIBILITY_VALUES = {"new", "backward-compatible", "migration-required", "breaking"}
PUBLIC_API_IMPACTS = {"none", "backward-compatible", "migration-required", "breaking"}
MIGRATION_STATUSES = {"migrated", "allowed-negative-fixture", "allowed-compatibility-shim", "verified-none"}
NOT_APPLICABLE_RE = re.compile(r"(?i)^not-applicable\s*:\s*\S.*$")
CODE_BLOCK_RE = re.compile(r"```([A-Za-z0-9_+-]+)\s+([\s\S]*?)```", re.IGNORECASE)
HTML_COMMENT_RE = re.compile(r"<!--[\s\S]*?-->")

def fail(message: str) -> None:
    print(f"doc-structure-check: {message}", file=sys.stderr)
    raise SystemExit(1)


def read_text(path: Path) -> str:
    if not path.exists():
        fail(f"missing required file: {path}")
    return path.read_text(encoding="utf-8")


def normalize_column(value: str) -> str:
    return re.sub(r"[^a-z0-9]+", "_", value.strip().lower()).strip("_")


def split_table_row(line: str) -> list[str]:
    parts = [part.strip() for part in line.strip().split("|")]
    if parts and parts[0] == "":
        parts = parts[1:]
    if parts and parts[-1] == "":
        parts = parts[:-1]
    return parts


def section_body(text: str, heading: str, path: Path) -> str:
    pattern = re.compile(rf"(?m)^##\s+{re.escape(heading)}\s*$")
    match = pattern.search(text)
    if not match:
        fail(f"{path} missing required section: ## {heading}")
    next_heading = re.search(r"(?m)^##\s+", text[match.end() :])
    end = match.end() + next_heading.start() if next_heading else len(text)
    return text[match.end() : end]


def visible_section_body(text: str, heading: str, path: Path) -> str:
    return HTML_COMMENT_RE.sub("", section_body(text, heading, path))


def section_declares_not_applicable(body: str) -> bool:
    return re.search(r"(?im)^\s*(?:[-*]\s*)?not-applicable\s*:\s*\S", body) is not None


def table_after_heading(text: str, heading: str, path: Path) -> list[dict[str, str]]:
    body = section_body(text, heading, path)
    lines = body.splitlines()
    table_start = None
    for index, line in enumerate(lines):
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
        row = {header: values[pos].strip() if pos < len(values) else "" for pos, header in enumerate(headers)}
        rows.append(row)
    if not rows:
        fail(f"{path} section ## {heading} has no data rows")
    return rows


def non_empty(value: str) -> bool:
    return value.strip().lower() not in EMPTY_VALUES


def check_line_count(path: Path, max_lines: int) -> None:
    line_count = len(read_text(path).splitlines())
    if line_count > max_lines:
        fail(f"{path} has {line_count} lines; split docs above {max_lines} lines")


def check_sections(path: Path, text: str) -> None:
    for heading in REQUIRED_SECTIONS[path.name]:
        section_body(text, heading, path)


def check_tables(path: Path, text: str) -> None:
    for (name, heading), columns in REQUIRED_TABLE_COLUMNS.items():
        if name != path.name:
            continue
        rows = table_after_heading(text, heading, path)
        available = set(rows[0])
        missing = [column for column in columns if column not in available]
        if missing:
            fail(f"{path} ## {heading} missing columns: {', '.join(missing)}")
        for index, row in enumerate(rows, start=1):
            if all(not non_empty(value) for value in row.values()):
                fail(f"{path} ## {heading} row {index} is empty placeholder content")
            first_value = row.get(columns[0], "").strip()
            if is_not_applicable(first_value):
                require_not_applicable_reason(path, f"## {heading} row {index}", first_value)
                continue
            for column in columns:
                if column in {
                    "depends_on",
                    "gap_manual_reason",
                    "deferred_checks_and_reason",
                    "required_checks",
                    "testplan_step_id",
                    "type",
                    "test_file",
                    "test_file_or_script",
                    "derived_cases",
                    "design_source",
                    "level",
                }:
                    continue
                if not non_empty(row.get(column, "")):
                    fail(f"{path} ## {heading} row {index} column {column} is empty placeholder content")


FORBIDDEN_DESIGN_TEST_HEADINGS = {
    "test cases",
    "test plan",
    "testing strategy",
    "validation rationale",
    "unit tests",
    "dv tests",
    "integration tests",
    "test implementation",
}
FORBIDDEN_DESIGN_TEST_TERMS_RE = re.compile(r"(?i)\\b(testability seams?|validation ids?|test fixtures?|expected test results?)\\b")


def check_design_has_no_test_design(path: Path, text: str) -> None:
    for heading in re.findall(r"(?m)^##+\\s+(.+?)\\s*$", text):
        normalized = heading.strip().lower()
        if normalized in FORBIDDEN_DESIGN_TEST_HEADINGS:
            fail(f"{path} design-stage documents must not define testing section: {heading}")
    match = FORBIDDEN_DESIGN_TEST_TERMS_RE.search(HTML_COMMENT_RE.sub("", text))
    if match:
        fail(f"{path} design-stage documents must not define testing details: {match.group(1)}")


def check_module_dependencies(path: Path, text: str) -> None:
    rows = table_after_heading(text, "Module Dependencies", path)
    graph: dict[tuple[str, str, str], list[str]] = {}
    for row in rows:
        key = tuple(row.get(field, "").strip().strip("`") for field in ("level", "parent", "node"))
        if not all(non_empty(value) for value in key):
            fail(f"{path} module dependency rows require level, parent, and node")
        if key in graph:
            fail(f"{path} duplicate dependency node: {key}")
        raw = row.get("depends_on", "").strip().strip("`")
        if not raw:
            fail(f"{path} node {key} requires depends_on; use none for no dependencies")
        graph[key] = [] if raw.lower() == "none" else [item.strip().strip("`") for item in raw.split(",")]
    visiting: set[tuple[str, str, str]] = set()
    visited: set[tuple[str, str, str]] = set()

    def visit(node: tuple[str, str, str]) -> None:
        if node in visiting:
            fail(f"{path} module dependency cycle at {node}")
        if node in visited:
            return
        visiting.add(node)
        for target in graph[node]:
            dependency = (node[0], node[1], target)
            if dependency not in graph:
                fail(f"{path} unknown or cross-parent dependency {node} -> {target}")
            visit(dependency)
        visiting.remove(node)
        visited.add(node)

    for node in graph:
        visit(node)


def check_key_flows(path: Path, text: str) -> None:
    body = visible_section_body(text, "Key Flows", path).strip()
    if not non_empty(body):
        fail(f"{path} Key Flows requires relevant call/failure behavior or not-applicable reason")
    if is_not_applicable(body):
        require_not_applicable_reason(path, "Key Flows", body)


def check_state_ownership(path: Path, text: str) -> None:
    body = visible_section_body(text, "State and Ownership", path)
    if section_declares_not_applicable(body):
        return
    if not re.search(r"(?im)^\s*-\s*Owner\s*:\s*\S", body):
        fail(f"{path} State and Ownership must name the single owner or record not-applicable")


def check_file_level_interfaces(path: Path, text: str) -> None:
    body = visible_section_body(text, "File-Level Interfaces", path)
    values: set[str] = set()
    if not section_declares_not_applicable(body):
        blocks = CODE_BLOCK_RE.findall(body)
        source_blocks = [lang.lower() for lang, code in blocks if lang.lower() not in {"mermaid", "text", "txt", "plain", "markdown", "md"} and non_empty(code)]
        if not source_blocks:
            fail(f"{path} ## File-Level Interfaces must use fenced code blocks with the project language, or record not-applicable")
        if not re.search(r"(?im)^\s*-\s*Consumer\s*:\s*\S", body):
            fail(f"{path} ## File-Level Interfaces must name the consumer module or change_id")
        compatibility = re.search(r"(?im)^\s*-\s*Compatibility\s*:\s*([^\n]+)", body)
        if not compatibility:
            fail(f"{path} ## File-Level Interfaces must record compatibility")
        values = {part.strip().lower() for part in re.split(r"[/,]", compatibility.group(1))}
        if not values & COMPATIBILITY_VALUES:
            fail(f"{path} ## File-Level Interfaces compatibility must include one of: {', '.join(sorted(COMPATIBILITY_VALUES))}")
    impact_body = visible_section_body(text, "API and Build Surface Impact", path)
    impact_match = re.search(r"(?im)^\s*-\s*Public API impact\s*:\s*([^\n]+)", impact_body)
    public_api = impact_match.group(1).strip().lower() if impact_match else ""
    if public_api not in PUBLIC_API_IMPACTS:
        fail(f"{path} ## API and Build Surface Impact has invalid Public API impact: {public_api or '<missing>'}")
    flags: list[bool] = []
    for label in ("Crate-root export change", "Build-surface change", "Documentation examples affected"):
        match = re.search(rf"(?im)^\s*-\s*{re.escape(label)}\s*:\s*(yes|no)\s*$", impact_body)
        if not match:
            fail(f"{path} ## API and Build Surface Impact missing yes/no value for {label}")
        flags.append(match.group(1).lower() == "yes")
    risky = (
        public_api in {"breaking", "migration-required"}
        or bool(values & {"breaking", "migration-required"})
        or any(flags)
    )
    if not risky:
        return
    rows = table_after_heading(text, "Consumer Migration Closure", path)
    for index, row in enumerate(rows, start=1):
        for column in ("old_symbol", "new_path", "change_id", "consumer_kind", "migration_status"):
            if not non_empty(row.get(column, "")):
                fail(f"{path} ## Consumer Migration Closure row {index} missing concrete {column}")
        status = row["migration_status"].strip().lower()
        if status not in MIGRATION_STATUSES:
            fail(f"{path} ## Consumer Migration Closure row {index} has invalid migration_status: {status}")
        if (public_api == "breaking" or "breaking" in values) and status == "allowed-compatibility-shim":
            fail(f"{path} breaking interfaces cannot retain an allowed-compatibility-shim")
        consumer = row.get("consumer_path", "").strip().strip("`")
        if status == "verified-none":
            if consumer.lower() not in {"none-found", "verified-none"}:
                fail(f"{path} verified-none consumer rows must use consumer_path none-found")
        elif (
            not non_empty(consumer)
            or consumer.endswith("/")
            or any(token in consumer for token in ("*", "?", "["))
            or consumer.lower() in {"all callers", "all consumers"}
        ):
            fail(f"{path} ## Consumer Migration Closure row {index} must name one concrete consumer file")


def is_not_applicable(value: str) -> bool:
    return value.strip().lower().startswith("not-applicable")


def require_not_applicable_reason(path: Path, context: str, value: str) -> None:
    if not NOT_APPLICABLE_RE.match(value.strip()):
        fail(f"{path} {context}: not-applicable entries need a concrete reason: `not-applicable: <reason>`")


def check_file_level_implementation_sequence(path: Path, text: str) -> None:
    rows = table_after_heading(text, "File-Level Implementation Sequence", path)
    for index, row in enumerate(rows, start=1):
        module = row.get("file_level_module", "").strip()
        sequence = row.get("sequence", "").strip()
        if is_not_applicable(sequence):
            require_not_applicable_reason(path, f"## File-Level Implementation Sequence row {index}", sequence)
            continue
        if is_not_applicable(module):
            require_not_applicable_reason(path, f"## File-Level Implementation Sequence row {index}", module)
            continue
        for column in ("sequence", "file_level_module", "action", "change_id"):
            if not non_empty(row.get(column, "")):
                fail(f"{path} ## File-Level Implementation Sequence row {index} column {column} is empty placeholder content")


def check_submodule_doc_placement(packet: Path, stage: str | None) -> None:
    if stage == "proposal":
        return
    folders = ("design", "testing") if stage in {"testing", "acceptance"} else ("design",)
    for folder_name in folders:
        folder = packet / folder_name
        if not folder.exists():
            continue
        for name in ("proposal.md", "design.md", "testing.md", "testplan.yaml"):
            matches = sorted(folder.rglob(name))
            if matches:
                rendered = ", ".join(str(path) for path in matches[:5])
                fail(f"submodule packet docs belong directly under the submodule packet, not {folder_name}/: {rendered}")


def check_doc(path: Path, max_lines: int) -> None:
    text = read_text(path)
    if path.name == "proposal.md":
        check_line_count(path, max_lines)
    check_sections(path, text)
    check_tables(path, text)
    if path.name == "design.md":
        check_module_dependencies(path, text)
        check_file_level_interfaces(path, text)
        check_key_flows(path, text)
        check_state_ownership(path, text)
        check_design_has_no_test_design(path, text)
        check_file_level_implementation_sequence(path, text)


def packet_path(root: Path, version: str, module: str, submodule: str | None) -> Path:
    packet = root / ".harness" / "tasks" / version / "modules" / module
    if submodule:
        packet = packet / submodule
    return packet


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=".")
    parser.add_argument("--version", required=True)
    parser.add_argument("--module", required=True)
    parser.add_argument("--submodule")
    parser.add_argument("--max-lines", type=int, default=MAX_PROPOSAL_LINES, help="proposal-only line limit")
    parser.add_argument(
        "--docs",
        choices=("all", "mandatory", "proposal", "design", "testing"),
        default="all",
    )
    args = parser.parse_args()

    packet = packet_path(Path(args.root), args.version, args.module, args.submodule)
    manifest = packet / "task.yaml"
    try:
        stage = parse_task_manifest(manifest).get("stage") if manifest.is_file() else None
    except TaskManifestError as error:
        fail(str(error))
    if args.docs in {"all", "mandatory", "proposal"}:
        check_doc(packet / "proposal.md", args.max_lines)
    if (
        args.docs in {"all", "mandatory", "design"}
        and stage != "proposal"
    ):
        check_doc(packet / "design.md", args.max_lines)
    if args.docs in {"all", "testing"}:
        testing = packet / "testing.md"
        if testing.exists():
            check_doc(testing, args.max_lines)

    check_submodule_doc_placement(packet, stage)
    print("doc-structure-check: passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
