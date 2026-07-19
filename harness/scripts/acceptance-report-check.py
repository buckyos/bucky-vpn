#!/usr/bin/env python3
"""Validate the minimal Harness acceptance report.

Acceptance reviews requirements and delivered implementation. When a design
source or testing document exists, the report also records whether the
implementation is consistent with it. Test execution, quality gates, risk
matrices, generated acceptance rules, and fixed audit-category tables belong
to their owning stages and are not acceptance-report requirements.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

from task_manifest import TaskManifestError, parse_task_manifest, stage_is_automatic


TABLE_SEPARATOR_RE = re.compile(
    r"^\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?\s*$"
)
EMPTY_VALUES = {"", "-", "n/a", "na", "tbd", "todo", "pending"}
ALLOWED_RESULTS = {"accepted", "rejected", "needs changes"}
ALLOWED_SEVERITIES = {"none", "low", "medium", "high", "critical"}
ALLOWED_FINDING_KINDS = {
    "requirement",
    "implementation",
    "design-consistency",
    "testing-consistency",
}
REVIEW_STATUSES = {"pass", "fail"}
DOCUMENT_STATUSES = {"pass", "fail", "not-present"}


def fail(message: str) -> None:
    print(f"acceptance-report-check: {message}", file=sys.stderr)
    raise SystemExit(1)


def read_text(path: Path) -> str:
    if not path.is_file():
        fail(f"missing required file: {path}")
    return path.read_text(encoding="utf-8")


def normalize_column(value: str) -> str:
    return re.sub(r"[^a-z0-9]+", "_", value.strip().lower()).strip("_")


def split_table_row(line: str) -> list[str]:
    cells = [cell.strip() for cell in line.strip().split("|")]
    if cells and not cells[0]:
        cells = cells[1:]
    if cells and not cells[-1]:
        cells = cells[:-1]
    return cells


def non_empty(value: str) -> bool:
    return value.strip().lower() not in EMPTY_VALUES


def section_body(text: str, heading: str, path: Path) -> str:
    match = re.search(rf"(?m)^##\s+{re.escape(heading)}\s*$", text)
    if not match:
        fail(f"{path} missing required section: ## {heading}")
    next_heading = re.search(r"(?m)^##\s+", text[match.end() :])
    end = match.end() + next_heading.start() if next_heading else len(text)
    return text[match.end() : end]


def table_rows(text: str, heading: str, path: Path) -> list[dict[str, str]]:
    lines = section_body(text, heading, path).splitlines()
    start = next(
        (
            index
            for index, line in enumerate(lines)
            if "|" in line
            and index + 1 < len(lines)
            and TABLE_SEPARATOR_RE.match(lines[index + 1])
        ),
        None,
    )
    if start is None:
        fail(f"{path} ## {heading} missing required table")
    headers = [normalize_column(cell) for cell in split_table_row(lines[start])]
    rows: list[dict[str, str]] = []
    for line in lines[start + 2 :]:
        if not line.strip() or not line.lstrip().startswith("|"):
            break
        values = split_table_row(line)
        rows.append(
            {
                header: values[index].strip() if index < len(values) else ""
                for index, header in enumerate(headers)
            }
        )
    if not rows:
        fail(f"{path} ## {heading} must contain at least one review row")
    return rows


def require_columns(
    path: Path, heading: str, rows: list[dict[str, str]], columns: tuple[str, ...]
) -> None:
    missing = [column for column in columns if column not in rows[0]]
    if missing:
        fail(f"{path} ## {heading} missing columns: {', '.join(missing)}")
    for index, row in enumerate(rows, start=1):
        empty = [column for column in columns if not non_empty(row.get(column, ""))]
        if empty:
            fail(
                f"{path} ## {heading} row {index} has empty fields: "
                + ", ".join(empty)
            )


def conclusion(text: str, path: Path) -> str:
    body = section_body(text, "Conclusion", path)
    match = re.search(
        r"(?im)^\s*-\s*Accepted / rejected / needs changes:\s*(.+)$", body
    )
    if not match:
        fail(f"{path} Conclusion missing result")
    result = match.group(1).strip().lower()
    if result not in ALLOWED_RESULTS:
        fail(f"{path} has unsupported conclusion: {result}")
    reason = re.search(r"(?im)^\s*-\s*Reason:\s*(.+)$", body)
    if not reason or not non_empty(reason.group(1)):
        fail(f"{path} Conclusion missing concrete reason")
    return result


def object_scope(text: str, path: Path, root: Path) -> dict[str, object]:
    body = section_body(text, "Object and Scope", path)
    match = re.search(r"(?im)^\s*-\s*Task manifest:\s*(.+)$", body)
    if not match or not non_empty(match.group(1)):
        fail(f"{path} Object and Scope missing Task manifest")
    value = match.group(1).strip().strip("`")
    manifest = path.parent / value
    try:
        manifest.resolve().relative_to(root.resolve())
    except ValueError:
        fail(f"{path} Task manifest resolves outside repository: {value}")
    try:
        task = parse_task_manifest(manifest)
    except TaskManifestError as error:
        fail(str(error))
    bindings: dict[str, str] = {}
    for field in ("version", "packet_module", "task_name"):
        if task.get(field) is None:
            fail(f"{manifest} missing {field}")
        bindings[field] = str(task[field])
    change_ids = {
        str(change.get("id"))
        for change in task.get("changes", [])
        if isinstance(change, dict) and change.get("id") is not None
    }
    if not change_ids:
        fail(f"{manifest} has no change ids")
    return {
        "module": bindings["packet_module"],
        "version": bindings["version"],
        "task_name": bindings["task_name"],
        "change_ids": change_ids,
        "task_scope": f"{bindings['packet_module']}/{bindings['task_name']}",
    }


def expected_document_sources(packet: Path) -> dict[str, list[str]]:
    manifest = packet / "task.yaml"
    if manifest.is_file():
        try:
            task = parse_task_manifest(manifest)
        except TaskManifestError as error:
            fail(str(error))
    else:
        task = {}
    policy = {
        "stage": str(task["stage"]) if task.get("stage") is not None else None,
        "mode": str(task["mode"]) if task.get("mode") is not None else None,
        "start": (
            str(task["auto_pipeline_start_stage"])
            if task.get("auto_pipeline_start_stage") is not None
            else None
        ),
    }
    automatic_design = stage_is_automatic(policy, "design")
    automatic_testing = stage_is_automatic(policy, "testing")
    design_candidates = (
        ("pipeline/plan.md",) if automatic_design else ("design.md",)
    )
    testing_candidates = (
        ("testplan.yaml",)
        if automatic_testing
        else ("testing.md", "testplan.yaml")
    )
    design_sources = [
        relative for relative in design_candidates if (packet / relative).is_file()
    ]
    testing_sources = [
        relative for relative in testing_candidates if (packet / relative).is_file()
    ]
    return {"design": design_sources, "testing": testing_sources}


def check_report(path: Path, text: str, root: Path) -> None:
    result = conclusion(text, path)
    scope = object_scope(text, path, root)
    expected = (
        root
        / "docs"
        / "versions"
        / str(scope["version"])
        / "modules"
        / str(scope["module"])
        / str(scope["task_name"])
        / "acceptance-report.md"
    )
    if path.resolve() != expected.resolve():
        fail(f"{path} Object and Scope does not match report task packet path: {expected}")
    proposal = path.parent / "proposal.md"
    if result == "accepted" and not proposal.is_file():
        fail(f"{path} cannot be accepted because required proposal is missing: {proposal}")

    findings = table_rows(text, "Findings", path)
    require_columns(
        path,
        "Findings",
        findings,
        ("id", "severity", "kind", "evidence", "problem", "blocking"),
    )
    for index, row in enumerate(findings, start=1):
        severity = row["severity"].lower()
        kind = row["kind"].lower()
        blocking = row["blocking"].lower()
        if severity not in ALLOWED_SEVERITIES:
            fail(f"{path} Findings row {index} has invalid severity: {severity}")
        if kind not in ALLOWED_FINDING_KINDS:
            fail(f"{path} Findings row {index} has invalid kind: {kind}")
        if blocking not in {"yes", "no"}:
            fail(f"{path} Findings row {index} blocking must be yes or no")
        if result == "accepted" and blocking == "yes":
            fail(f"{path} accepted conclusion contains a blocking finding: {row['id']}")

    for heading, columns in (
        (
            "Requirement Review",
            ("requirement_or_boundary", "source", "implementation_evidence", "finding", "status"),
        ),
        ("Implementation Review", ("area", "evidence", "finding", "status")),
    ):
        rows = table_rows(text, heading, path)
        require_columns(path, heading, rows, columns)
        for index, row in enumerate(rows, start=1):
            status = row["status"].lower()
            if status not in REVIEW_STATUSES:
                fail(f"{path} ## {heading} row {index} has invalid status: {status}")
            if result == "accepted" and status != "pass":
                fail(f"{path} accepted conclusion has failing {heading} row {index}")

    consistency = table_rows(text, "Document Consistency", path)
    require_columns(
        path,
        "Document Consistency",
        consistency,
        ("document", "source", "implementation_consistency", "finding", "status"),
    )
    rows_by_document = {row["document"].lower(): row for row in consistency}
    expected_sources = expected_document_sources(path.parent)
    for document, sources in expected_sources.items():
        row = rows_by_document.get(document)
        if not row:
            fail(f"{path} Document Consistency missing {document} row")
        status = row["status"].lower()
        if status not in DOCUMENT_STATUSES:
            fail(f"{path} Document Consistency {document} has invalid status: {status}")
        if sources:
            if status == "not-present":
                fail(f"{path} {document} source exists but report says not-present")
            missing_sources = [source for source in sources if source not in row["source"]]
            if missing_sources:
                fail(
                    f"{path} Document Consistency {document} does not name: "
                    + ", ".join(missing_sources)
                )
            if result == "accepted" and status != "pass":
                fail(f"{path} accepted conclusion has failing {document} consistency")
        elif status != "not-present":
            fail(f"{path} {document} source is absent and must use status not-present")

    summary = section_body(text, "Result Summary", path)
    for label in ("Overall result", "Outcome", "Blocking issues", "Next action"):
        match = re.search(rf"(?im)^\s*-\s*{re.escape(label)}:\s*(.+)$", summary)
        if not match or not non_empty(match.group(1)):
            fail(f"{path} Result Summary missing concrete {label}")
        if label == "Overall result" and result not in match.group(1).lower():
            fail(f"{path} Result Summary does not match Conclusion: {result}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("report")
    parser.add_argument("--root", default=".")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    path = Path(args.report)
    if not path.is_absolute():
        path = root / path
    check_report(path, read_text(path), root)
    print("acceptance-report-check: passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
