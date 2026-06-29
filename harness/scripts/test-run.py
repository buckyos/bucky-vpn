#!/usr/bin/env python3

from __future__ import annotations

import argparse
import datetime
import json
import subprocess
import sys
import time
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
LEVELS = ("unit", "dv", "integration")
RUN_ARTIFACT_SCHEMA = 1

RUNTIME_COMMANDS = {
    "vpn-frame": {
        "unit": [["cargo", "test", "-p", "vpn-frame"]],
        "dv": [["cargo", "build", "-p", "vpn-frame"]],
        "integration": [["cargo", "test", "--workspace"]],
    },
    "bucky-vpn": {
        "unit": [["cargo", "test", "-p", "bucky-vpn"]],
        "dv": [["cargo", "build", "-p", "bucky-vpn"]],
        "integration": [
            [
                "python3",
                "./harness/scripts/bucky-vpn-process-integration.py",
                "--use-base-image",
                "--parallel-instances",
                "2",
            ]
        ],
    },
    "bucky-vpn-server": {
        "unit": [["cargo", "test", "-p", "bucky-vpn-server"]],
        "dv": [["cargo", "build", "-p", "bucky-vpn-server"]],
        "integration": [["cargo", "test", "--workspace"]],
    },
    "vpn_web": {
        "unit": [["flutter", "test"]],
        "dv": [["flutter", "analyze"]],
        "integration": [["flutter", "build", "web"]],
    },
}

MODULE_PACKETS = {
    "repo-governance": "docs/versions/v0.1/modules/repo-governance",
    "vpn-frame": "docs/versions/v0.1/modules/vpn-frame",
    "bucky-vpn": "docs/versions/v0.1/modules/bucky-vpn",
    "bucky-vpn-server": "docs/versions/v0.1/modules/bucky-vpn-server",
    "vpn_web": "docs/versions/v0.1/modules/vpn_web",
}


def fail(message: str) -> int:
    print(f"ERROR: {message}", file=sys.stderr)
    return 1


def ensure_files(paths: list[str]) -> None:
    missing = [path for path in paths if not (REPO_ROOT / path).exists()]
    if missing:
        raise RuntimeError(f"missing required paths: {', '.join(missing)}")


def load_text(path: str) -> str:
    return (REPO_ROOT / path).read_text(encoding="utf-8")


def require_substrings(path: str, patterns: list[str]) -> None:
    text = load_text(path)
    missing = [pattern for pattern in patterns if pattern not in text]
    if missing:
        raise RuntimeError(f"{path} is missing required content: {', '.join(missing)}")


def parse_front_matter(path: str) -> dict[str, str]:
    lines = load_text(path).splitlines()
    if len(lines) < 3 or lines[0].strip() != "---":
        raise RuntimeError(f"{path} is missing YAML front matter")

    data: dict[str, str] = {}
    for line in lines[1:]:
        stripped = line.strip()
        if stripped == "---":
            break
        if ":" not in line:
            continue
        key, value = line.split(":", 1)
        data[key.strip()] = value.strip()
    return data


def run_command(command: list[str], cwd: Path | None = None) -> int:
    workdir = cwd or REPO_ROOT
    print(f"RUN {' '.join(command)}")
    result = subprocess.run(command, cwd=workdir, check=False)
    return result.returncode


def git_state() -> tuple[str | None, bool | None]:
    try:
        head = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        status = subprocess.run(
            ["git", "status", "--porcelain"],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError:
        return None, None
    if head.returncode != 0 or status.returncode != 0:
        return None, None
    return head.stdout.strip(), bool(status.stdout.strip())


def write_run_artifact(
    requested_module: str,
    requested_level: str,
    started_at: str,
    steps: list[dict[str, object]],
    exit_code: int,
) -> None:
    artifact_dir = REPO_ROOT / "test-results" / "test-runs"
    try:
        artifact_dir.mkdir(parents=True, exist_ok=True)
        timestamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
        artifact_path = artifact_dir / f"{timestamp}-{requested_module}-{requested_level}.json"
        head, dirty = git_state()
        artifact = {
            "schema": RUN_ARTIFACT_SCHEMA,
            "requested_module": requested_module,
            "requested_level": requested_level,
            "started_at": started_at,
            "finished_at": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
            "git_head": head,
            "worktree_dirty": dirty,
            "steps": steps,
            "exit_code": exit_code,
        }
        artifact_path.write_text(json.dumps(artifact, indent=2) + "\n", encoding="utf-8")
        print(f"test-run: run artifact written: {artifact_path}")
    except OSError as exc:
        print(f"test-run: warning: failed to write run artifact: {exc}", file=sys.stderr)


def run_repo_governance(level: str) -> int:
    required_paths = [
        "AGENTS.md",
        "docs/architecture/repository-workflow.md",
        "docs/architecture/cyfs-gateway-config-domain.md",
        "docs/modules/repo-governance.md",
        "docs/reviews/_template/acceptance-report.md",
        "docs/versions/v0.1/modules/_template/proposal.md",
        "docs/versions/v0.1/modules/_template/design.md",
        "docs/versions/v0.1/modules/_template/testing.md",
        "docs/versions/v0.1/modules/_template/testplan.yaml",
        "docs/versions/v0.1/modules/repo-governance/proposal.md",
        "docs/versions/v0.1/modules/repo-governance/design.md",
        "docs/versions/v0.1/modules/repo-governance/testing.md",
        "docs/versions/v0.1/modules/repo-governance/testplan.yaml",
        "docs/versions/v0.1/modules/repo-governance/acceptance.md",
        "harness/rules/proposal-doc-rules.md",
        "harness/rules/design-doc-rules.md",
        "harness/rules/testing-doc-rules.md",
        "harness/rules/implementation-admission-rules.md",
        "harness/rules/trigger-based-validation-rules.md",
        "harness/rules/unified-test-entry-rules.md",
        "harness/rules/acceptance-task-rules.md",
        "harness/rules/cyfs-gateway-config-spec-rules.md",
        "harness/rules/vpn-web-no-new-tests-rule.md",
        "harness/process_rules/cyfs-gateway-config-task.md",
        "harness/process_rules/implementation-task.md",
        "harness/checklists/cyfs-gateway-config-review-checklist.md",
        "harness/human-rules/contribution-modes.md",
        "harness/human-rules/module-tier-matrix.md",
        "harness/scripts/test-run.py",
    ]
    for packet_dir in MODULE_PACKETS.values():
        required_paths.extend(
            [
                f"{packet_dir}/proposal.md",
                f"{packet_dir}/design.md",
                f"{packet_dir}/testing.md",
                f"{packet_dir}/testplan.yaml",
                f"{packet_dir}/acceptance.md",
            ]
        )
    ensure_files(required_paths)

    if level == "unit":
        print("repo-governance unit checks passed")
        return 0

    packet_paths = [
        "docs/versions/v0.1/modules/repo-governance/proposal.md",
        "docs/versions/v0.1/modules/repo-governance/design.md",
        "docs/versions/v0.1/modules/repo-governance/testing.md",
        "docs/versions/v0.1/modules/repo-governance/acceptance.md",
    ]
    for path in packet_paths:
        front_matter = parse_front_matter(path)
        if front_matter.get("module") != "repo-governance":
            raise RuntimeError(f"{path} has unexpected module metadata")
        if front_matter.get("version") != "v0.1":
            raise RuntimeError(f"{path} has unexpected version metadata")
        if front_matter.get("status") != "approved":
            raise RuntimeError(f"{path} must be approved for this representative packet")

    require_substrings(
        "docs/versions/v0.1/modules/repo-governance/testplan.yaml",
        ["schema_version: 1", "module: repo-governance", "levels:"],
    )
    require_substrings(
        "docs/versions/v0.1/modules/repo-governance/testing.md",
        ["协作治理", "trigger-based validation"],
    )

    if level == "dv":
        print("repo-governance dv checks passed")
        return 0

    require_substrings(
        "AGENTS.md",
        [
            "docs/versions/v0.1/modules/<module>/",
            "docs/architecture/cyfs-gateway-config-domain.md",
            "harness/rules/cyfs-gateway-config-spec-rules.md",
            "harness/process_rules/cyfs-gateway-config-task.md",
            "harness/process_rules/implementation-task.md",
            "harness/rules/trigger-based-validation-rules.md",
            "harness/human-rules/contribution-modes.md",
            "harness/human-rules/module-tier-matrix.md",
            "harness/rules/vpn-web-no-new-tests-rule.md",
        ],
    )
    require_substrings(
        "docs/architecture/repository-workflow.md",
        [
            "harness/process_rules/",
            "harness/human-rules/",
            "harness/rules/trigger-based-validation-rules.md",
        ],
    )

    module_docs = [
        "docs/modules/bucky-vpn.md",
        "docs/modules/vpn-frame.md",
        "docs/modules/bucky-vpn-server.md",
        "docs/modules/vpn_web.md",
        "docs/modules/repo-governance.md",
    ]
    ensure_files(module_docs)

    for module, packet_dir in MODULE_PACKETS.items():
        for path in [
            f"{packet_dir}/proposal.md",
            f"{packet_dir}/design.md",
            f"{packet_dir}/testing.md",
            f"{packet_dir}/acceptance.md",
        ]:
            front_matter = parse_front_matter(path)
            if front_matter.get("module") != module:
                raise RuntimeError(f"{path} has unexpected module metadata")
            if front_matter.get("version") != "v0.1":
                raise RuntimeError(f"{path} has unexpected version metadata")

    print("repo-governance integration checks passed")
    return 0


def run_module_level(module: str, level: str, steps: list[dict[str, object]], dry_run: bool = False) -> int:
    if module == "repo-governance":
        if dry_run:
            print(f"DRY-RUN repo-governance {level}")
            steps.append(
                {
                    "module": module,
                    "level": level,
                    "command": ["repo-governance", level],
                    "exit_code": 0,
                    "duration_s": 0,
                }
            )
            return 0
        started = time.monotonic()
        try:
            code = run_repo_governance(level)
        except RuntimeError as exc:
            print(f"ERROR: {exc}", file=sys.stderr)
            code = 1
        steps.append(
            {
                "module": module,
                "level": level,
                "command": ["repo-governance", level],
                "exit_code": code,
                "duration_s": round(time.monotonic() - started, 3),
            }
        )
        return code

    commands = RUNTIME_COMMANDS.get(module)
    if not commands:
        available = ", ".join(sorted(["repo-governance", *RUNTIME_COMMANDS.keys()]))
        print(f"ERROR: unknown module '{module}'. available modules: {available}", file=sys.stderr)
        return 1

    workdir = REPO_ROOT / "vpn_web" if module == "vpn_web" else REPO_ROOT
    for command in commands[level]:
        if dry_run:
            print(f"DRY-RUN {' '.join(command)}")
            steps.append(
                {
                    "module": module,
                    "level": level,
                    "command": command,
                    "exit_code": 0,
                    "duration_s": 0,
                }
            )
            continue
        started = time.monotonic()
        code = run_command(command, cwd=workdir)
        steps.append(
            {
                "module": module,
                "level": level,
                "command": command,
                "exit_code": code,
                "duration_s": round(time.monotonic() - started, 3),
            }
        )
        if code != 0:
            return code
    return 0


def main() -> int:
    global REPO_ROOT
    parser = argparse.ArgumentParser(description="Run repository test entrypoints by module and level.")
    parser.add_argument("module", help="Module name, for example repo-governance or vpn-frame, or all.")
    parser.add_argument("level", choices=sorted([*LEVELS, "all"]), help="Validation level to run.")
    parser.add_argument("--list", action="store_true", help="List known modules and exit.")
    parser.add_argument("--root", default=None, help="Repository root for checker dry-runs.")
    parser.add_argument("--dry-run", action="store_true", help="Print reachable commands without executing them.")
    args = parser.parse_args()

    if args.root:
        REPO_ROOT = Path(args.root).resolve()

    known_modules = ["repo-governance", *RUNTIME_COMMANDS.keys()]
    if args.list:
        for module in sorted(known_modules):
            print(module)
        return 0

    selected_modules = known_modules if args.module == "all" else [args.module]
    selected_levels = list(LEVELS) if args.level == "all" else [args.level]
    started_at = datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds")
    steps: list[dict[str, object]] = []

    exit_code = 0
    for module in selected_modules:
        for level in selected_levels:
            exit_code = run_module_level(module, level, steps, dry_run=args.dry_run)
            if exit_code != 0:
                write_run_artifact(args.module, args.level, started_at, steps, exit_code)
                return exit_code

    if args.dry_run:
        return 0

    write_run_artifact(args.module, args.level, started_at, steps, exit_code)
    return exit_code


if __name__ == "__main__":
    raise SystemExit(main())
