#!/usr/bin/env python3
"""Regression coverage for proposal checks at the auto-design boundary."""

from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "harness" / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

SPEC = importlib.util.spec_from_file_location(
    "harness_check_auto_design_regression",
    SCRIPTS / "harness-check.py",
)
assert SPEC is not None and SPEC.loader is not None
HARNESS_CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(HARNESS_CHECK)


class AutoPipelineProposalTransitionTests(unittest.TestCase):
    def commands(self, *, automatic_design: bool) -> list[list[str]]:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            packet = (
                root
                / "docs"
                / "versions"
                / "v0.1"
                / "modules"
                / "repo-governance"
                / "001-proposal-transition"
            )
            (packet / "pipeline").mkdir(parents=True)
            (packet / "pipeline" / "plan.md").write_text(
                "# fixture\n", encoding="utf-8"
            )
            manifest = packet / "task.yaml"
            manifest.write_text("schema_version: 1\n", encoding="utf-8")
            task = {
                "schema_version": 1,
                "workflow_tier": "high-risk",
                "version": "v0.1",
                "packet_module": "repo-governance",
                "task_name": "001-proposal-transition",
                "stage": "proposal",
                "mode": "auto-pipeline" if automatic_design else "manual",
                "auto_pipeline_start_stage": "design" if automatic_design else None,
                "proposal": "proposal.md",
                "pipeline_plan": "pipeline/plan.md" if automatic_design else None,
                "risk_profile": "risk-profile.yaml",
                "changed_paths_file": ".harness/evidence/proposal.paths",
                "changes": [
                    {
                        "id": "CHG-proposal-transition",
                        "target_module": "repo-governance",
                        "scope_paths": ["harness/scripts/harness-check.py"],
                        "changed_paths_file": ".harness/evidence/implementation.paths",
                    }
                ],
            }
            return HARNESS_CHECK.build_commands(root, manifest, task, "completion")

    @staticmethod
    def schema_command(commands: list[list[str]]) -> list[str]:
        matches = [
            command
            for command in commands
            if any(part.endswith("schema-check.py") for part in command)
        ]
        if len(matches) != 1:
            raise AssertionError(f"expected one schema command, got {matches!r}")
        return matches[0]

    def test_automatic_design_proposal_uses_launch_bound_schema_check(self) -> None:
        command = self.schema_command(self.commands(automatic_design=True))

        self.assertNotIn("--require-approved", command)

    def test_manual_proposal_still_requires_approval(self) -> None:
        command = self.schema_command(self.commands(automatic_design=False))

        self.assertIn("--require-approved", command)


if __name__ == "__main__":
    unittest.main()
