"""Reproduce held-worker retention against pinned original runtime sources."""
from pathlib import Path
import hashlib
import json
import os
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[3]
PROVENANCE = json.loads(Path(__file__).with_name("idle_baseline.json").read_text(encoding="utf-8"))
BASELINE = PROVENANCE["commit"]
MARKER = "idle-held-sender-not-reclaimed"


def main():
    outputs = ROOT / ".harness" / "evidence" / "idle-send-red-green"
    outputs.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="baseline-", dir=outputs) as temporary:
        fixture = Path(temporary)
        source = fixture / "src"
        source.mkdir()
        provenance = {}
        for name in ("tunnel_manager", "packet_dispatcher"):
            path = f"vpn-frame/src/client/{name}.rs"
            original_bytes = subprocess.check_output(["git", "show", f"{BASELINE}:{path}"], cwd=ROOT)
            assert hashlib.sha256(original_bytes).hexdigest() == PROVENANCE["sha256"][path]
            original = original_bytes.decode("utf-8")
            transformed = original
            if name == "tunnel_manager":
                old = "Duration::from_secs(10 * 60)"
                assert original.count(old) == 1, "baseline timeout normalization must be exact"
                transformed = original.replace(old, "Duration::from_millis(120)")
                assert transformed.replace("Duration::from_millis(120)", old) == original
            (source / f"{name}.rs").write_text(transformed, encoding="utf-8")
            provenance[path] = "original" if transformed == original else "only idle timeout normalized from 600s to 120ms"
        support = (ROOT / "vpn-frame/tests/support/idle_dv.rs").as_posix()
        (source / "lib.rs").write_text(
            '#![allow(dead_code, unexpected_cfgs, hidden_glob_reexports)]\npub use vpn_frame::*;\n'
            'mod client {\n'
            'mod tunnel_manager { include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/tunnel_manager.rs")); }\n'
            'mod packet_dispatcher { include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/packet_dispatcher.rs")); }\n'
            f'#[path = "{support}"] mod idle_dv;\n}}\n', encoding="utf-8")
        package = (ROOT / "vpn-frame").as_posix()
        (fixture / "Cargo.toml").write_text(
            '[package]\nname="idle-baseline-regression"\nversion="0.0.0"\nedition="2024"\n'
            '[workspace]\n[features]\nidle_baseline=[]\n[dependencies]\n'
            f'vpn-frame={{path="{package}"}}\n'
            'tokio={version="1",features=["full"]}\nasync-trait="0.1.85"\n'
            'bucky-raw-codec={version="0.1",features=["derive"]}\n'
            'sfo-pool="0.2.1"\nlog="0.4.25"\npnet_packet="0.35.0"\ncallback-trait="0.1.3"\n', encoding="utf-8")
        env = os.environ.copy()
        env["CARGO_TARGET_DIR"] = str(ROOT / "target")
        command = ["cargo", "test", "--offline", "--manifest-path", str(fixture / "Cargo.toml"),
                   "--features", "idle_baseline", "dispatcher_held_idle_channel_is_reclaimed_and_first_packet_reconnects", "--", "--nocapture"]
        result = subprocess.run(command, cwd=ROOT, env=env, capture_output=True, text=True, encoding="utf-8", errors="replace")
        combined = result.stdout + result.stderr
        (outputs / "baseline-output.log").write_text(combined, encoding="utf-8")
        observed = result.returncode != 0 and MARKER in combined and "test result: FAILED. 0 passed; 1 failed" in combined
        (outputs / "baseline-result.json").write_text(json.dumps({
            "baseline": BASELINE, "transformations": provenance, "expected_failure": MARKER,
            "observed": observed, "exit_code": result.returncode,
            "green_step": "task-scoped dv-current-runtime",
        }, indent=2), encoding="utf-8")
        if not observed:
            print(combined)
            raise SystemExit("baseline did not fail for the required runtime resource-retention assertion")
        print(f"Expected runtime regression observed at {BASELINE}: {MARKER}; original sender drop count remained zero.")


if __name__ == "__main__":
    main()
