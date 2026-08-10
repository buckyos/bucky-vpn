fn legacy_unchanged_response(
    is_first: bool,
    server_version: u16,
    cur_version: u16,
    pn_info_version: u32,
    cur_pn_info_version: u32,
) -> bool {
    !is_first && server_version == cur_version && pn_info_version == cur_pn_info_version
}

#[test]
fn equal_versions_are_unchanged_only_for_an_empty_response() {
    for (network_version, pn_version) in [(0, 0), (u16::MAX, u32::MAX)] {
        assert!(super::is_unchanged_vpn_info_response(
            false,
            network_version,
            network_version,
            pn_version,
            pn_version,
            true,
        ));
        assert!(!super::is_unchanged_vpn_info_response(
            false,
            network_version,
            network_version,
            pn_version,
            pn_version,
            false,
        ));
    }
}

#[test]
fn first_sync_and_version_mismatches_are_never_unchanged() {
    assert!(!super::is_unchanged_vpn_info_response(
        true, 7, 7, 9, 9, true,
    ));
    assert!(!super::is_unchanged_vpn_info_response(
        false, 8, 7, 9, 9, true,
    ));
    assert!(!super::is_unchanged_vpn_info_response(
        false, 7, 7, 10, 9, true,
    ));
}

#[test]
fn regression_model_exposes_the_restart_version_collision() {
    assert!(legacy_unchanged_response(false, 0, 0, 0, 0));
    assert!(!super::is_unchanged_vpn_info_response(
        false, 0, 0, 0, 0, false,
    ));
}

#[test]
fn pn_seconds_comparison_uses_equality_not_numeric_ordering() {
    assert!(!super::is_unchanged_vpn_info_response(
        false,
        7,
        7,
        70_000,
        1_723_456_789,
        true,
    ));
    assert!(!super::is_unchanged_vpn_info_response(
        false,
        7,
        7,
        1_723_456_789,
        70_000,
        true,
    ));
}
