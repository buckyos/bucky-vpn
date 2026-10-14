#!/usr/bin/env python3
"""Advance or return a high-risk task through legal lifecycle transitions."""

from __future__ import annotations

import argparse
import importlib.util
import re
import subprocess
import sys
from pathlib import Path


STAGES = ("proposal", "design", "implementation", "testing", "acceptance")


def fail(message: str) -> None:
    print(f"task-transition: {message}", file=sys.stderr)
    raise SystemExit(1)


def sibling_script(name: str) -> Path:
    installed = Path(__file__).with_name(f"{name}.py")
    return installed if installed.is_file() else Path(__file__).with_name(f"{name}.template.py")


def load_lifecycle():
    path = sibling_script("lifecycle-check")
    spec = importlib.util.spec_from_file_location("lifecycle_check", path)
    if spec is None or spec.loader is None:
        fail(f"cannot load lifecycle checker: {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def run_completion(root: Path, task_path: Path) -> None:
    command = [
        sys.executable,
        str(sibling_script("harness-check")),
        "--root", str(root),
        "--task", str(task_path),
        "--profile", "completion",
    ]
    completed = subprocess.run(command, capture_output=True, text=True)
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip() or "unknown error"
        fail(f"stage completion failed: {detail}")


def require_accepted_report(task_path: Path, task: dict[str, object]) -> None:
    report_name = str(task.get("acceptance_report") or "acceptance-report.md")
    report = task_path.parent / report_name
    if not report.is_file():
        fail(f"accepted completion requires acceptance report: {report}")
    text = report.read_text(encoding="utf-8")
    if not re.search(
        r"(?im)^\s*-\s*Accepted / rejected / needs changes:\s*accepted\s*$",
        text,
    ):
        fail(
            "complete is valid only for an accepted report; use return --to "
            "<design|implementation|testing> for needs changes"
        )


def write_stage(task_path: Path, stage: str, *, pending: bool = False) -> None:
    text = task_path.read_text(encoding="utf-8")
    updated, count = re.subn(r"(?m)^stage:\s*.*$", f"stage: {stage}", text)
    if count != 1:
        fail(f"{task_path} must contain exactly one top-level stage field")
    if pending:
        updated, count = re.subn(r"(?m)^workflow_tier:[ \t]*.*$", "workflow_tier: pending", updated)
        if count != 1:
            fail(f"{task_path} must contain exactly one top-level workflow_tier field")
    temporary = task_path.with_name(task_path.name + ".tmp")
    temporary.write_text(updated, encoding="utf-8")
    temporary.replace(task_path)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=".")
    parser.add_argument("--task", required=True)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("advance")
    commands.add_parser("complete")
    returned = commands.add_parser("return")
    returned.add_argument("--to", required=True, choices=STAGES[:-1])
    args = parser.parse_args()

    root = Path(args.root).resolve()
    task_path = Path(args.task)
    if not task_path.is_absolute():
        task_path = root / task_path
    task_path = task_path.resolve()
    lifecycle = load_lifecycle()
    task = lifecycle.validate_task(task_path)
    current = str(task["stage"])

    if args.command == "advance":
        if current == "acceptance":
            fail("acceptance has no next stage; use complete")
        lifecycle.verify_receipts(root, task_path, task, STAGES[:STAGES.index(current)])
        run_completion(root, task_path)
        if current in {"design", "testing"}:
            lifecycle.receipt_payload(root, task_path, task, current)
            document = task_path.parent / str(task.get(current) or f"{current}.md")
            if document.is_file():
                text = document.read_text(encoding="utf-8")
                approved, count = re.subn(r"(?m)^status:[ \t]*(?:draft|approved)[ \t]*$", "status: approved", text, count=1)
                if count != 1:
                    fail(f"{document} must contain a valid status field")
                document.write_text(approved, encoding="utf-8")
        lifecycle.record_stage(root, task_path, task, current)
        next_stage = STAGES[STAGES.index(current) + 1]
        write_stage(task_path, next_stage)
        print(f"task-transition: advanced {current} -> {next_stage}")
        return 0

    if args.command == "complete":
        if current != "acceptance":
            fail(f"complete is valid only in acceptance; current stage: {current}")
        require_accepted_report(task_path, task)
        run_completion(root, task_path)
        lifecycle.record_stage(root, task_path, task, current)
        print("task-transition: completed acceptance")
        return 0

    if STAGES.index(args.to) > STAGES.index(current):
        fail(f"return cannot skip forward from {current} to {args.to}")
    # Validate all status edits before invalidating evidence. Keep document content
    # and historical reports available for correction; only approval is reset.
    drafts: list[tuple[Path, str]] = []
    for stage in STAGES[STAGES.index(args.to):]:
        if stage not in {"proposal", "design", "testing"}:
            continue
        document = task_path.parent / str(task.get(stage) or f"{stage}.md")
        if not document.is_file():
            continue
        text = document.read_text(encoding="utf-8")
        front = re.match(r"\A---[ \t]*\n(.*?)\n---(?:\n|$)", text, re.S)
        if front is None:
            fail(f"{document} must have front matter before return")
        metadata, count = re.subn(r"(?m)^status:[ \t]*(?:draft|approved|rejected|superseded)[ \t]*$", "status: draft", front.group(1))
        if count != 1:
            fail(f"{document} must contain exactly one valid status field")
        drafts.append((document, text[:front.start(1)] + metadata + text[front.end(1):]))
    lifecycle.clear_from(task_path, task, args.to)
    for document, text in drafts:
        document.write_text(text, encoding="utf-8")
    write_stage(task_path, args.to, pending=args.to == "proposal")
    print(f"task-transition: returned {current} -> {args.to}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
