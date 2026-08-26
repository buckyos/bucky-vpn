#!/usr/bin/env python3
"""Focused contracts for cross-platform packaging and GitHub publication."""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import textwrap
import tomllib
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
WORKFLOW_PATH = ROOT / ".github" / "workflows" / "build.yml"
UPLOAD_ARTIFACT_V7 = (
    "actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a"
)
DOWNLOAD_ARTIFACT_V8 = (
    "actions/download-artifact@3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c"
)
WINDOWS_POWERSHELL = Path(
    "/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe"
)
ALL_CHANGE_IDS = [
    "CHG-github-deb-build",
    "CHG-github-macos-build",
    "CHG-github-windows-build",
    "CHG-github-server-build",
    "CHG-github-release",
]


def load_workflow() -> dict[str, object]:
    import yaml

    return yaml.load(WORKFLOW_PATH.read_text(encoding="utf-8"), Loader=yaml.BaseLoader)


def step_by_name(job: dict[str, object], name: str) -> dict[str, object]:
    for step in job["steps"]:
        if step.get("name") == name:
            return step
    raise AssertionError(f"missing workflow step: {name}")


def write_executable(path: Path, content: str) -> None:
    path.write_text(textwrap.dedent(content).lstrip(), encoding="utf-8")
    path.chmod(0o755)


def read_client_version(root: Path = ROOT) -> str:
    with (root / "vpn-client" / "Cargo.toml").open("rb") as stream:
        return tomllib.load(stream)["package"]["version"]


def verify_native_artifacts(root: Path = ROOT) -> None:
    """Verify the real Windows and Debian outputs against Cargo's version."""
    version = read_client_version(root)
    windows_installer = root / "dist" / f"BuckyVPN_{version}_amd64_Setup.exe"
    debian_package = root / "dist" / f"bucky-vpn_{version}_amd64.deb"
    missing = [str(path) for path in (windows_installer, debian_package) if not path.is_file()]
    if missing:
        raise RuntimeError(f"missing versioned native artifact(s): {', '.join(missing)}")

    completed = subprocess.run(
        ["dpkg-deb", "--field", str(debian_package), "Version"],
        cwd=root,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        raise RuntimeError(
            f"failed to read Debian control Version: {completed.stderr.strip()}"
        )
    packaged_version = completed.stdout.strip()
    if packaged_version != version:
        raise RuntimeError(
            f"Debian control Version {packaged_version!r} does not match Cargo version {version!r}"
        )


def verify_windows_native_behavior(root: Path = ROOT) -> None:
    """Execute the negative ISPP guards and inspect the built EXE resources."""
    if not WINDOWS_POWERSHELL.is_file():
        raise RuntimeError(f"Windows PowerShell is unavailable: {WINDOWS_POWERSHELL}")

    script = r"""
$ErrorActionPreference = 'Stop'

$json = cargo metadata --no-deps --format-version 1
if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed' }
$metadata = ConvertFrom-Json -InputObject ($json -join [Environment]::NewLine)
$packages = @(@($metadata.packages).Where({ $_.name -eq 'bucky-vpn' }))
if ($packages.Count -ne 1) { throw 'expected exactly one bucky-vpn package' }
$version = $packages[0].version

$versionInfo = (Get-Item -LiteralPath 'target\release\bucky-vpn.exe').VersionInfo
if ($versionInfo.FileVersion -ne $version) {
    throw "FileVersion $($versionInfo.FileVersion) does not match Cargo version $version"
}
if ($versionInfo.ProductVersion -ne $version) {
    throw "ProductVersion $($versionInfo.ProductVersion) does not match Cargo version $version"
}

function Assert-IsccFailure {
    param([string[]]$IsccArgs, [string]$ExpectedText)
    $savedErrorActionPreference = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    $outputLines = & ISCC.exe @IsccArgs 2>&1
    $exitCode = $LASTEXITCODE
    $ErrorActionPreference = $savedErrorActionPreference
    $output = $outputLines | Out-String
    if ($exitCode -eq 0) { throw "ISCC unexpectedly accepted: $($IsccArgs -join ' ')" }
    if (-not $output.Contains($ExpectedText)) {
        throw "ISCC failure did not contain '$ExpectedText': $output"
    }
}

Assert-IsccFailure -IsccArgs @('install.iss') -ExpectedText 'AppVersion must be provided by build_win.bat'
Assert-IsccFailure -IsccArgs @('/DAppVersion=', 'install.iss') -ExpectedText 'AppVersion provided by build_win.bat must not be empty'
Write-Output "Windows EXE resources and invalid AppVersion guards match Cargo version $version"
"""
    completed = subprocess.run(
        [str(WINDOWS_POWERSHELL), "-NoProfile", "-Command", script],
        cwd=root,
        capture_output=True,
        text=True,
        errors="replace",
    )
    if completed.returncode != 0:
        output = "\n".join(
            part.strip() for part in (completed.stdout, completed.stderr) if part.strip()
        )
        raise RuntimeError(f"Windows native version verification failed: {output}")
    print(completed.stdout.strip())


class GitHubActionsBuildContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.workflow = load_workflow()
        cls.jobs = cls.workflow["jobs"]
        cls.version = read_client_version()

    def test_workflow_uses_manual_builds_and_tag_releases(self) -> None:
        triggers = self.workflow["on"]
        self.assertEqual(
            triggers,
            {
                "push": {"tags": ["v*"]},
                "workflow_dispatch": {
                    "inputs": {
                        "publish": {
                            "description": (
                                "Publish an existing version tag to GitHub Releases and GHCR"
                            ),
                            "required": "true",
                            "type": "boolean",
                            "default": "false",
                        },
                        "release_tag": {
                            "description": (
                                "Existing version tag to publish, for example v1.2.0"
                            ),
                            "required": "false",
                            "type": "string",
                        },
                    }
                },
            },
        )
        self.assertEqual(self.workflow["permissions"], {"contents": "read"})

        expected = {
            "build-deb": ("ubuntu-24.04", "./build_deb.sh"),
            "build-macos": ("macos-15-intel", "./build_macos.sh"),
            "build-windows": ("windows-2022", "build_win.bat"),
            "build-server": ("ubuntu-24.04", "./build_server.sh"),
        }
        for job_name, (runner, command) in expected.items():
            job = self.jobs[job_name]
            self.assertEqual(job["runs-on"], runner)
            self.assertNotIn("permissions", job)
            runs = "\n".join(str(step.get("run", "")) for step in job["steps"])
            self.assertIn(command, runs)

    def test_every_external_action_is_immutable(self) -> None:
        immutable = re.compile(r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+@[0-9a-f]{40}$")
        uses = [
            step["uses"]
            for job in self.jobs.values()
            for step in job.get("steps", [])
            if "uses" in step
        ]
        self.assertTrue(uses)
        for reference in uses:
            self.assertRegex(reference, immutable)

    def test_publication_is_push_tag_and_repository_gated(self) -> None:
        version_job = self.jobs["version"]
        self.assertIn("publish", version_job["outputs"])
        self.assertIn("release_tag", version_job["outputs"])
        self.assertIn("source_sha", version_job["outputs"])
        script = step_by_name(version_job, "Read and validate vpn-client version")["run"]
        self.assertIn('"${GITHUB_EVENT_NAME}" == "push"', script)
        self.assertIn('"${GITHUB_REF_TYPE}" == "tag"', script)
        self.assertIn('"${GITHUB_REPOSITORY}" == "buckyos/bucky-vpn"', script)
        self.assertIn('"${GITHUB_EVENT_NAME}" == "workflow_dispatch"', script)
        self.assertIn('"${manual_publish}" == "true"', script)
        self.assertIn('"${GITHUB_REPOSITORY}" != "buckyos/bucky-vpn"', script)
        self.assertIn('"${release_tag}" != "v${version}"', script)

        for job_name in ("publish-server", "release"):
            self.assertEqual(
                self.jobs[job_name]["if"],
                "needs.version.outputs.publish == 'true'",
            )
        for step_name in (
            "Export server image for release publication",
            "Store server image for release publication",
        ):
            self.assertEqual(
                step_by_name(self.jobs["build-server"], step_name)["if"],
                "needs.version.outputs.publish == 'true'",
            )

    def test_all_builds_use_the_single_resolved_source_commit(self) -> None:
        version_checkout = step_by_name(self.jobs["version"], "Check out source")
        self.assertEqual(
            version_checkout["with"]["ref"],
            "${{ github.event_name == 'workflow_dispatch' && inputs.publish && "
            "inputs.release_tag && format('refs/tags/{0}', inputs.release_tag) || "
            "github.ref }}",
        )

        for job_name in ("build-deb", "build-macos", "build-windows", "build-server"):
            with self.subTest(job=job_name):
                checkout = step_by_name(self.jobs[job_name], "Check out source")
                self.assertEqual(
                    checkout["with"]["ref"],
                    "${{ needs.version.outputs.source_sha }}",
                )

        script = step_by_name(
            self.jobs["version"], "Read and validate vpn-client version"
        )["run"]
        self.assertIn("git rev-parse --verify HEAD^{commit}", script)
        self.assertIn('git rev-parse --verify "refs/tags/${release_tag}^{commit}"', script)
        self.assertIn('"${source_sha}" != "${tag_sha}"', script)

    def test_cargo_update_lock_is_shared_by_all_builds(self) -> None:
        version_steps = self.jobs["version"]["steps"]
        version_step_names = [step["name"] for step in version_steps]
        self.assertLess(
            version_step_names.index("Check out source"),
            version_step_names.index("Update Cargo dependencies"),
        )
        self.assertLess(
            version_step_names.index("Update Cargo dependencies"),
            version_step_names.index("Read and validate vpn-client version"),
        )
        self.assertLess(
            version_step_names.index("Read and validate vpn-client version"),
            version_step_names.index("Store updated Cargo lockfile"),
        )

        update = step_by_name(self.jobs["version"], "Update Cargo dependencies")
        self.assertEqual(update["run"], "cargo update")
        stored_lock = step_by_name(
            self.jobs["version"], "Store updated Cargo lockfile"
        )
        self.assertEqual(stored_lock["uses"], UPLOAD_ARTIFACT_V7)
        self.assertEqual(
            stored_lock["with"],
            {
                "name": "cargo-lock",
                "path": "Cargo.lock",
                "if-no-files-found": "error",
                "retention-days": "1",
            },
        )

        build_steps = {
            "build-deb": "Build Debian package",
            "build-macos": "Build macOS package",
            "build-windows": "Build Windows installer",
            "build-server": "Build and verify server image",
        }
        for job_name, build_step_name in build_steps.items():
            with self.subTest(job=job_name):
                steps = self.jobs[job_name]["steps"]
                step_names = [step["name"] for step in steps]
                self.assertLess(
                    step_names.index("Check out source"),
                    step_names.index("Restore updated Cargo lockfile"),
                )
                self.assertLess(
                    step_names.index("Restore updated Cargo lockfile"),
                    step_names.index(build_step_name),
                )
                restored_lock = step_by_name(
                    self.jobs[job_name], "Restore updated Cargo lockfile"
                )
                self.assertEqual(restored_lock["uses"], DOWNLOAD_ARTIFACT_V8)
                self.assertEqual(
                    restored_lock["with"],
                    {"name": "cargo-lock", "path": "."},
                )
                self.assertNotIn("if", restored_lock)
                self.assertNotIn("continue-on-error", restored_lock)

        cargo_update_steps = [
            step
            for job in self.jobs.values()
            for step in job.get("steps", [])
            if str(step.get("run", "")).strip() == "cargo update"
        ]
        self.assertEqual(cargo_update_steps, [update])

    def test_publication_permissions_are_job_local(self) -> None:
        self.assertEqual(
            self.jobs["authorize-publication"]["permissions"],
            {"contents": "read"},
        )
        self.assertEqual(
            self.jobs["publish-server"]["permissions"],
            {"actions": "read", "contents": "read", "packages": "write"},
        )
        self.assertEqual(
            self.jobs["release"]["permissions"],
            {"actions": "read", "contents": "write"},
        )
        publish_script = step_by_name(
            self.jobs["publish-server"], "Log in to GHCR and publish image tags"
        )["run"]
        self.assertIn("secrets.GITHUB_TOKEN", json.dumps(self.jobs["publish-server"]))
        self.assertNotIn("personal", publish_script.lower())

    def test_publication_revalidates_tag_after_all_builds(self) -> None:
        authorization = self.jobs["authorize-publication"]
        self.assertEqual(
            set(authorization["needs"]),
            {"version", "build-deb", "build-macos", "build-windows", "build-server"},
        )
        self.assertEqual(
            authorization["if"],
            "needs.version.outputs.publish == 'true'",
        )
        verify = step_by_name(
            authorization, "Verify release tag still selects built source"
        )
        self.assertEqual(
            verify["env"]["RELEASE_TAG"],
            "${{ needs.version.outputs.release_tag }}",
        )
        self.assertEqual(
            verify["env"]["SOURCE_SHA"],
            "${{ needs.version.outputs.source_sha }}",
        )
        script = verify["run"]
        self.assertIn("git/ref/tags/${RELEASE_TAG}", script)
        self.assertIn("git/tags/${object_sha}", script)
        self.assertIn('"$object_type" != "commit"', script)
        self.assertIn('"$object_sha" != "$SOURCE_SHA"', script)
        self.assertIn("secrets.GITHUB_TOKEN", json.dumps(authorization))
        self.assertIn("authorize-publication", self.jobs["publish-server"]["needs"])

    def test_publication_tag_revalidation_handles_tag_objects_and_drift(self) -> None:
        authorization = self.jobs["authorize-publication"]
        script = step_by_name(
            authorization, "Verify release tag still selects built source"
        )["run"]
        source_sha = "a" * 40

        with tempfile.TemporaryDirectory() as temporary:
            fake_bin = Path(temporary)
            write_executable(
                fake_bin / "gh",
                """
                #!/usr/bin/env bash
                set -euo pipefail
                if [[ "$2" == *"/git/ref/tags/"* ]]; then
                  if [[ "$TAG_KIND" == "annotated" ]]; then
                    printf 'tag\t%s\n' "tag-object-sha"
                  else
                    printf 'commit\t%s\n' "$REMOTE_SHA"
                  fi
                elif [[ "$2" == *"/git/tags/tag-object-sha" ]]; then
                  printf 'commit\t%s\n' "$REMOTE_SHA"
                else
                  exit 1
                fi
                """,
            )

            def run_revalidation(
                tag_kind: str, remote_sha: str
            ) -> subprocess.CompletedProcess[str]:
                env = os.environ.copy()
                env.update(
                    {
                        "PATH": f"{fake_bin}{os.pathsep}{env['PATH']}",
                        "GITHUB_REPOSITORY": "buckyos/bucky-vpn",
                        "RELEASE_TAG": f"v{self.version}",
                        "SOURCE_SHA": source_sha,
                        "TAG_KIND": tag_kind,
                        "REMOTE_SHA": remote_sha,
                    }
                )
                return subprocess.run(
                    ["bash", "-c", script],
                    cwd=ROOT,
                    env=env,
                    capture_output=True,
                    text=True,
                )

            self.assertEqual(run_revalidation("lightweight", source_sha).returncode, 0)
            self.assertEqual(run_revalidation("annotated", source_sha).returncode, 0)
            moved = run_revalidation("lightweight", "b" * 40)
            self.assertNotEqual(moved.returncode, 0)
            self.assertIn("moved from", moved.stderr)

    def test_client_installers_are_uploaded_as_direct_versioned_files(self) -> None:
        expected = {
            "build-deb": (
                "Store Debian installer",
                "dist/bucky-vpn_${{ needs.version.outputs.version }}_amd64.deb",
            ),
            "build-macos": (
                "Store macOS installer",
                "dist/BuckyVPN-${{ needs.version.outputs.version }}.pkg",
            ),
            "build-windows": (
                "Store Windows installer",
                "dist/BuckyVPN_${{ needs.version.outputs.version }}_amd64_Setup.exe",
            ),
        }
        for job_name, (step_name, path) in expected.items():
            with self.subTest(job=job_name):
                upload = step_by_name(self.jobs[job_name], step_name)
                self.assertEqual(upload["uses"], UPLOAD_ARTIFACT_V7)
                self.assertEqual(upload["with"]["path"], path)
                self.assertEqual(upload["with"]["archive"], "false")
                self.assertNotIn("name", upload["with"])
                self.assertEqual(upload["with"]["if-no-files-found"], "error")
                self.assertEqual(upload["with"]["retention-days"], "14")

    def test_release_uses_three_installers_and_automatic_source_archives(self) -> None:
        release = self.jobs["release"]
        self.assertIn("publish-server", release["needs"])
        expected_downloads = {
            "Download Debian installer": (
                "bucky-vpn_${{ needs.version.outputs.version }}_amd64.deb"
            ),
            "Download macOS installer": (
                "BuckyVPN-${{ needs.version.outputs.version }}.pkg"
            ),
            "Download Windows installer": (
                "BuckyVPN_${{ needs.version.outputs.version }}_amd64_Setup.exe"
            ),
        }
        for step_name, artifact_name in expected_downloads.items():
            with self.subTest(step=step_name):
                download = step_by_name(release, step_name)
                self.assertEqual(download["uses"], DOWNLOAD_ARTIFACT_V8)
                self.assertEqual(download["with"]["name"], artifact_name)
                self.assertEqual(download["with"]["path"], "release-assets")
                self.assertNotIn("pattern", download["with"])
                self.assertNotIn("merge-multiple", download["with"])
        script = step_by_name(release, "Create GitHub Release")["run"]
        self.assertIn("release-assets/*.deb", script)
        self.assertIn("release-assets/*.pkg", script)
        self.assertIn("release-assets/*.exe", script)
        self.assertIn("${#assets[@]} -ne 3", script)
        self.assertIn("gh release create", script)
        self.assertIn('gh release create "$RELEASE_TAG"', script)
        self.assertIn('--repo "$GITHUB_REPOSITORY"', script)
        self.assertEqual(
            release["steps"][-1]["env"]["RELEASE_TAG"],
            "${{ needs.version.outputs.release_tag }}",
        )
        self.assertNotIn("GITHUB_REF_NAME", script)
        self.assertNotRegex(script, re.compile(r"(?:zip|tar\.gz|git archive)", re.IGNORECASE))

    def test_server_version_and_latest_tags_share_one_image(self) -> None:
        publish = self.jobs["publish-server"]
        self.assertEqual(
            set(publish["needs"]),
            {
                "version",
                "build-deb",
                "build-macos",
                "build-windows",
                "build-server",
                "authorize-publication",
            },
        )
        load_script = step_by_name(publish, "Load and tag server image")["run"]
        push_script = step_by_name(
            publish, "Log in to GHCR and publish image tags"
        )["run"]
        image = "ghcr.io/buckyos/bucky-vpn-server"
        self.assertIn(f'{image}:${{VERSION}}', load_script)
        self.assertIn(f"{image}:latest", load_script)
        self.assertIn("docker image inspect --format '{{.Id}}'", load_script)
        self.assertIn("docker buildx imagetools inspect", push_script)
        self.assertIn('test "$version_digest" = "$latest_digest"', push_script)

    def test_version_job_accepts_only_controlled_publication_requests(self) -> None:
        script = step_by_name(
            self.jobs["version"], "Read and validate vpn-client version"
        )["run"]
        cases = [
            (
                "push", "tag", f"v{self.version}", "buckyos/bucky-vpn",
                "", "", 0, "true", f"v{self.version}",
            ),
            (
                "workflow_dispatch", "branch", "master", "buckyos/bucky-vpn",
                "false", "", 0, "false", "",
            ),
            (
                "workflow_dispatch", "branch", "master", "buckyos/bucky-vpn",
                "true", f"v{self.version}", 0, "true", f"v{self.version}",
            ),
            (
                "push", "tag", f"v{self.version}", "someone/bucky-vpn",
                "", "", 0, "false", f"v{self.version}",
            ),
            (
                "push", "tag", "v9.9.9", "buckyos/bucky-vpn",
                "", "", 1, None, None,
            ),
            (
                "workflow_dispatch", "branch", "master", "buckyos/bucky-vpn",
                "true", "", 1, None, None,
            ),
            (
                "workflow_dispatch", "branch", "master", "someone/bucky-vpn",
                "true", f"v{self.version}", 1, None, None,
            ),
            (
                "workflow_dispatch", "branch", "master", "buckyos/bucky-vpn",
                "true", "v9.9.9", 1, None, None,
            ),
            (
                "workflow_dispatch", "branch", "master", "buckyos/bucky-vpn",
                "false", f"v{self.version}", 1, None, None,
            ),
            (
                "workflow_dispatch", "branch", "master", "buckyos/bucky-vpn",
                "yes", f"v{self.version}", 1, None, None,
            ),
        ]
        for (
            event,
            ref_type,
            ref_name,
            repository,
            manual_publish,
            manual_release_tag,
            expected_code,
            expected_publish,
            expected_release_tag,
        ) in cases:
            with self.subTest(
                event=event,
                ref_name=ref_name,
                repository=repository,
                manual_publish=manual_publish,
                manual_release_tag=manual_release_tag,
            ):
                with tempfile.TemporaryDirectory() as temporary:
                    output = Path(temporary) / "output"
                    env = os.environ.copy()
                    env.update(
                        {
                            "GITHUB_EVENT_NAME": event,
                            "GITHUB_REF_TYPE": ref_type,
                            "GITHUB_REF_NAME": ref_name,
                            "GITHUB_REPOSITORY": repository,
                            "GITHUB_OUTPUT": str(output),
                            "MANUAL_PUBLISH": manual_publish,
                            "MANUAL_RELEASE_TAG": manual_release_tag,
                        }
                    )
                    completed = subprocess.run(
                        ["bash", "-c", script],
                        cwd=ROOT,
                        env=env,
                        capture_output=True,
                        text=True,
                    )
                    self.assertEqual(completed.returncode, expected_code, completed.stderr)
                    if expected_publish is not None:
                        values = dict(
                            line.split("=", 1)
                            for line in output.read_text(encoding="utf-8").splitlines()
                        )
                        self.assertEqual(values["version"], self.version)
                        self.assertEqual(values["publish"], expected_publish)
                        self.assertEqual(values["release_tag"], expected_release_tag)
                        self.assertEqual(
                            values["source_sha"],
                            subprocess.run(
                                ["git", "rev-parse", "--verify", "HEAD^{commit}"],
                                cwd=ROOT,
                                check=True,
                                capture_output=True,
                                text=True,
                            ).stdout.strip(),
                        )

    def test_posix_build_scripts_are_syntax_valid(self) -> None:
        completed = subprocess.run(
            ["bash", "-n", "build_deb.sh", "build_macos.sh", "build_server.sh"],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)

    def test_debian_script_stages_versioned_metadata_without_dirtying_template(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            shutil.copy2(ROOT / "build_deb.sh", fixture / "build_deb.sh")
            shutil.copytree(ROOT / "vpn_deb", fixture / "vpn_deb")
            fake_bin = self._write_fake_tools(fixture)
            control_before = (fixture / "vpn_deb" / "DEBIAN" / "control").read_bytes()
            completed = self._run_fixture(fixture, fake_bin, "build_deb.sh")
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertTrue((fixture / f"dist/bucky-vpn_{self.version}_amd64.deb").is_file())
            self.assertEqual(
                (fixture / "vpn_deb" / "DEBIAN" / "control").read_bytes(),
                control_before,
            )

    def test_macos_script_builds_versioned_staged_package(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            shutil.copy2(ROOT / "build_macos.sh", fixture / "build_macos.sh")
            shutil.copytree(ROOT / "vpn_macos", fixture / "vpn_macos")
            fake_bin = self._write_fake_tools(fixture)
            completed = self._run_fixture(fixture, fake_bin, "build_macos.sh")
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertTrue((fixture / f"dist/BuckyVPN-{self.version}.pkg").is_file())
            self.assertFalse(
                (fixture / "vpn_macos" / "BuckyVPN.app" / "Contents" / "Info.plist").exists()
            )

    def test_server_script_passes_product_version_to_docker(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            shutil.copy2(ROOT / "build_server.sh", fixture / "build_server.sh")
            shutil.copy2(ROOT / "Dockerfile", fixture / "Dockerfile")
            (fixture / "vpn_web").mkdir()
            fake_bin = self._write_fake_tools(fixture)
            completed = self._run_fixture(fixture, fake_bin, "build_server.sh")
            self.assertEqual(completed.returncode, 0, completed.stderr)
            calls = (fixture / "tool-calls.log").read_text(encoding="utf-8")
            self.assertIn(f"docker build --build-arg VERSION={self.version}", calls)
            self.assertIn("-t bucky-vpn-server:latest", calls)

    def test_unsupported_cargo_version_fails_before_build(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            shutil.copy2(ROOT / "build_deb.sh", fixture / "build_deb.sh")
            shutil.copytree(ROOT / "vpn_deb", fixture / "vpn_deb")
            fake_bin = self._write_fake_tools(fixture, version="1.0.0-beta.1")
            completed = self._run_fixture(fixture, fake_bin, "build_deb.sh")
            self.assertNotEqual(completed.returncode, 0)
            calls = fixture / "tool-calls.log"
            self.assertFalse(calls.exists() and "cargo build" in calls.read_text(encoding="utf-8"))

    def test_windows_version_is_passed_to_inno_setup_without_cmd_caret_leak(self) -> None:
        batch = (ROOT / "build_win.bat").read_text(encoding="utf-8")
        installer = (ROOT / "install.iss").read_text(encoding="utf-8")
        self.assertNotIn("$json = ^& cargo", batch)
        self.assertIn("$json = cargo metadata --no-deps --format-version 1", batch)
        self.assertIn('ISCC.exe "/DAppVersion=%APP_VERSION%" "install.iss"', batch)
        self.assertIn("BuckyVPN_%APP_VERSION%_amd64_Setup.exe", batch)
        self.assertIn("#ifndef AppVersion", installer)
        self.assertIn('#if AppVersion == ""', installer)
        self.assertIn("AppVersion must be provided by build_win.bat", installer)
        self.assertIn("AppVersion provided by build_win.bat must not be empty", installer)
        self.assertNotIn('#define AppVersion "1.2.0"', installer)

    def test_winres_metadata_inherits_cargo_package_version(self) -> None:
        with (ROOT / "vpn-client" / "Cargo.toml").open("rb") as stream:
            winres = tomllib.load(stream)["package"]["metadata"]["winres"]
        self.assertNotIn("FileVersion", winres)
        self.assertNotIn("ProductVersion", winres)

    def test_native_artifact_verifier_checks_names_and_debian_control_version(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            (fixture / "vpn-client").mkdir()
            (fixture / "vpn-client" / "Cargo.toml").write_text(
                '[package]\nname = "bucky-vpn"\nversion = "3.2.1"\n',
                encoding="utf-8",
            )
            (fixture / "dist").mkdir()
            (fixture / "dist" / "BuckyVPN_3.2.1_amd64_Setup.exe").touch()
            (fixture / "dist" / "bucky-vpn_3.2.1_amd64.deb").touch()
            fake_bin = fixture / "fake-bin"
            fake_bin.mkdir()
            write_executable(
                fake_bin / "dpkg-deb",
                """
                #!/usr/bin/env bash
                set -euo pipefail
                test "$1" = "--field"
                test "$3" = "Version"
                printf '3.2.1\\n'
                """,
            )
            with mock.patch.dict(
                os.environ,
                {"PATH": f"{fake_bin}{os.pathsep}{os.environ['PATH']}"},
            ):
                verify_native_artifacts(fixture)

    @staticmethod
    def _run_fixture(fixture: Path, fake_bin: Path, script: str) -> subprocess.CompletedProcess[str]:
        env = os.environ.copy()
        env["PATH"] = f"{fake_bin}{os.pathsep}{env['PATH']}"
        env["TMPDIR"] = str(fixture / "tmp")
        (fixture / "tmp").mkdir()
        return subprocess.run(
            ["bash", script],
            cwd=fixture,
            env=env,
            capture_output=True,
            text=True,
        )

    def _write_fake_tools(self, fixture: Path, version: str | None = None) -> Path:
        version = version or self.version
        fake_bin = fixture / "fake-bin"
        fake_bin.mkdir()
        metadata = json.dumps({"packages": [{"name": "bucky-vpn", "version": version}]})
        write_executable(
            fake_bin / "cargo",
            f"""
            #!/usr/bin/env bash
            set -euo pipefail
            if [[ "$1" == "metadata" ]]; then
              printf '%s\n' '{metadata}'
              exit 0
            fi
            printf 'cargo %s\n' "$*" >> tool-calls.log
            mkdir -p target/x86_64-unknown-linux-musl/release target/x86_64-apple-darwin/release target/aarch64-apple-darwin/release target/release
            : > target/x86_64-unknown-linux-musl/release/bucky-vpn
            : > target/x86_64-unknown-linux-musl/release/bucky-vpn-server
            : > target/x86_64-apple-darwin/release/bucky-vpn
            : > target/aarch64-apple-darwin/release/bucky-vpn
            : > target/release/bucky-vpn.exe
            """,
        )
        write_executable(
            fake_bin / "dpkg-deb",
            f"""
            #!/usr/bin/env bash
            set -euo pipefail
            root="${{@: -2:1}}"
            output="${{@: -1}}"
            grep -Fx 'Version: {version}' "$root/DEBIAN/control" >/dev/null
            grep -Fx 'Maintainer: BuckyOS Developers <buckyos@users.noreply.github.com>' "$root/DEBIAN/control" >/dev/null
            [[ "$(stat -c %a "$root/DEBIAN")" == "755" ]]
            [[ "$(stat -c %a "$root/DEBIAN/control")" == "644" ]]
            [[ "$(stat -c %a "$root/DEBIAN/preinst")" == "755" ]]
            [[ "$(stat -c %a "$root/DEBIAN/postinst")" == "755" ]]
            [[ "$(stat -c %a "$root/DEBIAN/postrm")" == "755" ]]
            [[ "$(stat -c %a "$root/lib/systemd/system/bucky-vpn.service")" == "644" ]]
            printf 'dpkg-deb %s\n' "$*" >> tool-calls.log
            mkdir -p "$(dirname "$output")"
            : > "$output"
            """,
        )
        write_executable(
            fake_bin / "lipo",
            """
            #!/usr/bin/env bash
            set -euo pipefail
            while [[ $# -gt 0 ]]; do
              if [[ "$1" == "-output" ]]; then
                output="$2"
                break
              fi
              shift
            done
            mkdir -p "$(dirname "$output")"
            : > "$output"
            printf 'lipo\n' >> tool-calls.log
            """,
        )
        write_executable(
            fake_bin / "pkgbuild",
            f"""
            #!/usr/bin/env bash
            set -euo pipefail
            args=("$@")
            output="${{args[${{#args[@]}}-1]}}"
            root=""
            for ((i=0; i<${{#args[@]}}; i++)); do
              if [[ "${{args[$i]}}" == "--root" ]]; then root="${{args[$((i+1))]}}"; fi
            done
            grep -F '<string>{version}</string>' "$root/BuckyVPN.app/Contents/Info.plist" >/dev/null
            mkdir -p "$(dirname "$output")"
            : > "$output"
            printf 'pkgbuild %s\n' "$*" >> tool-calls.log
            """,
        )
        write_executable(
            fake_bin / "flutter",
            """
            #!/usr/bin/env bash
            set -euo pipefail
            printf 'flutter %s\n' "$*" >> ../tool-calls.log
            mkdir -p build/web
            : > build/web/index.html
            """,
        )
        write_executable(
            fake_bin / "docker",
            """
            #!/usr/bin/env bash
            set -euo pipefail
            printf 'docker %s\n' "$*" >> tool-calls.log
            """,
        )
        return fake_bin


if __name__ == "__main__":
    if sys.argv[1:] == ["--verify-native-artifacts"]:
        verify_native_artifacts()
        print("Windows and Debian artifacts match vpn-client/Cargo.toml version")
    elif sys.argv[1:] == ["--verify-windows-native-behavior"]:
        verify_windows_native_behavior()
    else:
        unittest.main(verbosity=2)
