---
task_manifest: task.yaml
status: approved
---

# Restore Server Image Build by Updating Flutter Proposal

Risk profile: not-created (replace with ./risk-profile.yaml only after high-risk confirmation)

## Workflow Tier Judgment
- Proposed tier: high-risk
- Final tier: trivial
- Tier rationale / triggered boundaries: The implementation is a one-line toolchain pin update, but it changes the compiler used for the Flutter Web bundle inside the produced server Docker image and gates GHCR publication on release tags. Produced-artifact, compatibility, and release-pipeline impact therefore requires high-risk treatment rather than classification by diff size.
- Proposal and tier confirmation: confirmed by the user's explicit `当简单任务处理就好` instruction on 2026-08-14; with no unresolved proposal question, this confirms the displayed Flutter 3.35.6 proposal and selects the trivial tier.

## Background and Goal
GitHub Actions run 31716578412 / server job 94502776484 uses the workflow-pinned Flutter 3.32.8 SDK and fails while compiling `vpn_web/lib/network_members_page.dart` because `DropdownButtonFormField.initialValue` is not available in that SDK. Flutter's official migration documentation records that `initialValue` landed in Flutter 3.35.

The user corrected the first draft's implementation direction: retain the current `initialValue` source and update the hosted Flutter SDK. The repository's available Windows SDK is Flutter 3.35.6 with Dart 3.9.2, while the tracked `vpn_web/pubspec.lock` allows Dart `>=3.8.0-0 <4.0.0`. The goal is to restore the server-image build with the smallest locally verifiable stable toolchain upgrade.

## Scope
### In scope
- Update only the `build-server` job's Flutter pin from 3.32.8 to 3.35.6.
- Retain `DropdownButtonFormField<JoinedNode>(initialValue: addingNode)` and all other frontend source behavior.
- Verify the frontend using Flutter 3.35.6 with the repository-required `flutter analyze` and `flutter build web` paths.
- Verify that `build_server.sh` reaches Docker image construction when the required Docker environment is available; otherwise keep that environment gap explicit and require the next hosted `build-server` run.

### Out of scope
- Upgrading beyond Flutter 3.35.6 or changing the exact stable pin to a floating channel/latest version.
- Changing `vpn_web` source, dependencies, lockfiles, generated API models, `build_server.sh`, Dockerfile contents, Rust server behavior, GHCR publication rules, or other build jobs.
- Adding or modifying `vpn_web` tests; repository custom policy requires analysis/build/manual verification unless the user explicitly authorizes a test exception.
- Fixing unrelated dirty-tree changes in `vpn_web/README.md` or `vpn_web/lib/base58.dart`.

### Boundary with neighboring modules
The GitHub Actions workflow owns the hosted Flutter toolchain pin. `vpn_web` remains an unchanged consumer requiring the Flutter 3.35 `initialValue` API. `build_server.sh`, the Rust server build, Dockerfile, image inspection, and GHCR publisher remain unchanged downstream consumers.

## Requirement Review
The updated direction is reasonable: the checked-in UI already targets an API introduced in stable Flutter 3.35, so aligning the hosted toolchain avoids reverting current source syntax. Pinning 3.35.6 is preferable to jumping immediately from 3.32.8 to the current 3.44 line because it is the narrowest stable upgrade that satisfies the API, it matches the locally available SDK exactly, and it enables direct analysis and web-build verification before relying on hosted CI. The tradeoff is that 3.35.6 is not the newest stable release; a newest-stable migration would materially expand breaking-change and dependency validation beyond this failure repair.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-update-server-build-flutter | Pin the server-image job to Flutter 3.35.6 so the current joined-node dropdown API compiles without changing UI source behavior. | `.github/workflows/build.yml` `build-server` Flutter setup only. | Advances the hosted Dart/Flutter compiler while avoiding the broader migration surface of the newest stable line. | Flutter 3.35.6 analysis/web build passes the prior compile location; server build reaches Docker image construction locally or in the next hosted run. | No frontend source, dependency, generated-code, server runtime, Docker, release-policy, other-job, or unrelated UI change. |

## Success Criteria
- Concrete user-visible or system-visible result: the server-image job no longer fails at `DropdownButtonFormField(initialValue:)` and can proceed to build and inspect `bucky-vpn-server:latest`.
- Required evidence: exact workflow pin validation, Flutter 3.35.6 `flutter analyze` and `flutter build web`, unchanged source/build/Docker boundaries, and server-image construction locally or in the next hosted run. If Docker or hosted execution is unavailable, the gap remains explicit rather than being claimed as passed.
- Explicit non-goals: frontend source migration, dependency/lockfile updates, generated-file changes, newest-Flutter migration, Docker/runtime changes, and changes to other jobs.

## Risks
- Flutter 3.35.6 includes newer Dart and framework versions than 3.32.8; analysis and web compilation must validate the existing dependency lock and full frontend rather than assuming compatibility from `initialValue` alone.
- Local Flutter 3.35.6 can prove the frontend compile boundary, but only Docker execution or the hosted job can prove the complete server-image path.
- The current worktree contains unrelated frontend edits and generated/platform files; implementation must keep them outside this task's attribution and avoid formatting churn.
