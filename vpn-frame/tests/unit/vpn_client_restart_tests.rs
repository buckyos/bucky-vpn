fn legacy_unchanged_response(
    is_first: bool,
    server_version: u16,
    cur_version: u16,
    pn_info_version: u16,
    cur_pn_info_version: u16,
) -> bool {
    !is_first && server_version == cur_version && pn_info_version == cur_pn_info_version
}

#[test]
fn equal_versions_are_unchanged_only_for_an_empty_response() {
    for version in [0, u16::MAX] {
        assert!(super::is_unchanged_vpn_info_response(
            false, version, version, version, version, true,
        ));
        assert!(!super::is_unchanged_vpn_info_response(
            false, version, version, version, version, false,
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
