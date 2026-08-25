# Bind GitHub Release Repository

- Status: complete
- Task manifest: docs/versions/v0.1/modules/bucky-vpn/024-bind-release-repository/task.yaml
- Approved proposal: docs/versions/v0.1/modules/bucky-vpn/024-bind-release-repository/proposal.md
- Affected paths: .github/workflows/build.yml, tests/github_actions_build_contract.py

## Approach
Keep the Release job checkout-free and pass GitHub Actions' canonical `GITHUB_REPOSITORY` identity to `gh release create` through its inherited `--repo` option. Extend the existing Release contract test to require this binding while retaining its asset-count, generated-notes, title, tag verification, permission, and publication-gate assertions.

## Risk Screen
The user selected the standard tier for this bounded repair. The command changes an external Release publication boundary, but it does not broaden when or where publication occurs: the official-repository and matching-tag gate already fixes `GITHUB_REPOSITORY` to `buckyos/bucky-vpn`, and job-local `contents: write` remains unchanged. The validation surface is unchanged because the existing GitHub Actions contract suite already owns the Release command; one focused assertion covers the missing repository binding. Hosted Release creation remains the final external proof.

## Verification
- Targeted check: `UV_CACHE_DIR=.harness/uv-cache uv run --active --with PyYAML==6.0.2 python ./tests/github_actions_build_contract.py` (16 tests); the new assertion was first run against the task-start workflow as a negative control; `git diff --check` over the delivery paths; `gh release list --repo buckyos/bucky-vpn --limit 1` from `/tmp` without a Git checkout
- Result: passed
- Residual risk or follow-up: The negative control failed only on the missing explicit repository binding, then all 16 contracts passed after the one-line workflow fix. The checkout-free read-only CLI probe succeeded, proving `--repo` avoids local Git discovery. An actual Release mutation was intentionally not performed locally; the next valid hosted tag run remains the final publication proof, and rerunning failed run `31784491251` would reuse its old workflow revision.
