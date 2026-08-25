#!/usr/bin/env python3
"""Focused contract checks for Windows GitHub Actions NASM provisioning."""

from __future__ import annotations

import copy
import json
import sys
import tomllib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW_PATH = ROOT / ".github" / "workflows" / "build.yml"
BUILD_SCRIPT_PATH = ROOT / "build_win.bat"

INSTALL_COMMAND = "choco install nasm --version=2.16.3 --yes --no-progress"
VERIFY_COMMAND = "nasm -v"
NASM_DIRECTORY = '$nasmDirectory = Join-Path $env:ProgramFiles "NASM"'
NASM_EXECUTABLE = '$nasm = Join-Path $nasmDirectory "nasm.exe"'
NASM_MISSING_GUARD = (
    "if (-not (Test-Path -LiteralPath $nasm -PathType Leaf)) {"
)
NASM_MISSING_ERROR = (
    'throw "NASM installation completed, but the expected executable was not found: '
    '$nasm"'
)
NASM_DIRECTORY_OUTPUT = "$nasmDirectory |"
GITHUB_PATH_PERSISTENCE = (
    "Out-File -FilePath $env:GITHUB_PATH -Encoding utf8 -Append"
)
FORBIDDEN_PATH_DISCOVERY = (
    '[Environment]::GetEnvironmentVariable("Path", "Machine")',
    '[Environment]::GetEnvironmentVariable("Path", "User")',
    "Get-Command nasm.exe",
)
EXPECTED_INSTALLER_CHECK = """\
$installer = "dist\\BuckyVPN_${{ needs.version.outputs.version }}_amd64_Setup.exe"
if (-not (Test-Path $installer)) {
  throw "Expected Windows installer was not produced: $installer"
}
"""


class ContractViolation(AssertionError):
    """Raised when the workflow no longer satisfies the NASM contract."""


def load_workflow() -> dict[str, object]:
    import yaml

    return yaml.load(WORKFLOW_PATH.read_text(encoding="utf-8"), Loader=yaml.BaseLoader)


def named_step(job: dict[str, object], name: str) -> tuple[int, dict[str, object]]:
    matches = [
        (index, step)
        for index, step in enumerate(job["steps"])
        if step.get("name") == name
    ]
    if len(matches) != 1:
        raise ContractViolation(f"expected exactly one {name!r} step, found {len(matches)}")
    return matches[0]


def validate_windows_nasm_contract(workflow: dict[str, object]) -> None:
    jobs = workflow.get("jobs", {})
    if "build-windows" not in jobs:
        raise ContractViolation("missing build-windows job")
    job = jobs["build-windows"]
    if job.get("runs-on") != "windows-2022":
        raise ContractViolation("build-windows must remain on windows-2022")

    checkout_index, _ = named_step(job, "Check out source")
    install_index, install = named_step(job, "Install NASM")
    verify_index, verify = named_step(job, "Verify NASM")
    build_index, build = named_step(job, "Build Windows installer")
    installer_index, installer = named_step(job, "Verify Windows installer")

    if set(install) != {"name", "shell", "run"} or install.get("shell") != "pwsh":
        raise ContractViolation(
            "Install NASM must remain a standalone pwsh step with only name, shell, "
            f"and run fields; got {install!r}"
        )
    install_run = install.get("run", "")
    install_lines = [line.strip() for line in install_run.splitlines() if line.strip()]
    nasm_install_lines = [
        line for line in install_lines if line.lower().startswith("choco install nasm")
    ]
    if nasm_install_lines != [INSTALL_COMMAND]:
        raise ContractViolation(
            "Install NASM must contain the exact pinned command "
            f"{INSTALL_COMMAND!r}; got {nasm_install_lines!r}"
        )

    required_path_lines = (
        NASM_DIRECTORY,
        NASM_EXECUTABLE,
        NASM_MISSING_GUARD,
        NASM_MISSING_ERROR,
        NASM_DIRECTORY_OUTPUT,
        GITHUB_PATH_PERSISTENCE,
    )
    missing_path_lines = [line for line in required_path_lines if line not in install_lines]
    if missing_path_lines:
        raise ContractViolation(
            "Install NASM must validate the package-defined Program Files executable, "
            "fail when it is missing, and persist its directory through GITHUB_PATH; "
            f"missing {missing_path_lines!r}"
        )
    path_line_indexes = [install_lines.index(line) for line in required_path_lines]
    if path_line_indexes != sorted(path_line_indexes):
        raise ContractViolation(
            "Install NASM must derive the Program Files NASM directory and executable, "
            "then fail when it is missing and persist that directory through GITHUB_PATH"
        )
    if install_lines[install_lines.index(NASM_MISSING_GUARD) + 1] != NASM_MISSING_ERROR:
        raise ContractViolation("Install NASM must fail explicitly when nasm.exe is missing")
    if install_lines[install_lines.index(NASM_DIRECTORY_OUTPUT) + 1] != GITHUB_PATH_PERSISTENCE:
        raise ContractViolation(
            "Install NASM must persist the package-defined NASM directory through "
            "GITHUB_PATH"
        )
    forbidden_discovery = [
        token for token in FORBIDDEN_PATH_DISCOVERY if token in install_run
    ]
    if forbidden_discovery:
        raise ContractViolation(
            "Install NASM must use the deterministic Program Files package location, "
            "not Machine/User PATH discovery: " + ", ".join(forbidden_discovery)
        )
    expected_verify = {
        "name": "Verify NASM",
        "shell": "pwsh",
        "run": VERIFY_COMMAND,
    }
    if verify != expected_verify:
        raise ContractViolation(
            "Verify NASM must be a standalone pwsh step running exactly 'nasm -v'"
        )
    if not checkout_index < install_index < verify_index < build_index < installer_index:
        raise ContractViolation(
            "required order is checkout -> install NASM -> verify NASM -> build -> "
            "verify installer"
        )

    if build != {
        "name": "Build Windows installer",
        "shell": "cmd",
        "run": "build_win.bat",
    }:
        raise ContractViolation("the existing cmd build_win.bat step must remain unchanged")
    if installer.get("shell") != "pwsh" or installer.get("run", "").strip() != EXPECTED_INSTALLER_CHECK.strip():
        raise ContractViolation("the existing versioned installer verification must remain unchanged")

    serialized_job = json.dumps(job, sort_keys=True).lower()
    forbidden = (
        "aws_lc_sys_prebuilt_nasm",
        "aws-lc-sys-prebuilt-nasm",
        "--no-default-features",
        "--features",
    )
    found = [token for token in forbidden if token in serialized_job]
    if found:
        raise ContractViolation(
            "Windows job must not replace NASM provisioning with AWS-LC or crypto "
            f"feature workarounds: {', '.join(found)}"
        )

    build_script = BUILD_SCRIPT_PATH.read_text(encoding="utf-8").lower()
    script_forbidden = ("aws_lc", "aws-lc", "prebuilt_nasm", "--no-default-features", "--features")
    found = [token for token in script_forbidden if token in build_script]
    if found:
        raise ContractViolation(
            "build_win.bat must remain outside AWS-LC/crypto feature provisioning: "
            + ", ".join(found)
        )


def verify_local_windows_artifact() -> None:
    with (ROOT / "vpn-client" / "Cargo.toml").open("rb") as stream:
        version = tomllib.load(stream)["package"]["version"]
    artifact = ROOT / "dist" / f"BuckyVPN_{version}_amd64_Setup.exe"
    if not artifact.is_file():
        raise RuntimeError(f"expected local Windows installer was not produced: {artifact}")
    print(f"Local Windows installer exists for Cargo version {version}: {artifact}")


class WindowsActionNasmContractTests(unittest.TestCase):
    def test_current_workflow_satisfies_nasm_contract(self) -> None:
        validate_windows_nasm_contract(load_workflow())

    def test_pre_fix_missing_nasm_steps_are_rejected(self) -> None:
        workflow = copy.deepcopy(load_workflow())
        job = workflow["jobs"]["build-windows"]
        job["steps"] = [
            step
            for step in job["steps"]
            if step.get("name") not in {"Install NASM", "Verify NASM"}
        ]
        with self.assertRaisesRegex(ContractViolation, "Install NASM"):
            validate_windows_nasm_contract(workflow)

    def test_unpinned_or_wrong_nasm_versions_are_rejected(self) -> None:
        for command in (
            "choco install nasm --yes --no-progress",
            "choco install nasm --version=2.16.01 --yes --no-progress",
        ):
            with self.subTest(command=command):
                workflow = copy.deepcopy(load_workflow())
                _, install = named_step(workflow["jobs"]["build-windows"], "Install NASM")
                install["run"] = install["run"].replace(INSTALL_COMMAND, command)
                with self.assertRaisesRegex(ContractViolation, "exact pinned command"):
                    validate_windows_nasm_contract(workflow)

    def test_missing_cross_step_path_persistence_is_rejected(self) -> None:
        workflow = copy.deepcopy(load_workflow())
        _, install = named_step(workflow["jobs"]["build-windows"], "Install NASM")
        install["run"] = install["run"].replace(GITHUB_PATH_PERSISTENCE, "")
        with self.assertRaisesRegex(ContractViolation, "persist.*GITHUB_PATH"):
            validate_windows_nasm_contract(workflow)

    def test_wrong_package_install_location_is_rejected(self) -> None:
        workflow = copy.deepcopy(load_workflow())
        _, install = named_step(workflow["jobs"]["build-windows"], "Install NASM")
        install["run"] = install["run"].replace(NASM_DIRECTORY, '$nasmDirectory = "C:\\NASM"')
        with self.assertRaisesRegex(ContractViolation, "Program Files executable"):
            validate_windows_nasm_contract(workflow)

    def test_machine_or_user_path_discovery_is_rejected(self) -> None:
        workflow = copy.deepcopy(load_workflow())
        _, install = named_step(workflow["jobs"]["build-windows"], "Install NASM")
        install["run"] += (
            '\n$machinePath = [Environment]::GetEnvironmentVariable("Path", "Machine")\n'
            '$nasmFromPath = Get-Command nasm.exe\n'
        )
        with self.assertRaisesRegex(ContractViolation, "not Machine/User PATH discovery"):
            validate_windows_nasm_contract(workflow)

    def test_prebuilt_or_crypto_feature_workarounds_are_rejected(self) -> None:
        workflow = copy.deepcopy(load_workflow())
        job = workflow["jobs"]["build-windows"]
        job["env"] = {"AWS_LC_SYS_PREBUILT_NASM": "1"}
        with self.assertRaisesRegex(ContractViolation, "feature workarounds"):
            validate_windows_nasm_contract(workflow)


if __name__ == "__main__":
    if sys.argv[1:] == ["--verify-local-windows-artifact"]:
        verify_local_windows_artifact()
    else:
        unittest.main()
