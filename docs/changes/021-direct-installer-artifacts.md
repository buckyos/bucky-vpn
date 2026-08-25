# Direct Installer Artifacts

- Status: complete
- Task manifest: docs/versions/v0.1/modules/bucky-vpn/021-direct-installer-artifacts/task.yaml
- Approved proposal: docs/versions/v0.1/modules/bucky-vpn/021-direct-installer-artifacts/proposal.md
- Affected paths: .github/workflows/build.yml, tests/github_actions_build_contract.py

## Approach
Use the pinned `actions/upload-artifact@v7` single-file direct-upload mode for the Debian, macOS, and Windows installers. Remove the ignored logical artifact names, keep each exact versioned path and retention/error policy, and replace the Release job's `installer-*` pattern download with three explicit `actions/download-artifact@v8` downloads named after the direct artifacts.

## Risk Screen
The user selected the standard tier despite the produced-artifact and Release-handoff boundary. A mismatch between an upload basename and a Release download name would break tag publication after otherwise successful platform builds. Focused contract tests therefore bind all three upload paths to their exact download names and retain the existing exactly-three-release-assets assertion. Hosted Actions execution remains the final evidence for GitHub UI presentation and platform transfer behavior.

## Verification
- Targeted check: `UV_CACHE_DIR=.harness/uv-cache uv run --active --with PyYAML==6.0.2 python ./tests/github_actions_build_contract.py` (16 tests); `UV_CACHE_DIR=.harness/uv-cache uv run --active --with PyYAML==6.0.2 python ./tests/windows_action_nasm_contract.py` (7 tests); focused `git diff --check`; direct-upload/download name inspection
- Result: passed
- Residual risk or follow-up: The new contract test failed against the pre-change workflow for all three archived uploads and all three missing direct download steps, then all 16 tests passed after implementation; the unrelated Windows NASM contract remains green. A hosted GitHub Actions run is still required to confirm the GitHub UI presents the three files directly and that a matching version tag completes the real cross-job downloads. Local static contracts cannot emulate GitHub artifact storage, native macOS/Windows runners, or an actual Release mutation.
