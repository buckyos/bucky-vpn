# Completion Report

## Object and Scope
- Task manifest: task.yaml
- Workflow tier: trivial
- Change record: not-applicable

## Delivery Summary
- Outcome: `VpnDevice::create_device` now passes `NodeNetwork::ipv6_mask` to `tun_rs::DeviceBuilder::ipv6`, so operating-system IPv6 TUN configuration uses the IPv6-specific prefix instead of the IPv4 mask.
- Handoff: IPv4 configuration, receive-side filtering, device lifecycle, polling, PN connection behavior, public APIs, and wire data remain unchanged.

## Proposal Consistency
| change_id | requirement_or_boundary | proposal_source | delivery_evidence | finding | status |
|-----------|-------------------------|-----------------|-------------------|---------|--------|
| CHG-use-ipv6-mask-for-tun | Configure the IPv6 TUN address with `NodeNetwork::ipv6_mask` and prevent regression to the IPv4 field without changing neighboring behavior | proposal.md P-001, Scope, and Success Criteria | One argument replacement in `VpnDevice::create_device` plus `ipv6_tun_configuration_uses_the_ipv6_prefix` in the existing focused contract test | Delivery exactly matches the approved correction and preserves all stated non-goals | pass |

## Implementation Review
| area | evidence | finding | status |
|------|----------|---------|--------|
| Builder argument | `vpn-frame/src/client/vpn_device.rs` passes `self.network.ipv6_mask` as the second `config.ipv6` argument | The OS-device prefix now matches the dedicated IPv6 field already used by packet filtering and change classification | pass |
| Regression coverage | `vpn-frame/tests/tun_recovery_contract.rs::ipv6_tun_configuration_uses_the_ipv6_prefix` requires the IPv6-specific argument and rejects the former IPv4 argument | A future field regression fails deterministically without requiring a privileged live TUN | pass |
| Scope discipline | Task-start baseline comparison contains one production-line replacement and one focused test addition | No pre-existing dirty source, polling, retry, PN, IPv4, protocol, or lifecycle change was modified by this task | pass |

## Verification
- Targeted check: `cargo test -p vpn-frame --test tun_recovery_contract --locked --target-dir .harness/target/013-use-ipv6-mask-for-tun`; `cargo check -p vpn-frame --all-targets --locked --target-dir .harness/target/013-use-ipv6-mask-for-tun`; `git diff --check -- vpn-frame/src/client/vpn_device.rs vpn-frame/tests/tun_recovery_contract.rs`; focused task-start baseline diff
- Result: passed
- Exception reason: The focused contract suite passed 7 tests, including the new IPv6-prefix regression, and the all-target check passed with only the existing `get_all_send` dead-code warning. Whole-file rustfmt checking still reports pre-existing formatting differences in both already-dirty files; it reports no formatting difference in the task-added test or one-line argument replacement, so unrelated formatting churn was intentionally not applied.

## Findings
| id | severity | evidence | problem | blocking |
|----|----------|----------|---------|----------|
| F-001 | none | One-line production delta, passing seven-test contract suite, successful all-target compile, and clean focused whitespace check | No requirement mismatch or implementation defect was found in the delivered scope | no |
| F-002 | low | Live OS TUN creation requires platform privileges and was not executed | Verification binds the real builder argument and compiles the `tun-rs` integration but does not create a live IPv6 interface | no |

## Conclusion
- Accepted / rejected / needs changes: accepted
- Reason: The approved IPv6 prefix field is now passed to the real TUN builder, the former incorrect field is guarded by a focused regression, all targeted tests and compilation checks pass, and no unrelated dirty-worktree behavior was changed.
