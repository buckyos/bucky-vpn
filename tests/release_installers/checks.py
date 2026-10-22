"""Task-scoped installer checks; generated outputs never enter the source tree."""
from __future__ import annotations

import copy
import hashlib
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
OUTPUT = ROOT / ".harness" / "test-results" / "release-installers"


def execute(argv: list[str]) -> None:
    subprocess.run(argv, cwd=ROOT, check=True, timeout=1000)


def wsl_path(path: Path) -> str:
    drive, tail = path.resolve().as_posix().split(":", 1)
    return "/mnt/" + drive.lower() + tail


def ps(mode: str, directory: Path) -> None:
    execute(["powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File",
             str(HERE / ("progress.ps1" if mode == "progress" else "powershell.ps1")),
             mode, str(ROOT), str(directory)])


def unix(mode: str, directory: Path) -> None:
    execute(["wsl.exe", "-d", "Ubuntu-24.04", "--", "bash", wsl_path(HERE / "unix.sh"),
             mode, wsl_path(ROOT), wsl_path(directory)])


def release() -> dict:
    names = ["BuckyVPN_2.3.4_amd64_Setup.exe", "bucky-vpn_2.3.4_amd64.deb", "BuckyVPN-2.3.4.pkg"]
    return {"tag_name": "v2.3.4", "draft": False, "prerelease": False, "assets": [
        {"name": name, "state": "uploaded", "digest": "sha256:" + hashlib.sha256(b"package").hexdigest(),
         "browser_download_url": "https://github.com/buckyos/bucky-vpn/releases/download/v2.3.4/" + name}
        for name in names]}


def fixtures(directory: Path) -> None:
    cases = {"valid": release()}
    for key, value in [("draft", True), ("prerelease", True), ("tag_name", "v2.3"),
                       ("assets", []), ("draft", "false"), ("tag_name", "v2.3.4/evil")]:
        item = release()
        item[key] = value
        cases["invalid-" + key + "-" + str(len(cases))] = item
    for key, value in [("digest", None), ("digest", "sha256:abc"), ("state", "new"),
                       ("browser_download_url", "https://evil.example/package"),
                       ("browser_download_url", "http://github.com/buckyos/bucky-vpn/package")]:
        item = release()
        for asset in item["assets"]:
            asset[key] = value
        cases["invalid-asset-" + str(len(cases))] = item
    item = release()
    item["assets"] *= 2
    cases["invalid-duplicates"] = item
    item = release()
    del item["prerelease"]
    cases["invalid-missing-field"] = item
    cases["invalid-array"] = [release()]
    cases["invalid-empty"] = {}
    item = release()
    item["tag_name"] = "2.3.4"
    for asset in item["assets"]:
        asset["browser_download_url"] = asset["browser_download_url"].replace("/v2.3.4/", "/2.3.4/")
    cases["valid-no-v"] = item
    for name, value in cases.items():
        (directory / (name + ".json")).write_text(json.dumps(value), encoding="utf-8")
    (directory / "invalid-json.json").write_text("{bad json", encoding="utf-8")
    print(f"Generated {len(cases) + 1} metadata scenarios", flush=True)


def docs() -> None:
    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    windows = re.search(r"```powershell\n(.*?)\n```", readme, re.S).group(1)
    assert "https://raw.githubusercontent.com/buckyos/bucky-vpn/master/install.ps1" in windows
    assert "powershell.exe -NoProfile -ExecutionPolicy Bypass -File $script" in windows
    assert "-ErrorAction Stop" in windows and "finally" in windows
    unix_commands = re.findall(r"```bash\n(curl --fail.*?bash install.sh)\n```", readme, re.S)
    assert len(unix_commands) == 2
    for command in unix_commands:
        assert "https://raw.githubusercontent.com/buckyos/bucky-vpn/master/install.sh" in command
        assert "&&\nbash install.sh" in command and "--proto-redir '=https'" in command
    for text in ["3010", "jq", "dpkg -i", "-allowUntrusted", "com.apple.quarantine", "ARM64"]:
        assert text in readme, text
    OUTPUT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="docs-", dir=OUTPUT) as temporary:
        directory = Path(temporary)
        (directory / "snippet.ps1").write_text(windows, encoding="utf-8")
        execute(["powershell.exe", "-NoProfile", "-Command",
                 "$t=$null;$e=$null;[void][System.Management.Automation.Language.Parser]::ParseFile('" +
                 str(directory / "snippet.ps1").replace("'", "''") +
                 "',[ref]$t,[ref]$e);if($e.Count){$e;exit 1}"])
        for number, command in enumerate(unix_commands):
            target = directory / f"snippet-{number}.sh"
            target.write_text(command, encoding="utf-8", newline="\n")
            execute(["wsl.exe", "-d", "Ubuntu-24.04", "--", "bash", "-n", wsl_path(target)])
    print("README raw URL, invocation, prerequisite and syntax contracts passed", flush=True)


def main() -> None:
    mode = sys.argv[1]
    if mode in {"docs", "--documentation-examples"}:
        docs()
        return
    OUTPUT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=mode + "-", dir=OUTPUT) as temporary:
        directory = Path(temporary)
        if mode in {"unit", "dv"}:
            fixtures(directory)
        ps(mode, directory)
        unix(mode, directory)
    print(mode + " installer checks passed", flush=True)


if __name__ == "__main__":
    main()
