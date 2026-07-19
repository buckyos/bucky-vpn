#!/usr/bin/env python3
"""Validate lightweight trivial/standard acceptance against an approved proposal."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path, PurePosixPath

from task_manifest import TaskManifestError, parse_task_manifest


TABLE_SEPARATOR_RE = re.compile(r"^\s*\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)+\|?\s*$")
EMPTY_VALUES = {"", "-", "none", "null", "pending", "tbd", "todo"}
RESULTS = {"accepted", "needs-changes", "rejected"}
REVIEW_STATUSES = {"pass", "fail"}


def fail(message: str) -> None:
    print(f"completion-report-check: {message}", file=sys.stderr)
    raise SystemExit(1)


def non_empty(value: str, *, allow_not_applicable: bool = False) -> bool:
    normalized = value.strip().strip('"').strip("'")
    if re.search(r"<[^>]+>", normalized):
        return False
    if allow_not_applicable and normalized.lower() in {"none", "n/a", "na", "not-applicable"}:
        return True
    return normalized.lower() not in EMPTY_VALUES | {"n/a", "na", "not-applicable"}


def read_text(path: Path) -> str:
    if not path.is_file():
        fail(f"missing required file: {path}")
    try:
        return path.read_text(encoding="utf-8")
    except UnicodeDecodeError as error:
        fail(f"{path} is not valid utf-8: {error}")


def normalize_column(value: str) -> str:
    return re.sub(r"[^a-z0-9]+", "_", value.strip().lower()).strip("_")


def split_table_row(line: str) -> list[str]:
    cells = [cell.strip() for cell in line.strip().split("|")]
    if cells and not cells[0]:
        cells = cells[1:]
    if cells and not cells[-1]:
        cells = cells[:-1]
    return cells


def section_body(text: str, heading: str, path: Path) -> str:
    match = re.search(rf"(?m)^##\s+{re.escape(heading)}\s*$", text)
    if not match:
        fail(f"{path} missing required section: ## {heading}")
    following = re.search(r"(?m)^##\s+", text[match.end() :])
    end = match.end() + following.start() if following else len(text)
    return text[match.end() : end]


def bullet(body: str, label: str, path: Path, *, allow_not_applicable: bool = False) -> str:
    match = re.search(rf"(?im)^\s*-\s*{re.escape(label)}:\s*(.+)$", body)
    if not match or not non_empty(match.group(1), allow_not_applicable=allow_not_applicable):
        fail(f"{path} missing concrete {label}")
    return match.group(1).strip()


def table_rows(text: str, heading: str, path: Path) -> list[dict[str, str]]:
    lines = section_body(text, heading, path).splitlines()
    start: int | None = None
    for index, line in enumerate(lines[:-1]):
        if "|" in line and TABLE_SEPARATOR_RE.match(lines[index + 1]):
            start = index
            break
    if start is None:
        fail(f"{path} ## {heading} missing required table")
    headers = [normalize_column(cell) for cell in split_table_row(lines[start])]
    rows: list[dict[str, str]] = []
    for line in lines[start + 2 :]:
        if not line.strip() or not line.lstrip().startswith("|"):
            break
        values = split_table_row(line)
        rows.append({header: values[index] if index < len(values) else "" for index, header in enumerate(headers)})
    if not rows:
        fail(f"{path} ## {heading} has no data rows")
    return rows


def require_columns(
    path: Path,
    heading: str,
    rows: list[dict[str, str]],
    columns: tuple[str, ...],
    *,
    allow_none: tuple[str, ...] = (),
) -> None:
    missing = sorted(set(columns) - set(rows[0]))
    if missing:
        fail(f"{path} ## {heading} missing columns: {', '.join(missing)}")
    for index, row in enumerate(rows, start=1):
        empty = [
            column for column in columns
            if not non_empty(row.get(column, ""))
            and not (column in allow_none and row.get(column, "").strip().lower() == "none")
        ]
        if empty:
            fail(f"{path} ## {heading} row {index} has empty fields: {', '.join(empty)}")


def safe_root_relative(root: Path, value: str, label: str) -> tuple[str, Path]:
    normalized = value.strip().strip("`").replace("\\", "/")
    relative = PurePosixPath(normalized)
    if relative.is_absolute() or ".." in relative.parts:
        fail(f"unsafe {label}: {value}")
    path = (root / Path(*relative.parts)).resolve()
    try:
        path.relative_to(root.resolve())
    except ValueError:
        fail(f"{label} resolves outside repository: {value}")
    return relative.as_posix(), path


def validate_change_record(root: Path, task_path: Path, task: dict[str, object]) -> str:
    value = task.get("change_record")
    if not isinstance(value, str) or not value:
        fail(f"{task_path} standard task requires change_record")
    relative, path = safe_root_relative(root, value, "change_record")
    parts = PurePosixPath(relative).parts
    if len(parts) != 3 or parts[:2] != ("docs", "changes") or not relative.endswith(".md"):
        fail("standard change_record must use docs/changes/<change>.md")
    text = read_text(path)
    status = re.search(r"(?mi)^\s*-\s*Status:\s*(.+)$", text)
    if not status or status.group(1).strip().lower() != "complete":
        fail(f"{path} Status must be complete")
    expected_task = task_path.relative_to(root).as_posix()
    expected_proposal = (task_path.parent / "proposal.md").relative_to(root).as_posix()
    for label, expected in (("Task manifest", expected_task), ("Approved proposal", expected_proposal)):
        match = re.search(rf"(?mi)^\s*-\s*{re.escape(label)}:\s*(.+)$", text)
        actual = match.group(1).strip().strip("`") if match else ""
        if actual != expected:
            fail(f"{path} {label} must bind {expected}")
    affected = re.search(r"(?mi)^\s*-\s*Affected paths:\s*(.+)$", text)
    if not affected or not non_empty(affected.group(1)):
        fail(f"{path} requires concrete Affected paths")
    verification = section_body(text, "Verification", path)
    check = bullet(verification, "Targeted check", path)
    result = bullet(verification, "Result", path)
    residual = bullet(verification, "Residual risk or follow-up", path, allow_not_applicable=True)
    if check.lower() == "not-run" or result.lower() not in {
        "pass", "passed", "success", "succeeded"
    }:
        fail(f"{path} complete change record requires passing targeted verification")
    return relative


def check_report(path: Path, root: Path) -> None:
    try:
        path.relative_to(root)
    except ValueError:
        fail(f"completion report resolves outside repository: {path}")
    text = read_text(path)
    scope = section_body(text, "Object and Scope", path)
    manifest_value = bullet(scope, "Task manifest", path)
    if manifest_value != "task.yaml":
        fail(f"{path} Task manifest must be task.yaml")
    task_path = path.parent / manifest_value
    try:
        task = parse_task_manifest(task_path)
    except TaskManifestError as error:
        fail(str(error))
    tier = str(task.get("workflow_tier") or "")
    if tier not in {"trivial", "standard"}:
        fail(f"{path} applies only to confirmed trivial or standard tasks")
    if bullet(scope, "Workflow tier", path).lower() != tier:
        fail(f"{path} Workflow tier does not match task.yaml: {tier}")
    report_value = task.get("completion_report") or "completion-report.md"
    if report_value != "completion-report.md":
        fail(f"{task_path} completion_report must be completion-report.md")
    expected = task_path.parent / str(report_value)
    if path.resolve() != expected.resolve() or path.name != "completion-report.md":
        fail(f"completion report must use canonical task-packet path: {expected}")
    proposal = task_path.parent / "proposal.md"
    if not proposal.is_file():
        fail(f"completion report requires proposal: {proposal}")
    proposal_text = read_text(proposal)
    if not re.search(r"(?m)^status:\s*approved\s*$", proposal_text):
        fail(f"completion report requires approved proposal: {proposal}")
    if not re.search(
        rf"(?mi)^\s*-\s*Final tier:\s*{re.escape(tier)}\s*$",
        proposal_text,
    ):
        fail(f"proposal final tier does not match task.yaml: {proposal}")
    expected_ids = {
        str(change.get("id"))
        for change in task.get("changes", [])
        if isinstance(change, dict) and change.get("id")
    }
    if not expected_ids:
        fail(f"{task_path} requires at least one change_id for lightweight acceptance")
    proposal_items = table_rows(proposal_text, "Proposal Items", proposal)
    require_columns(
        proposal,
        "Proposal Items",
        proposal_items,
        ("proposal_id", "change_id", "requirement", "success_evidence"),
    )
    proposal_ids = {row["change_id"] for row in proposal_items}
    if proposal_ids != expected_ids:
        fail(f"{proposal} Proposal Items change_id coverage must exactly match task.yaml")

    change_record = bullet(scope, "Change record", path, allow_not_applicable=True)
    if tier == "standard":
        expected_record = validate_change_record(root, task_path, task)
        if change_record != expected_record:
            fail(f"{path} Change record must bind {expected_record}")
    elif change_record.lower() not in {"n/a", "na", "not-applicable"}:
        fail(f"{path} trivial task Change record must be not-applicable")

    delivery = section_body(text, "Delivery Summary", path)
    bullet(delivery, "Outcome", path)
    bullet(delivery, "Handoff", path)

    consistency = table_rows(text, "Proposal Consistency", path)
    columns = ("change_id", "requirement_or_boundary", "proposal_source", "delivery_evidence", "finding", "status")
    require_columns(path, "Proposal Consistency", consistency, columns)
    actual_ids = {row["change_id"] for row in consistency}
    if actual_ids != expected_ids:
        fail(f"{path} Proposal Consistency change_id coverage must exactly match task.yaml")
    if any("proposal.md" not in row["proposal_source"] for row in consistency):
        fail(f"{path} every Proposal Consistency row must cite proposal.md")
    for index, row in enumerate(consistency, start=1):
        if row["status"].lower() not in REVIEW_STATUSES:
            fail(f"{path} ## Proposal Consistency row {index} has invalid status: {row['status']}")

    implementation = table_rows(text, "Implementation Review", path)
    require_columns(
        path,
        "Implementation Review",
        implementation,
        ("area", "evidence", "finding", "status"),
    )
    for index, row in enumerate(implementation, start=1):
        if row["status"].lower() not in REVIEW_STATUSES:
            fail(f"{path} ## Implementation Review row {index} has invalid status: {row['status']}")

    verification = section_body(text, "Verification", path)
    check = bullet(verification, "Targeted check", path)
    result = bullet(verification, "Result", path)
    exception = bullet(verification, "Exception reason", path, allow_not_applicable=True)
    not_run = check.lower() == "not-run" or result.lower() == "not-run"
    if not_run and not non_empty(exception):
        fail(f"{path} not-run verification requires a concrete Exception reason")

    findings = table_rows(text, "Findings", path)
    require_columns(
        path,
        "Findings",
        findings,
        ("id", "severity", "evidence", "problem", "blocking"),
        allow_none=("severity",),
    )
    if any(row["blocking"].lower() not in {"yes", "no"} for row in findings):
        fail(f"{path} Findings blocking values must be yes or no")

    conclusion = section_body(text, "Conclusion", path)
    result_value = bullet(conclusion, "Accepted / rejected / needs changes", path).lower()
    if result_value not in RESULTS:
        fail(f"{path} has unsupported conclusion: {result_value}")
    bullet(conclusion, "Reason", path)
    if result_value == "accepted":
        if check.lower() == "not-run" or result.lower() not in {
            "pass", "passed", "success", "succeeded"
        }:
            fail(f"{path} accepted conclusion requires completed passing targeted verification")
        if any(row["status"].lower() != "pass" for row in consistency):
            fail(f"{path} accepted conclusion has failing proposal consistency")
        if any(row["status"].lower() != "pass" for row in implementation):
            fail(f"{path} accepted conclusion has failing implementation review")
        if any(row["blocking"].lower() == "yes" for row in findings):
            fail(f"{path} accepted conclusion contains blocking findings")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("report")
    parser.add_argument("--root", default=".")
    args = parser.parse_args()
    root = Path(args.root).resolve()
    report = Path(args.report)
    if not report.is_absolute():
        report = root / report
    check_report(report.resolve(), root)
    print("completion-report-check: passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
