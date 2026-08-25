---
task_manifest: task.yaml
status: approved
---

# Fix sfo-cmd-server 0.4 Compile Errors Proposal

Risk profile: ./risk-profile.yaml

## Workflow Tier Judgment
- Proposed tier: high-risk
- Final tier: high-risk
- Tier rationale / triggered boundaries: `vpn-frame/Cargo.toml` has moved `sfo-cmd-server` from `0.3.2` to `0.4`, and the lockfile resolves `0.4.0`. Adapting repository call sites crosses an external dependency API and build-compatibility boundary, so the repository's dependency/build-graph trigger selects high-risk by default.
- Proposal and tier confirmation: explicitly confirmed and launched for automatic downstream completion by the user statement “确认，自动完成后续任务” on 2026-07-21

## Background and Goal
The existing `vpn-frame` integration was written against `sfo-cmd-server 0.3.2`. After the dependency was updated to `0.4`, the workspace no longer compiles because the dependency's public API changed. A diagnostic `cargo check -p vpn-frame --locked` reproduced 43 compiler errors, all rooted in the new `CmdPkgLen` bound: raw `u16` is no longer accepted as the command package-length type and must be replaced by the dependency's `U16` wrapper. The goal is to migrate the affected `vpn-frame` call sites to the `0.4` API while preserving the VPN control-channel behavior.

## Scope
### In scope
- Reproduce and enumerate compile failures caused by the `sfo-cmd-server 0.4.0` API changes.
- Update affected `vpn-frame` imports, types, trait implementations, constructors, and call sites to the supported `0.4` API.
- Add focused regression coverage where a changed adapter behavior can be exercised locally; otherwise record why compile-closure evidence is the appropriate regression signal.
- Verify that `vpn-frame` and its workspace consumers compile against the updated dependency.

### Out of scope
- Reverting `sfo-cmd-server` to `0.3.2`.
- Changing VPN protocol semantics, command identifiers, serialization formats, authentication behavior, or tunnel lifecycle behavior unless the `0.4` API makes a compatibility decision unavoidable and it is returned for proposal revision.
- Broad refactoring or cleanup unrelated to the dependency migration.
- Modifying the Flutter Web UI or packaging behavior.

### Boundary with neighboring modules
The migration is owned by `vpn-frame`, which directly integrates `sfo-cmd-server`; `vpn-client` and `vpn-server` are compile consumers and are included in verification, but no production changes outside `vpn-frame` are proposed unless diagnostics prove a direct compatibility requirement.

## Requirement Review
The request is reasonable: retaining the already-selected `0.4` dependency requires adapting the integration rather than suppressing compiler errors. The safest direction is a mechanical API migration that preserves existing control-channel semantics, using compiler diagnostics and the dependency's installed source as the compatibility reference. The installed `0.4.0` source confirms that `U16` encodes through the same underlying `u16` raw codec and caps its default limit at `u16::MAX`, so the identified migration preserves the existing two-byte length field and range. If later diagnostics expose a semantic or protocol change rather than this mechanical adaptation, execution must return to this proposal before that behavior is chosen.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-sfo-cmd-server-0-4-api-migration | Migrate all affected `vpn-frame` integrations so the workspace builds with `sfo-cmd-server 0.4.0` while retaining existing VPN command/control behavior. | Limit production edits to dependency-affected Rust call sites; neighboring crates are verification consumers unless compilation proves otherwise. | Prefer direct API adaptation over compatibility shims; any unavoidable semantic change requires proposal revision. | Initial failures are reproduced; targeted regression or a concrete infeasibility record is provided; `vpn-frame` and affected workspace consumer checks pass with `0.4.0`. | No dependency rollback, protocol redesign, unrelated refactor, UI change, or packaging change. |

## Success Criteria
- Concrete user-visible or system-visible result: the Rust workspace compiles successfully with `sfo-cmd-server 0.4.0`, with existing VPN command and tunnel behavior preserved.
- Required evidence: the captured 43 pre-fix compiler failures, source-level mapping from raw `u16` to `sfo_cmd_server::U16`, passing focused verification for `vpn-frame`, and compile closure for `vpn-client` and `vpn-server` consumers.
- Explicit non-goals: dependency rollback, new features, protocol/schema changes, UI work, or unrelated cleanup.

## Risks
- The `0.4` release may contain semantic changes beyond renamed or reshaped Rust APIs; those cannot be guessed from compiler success alone and must be checked against the dependency implementation.
- `vpn-frame` re-exports `sfo_cmd_server`, so changed public types may affect downstream consumers even when their source files do not import the dependency directly.
- The working tree already contains user changes and many untracked files; the task must preserve them and keep its delivery manifest limited to task-owned changes.
