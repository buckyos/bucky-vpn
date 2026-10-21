#!/usr/bin/env python3
"""Advance or return a high-risk task through legal lifecycle transitions."""

from __future__ import annotations

import argparse
import copy
import importlib.util
import json
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
    for line in completed.stderr.splitlines():
        if line.startswith("harness-check: timings "):
            print(line, file=sys.stderr)


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


def run_document_checks(root: Path, task: dict[str, object], stage: str) -> None:
    identity = ["--root", str(root), "--version", str(task["version"]),
                "--module", str(task["packet_module"]), "--submodule", str(task["task_name"])]
    for name, extra in (
        ("schema-check", ["--require-approved"]),
        ("doc-structure-check", ["--docs", stage]),
    ):
        result = subprocess.run(
            [sys.executable, str(sibling_script(name)), *identity, *extra],
            capture_output=True, text=True,
        )
        if result.returncode:
            fail(f"documentation check failed: {result.stderr.strip() or result.stdout.strip()}")


def refresh_documentation(root: Path, task_path: Path, task: dict[str, object],
                          stage: str, reason: str, review_evidence: str) -> None:
    """Renew an explanatory edit without replaying unaffected delivery stages."""
    if not reason.strip() or not review_evidence.strip():
        fail("documentation refresh requires impact assessment and independent consistency review")
    if STAGES.index(stage) > STAGES.index(str(task["stage"])):
        fail("cannot refresh documentation from a future stage")
    lifecycle = load_lifecycle()
    state = lifecycle.load_state(task_path, task, required=True)
    stages = state["stages"]
    receipt = stages.get(stage)
    if not isinstance(receipt, dict) or receipt.get("status") != "complete":
        fail("documentation refresh requires an existing completed owner receipt")
    if receipt.get("task_binding_schema") != lifecycle.TASK_BINDING_SCHEMA or receipt.get("task_binding_sha256") != lifecycle.task_binding(task, stage):
        fail("documentation refresh cannot change task, scope, or artifact bindings")
    inputs = lifecycle.expected_receipt_inputs(root, task_path, task, stage)
    old_inputs = receipt.get("inputs", {})
    document = (task_path.parent / str(task.get(stage) or f"{stage}.md")).relative_to(root).as_posix()
    documents = {document}
    if stage == "design":
        documents.update(path for path in inputs if path != (task_path.parent / str(task["risk_profile"])).relative_to(root).as_posix())
    if set(inputs) != set(old_inputs) or any(
        inputs[path] != digest for path, digest in old_inputs.items() if path not in documents
    ):
        fail("documentation refresh cannot change the source collection, risk policy or testplan; use return --to")
    changed = sorted(path for path in documents if path in inputs and inputs[path] != old_inputs[path])
    if not changed:
        fail("documentation refresh requires a changed owner document")
    updated = copy.deepcopy(stages)
    updated[stage]["inputs"] = inputs
    for path in changed:
        review = {"path": path, "before": old_inputs[path], "after": inputs[path],
                  "reason": reason, "review_evidence": review_evidence}
        review["id"] = lifecycle.sha256_bytes(
            json.dumps(review, sort_keys=True, ensure_ascii=False).encode("utf-8")
        )
        updated[stage].setdefault("documentation_reviews", []).append(review)
        if stage == "design" and "testing" in updated:
            updated["testing"].setdefault("documentation_updates", []).append(review)
    # Check every retained receipt, including latest-run freshness. Only the
    # reviewed design hash may differ in a test snapshot; no run is rewritten.
    for name, retained in updated.items():
        if name in {stage, "acceptance"}:
            continue
        if retained.get("task_binding_schema") != lifecycle.TASK_BINDING_SCHEMA or retained.get("task_binding_sha256") != lifecycle.task_binding(task, name):
            fail(f"cannot retain stale {name} receipt")
        if name == "testing":
            if retained.get("inputs") != lifecycle.expected_receipt_inputs(root, task_path, task, name):
                fail("cannot retain changed testing inputs")
            artifact = lifecycle.latest_test_artifact(
                root, task_path, task, documentation_updates=retained.get("documentation_updates", [])
            )
            if retained.get("test_run") != artifact.relative_to(root).as_posix() or retained.get("test_run_sha256") != lifecycle.sha256_file(artifact):
                fail("cannot retain superseded or modified testing evidence")
        else:
            lifecycle.verify_receipt_evidence(root, task_path, task, name, retained)
    if stage == "testing":
        # The owner receipt is renewed, but its original run must still be valid.
        lifecycle.verify_receipt_evidence(root, task_path, task, stage, updated[stage])
    run_document_checks(root, task, stage)
    updated.pop("acceptance", None)
    state["stages"] = updated
    lifecycle.write_state(lifecycle.state_path(task_path, task), state)
    for review in updated[stage]["documentation_reviews"][-len(changed):]:
        print(f"task-transition: pending acceptance review {review['id']}: {review['path']}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=".")
    parser.add_argument("--task", required=True)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("advance")
    commands.add_parser("complete")
    refreshed = commands.add_parser("refresh-documentation")
    refreshed.add_argument("--stage", required=True, choices=("design", "testing"))
    refreshed.add_argument("--reason", required=True)
    refreshed.add_argument("--review-evidence", required=True)
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

    if args.command == "refresh-documentation":
        refresh_documentation(root, task_path, task, args.stage, args.reason, args.review_evidence)
        print(f"task-transition: refreshed {args.stage} documentation; independent acceptance required")
        return 0

    if args.command == "advance":
        if current == "acceptance":
            fail("acceptance has no next stage; use complete")
        lifecycle.verify_receipts(root, task_path, task, STAGES[:STAGES.index(current)])
        run_completion(root, task_path)
        payload = None
        if current in {"design", "testing"}:
            payload = lifecycle.receipt_payload(root, task_path, task, current)
            document = task_path.parent / str(task.get(current) or f"{current}.md")
            if document.is_file():
                text = document.read_text(encoding="utf-8")
                approved, count = re.subn(r"(?m)^status:[ \t]*(?:draft|approved)[ \t]*$", "status: approved", text, count=1)
                if count != 1:
                    fail(f"{document} must contain a valid status field")
                document.write_text(approved, encoding="utf-8")
                # Approval changes only the owner document. Retain every other
                # fingerprint and the test-run validation performed above.
                payload["inputs"][document.relative_to(root).as_posix()] = lifecycle.sha256_file(document)
        lifecycle.record_stage(root, task_path, task, current, payload=payload)
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
