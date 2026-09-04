---
task_manifest: task.yaml
status: approved
---

# Enable SN NAT Type Probe Proposal

Risk profile: not-created (replace with ./risk-profile.yaml only after high-risk confirmation)

## Workflow Tier Judgment
- Proposed tier: standard
- Final tier: standard
- Tier rationale / triggered boundaries: This is a bounded `bucky-vpn-server` configuration and assembly feature. It opts the embedded SN into an already implemented and validated `p2p-frame` NAT probe service without changing the P2P wire protocol, dependency declaration, persistent data, authentication, or default runtime behavior. The configured path opens additional UDP listeners and therefore needs explicit validation, deployment guidance, targeted configuration tests, and an affected-crate build, so it is not trivial. Because the feature remains disabled unless ports are configured and lifecycle/security ownership stays in `p2p-frame`, no material cross-project protocol or architectural change is currently confirmed; the standard flow is proportionate.
- Proposal and tier confirmation: confirmed by the user with “确认” on 2026-09-04; the displayed proposal and standard tier are approved.

## Background and Goal
The locked `p2p-frame` revision now supports SN-directed NAT type detection. Its server-side opt-in is `SnServiceConfig::set_nat_probe_ports`: an empty port list disables the service, while a valid list starts signed UDP reflectors and advertises their endpoints to capable clients.

`bucky-vpn-server` currently constructs `SnServiceConfig` without passing NAT probe ports and builds only LAN-area identity endpoints, so operators cannot enable this capability. The goal is to add an explicit VPN server configuration contract and wire it into the embedded SN service.

## Scope
### In scope
- Add optional `sn.nat_probe_ports` configuration, represented in YAML as a list of UDP ports and overridable through the existing environment configuration path.
- Keep NAT type probing disabled when the option is absent or empty.
- Validate the application-owned configuration boundary before runtime assembly: enabled probing requires 2 through 8 non-zero, unique ports, `sn.enabled: true`, and a concrete IPv4 value in the existing top-level `ip` setting.
- When probing is enabled, derive the SN identity endpoint shape expected by `p2p-frame`: advertise the configured public IPv4 as mapped QUIC/TCP endpoints and pair each with wildcard local listen endpoints, while preserving the existing ordinary SN/PN endpoint behavior outside the embedded SN identity.
- Pass the validated ports to `SnServiceConfig::set_nat_probe_ports` before `create_sn_service`.
- Document in `config.example.yaml` that the probe ports are additional UDP listeners, must be opened or forwarded to this process, and require top-level `ip` to be the externally reachable IPv4.
- Add focused inline Rust tests for disabled/default behavior, accepted YAML/environment values, rejected port sets/modes/addresses, and endpoint mapping; run affected-crate tests and an all-targets compile check.

### Out of scope
- No change to `p2p-frame` NAT classification, probe packet/signature protocol, scheduling, client behavior, retry policy, rate limits, or lifecycle implementation.
- No automatic public-IP discovery, STUN service, firewall/NAT rule mutation, container manifest change, or port allocation.
- No IPv6 NAT probing; the upstream feature currently requires one advertised static-WAN IPv4.
- No default activation and no hard-coded production probe ports.
- No change to VPN PN advertised-address or `pn.port_mapping` semantics.

### Boundary with neighboring modules
`bucky-vpn-server` owns parsing, validation, deployment documentation, identity endpoint assembly, and passing the opt-in into `SnServiceConfig`. `p2p-frame` remains the owner of signed UDP reflectors, client directives, NAT observations/classification, rate limiting, maintenance, and shutdown. `vpn-frame`, `vpn-client`, the HTTP API, SQLite schema, and Flutter UI are not changed.

## Requirement Review
The request is reasonable, but a standalone boolean would be insufficient: upstream needs at least two distinct reflector ports to compare mappings and also derives the advertised probe IP from the SN identity. The proposed configuration therefore uses the upstream-aligned `sn.nat_probe_ports` list as the enable signal and validates the required public IPv4 at startup.

Reusing top-level `ip` avoids introducing two competing SN address settings. On the enabled path, the public endpoint is paired with a wildcard local endpoint so the process can bind behind ordinary host/container port forwarding while still advertising the configured public address. The tradeoff is that an operator who wants NAT probing must set `ip` to the externally reachable IPv4 rather than leaving it at `0.0.0.0`; this is documented and invalid combinations fail closed.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-enable-sn-nat-type-probe | Add validated `sn.nat_probe_ports` configuration and wire it through mapped/public plus wildcard/local identity endpoints into `SnServiceConfig::set_nat_probe_ports`. | `bucky-vpn-server` config, startup assembly, example config, and focused inline tests only; upstream NAT probe implementation remains unchanged. | Enabling the feature consumes 2–8 additional UDP ports and requires top-level `ip` to be an externally reachable IPv4, but absent/empty configuration preserves the current behavior. | Focused tests prove default/empty disablement, YAML and environment parsing, invalid-set/mode/address rejection, mapped/local endpoint construction, and service-config wiring; `bucky-vpn-server` tests and all-targets compilation pass against the locked `p2p-frame` revision. | No new NAT algorithm/protocol, automatic public-IP or firewall management, IPv6 support, default activation, PN address semantic change, or unrelated lockfile modification. |

## Success Criteria
- Concrete user-visible or system-visible result: an operator can configure two or more UDP ports under `sn.nat_probe_ports`; with `sn.enabled: true` and a concrete public IPv4 in top-level `ip`, `vpn-server` starts the upstream SN NAT probe reflectors and advertises them to capable P2P clients.
- Required evidence: config parsing and validation tests cover normal, disabled, boundary, duplicate/zero/one-port, SN-disabled, and invalid-address cases; endpoint tests prove mapped-public endpoints are immediately paired with wildcard-local endpoints; source/compile evidence proves the configured ports reach `SnServiceConfig`; affected-crate tests and `cargo check -p bucky-vpn-server --all-targets --locked` pass.
- Explicit non-goals: no live public-NAT claim from local tests, no automatic network configuration, no upstream protocol/runtime changes, and no default probe activation.

## Risks
- The probe ports are externally reachable UDP services when enabled. Operators must explicitly open or forward exactly the configured ports; repository tests cannot prove a deployment firewall or public NAT mapping.
- Advertising the wrong public IPv4 makes probe endpoints unreachable. Startup validation can reject malformed, unspecified, loopback, private, multicast, and broadcast IPv4 values, but cannot prove external reachability.
- Identity endpoint ordering is a runtime contract: each mapped endpoint must be directly followed by its compatible local endpoint so `p2p-frame` binds locally while advertising the mapped address.
- The working tree already contains a user-owned `Cargo.lock` update, including the required `p2p-frame` revision. This task will preserve it and will not claim unrelated transitive lockfile churn as task-authored work.
