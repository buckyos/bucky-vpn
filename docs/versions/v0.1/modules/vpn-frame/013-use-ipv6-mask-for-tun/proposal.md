---
task_manifest: task.yaml
status: approved
---

# Use IPv6 Mask for TUN Configuration Proposal

## Workflow Tier Judgment
- Proposed tier: trivial
- Final tier: trivial
- Tier rationale / triggered boundaries: The correction changes one private argument at the existing `tun-rs` device-construction boundary inside `vpn-frame`. It does not change public APIs, wire formats, persistence, dependencies, lifecycle ownership, retry behavior, or cross-module contracts, and a focused source-contract regression plus package compile check can verify it.
- Proposal and tier confirmation: approved by the user with “确认” on 2026-08-06.

## Background and Goal
`VpnDevice::create_device` currently passes the IPv4 `mask` field to `DeviceBuilder::ipv6`, even though `NodeNetwork` carries a separate `ipv6_mask` and packet filtering already uses that IPv6-specific prefix. The goal is to configure the operating-system TUN with the requested IPv6 prefix.

## Scope
### In scope
- Pass `NodeNetwork::ipv6_mask` to `tun_rs::DeviceBuilder::ipv6`.
- Add a focused regression assertion that binds the IPv6 builder call to `ipv6_mask` rather than `mask`.
- Run focused formatting, regression-test, and `vpn-frame` compile verification.

### Out of scope
- Changing IPv4 mask handling, packet filtering, TUN recreation classification, retry behavior, server polling, PN connection handling, public APIs, or wire data.
- Refactoring `VpnDevice` or introducing a new device-builder abstraction.

### Neighboring boundaries
`vpn-frame::client::VpnDevice` remains the sole implementation owner. The serialized `NodeNetwork` fields and server/client protocol remain unchanged; only the existing IPv6 builder argument is corrected.

## Requirement Review
The requested change is reasonable and narrowly addresses the confirmed mismatch. Using `ipv6_mask` aligns OS device configuration with both the data model and the receive-side IPv6 filter. No material tradeoff is introduced beyond correcting deployments that may have relied on the erroneous IPv4 prefix value for IPv6 configuration.

## Proposal Items
| proposal_id | change_id | requirement | boundary | tradeoff | success_evidence | non_goal |
|-------------|-----------|-------------|----------|----------|------------------|----------|
| P-001 | CHG-use-ipv6-mask-for-tun | Configure IPv6 TUN addresses with `NodeNetwork::ipv6_mask` and prevent regression to the IPv4 field. | `vpn-frame` device construction and focused contract test only. | Existing incorrect IPv6 prefix behavior is intentionally corrected. | Source-contract regression passes, the changed Rust files pass focused rustfmt checking, and `vpn-frame` compiles. | No polling, retry, PN, IPv4, serialization, or lifecycle redesign. |

## Success Criteria
- `DeviceBuilder::ipv6` receives `self.network.ipv6_mask`.
- A focused regression fails if the builder call uses `self.network.mask` again.
- Focused tests and `cargo check -p vpn-frame --all-targets --locked` pass.
- No unrelated working-tree changes are modified.

## Risks
- Runtime IPv6 TUN creation is platform/privilege dependent, so portable automated verification will bind the correct builder argument and compile the real `tun-rs` integration rather than creating a live interface.
