#!/usr/bin/env python3
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "tests/fixtures/sfo_cmd_server_0_4_consumer/Cargo.toml"


def main() -> int:
    completed = subprocess.run(
        [
            "cargo",
            "check",
            "--manifest-path",
            str(MANIFEST),
            "--example",
            "removed_vpn_control_api",
            "--features",
            "removed-vpn-control-api-negative",
            "--locked",
        ],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    output = completed.stdout + completed.stderr
    if completed.returncode == 0:
        print("old VpnServerClient PN control API unexpectedly compiled", file=sys.stderr)
        return 1
    expected = (
        "report_pn_traffic_stats",
        "report_proxy_heartbeat",
        "report_proxy_traffic",
        "validate_pn_connection",
    )
    missing = [symbol for symbol in expected if symbol not in output]
    if missing:
        print(
            "negative compilation failed for an unexpected reason; missing diagnostics: "
            + ", ".join(missing),
            file=sys.stderr,
        )
        print(output, file=sys.stderr)
        return 1
    print("vpn-control-client removed API negative contract: passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
