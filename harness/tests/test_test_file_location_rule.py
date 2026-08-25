#!/usr/bin/env python3
"""Regression coverage for the independent-test-file placement rule."""

from __future__ import annotations

import subprocess
import shutil
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CONTEXT = ROOT / "harness" / "scripts" / "context.py"
RULE = "harness/custom-rules/test-file-location-rule.md"


class TestFileLocationRuleTests(unittest.TestCase):
    def run_context(self, *arguments: str) -> list[str]:
        completed = subprocess.run(
            [sys.executable, str(CONTEXT), "--root", str(ROOT), *arguments, "--format", "paths"],
            cwd=ROOT,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        return [line.strip() for line in completed.stdout.splitlines() if line.strip()]

    def test_custom_rule_index_is_valid(self) -> None:
        subprocess.run(
            [sys.executable, str(CONTEXT), "--root", str(ROOT), "--validate-index"],
            cwd=ROOT,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )

    def test_invalid_rule_reference_fails_index_validation(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture_root = Path(directory)
            shutil.copytree(ROOT / "harness" / "rules", fixture_root / "harness" / "rules")
            shutil.copytree(
                ROOT / "harness" / "custom-rules",
                fixture_root / "harness" / "custom-rules",
            )
            index = fixture_root / "harness" / "custom-rules" / "index.yaml"
            index.write_text(
                index.read_text(encoding="utf-8").replace(
                    "harness/custom-rules/test-file-location-rule.md",
                    "harness/custom-rules/missing-test-file-location-rule.md",
                    1,
                ),
                encoding="utf-8",
            )

            completed = subprocess.run(
                [sys.executable, str(CONTEXT), "--root", str(fixture_root), "--validate-index"],
                cwd=ROOT,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )

            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("missing", completed.stderr.lower())

    def test_rule_routes_for_root_and_module_test_paths_in_all_governed_stages(self) -> None:
        cases = (
            (
                "root src in implementation",
                ("--workflow-tier", "high-risk", "--stage", "implementation", "--mode", "auto-pipeline", "--auto-pipeline-start-stage", "implementation", "--module", "repo-governance", "--trigger", "test", "--changed-path", "src/example_tests.rs"),
            ),
            (
                "module src in testing",
                ("--workflow-tier", "high-risk", "--stage", "testing", "--mode", "manual", "--module", "repo-governance", "--trigger", "testing", "--changed-path", "vpn-frame/src/client/example_tests.rs"),
            ),
            (
                "root tests in acceptance",
                ("--workflow-tier", "standard", "--stage", "acceptance", "--mode", "manual", "--module", "repo-governance", "--changed-path", "tests/example_test.py"),
            ),
            (
                "module tests in implementation",
                ("--workflow-tier", "trivial", "--stage", "implementation", "--mode", "manual", "--module", "repo-governance", "--changed-path", "vpn-frame/tests/client/example.rs"),
            ),
        )

        for label, arguments in cases:
            with self.subTest(label=label):
                paths = self.run_context(*arguments)
                self.assertIn(RULE, paths)
                generated_rule_positions = [
                    index for index, path in enumerate(paths) if path.startswith("harness/rules/")
                ]
                if generated_rule_positions:
                    self.assertLess(paths.index(RULE), min(generated_rule_positions))

    def test_rule_does_not_route_during_design(self) -> None:
        paths = self.run_context(
            "--workflow-tier",
            "high-risk",
            "--stage",
            "design",
            "--mode",
            "manual",
            "--module",
            "repo-governance",
            "--trigger",
            "test",
            "--changed-path",
            "src/example_tests.rs",
        )

        self.assertNotIn(RULE, paths)

    def test_rule_text_preserves_the_approved_boundary(self) -> None:
        text = (ROOT / RULE).read_text(encoding="utf-8")

        self.assertIn("独立测试文件禁止放在 `src` 目录及其任意子目录中", text)
        self.assertIn("必须放在与对应 `src` 目录同级的 `tests` 目录", text)
        self.assertIn("内联测试块不是独立测试文件", text)
        self.assertIn("如果当前任务触及了已有的违规独立测试文件", text)


if __name__ == "__main__":
    unittest.main()
