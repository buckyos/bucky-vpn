---
task_manifest: task.yaml
status: approved
---

# Remove Unused PN Observation Report API Proposal

Risk profile: not-created (replace with ./risk-profile.yaml only after high-risk confirmation)

## Workflow Tier Judgment
- Proposed tier: trivial
- Final tier: trivial
- Tier rationale / triggered boundaries: The accept-time observation API is no longer called by production code after task 027 moved observation refresh to heartbeat handling. Removing it and its private helper only affects unused `bucky-vpn-server` code plus focused unit-test setup; no runtime path, protocol, persistence, configuration, or public crate export is known to depend on it, and targeted compile/tests provide a clear verification signal.
- Proposal and tier confirmation: confirmed by the user on 2026-08-18 with “确认”.

## Background and Goal
`PnServerManager::report_observed_heartbeat` and its private `update_observed_remote_heartbeat` helper remain from the old connection-accept observation path. Current production observation is supplied through `report_heartbeat_with_observation`; the retained API is only used by tests and produces dead-code warnings.

The goal is to remove the obsolete accept-time observation path and update focused tests to exercise observation state through the current heartbeat contract, eliminating the misleading unused API without changing PN behavior.

## Scope
### In scope
- Remove `PnServerManager::report_observed_heartbeat`.
- Remove its now-unneeded private observation-only update helper if no production path remains.
- Refactor focused `PnServerManager` unit tests that currently use the obsolete API to use the current heartbeat-with-observation path or another current manager contract.
- Preserve heartbeat TTL, observation retention, identity validation, endpoint merging, approval, and selection behavior.

### Out of scope
- Do not change cmd158 wire protocol or `vpn-frame` interfaces.
- Do not reintroduce connection-accept or connection-disconnect observation handling.
- Do not change PN approval, persistence, traffic accounting, client PN-list behavior, or configuration.
- Do not perform broader API redesign or cleanup beyond this obsolete path.

### Boundary with neighboring modules
The change is limited to `bucky-vpn-server` manager code and its focused unit test. `vpn-frame` remains the owner of the shared selector/observer contracts, while `bucky-vpn-server` retains only the current heartbeat-driven observation implementation.

## Requirement Review
The cleanup is reasonable because production no longer calls the accept-time report API. Tests should not preserve an obsolete public surface merely as setup convenience. The main tradeoff is slightly more verbose test setup through the current heartbeat contract; that is preferable to retaining misleading dead code.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-remove-unused-pn-observation-report | Remove the unused accept-time observation report path and keep focused coverage on the current heartbeat-with-observation contract. | Only `vpn-server` manager implementation and its focused unit test. | Tests may use more explicit heartbeat setup. | No production references remain; focused recovery tests and affected-crate all-target compilation pass without the existing unused-method warning. | No behavior, protocol, selector contract, or lifecycle redesign. |

## Success Criteria
- Concrete user-visible or system-visible result: `PnServerManager::report_observed_heartbeat` is gone, and current tests cover equivalent observation state without that API.
- Required evidence: repository search shows no remaining call sites; focused PN recovery tests pass; `cargo check -p bucky-vpn-server --all-targets --locked` passes.
- Explicit non-goals: no PN behavior change, wire/API redesign, or broad workspace cleanup.

## Risks
- Existing tests may rely on implicit accept-time semantics that differ from heartbeat observation; test refactoring must preserve their intended state-transition coverage.
- The workspace already contains extensive unrelated dirty and untracked files; implementation must not stage, revert, or remove them.
