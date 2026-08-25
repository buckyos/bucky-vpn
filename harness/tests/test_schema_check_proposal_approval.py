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
            / "docs"
            / "versions"
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
mode: manual
auto_pipeline_start_stage:
changes:
  - id: CHG-proposal-approval-gate
    target_module: repo-governance
    scope_paths: [\"harness/scripts/schema-check.py\"]
""",
            encoding="utf-8",
        )
        index = self.root / "docs" / "versions" / "v0.1" / "modules" / "tasks.json"
        index.write_text(
            json.dumps(
                {
                    "schema_version": 1,
                    "version": "v0.1",
                    "tasks": [
                        {
                            "task_id": "001-proposal-approval",
                            "task_manifest": (
                                "docs/versions/v0.1/modules/repo-governance/"
                                "001-proposal-approval/task.yaml"
                            ),
                        }
                    ],
                }
            ),
            encoding="utf-8",
        )

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
                "python3",
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


class VendoredSwaggerUiResolutionTests(unittest.TestCase):
    def test_application_manifests_enable_vendored_swagger_ui_9_0_2(self) -> None:
        for relative in ("vpn-client/Cargo.toml", "vpn-server/Cargo.toml"):
            with self.subTest(manifest=relative):
                with (ROOT / relative).open("rb") as source:
                    manifest = tomllib.load(source)
                dependency = manifest["dependencies"]["utoipa-swagger-ui"]
                self.assertEqual(dependency["version"], "9.0.2")
                self.assertIn("vendored", dependency["features"])

    def test_lockfile_links_applications_to_vendored_swagger_ui(self) -> None:
        with (ROOT / "Cargo.lock").open("rb") as source:
            packages = tomllib.load(source)["package"]

        by_name = {package["name"]: package for package in packages}
        for application in ("bucky-vpn", "bucky-vpn-server"):
            with self.subTest(application=application):
                self.assertIn(
                    "utoipa-swagger-ui",
                    by_name[application].get("dependencies", []),
                )

        swagger_ui = by_name["utoipa-swagger-ui"]
        self.assertEqual(swagger_ui["version"], "9.0.2")
        self.assertIn(
            "utoipa-swagger-ui-vendored",
            swagger_ui.get("dependencies", []),
        )
        self.assertIn("utoipa-swagger-ui-vendored", by_name)


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
            "mode": "manual",
            "auto_pipeline_start_stage": None,
            "proposal": "proposal.md",
            "design": "design.md",
            "testing": "testing.md",
            "testplan": "testplan.yaml",
            "acceptance_report": "acceptance-report.md",
            "risk_profile": "risk-profile.yaml",
            "completion_report": "completion-report.md",
            "change_record": "change-record.md",
            "pipeline_plan": "pipeline/plan.md",
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
            "scope_paths": ["harness/tests/replacement.py"],
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
            / "docs"
            / "versions"
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
        task["mode"] = "auto-pipeline"
        task["auto_pipeline_start_stage"] = "design"
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

    def test_refresh_migrates_legacy_binding_when_receipt_inputs_are_current(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root, task_path, task = self.lifecycle_fixture(directory)

            with mock.patch.object(LIFECYCLE, "run_pipeline_check") as pipeline_check:
                refreshed = LIFECYCLE.refresh_manual_bindings(root, task_path, task)

            self.assertEqual(refreshed, ("proposal",))
            pipeline_check.assert_called_once_with(root, task_path, task, complete=False)
            state = json.loads((task_path.parent / "lifecycle.json").read_text(encoding="utf-8"))
            self.assertEqual(
                state["stages"]["proposal"]["task_binding_sha256"],
                LIFECYCLE.task_binding(task),
            )

    def test_refresh_refuses_stale_receipt_inputs_without_mutating_state(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root, task_path, task = self.lifecycle_fixture(directory, stale_inputs=True)
            state_path = task_path.parent / "lifecycle.json"
            before = state_path.read_bytes()
            stderr = io.StringIO()

            with mock.patch.object(LIFECYCLE, "run_pipeline_check"):
                with contextlib.redirect_stderr(stderr), self.assertRaises(SystemExit):
                    LIFECYCLE.refresh_manual_bindings(root, task_path, task)

            self.assertIn("receipt inputs are missing or stale", stderr.getvalue())
            self.assertEqual(state_path.read_bytes(), before)

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
