---
task_manifest: task.yaml
status: approved
---

# Allow Wildcard SN Listen Address for NAT Probing Proposal

Risk profile: not-created (replace with ./risk-profile.yaml only after high-risk confirmation)

## Workflow Tier Judgment
- Proposed tier: standard
- Final tier: standard
- Tier rationale / triggered boundaries: This is a bounded `bucky-vpn-server` configuration and startup-assembly change that adapts the existing opt-in SN NAT probe wiring to the updated port-only `p2p-frame` wire contract. It changes accepted configuration semantics and startup validation behavior, so it is not trivial. It remains inside `bucky-vpn-server` and the already-validated upstream `p2p-frame` probe implementation; no protocol change, persistence schema, security boundary, dependency declaration, or default runtime activation is introduced by this repository.
- Proposal and tier confirmation: confirmed by the user with “确认” on 2026-09-06; the displayed proposal and standard tier are approved.

## Background and Goal
The locked `p2p-frame` revision has moved to a port-only NAT probe wire contract: `NatProbeDirective` and `ReportSnResp` carry UDP ports only, the server binds every reflector to `0.0.0.0:<port>`, and clients reconstruct complete probe endpoints from the authenticated QUIC SN IPv4 plus the delivered port vector. The old requirement that VPN server top-level `ip` be an externally reachable IPv4 when NAT probing is enabled comes from the previous endpoint-bearing revision and no longer applies.

The goal is to make `vpn-server` consistent with that contract so operators can keep the default SN listen address `0.0.0.0` and still enable `sn.nat_probe_ports`, without inventing a static public IPv4 or generating obsolete mapped/public identity endpoint pairs.

## Scope
### In scope
- Stop rejecting `0.0.0.0` as the top-level `ip` when `sn.nat_probe_ports` is enabled.
- Remove the obsolete conclusion that enabling NAT probing requires a public IPv4 and mapped/local identity endpoint pairs.
- Keep ordinary service endpoints as the SN identity endpoints whether or not NAT probing is enabled, so wildcard listen works in both paths.
- Keep the existing 2–8 port validation, uniqueness/non-zero checks, and `sn.enabled` requirement.
- Keep passing the validated ports to `SnServiceConfig::set_nat_probe_ports` before `create_sn_service`.
- Update `config.example.yaml` so it no longer tells operators to change top-level `ip` to a public IPv4 for NAT probing.
- Add/replace focused tests proving wildcard listen addresses are accepted and identity endpoints stay unchanged when probing is enabled.

### Out of scope
- No change to `p2p-frame` NAT probe protocol, classification, scheduling, signatures, rate limits, reflector bind semantics, or client reconstruction.
- No automatic public-IP discovery, STUN, firewall/NAT mutation, or container change.
- No IPv6 NAT probing support.
- No default activation or new configuration fields.
- No change to PN advertised-address, `pn.advertised_ip`, or `pn.port_mapping` semantics.

### Boundary with neighboring modules
`bucky-vpn-server` owns the configuration contract, identity endpoint assembly, and passing the opt-in into `SnServiceConfig`. Port validation and reflector lifecycle remain owned by `p2p-frame`. `vpn-frame`, `vpn-client`, HTTP API, SQLite, and Flutter UI are not changed.

## Requirement Review
The request is reasonable and matches the new upstream contract. Keeping the old public-IPv4 requirement would force operators behind NAT/containers to configure a static WAN address even though the new client wire only needs the authenticated QUIC SN IPv4 plus probe ports. The correct direction is to treat the ordinary service endpoint set (including `0.0.0.0`) as the SN identity regardless of probing, because `p2p-frame` already binds reflectors to `0.0.0.0` and does not read probe endpoint addresses from the identity in this revision.

One tradeoff: local tests and startup validation can no longer reject a deployment where the reflector UDP ports are not actually reachable from outside. That reachability was already an owner-side operational requirement and remains documented; repository tests already cannot prove live NAT/firewall behavior.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-allow-wildcard-sn-listen | Allow the wildcard SN listen address (`0.0.0.0`) when NAT probing is enabled and stop deriving mapped/public + wildcard/local identity endpoint pairs; keep ordinary service endpoints for the SN identity and wire validated ports into `SnServiceConfig`. | `bucky-vpn-server` config parsing, startup assembly, example config, and focused tests only; `p2p-frame` remains unchanged. | Operators no longer need to set a static public IPv4 for probing, but external firewall/NAT reachability of reflector ports still cannot be proven locally. | Focused tests prove wildcard and other listen addresses are accepted, identity endpoints remain service endpoints when ports are enabled, disabled path still works, and the locked `9b7e2d6` `p2p-frame` API closes over `SnServiceConfig::set_nat_probe_ports`; affected-crate tests and all-targets checkout compile pass. | No protocol/API field changes in this repository, no new config key, no live public-NAT evidence, no IPv6 support, and no unrelated lockfile churn. |

## Success Criteria
- Concrete user-visible or system-visible result: with `sn.nat_probe_ports: [3630, 3631]` and top-level `ip: "0.0.0.0"`, `vpn-server` parses the configuration successfully and starts the embedded SN with UDP probe reflectors bound by upstream `p2p-frame`.
- Required evidence: config/validation tests cover wildcard acceptance, disabled/default behavior, invalid port sets, and identity-endpoint preservation; `cargo test -p bucky-vpn-server --locked sn_nat_probe -- --test-threads=1` passes; `cargo check -p bucky-vpn-server --all-targets --locked` passes; `git diff --check` passes; completion report performs an independent defect-discovery pass.
- Explicit non-goals: no claim that local tests prove public reachability, no new network-address discovery, no upgrade to a higher-risk artifact set, and no change to upstream `p2p-frame`.

## Risks
- If the UDP reflector ports are not opened/forwarded on the actual deployment, clients cannot probe even though startup succeeds. This risk existed in task 034 and is still documented; it is not newly introduced.
- The repository working tree already contains a user-owned `Cargo.lock` change that pins `p2p-frame` to `9b7e2d6...`. This proposal treats that lock update as required context and preserves it, but does not claim unrelated transitive lockfile churn as task-authored work.
- Removing the public-IPv4 guard is intentional; it is sourced from the new port-only wire semantics, not from weakening local validation for its own sake.
