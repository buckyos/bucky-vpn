# Refresh README Release and Installation

- Status: complete
- Task manifest: docs/versions/v0.1/modules/globals/023-refresh-readme-release-installation/task.yaml
- Approved proposal: docs/versions/v0.1/modules/globals/023-refresh-readme-release-installation/proposal.md
- Affected paths: README.md

## Approach
Regenerate the root README as a Chinese user-first installation and deployment guide. Derive client asset names and release gates from `.github/workflows/build.yml`, derive the server image and tags from the publication job, and derive container paths, ports, configuration, installed services, source-build outputs, and CLI parameters from the current repository implementation.

## Risk Screen
This is documentation-only work and changes no package, image, runtime, build, or publication behavior. The material documentation risks are stale release links, incorrect installer filenames, a non-runnable Docker example, misleading port/protocol mappings, and claims that confuse configured publication behavior with confirmed hosted availability. The verification therefore cross-checks those boundaries directly against current primary sources.

## Verification
- Targeted check: `UV_CACHE_DIR=.harness/uv-cache uv run --active --with PyYAML==6.0.2 python ./tests/github_actions_build_contract.py` (16 passed); temporary README/source contract validator covering YAML, Bash syntax, assets, image tags, ports, mounts, build commands, links, and CLI options; `git diff --check -- README.md`; focused source/diff inspection
- Result: passed
- Residual risk or follow-up: Unauthenticated requests to the GitHub Releases pages currently return 404, so the README describes configured publication behavior and explicitly conditions downloads on a successful matching-Tag publication; an authenticated hosted release remains external evidence.
