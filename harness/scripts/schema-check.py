#!/usr/bin/env python3
"""Validate the generated Harness Engineering module packet shape.

This template intentionally uses only the Python standard library. Adapt paths
or stricter YAML parsing after the target repository chooses its dependencies.
Proposal.md and design.md are the mandatory implementation inputs.
testplan.yaml is validated when present; testing-coverage-check.py enforces it
for completed testing work.
"""

from __future__ import annotations

import argparse
import ast
import importlib.util
import json
import re
import sys
from pathlib import Path

from task_manifest import TaskManifestError, parse_task_manifest


REQUIRED_DOCUMENT_FRONT_MATTER = ("status",)
ALLOWED_LEVELS = {"unit", "dv", "integration"}
ALLOWED_MODES = {"enabled", "manual", "disabled"}
CONTRACT_KINDS = {
    "external-positive",
    "external-negative",
    "removed-symbol-scan",
    "repository-compile-closure",
    "documentation-examples",
}
CONTRACT_ASSERTIONS = {
    "external-positive": "new-path-compiles",
    "external-negative": "old-path-rejected-for-removed-symbol",
    "removed-symbol-scan": "no-unallowlisted-old-symbol-references",
    "repository-compile-closure": "repository-consumers-compile",
    "documentation-examples": "documentation-examples-compile",
}
PUBLIC_API_IMPACTS = {"none", "backward-compatible", "migration-required", "breaking"}
ALLOWED_DOCUMENT_STATUSES = {"draft", "approved", "rejected", "superseded"}


def load_task_index_module():
    path = Path(__file__).with_name("task-index.py")
    if not path.is_file():
        path = Path(__file__).with_name("task-index.template.py")
    spec = importlib.util.spec_from_file_location("task_index", path)
    if spec is None or spec.loader is None:
        fail(f"cannot load required sibling script: {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module
TASK_NAME_RE = re.compile(r"^\d{3,}-[a-z0-9][a-z0-9_.-]*$")


def fail(message: str) -> None:
    print(f"schema-check: {message}", file=sys.stderr)
    raise SystemExit(1)


def read_text(path: Path) -> str:
    if not path.exists():
        fail(f"missing required file: {path}")
    return path.read_text(encoding="utf-8")


def front_matter(text: str, path: Path) -> dict[str, str]:
    if not text.startswith("---\n"):
        fail(f"missing front matter: {path}")
    end = text.find("\n---", 4)
    if end == -1:
        fail(f"unterminated front matter: {path}")
    data: dict[str, str] = {}
    for line in text[4:end].splitlines():
        if ":" not in line:
            continue
        key, value = line.split(":", 1)
        data[key.strip()] = value.strip()
    return data


def validate_task_name(value: str, label: str) -> None:
    if not TASK_NAME_RE.fullmatch(value):
        fail(
            f"{label} must match <task-seq>-<task-slug> with a 3+ digit version-local "
            f"sequence prefix, for example 001-example-task: {value}"
        )


def validate_doc(
    root: Path,
    path: Path,
    module: str,
    version: str,
    submodule: str | None = None,
    *,
    require_approved: bool = False,
) -> None:
    text = read_text(path)
    data = front_matter(text, path)
    missing = [field for field in REQUIRED_DOCUMENT_FRONT_MATTER if field not in data]
    if missing:
        fail(f"{path} missing front matter fields: {', '.join(missing)}")
    manifest_ref = data.get("task_manifest")
    if manifest_ref != "task.yaml":
        fail(f"{path} must use task_manifest: task.yaml")
    if not (path.parent / manifest_ref).is_file():
        fail(f"{path} references missing task manifest: {path.parent / manifest_ref}")
    duplicated = [field for field in ("module", "version", "task_name", "submodule") if field in data]
    if duplicated:
        fail(f"{path} duplicates canonical task identity fields: {', '.join(duplicated)}")
    if submodule:
        validate_task_name(submodule, "--submodule")
    status = data.get("status", "").strip()
    if status not in ALLOWED_DOCUMENT_STATUSES:
        fail(
            f"{path} status must be one of: "
            + ", ".join(sorted(ALLOWED_DOCUMENT_STATUSES))
        )
    if require_approved and status != "approved":
        fail(f"{path} must be approved before implementation; got status: {status}")


def extract_level_blocks(text: str) -> dict[str, str]:
    match = re.search(r"(?m)^levels:\s*$", text)
    if not match:
        fail("testplan.yaml missing levels")
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


def validate_testplan(path: Path, module: str, version: str, submodule: str | None = None) -> None:
    text = read_text(path)
    step_ids: set[str] = set()
    manifest_bound = bool(re.search(r"(?m)^task_manifest:\s*task\.yaml\s*$", text))
    if not manifest_bound:
        fail(f"{path} must use task_manifest: task.yaml")
    if not (path.parent / "task.yaml").is_file():
        fail(f"{path} references missing task manifest: {path.parent / 'task.yaml'}")
    duplicated = [
        field for field in ("module", "version", "task_name", "submodule")
        if re.search(rf"(?m)^{field}:\s*\S+", text)
    ]
    if duplicated:
        fail(f"{path} duplicates canonical task identity fields: {', '.join(duplicated)}")
    required_bindings = (("schema_version", "1"),)
    for key, value in required_bindings:
        if not re.search(rf"(?m)^{re.escape(key)}:\s*{re.escape(value)}\s*$", text):
            fail(f"{path} missing or mismatched {key}: {value}")
    task_name = submodule
    if task_name:
        validate_task_name(task_name, f"{path} task_name")

    impact = re.search(r"(?ms)^api_impact:\s*$\n(.*?)(?=^[A-Za-z0-9_-]+:\s*|\Z)", text)
    if not impact:
        fail(f"{path} missing api_impact")
    impact_body = impact.group(1)
    public_api = re.search(r"(?m)^  public_api:\s*([A-Za-z0-9_-]+)\s*$", impact_body)
    if not public_api or public_api.group(1) not in PUBLIC_API_IMPACTS:
        fail(f"{path} api_impact.public_api must be one of: {', '.join(sorted(PUBLIC_API_IMPACTS))}")
    for key in ("crate_root_export_change", "build_surface_change", "documentation_examples_affected"):
        if not re.search(rf"(?m)^  {re.escape(key)}:\s*(true|false)\s*$", impact_body):
            fail(f"{path} api_impact.{key} must be true or false")

    evidence_inputs = re.search(r"(?m)^evidence_inputs:\s*(\[[^\n]*\])\s*$", text)
    if not evidence_inputs:
        fail(f"{path} missing inline evidence_inputs list")
    try:
        parsed_inputs = ast.literal_eval(evidence_inputs.group(1))
    except (SyntaxError, ValueError) as error:
        fail(f"{path} evidence_inputs is invalid: {error}")
    if not isinstance(parsed_inputs, list) or not parsed_inputs or not all(
        isinstance(item, str) and item not in {"", "."} for item in parsed_inputs
    ):
        fail(f"{path} evidence_inputs must be a non-empty list of repository-relative paths")
    if any(Path(item).is_absolute() or any(part == ".." for part in Path(item).parts) for item in parsed_inputs):
        fail(f"{path} evidence_inputs must stay inside the repository")

    contract = re.search(r"(?ms)^contract_checks:\s*$\n(.*?)(?=^[A-Za-z0-9_-]+:\s*|\Z)", text)
    if not contract:
        fail(f"{path} missing contract_checks")
    contract_body = contract.group(1)
    contract_mode = re.search(r"(?m)^  mode:\s*(enabled|disabled)\s*$", contract_body)
    if not contract_mode:
        fail(f"{path} contract_checks.mode must be enabled or disabled")
    contract_ids = re.findall(r"(?m)^    - id:\s*([A-Za-z0-9_.-]+)\s*$", contract_body)
    if contract_mode.group(1) == "disabled":
        if not re.search(r"(?m)^  reason:\s*\S.+$", contract_body):
            fail(f"{path} disabled contract_checks require a concrete reason")
        if contract_ids:
            fail(f"{path} disabled contract_checks must not declare steps")
    elif not contract_ids:
        fail(f"{path} enabled contract_checks require steps")
    for step_id in contract_ids:
        if step_id in step_ids:
            fail(f"{path} duplicate step id: {step_id}")
        step_ids.add(step_id)
        start = re.search(rf"(?m)^    - id:\s*{re.escape(step_id)}\s*$", contract_body)
        assert start is not None
        following = re.search(r"(?m)^    - id:\s*", contract_body[start.end() :])
        end = start.end() + following.start() if following else len(contract_body)
        block = contract_body[start.end() : end]
        kind = re.search(r"(?m)^      kind:\s*([A-Za-z0-9_-]+)\s*$", block)
        if not kind or kind.group(1) not in CONTRACT_KINDS:
            fail(f"{path} contract step {step_id} has missing or unsupported kind")
        assertion = re.search(r"(?m)^      assertion:\s*([A-Za-z0-9_-]+)\s*$", block)
        if not assertion or assertion.group(1) != CONTRACT_ASSERTIONS[kind.group(1)]:
            fail(f"{path} contract step {step_id} assertion does not match kind {kind.group(1)}")
        for key in ("name", "change_ids", "run"):
            pattern = rf"(?m)^      {key}:\s*\S.+$" if key == "name" else rf"(?m)^      {key}:\s*\[.+\]\s*$"
            if not re.search(pattern, block):
                fail(f"{path} contract step {step_id} must define {key}")

    blocks = extract_level_blocks(text)
    unknown = set(blocks) - ALLOWED_LEVELS
    if unknown:
        fail(f"{path} has unknown test levels: {', '.join(sorted(unknown))}")

    for level in sorted(ALLOWED_LEVELS):
        if level not in blocks:
            fail(f"{path} missing test level: {level}")
        block = blocks[level]
        mode_match = re.search(r"(?m)^    mode:\s*([A-Za-z0-9_-]+)\s*$", block)
        if not mode_match:
            fail(f"{path} level {level} missing mode")
        mode = mode_match.group(1)
        if mode not in ALLOWED_MODES:
            fail(f"{path} level {level} has invalid mode: {mode}")

        ids = re.findall(r"(?m)^      - id:\s*([A-Za-z0-9_.-]+)\s*$", block)
        if mode == "enabled" and not ids:
            fail(f"{path} enabled level {level} has no steps")
        if mode in {"manual", "disabled"} and not re.search(r"(?mi)reason:\s*\S+", block):
            fail(f"{path} {mode} level {level} missing reason")
        for step_id in ids:
            if step_id in step_ids:
                fail(f"{path} duplicate step id: {step_id}")
            step_ids.add(step_id)
            step_pattern = (
                rf"(?ms)^      - id:\s*{re.escape(step_id)}\s*$"
                rf".*?^        name:\s*\S+"
                rf".*?^        change_ids:\s*\[.+\]\s*$"
                rf".*?^        run:\s*\[.+\]\s*$"
            )
            if not re.search(step_pattern, block):
                fail(f"{path} step {step_id} must define name, change_ids, and run")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", default=".")
    parser.add_argument("--version")
    parser.add_argument("--module")
    parser.add_argument("--submodule")
    parser.add_argument(
        "--require-approved",
        action="store_true",
        help="require mandatory proposal/design documents to be approved",
    )
    args = parser.parse_args()

    root = Path(args.root)
    if not (args.version and args.module):
        fail("--version and --module are required")
    if args.submodule:
        validate_task_name(args.submodule, "--submodule")

    task_index = load_task_index_module()
    task_index.load_index(root.resolve(), args.version)
    packet = root / ".harness" / "tasks" / args.version / "modules" / args.module
    if args.submodule:
        packet = packet / args.submodule
    manifest = packet / "task.yaml"
    try:
        task = parse_task_manifest(manifest) if manifest.is_file() else {}
    except TaskManifestError as error:
        fail(str(error))
    stage = task.get("stage")
    design_required = stage != "proposal"
    required_docs = ["proposal.md"] + (["design.md"] if design_required else [])
    for name in required_docs:
        validate_doc(
            root,
            packet / name,
            args.module,
            args.version,
            args.submodule,
            require_approved=args.require_approved,
        )
    optional_testing = packet / "testing.md"
    if (
        stage in {"testing", "acceptance"}
        and optional_testing.exists()
    ):
        validate_doc(root, optional_testing, args.module, args.version, args.submodule)
    optional_testplan = packet / "testplan.yaml"
    if stage in {"testing", "acceptance"} and optional_testplan.exists():
        validate_testplan(optional_testplan, args.module, args.version, args.submodule)
    print("schema-check: passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
