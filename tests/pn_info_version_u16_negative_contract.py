#!/usr/bin/env python3
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "tests/fixtures/pn_info_version_consumer/Cargo.toml"


def reject_legacy_example(example: str, feature: str, expected: list[str]) -> bool:
    completed = subprocess.run(
        [
            "cargo",
            "check",
            "--manifest-path",
            str(MANIFEST),
            "--example",
            example,
            "--features",
            feature,
            "--locked",
            "--target-dir",
            str(ROOT / ".harness/target/016-pn-info-version-negative"),
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    output = completed.stdout + completed.stderr
    if completed.returncode == 0:
        print(f"legacy {example} pn_info_version consumer unexpectedly compiled", file=sys.stderr)
        return False
    missing = [diagnostic for diagnostic in expected if diagnostic not in output]
    if missing:
        print(
            "negative compilation failed for an unexpected reason; missing diagnostics: "
            + ", ".join(missing),
            file=sys.stderr,
        )
        print(output, file=sys.stderr)
        return False
    return True


def main() -> int:
    contracts = [
        (
            "old_u16_negative",
            "old-u16-negative",
            [
                "expected `u32`, found `u16`",
                "expected `u16`, found `u32`",
                "pn_info_version",
            ],
        ),
        (
            "old_u64_negative",
            "old-u64-negative",
            [
                "expected `u32`, found `u64`",
                "expected `u64`, found `u32`",
                "pn_info_version",
            ],
        ),
    ]
    if not all(reject_legacy_example(*contract) for contract in contracts):
        return 1
    print("pn_info_version legacy u16/u64 negative contracts: passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
