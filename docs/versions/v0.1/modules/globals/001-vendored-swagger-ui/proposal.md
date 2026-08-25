---
task_manifest: task.yaml
status: approved
---

# Vendored Swagger UI Proposal

Risk profile: ./risk-profile.yaml

## Workflow Tier Judgment
- Proposed tier: high-risk
- Final tier: high-risk
- Tier rationale / triggered boundaries: Enabling `vendored` changes the Rust dependency/build graph and supply-chain inputs for both binaries, while removing a build-time network dependency. The repository classifies material dependency/build-graph changes as high-risk.
- Proposal and tier confirmation: the original vendored-only scope and the revised Harness repair scope were explicitly confirmed by the user on 2026-07-19

## Background and Goal
Windows builds currently resolve `sfo-http` with its `openapi` feature, which brings in `utoipa-swagger-ui 9.0.2`. Because neither `reqwest` nor `vendored` is enabled, that crate invokes the first `curl.exe` on PATH to download Swagger UI 5.17.14 during its build script. On the current Windows host, the system Schannel curl fails certificate-revocation checking with `CRYPT_E_REVOCATION_OFFLINE`.

The goal is to make both Rust application builds use the crate-provided vendored Swagger UI archive and stop downloading Swagger UI at compile time.

## Scope
### In scope
- Enable `utoipa-swagger-ui` feature unification with `vendored` in `vpn-client/Cargo.toml`.
- Enable the same feature for `vpn-server/Cargo.toml`, so workspace/server builds do not retain the same failure.
- Update dependency resolution metadata as required by Cargo, preserving unrelated existing lockfile content.
- Verify that the resolved feature graph includes `utoipa-swagger-ui/vendored` and that the affected packages compile through the Swagger UI build step without invoking the remote download path.
- Correct the Harness proposal approval gate so `schema-check.py --require-approved` validates an approved proposal while `task.yaml` is still in proposal stage, matching `harness-check.py` and `task-transition.py` lifecycle behavior.
- Add and register a focused regression test that fails on the current contradictory gate and passes after the repair.

### Out of scope
- Changing OpenAPI routes, schemas, or runtime Swagger UI behavior.
- Upgrading `sfo-http`, `utoipa`, or `utoipa-swagger-ui` beyond the versions selected by the existing dependency constraints.
- Changing Windows TLS, proxy, certificate-revocation, or global curl configuration.
- Refactoring unrelated dependency declarations or cleaning unrelated working-tree changes.
- Broad Harness lifecycle or schema refactoring beyond the proposal approval contradiction.

### Boundary with neighboring modules
Product behavior changes remain limited to dependency activation for the `bucky-vpn` and `bucky-vpn-server` application crates. The additional `repo-governance` scope is limited to restoring the already-declared proposal approval transition and its regression coverage. `vpn-frame`, Flutter Web UI, runtime API contracts, and packaging behavior are not changed.

## Requirement Review
The vendored request is reasonable and preferable to weakening TLS revocation checks or relying on a developer-specific PATH order. Vendoring removes the fragile compile-time GitHub dependency and makes offline/restricted-network builds reproducible. The tradeoff is an additional vendored-assets crate in the dependency graph and a larger downloaded Cargo dependency/cache footprint. Direct dependencies will be used only to unify the transitive crate's feature because `sfo-http 0.6.8` does not expose a feature that forwards `vendored`.

The blocking Harness repair is also necessary and bounded. `harness-check.py` intentionally invokes `schema-check.py --require-approved` before recording proposal completion, while `schema-check.py` currently rejects that exact stage before it can inspect the approved proposal. Allowing the flag at proposal stage makes the checker enforce approval rather than bypass it. A focused subprocess-based regression test will cover both approved success and draft rejection.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-vendored-swagger-ui-client | Activate `utoipa-swagger-ui/vendored` for the client dependency graph. | `vpn-client/Cargo.toml` and required lock resolution only. | Adds vendored Swagger UI asset dependency. | Cargo feature resolution shows `vendored`; client compilation passes the Swagger UI build step without remote curl download. | No client API/runtime behavior change. |
| P-002 | CHG-vendored-swagger-ui-server | Activate `utoipa-swagger-ui/vendored` for the server dependency graph. | `vpn-server/Cargo.toml` and required lock resolution only. | Same additional vendored asset dependency. | Cargo feature resolution shows `vendored`; server/workspace resolution retains the feature. | No server API/runtime behavior change. |
| P-003 | CHG-proposal-approval-gate | Make proposal-stage `--require-approved` validate the proposal status and add task-runner-reachable regression coverage. | `schema-check.py`, one dedicated Harness test, and its repo-governance unit registration only. | Adds a small governance regression surface while unblocking the declared lifecycle. | Test proves approved proposal succeeds and draft proposal fails; the current task legally advances proposal to design. | No broader lifecycle/schema redesign or weakened approval check. |

## Success Criteria
- Concrete user-visible or system-visible result: Windows compilation no longer downloads `v5.17.14.zip` through `curl.exe` for `utoipa-swagger-ui`.
- Required evidence: resolved Cargo feature graph contains `utoipa-swagger-ui/vendored`; targeted build/check reaches beyond the previous custom-build failure; dependency changes contain no unrelated upgrade.
- Harness evidence: focused regression is red before/fixed green after, repo-governance unit entry can invoke it, and `task-transition.py` records a valid proposal receipt.
- Explicit non-goals: fixing the host's Schannel revocation connectivity or changing Swagger/OpenAPI application behavior.

## Risks
- Cargo feature unification must apply in each independently built package graph; changing only one application would leave the other vulnerable to the same failure.
- Lockfile regeneration could select unrelated newer compatible packages if performed without preserving the current resolution; verification must inspect lockfile drift.
- Vendored assets move trust from a build-time GitHub download to the published `utoipa-swagger-ui-vendored` crate, which is more reproducible but still a supply-chain input.
- An overly broad Harness change could weaken approval enforcement; the repair must preserve rejection of draft proposals and modify only the contradictory stage guard plus focused test registration.
