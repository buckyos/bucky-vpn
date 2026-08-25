# Controlled Manual GitHub Release

- Status: complete
- Task manifest: docs/versions/v0.1/modules/bucky-vpn/025-manual-controlled-release/task.yaml
- Approved proposal: docs/versions/v0.1/modules/bucky-vpn/025-manual-controlled-release/proposal.md
- Affected paths: .github/workflows/build.yml, tests/github_actions_build_contract.py

## Approach
Extend `workflow_dispatch` with a false-by-default publication switch and an optional existing release-tag input. When publication is requested, make checkout address the explicit tag namespace, validate the canonical repository and exact `v<Cargo version>` relationship, resolve one source commit SHA, and require every build job to checkout that SHA. Export the validated release tag separately so the checkout-free Release job never relies on the dispatch branch's `GITHUB_REF_NAME`. Preserve automatic matching-tag publication and all existing artifact, dependency, and least-privilege gates.

## Risk Screen
The user explicitly selected the standard tier despite the material release/deployment boundary. The main hazards are accidental publication, packaging a branch under a tag, tag movement between jobs, fork publication, and using the dispatch branch name as the Release tag. The workflow will fail closed for invalid manual input, default to build-only, pin downstream builds to one resolved commit, and keep all mutation jobs behind the existing `publish` output. The custom release-validation rule's platform-evidence requirement will be recorded here and in the completion review; the standard tier intentionally does not add high-risk `testing.md` or `testplan.yaml` artifacts.

## Verification
- Targeted check: `UV_CACHE_DIR=.harness/uv-cache uv run --active --with PyYAML==6.0.2 python ./tests/github_actions_build_contract.py` (19 passed); task-start workflow negative control (the new dispatch-input contract failed as expected); exact release-tag revalidation script against hosted `v1.2.0` (matching SHA passed, mismatched SHA rejected); focused whitespace/diff review.
- Result: passed
- Residual risk or follow-up: The suite executes the dispatch decision script, lightweight/annotated-tag revalidation, mismatch/fork/missing-input rejection paths, package-script fixtures, and workflow structure locally. Native hosted Debian/macOS/Windows/server builds and an actual `workflow_dispatch` publication were not run because they mutate or depend on hosted platform state; the first deliberately authorized hosted run remains the final operational proof.
