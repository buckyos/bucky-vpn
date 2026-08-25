# Completion Report

## Object and Scope
- Task manifest: task.yaml
- Workflow tier: standard
- Change record: docs/changes/009-expand-pn-control-command-limit.md

## Delivery Summary
- Outcome: The dedicated PN control channel now uses a symmetric three-byte `U24` command-length contract capped at 10 MiB, accepts traffic command bodies through 10 MiB and up to 25,000 records, and defaults PN traffic upload chunks to 25,000 records.
- Handoff: Ordinary VPN commands remain on `VpnCmdPkgLen = U16`; PN nodes and control-plane nodes must be upgraded together because mixed U16/U24 PN-control endpoints cannot parse each other. Smaller completed traffic batches still send immediately rather than waiting to fill 25,000 records.

## Proposal Consistency
| change_id | requirement_or_boundary | proposal_source | delivery_evidence | finding | status |
|-----------|-------------------------|-----------------|-------------------|---------|--------|
| CHG-pn-control-command-length-contract | Give only the PN control channel a 10 MiB U24 length/header contract, align byte validation, and raise the logical ceiling to 25,000 without changing ordinary VPN U16 framing | proposal.md P-001, Scope, Requirement Review, and Success Criteria | `VpnControlCmdPkgLen`/`VpnControlCmdHeader`, symmetric `VpnControlClient`/`PnControlServer` bounds, shared `VPN_CONTROL_CMD_MAX_BYTES`, 25,000 constant, and boundary/encoding tests | The implementation matches the approved shared contract; 25,000 maximum-size proxy records encode to 9,500,008 bytes and 25,001 records are rejected by the logical boundary | pass |
| CHG-pn-control-command-length-integration | Select the same PN-control type in both concrete runtime endpoints and make 25,000 the runtime/example upload default without delaying smaller batches | proposal.md P-002, Scope, Requirement Review, and Success Criteria | Concrete classified client and proxy-control service aliases use `VpnControlCmdPkgLen`; `VpnServer` keeps separate U16/U24 service bounds; configuration default/example are 25,000; the 25,001-record service test produces chunks of 25,000 and 1 | Both endpoint assembly and upload behavior match the approved integration requirement while the ordinary command server remains U16 | pass |

## Implementation Review
| area | evidence | finding | status |
|------|----------|---------|--------|
| Length contract | `VpnControlCmdPkgLen` aliases `U24` with a 10 MiB limit; `VpnControlCmdHeader` and the three-byte/MAX assertions bind the contract | A single exported PN-control type owns the frame width and maximum; no duplicated transport limit is used by the handler | pass |
| Endpoint symmetry | `VpnControlClient`, `PnControlServer`, `ControlCmdClient`, and `ProxyControlCmdService` all bind `VpnControlCmdPkgLen` | Sender and receiver cannot drift between U16 and U24 within the concrete runtime assembly | pass |
| Ordinary VPN isolation | `VpnCmdPkgLen = U16` remains in `vpn_protocol.rs`; `VpnCmdServer` remains bound to that U16 type; `VpnServer` now separates its ordinary and PN-control generic bounds | The larger framing is confined to the purpose-specific proxy-control tunnel | pass |
| Byte and record boundaries | Tests assert a three-byte header, acceptance at 10 MiB/25,000, rejection above either boundary, 380-byte maximum estimated proxy record, and a 9,500,008-byte maximum estimated request | The requested count is supported by actual codec measurement with about 9.4% frame headroom under the documented 32-byte node-ID assumption | pass |
| Upload default and behavior | `PnTrafficUploadConfig::default()` references `MAX_TRAFFIC_RECORDS_PER_COMMAND`; example YAML is 25000; the 25,001-record service test creates two immediate chunks | Runtime and documented defaults agree, smaller tail batches send immediately, and the configuration maximum remains tied to the shared ceiling | pass |
| Existing semantics | Four dedicated control-client regression tests and the PN control/client/service suites cover command forwarding, result errors, validation, retry, queue, and shutdown behavior | No unrelated command payload, retry, timeout, sequence, or accounting behavior changed | pass |
| Scope discipline | Baseline-derived task paths contain the ten intended Rust/config paths; additional changed `.gitignore`, installer, log, and Wintun paths were produced externally during the task and were neither edited nor removed by this delivery | User/external concurrent changes are preserved and excluded from the implementation claim | pass |

## Verification
- Targeted check: `cargo check -p vpn-frame -p bucky-vpn-server`; `cargo test -p vpn-frame` (22 passed); `cargo test -p bucky-vpn-server server_config::tests::sn_and_pn_default_enabled_without_config_file` (1 passed); `cargo test -p bucky-vpn-server pn_control_client_tests` (2 passed); `cargo test -p bucky-vpn-server pn_traffic_service::node_traffic_tests` (16 passed); focused `rustfmt --edition 2024 --check`; focused `git diff --check`; protocol-type source scan
- Result: passed
- Exception reason: Repository rules prohibit automatically running `cargo fmt`; focused rustfmt checks passed for the clean/new task files. Existing unrelated formatting differences remain in several already-dirty files, while every task-added hunk passed manual rustfmt comparison and the complete focused diff passed whitespace validation.

## Findings
| id | severity | evidence | problem | blocking |
|----|----------|----------|---------|----------|
| F-001 | none | Passing compile, 41 targeted/full test executions across the affected suites, boundary source scan, and focused diff checks | No requirement or implementation defect was found in the delivered scope | no |
| F-002 | low | U16-to-U24 PN-control frame-width change and approved proposal risk screen | Rolling mixed-version deployment is incompatible; PN and control-plane endpoints require coordinated upgrade | no |
| F-003 | low | Four concurrent commands times the 9.06 MiB worst-case estimated request, plus decoded/database work | The requested 25,000 default increases transient memory and processing load on very large deployments | no |
| F-004 | none | Baseline changed-path evidence lists external installer/log/Wintun changes beyond the ten task paths | Concurrent user/external files are unrelated to this task and remain untouched | no |

## Conclusion
- Accepted / rejected / needs changes: accepted
- Reason: Both PN-control endpoints use the same bounded U24 contract, byte/count/default boundaries match the approved proposal, ordinary VPN framing remains U16, and all focused cross-crate compile and behavior checks pass with no blocking finding.
