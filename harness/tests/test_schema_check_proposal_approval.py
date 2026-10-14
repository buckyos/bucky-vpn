#!/usr/bin/env python3
"""Regression coverage for proposal approval and vendored Swagger UI wiring."""

from __future__ import annotations

import contextlib
import copy
import importlib.util
import io
import json
import subprocess
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path
from unittest import mock


ROOT = Path(__file__).resolve().parents[2]
SCHEMA_CHECK = ROOT / "harness" / "scripts" / "schema-check.py"
LIFECYCLE_CHECK = ROOT / "harness" / "scripts" / "lifecycle-check.py"
SCRIPTS = str(LIFECYCLE_CHECK.parent)
if SCRIPTS not in sys.path:
    sys.path.insert(0, SCRIPTS)
LIFECYCLE_SPEC = importlib.util.spec_from_file_location(
    "lifecycle_check_regression", LIFECYCLE_CHECK
)
assert LIFECYCLE_SPEC is not None and LIFECYCLE_SPEC.loader is not None
LIFECYCLE = importlib.util.module_from_spec(LIFECYCLE_SPEC)
LIFECYCLE_SPEC.loader.exec_module(LIFECYCLE)


class ProposalApprovalRegressionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.tempdir = tempfile.TemporaryDirectory()
        self.root = Path(self.tempdir.name)
        self.packet = (
            self.root
            / ".harness"
            / "tasks"
            / "v0.1"
            / "modules"
            / "repo-governance"
            / "001-proposal-approval"
        )
        self.packet.mkdir(parents=True)
        (self.packet / "task.yaml").write_text(
            """schema_version: 1
workflow_tier: high-risk
version: v0.1
packet_module: repo-governance
task_name: 001-proposal-approval
stage: proposal
changes:
  - id: CHG-proposal-approval-gate
    target_module: repo-governance
    scope_paths: [\"harness/scripts/schema-check.py\"]
""",
            encoding="utf-8",
        )
        self.write_proposal("approved")
        subprocess.run([sys.executable, str(SCHEMA_CHECK.parent / 'task-index.py'), '--root', str(self.root), 'init', '--version', 'v0.1'], check=True, capture_output=True)
        subprocess.run([sys.executable, str(SCHEMA_CHECK.parent / 'task-index.py'), '--root', str(self.root), 'add', '--task', (self.packet / 'task.yaml').relative_to(self.root).as_posix()], check=True, capture_output=True)

    def tearDown(self) -> None:
        self.tempdir.cleanup()

    def write_proposal(self, status: str) -> None:
        (self.packet / "proposal.md").write_text(
            f"""---
task_manifest: task.yaml
status: {status}
---

# Proposal approval fixture
""",
            encoding="utf-8",
        )

    def run_schema_check(self) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                str(SCHEMA_CHECK),
                "--root",
                str(self.root),
                "--version",
                "v0.1",
                "--module",
                "repo-governance",
                "--submodule",
                "001-proposal-approval",
                "--require-approved",
            ],
            check=False,
            capture_output=True,
            text=True,
        )

    def test_require_approved_accepts_approved_proposal(self) -> None:
        self.write_proposal("approved")

        completed = self.run_schema_check()

        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("schema-check: passed", completed.stdout)

    def test_require_approved_rejects_draft_proposal(self) -> None:
        self.write_proposal("draft")

        completed = self.run_schema_check()

        self.assertNotEqual(completed.returncode, 0, completed.stdout)
        self.assertIn("must be approved", completed.stderr)
        self.assertIn("got status: draft", completed.stderr)


class LifecycleBindingRegressionTests(unittest.TestCase):
    @staticmethod
    def base_task() -> dict[str, object]:
        return {
            "schema_version": 1,
            "workflow_tier": "high-risk",
            "version": "v0.1",
            "packet_module": "globals",
            "task_name": "001-lifecycle-binding",
            "stage": "proposal",
                                    "proposal": "proposal.md",
            "design": "design.md",
            "testing": "testing.md",
            "testplan": "testplan.yaml",
            "acceptance_report": "acceptance-report.md",
            "risk_profile": "risk-profile.yaml",
            "completion_report": "completion-report.md",
            "change_record": "change-record.md",
                        "lifecycle_state": "lifecycle.json",
            "changed_paths_file": ".harness/evidence/all.txt",
            "changes": [
                {
                    "id": "CHG-lifecycle-binding",
                    "target_module": "repo-governance",
                    "scope_paths": ["harness/scripts/lifecycle-check.py"],
                    "changed_paths_file": ".harness/evidence/change.txt",
                }
            ],
        }

    def test_task_binding_ignores_legal_execution_and_evidence_mutations(self) -> None:
        task = self.base_task()
        expected = LIFECYCLE.task_binding(task)
        mutations = {
            "stage": "testing",
            "mode": "auto-pipeline",
            "auto_pipeline_start_stage": "implementation",
            "pipeline_plan": "pipeline/replanned.md",
            "changed_paths_file": ".harness/evidence/replaced-all.txt",
        }
        for field, value in mutations.items():
            with self.subTest(field=field):
                changed = copy.deepcopy(task)
                changed[field] = value
                self.assertEqual(LIFECYCLE.task_binding(changed), expected)

        for field, value in {
            "changed_paths_file": ".harness/evidence/replaced-change.txt",
        }.items():
            with self.subTest(change_field=field):
                changed = copy.deepcopy(task)
                changed["changes"][0][field] = value
                self.assertEqual(LIFECYCLE.task_binding(changed), expected)

    def test_task_binding_changes_for_identity_and_stable_artifact_mutations(self) -> None:
        task = self.base_task()
        expected = LIFECYCLE.task_binding(task)
        for field, value in {
            "proposal": "proposal-v2.md",
            "design": "design-v2.md",
            "testing": "testing-v2.md",
            "testplan": "testplan-v2.yaml",
            "acceptance_report": "acceptance-report-v2.md",
            "risk_profile": "risk-profile-v2.yaml",
            "completion_report": "completion-report-v2.md",
            "change_record": "change-record-v2.md",
            "lifecycle_state": "lifecycle-v2.json",
        }.items():
            with self.subTest(stable_artifact=field):
                changed = copy.deepcopy(task)
                changed[field] = value
                self.assertNotEqual(LIFECYCLE.task_binding(changed), expected)

        for field, value in {
            "id": "CHG-renamed-binding",
            "target_module": "different-module",
        }.items():
            with self.subTest(change_identity=field):
                changed = copy.deepcopy(task)
                changed["changes"][0][field] = value
                self.assertNotEqual(LIFECYCLE.task_binding(changed), expected)

    def lifecycle_fixture(
        self, directory: str, *, stale_inputs: bool = False
    ) -> tuple[Path, Path, dict[str, object]]:
        root = Path(directory)
        packet = (
            root
            / ".harness"
            / "tasks"
            / "v0.1"
            / "modules"
            / "globals"
            / "001-lifecycle-binding"
        )
        packet.mkdir(parents=True)
        task_path = packet / "task.yaml"
        task_path.write_text("fixture task manifest\n", encoding="utf-8")
        (packet / "proposal.md").write_text("approved proposal\n", encoding="utf-8")
        task = self.base_task()
        task["stage"] = "implementation"
        receipt = LIFECYCLE.receipt_payload(root, task_path, task, "proposal")
        receipt["task_binding_sha256"] = "legacy-task-binding"
        if stale_inputs:
            receipt["inputs"] = {
                path: "0" * 64 for path in receipt["inputs"]
            }
        state = {
            "schema_version": 1,
            "task_manifest": "task.yaml",
            "stages": {"proposal": receipt},
        }
        (packet / "lifecycle.json").write_text(
            json.dumps(state, indent=2) + "\n", encoding="utf-8"
        )
        return root, task_path, task

    def test_ordinary_verify_refuses_legacy_binding_without_mutating_state(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root, task_path, task = self.lifecycle_fixture(directory)
            state_path = task_path.parent / "lifecycle.json"
            before = state_path.read_bytes()
            stderr = io.StringIO()

            with contextlib.redirect_stderr(stderr), self.assertRaises(SystemExit):
                LIFECYCLE.verify_receipts(root, task_path, task, ("proposal",))

            self.assertIn("receipt has stale task binding", stderr.getvalue())
            self.assertEqual(state_path.read_bytes(), before)


if __name__ == "__main__":
    unittest.main()
