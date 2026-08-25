use bucky_raw_codec::{RawConvertTo, RawFrom};
use vpn_frame::{GetVpnInfoReq, GetVpnInfoResp, VPN_CMD_VERSION};

const VPN_SERVER_SOURCE: &str = include_str!("../src/server/vpn_server.rs");
const PN_CONTROL_SERVER_SOURCE: &str = include_str!("../src/server/pn_control_server.rs");
const VPN_PROTOCOL_SOURCE: &str = include_str!("../src/vpn_protocol.rs");

#[test]
fn request_raw_codec_preserves_u32_pn_timestamp_and_u16_network_version() {
    let request = GetVpnInfoReq {
        seq: 17u32.into(),
        info_version: Some(u16::MAX),
        pn_info_version: Some(u16::MAX as u32 + 1_723_456_789),
        client_version: Some("pn-u32-contract".to_owned()),
    };

    let encoded = request.to_vec().expect("encode GetVpnInfoReq");
    let decoded = GetVpnInfoReq::clone_from_slice(&encoded).expect("decode GetVpnInfoReq");
    let _: Option<u16> = decoded.info_version;
    let _: Option<u32> = decoded.pn_info_version;
    assert_eq!(decoded.seq.value(), 17);
    assert_eq!(decoded.info_version, Some(u16::MAX));
    assert_eq!(
        decoded.pn_info_version,
        Some(u16::MAX as u32 + 1_723_456_789)
    );
    assert_eq!(decoded.client_version.as_deref(), Some("pn-u32-contract"));
}

#[test]
fn response_raw_codec_preserves_u32_pn_timestamp_and_u16_network_version() {
    let response = GetVpnInfoResp {
        seq: 18u32.into(),
        result: 0,
        info_version: u16::MAX,
        pn_info_version: u16::MAX as u32 + 1_723_456_789,
        vpn_list: Vec::new(),
    };

    let encoded = response.to_vec().expect("encode GetVpnInfoResp");
    let decoded = GetVpnInfoResp::clone_from_slice(&encoded).expect("decode GetVpnInfoResp");
    let _: u16 = decoded.info_version;
    let _: u32 = decoded.pn_info_version;
    assert_eq!(decoded.seq.value(), 18);
    assert_eq!(decoded.result, 0);
    assert_eq!(decoded.info_version, u16::MAX);
    assert_eq!(decoded.pn_info_version, u16::MAX as u32 + 1_723_456_789);
    assert!(decoded.vpn_list.is_empty());
}

fn assert_strict_version_guard(source: &str, handler_marker: &str) {
    let handler = source
        .find(handler_marker)
        .unwrap_or_else(|| panic!("missing handler marker {handler_marker}"));
    let tail = &source[handler..];
    let guard = tail
        .find("if header.version() != VPN_CMD_VERSION")
        .unwrap_or_else(|| panic!("missing strict version guard after {handler_marker}"));
    let reject = guard
        + tail[guard..]
            .find("return Err")
            .unwrap_or_else(|| panic!("version guard does not reject after {handler_marker}"));
    let decode = tail
        .find("clone_from_slice")
        .unwrap_or_else(|| panic!("missing payload decode after {handler_marker}"));
    assert!(
        guard < reject && reject < decode,
        "old version must be rejected before payload decode"
    );
    assert!(tail[guard..decode].contains("unsupported vpn command version"));
}

fn assert_rejecting_version_guard_function(source: &str) {
    let start = source
        .find("fn require_version")
        .expect("missing require_version function");
    let tail = &source[start..];
    let end = tail
        .find("\n}\n")
        .expect("unterminated require_version function");
    let body = &tail[..end];
    assert!(body.contains("if header.version() != VPN_CMD_VERSION"));
    assert!(body.contains("return Err"));
    assert!(body.contains("unsupported vpn command version"));
}

#[test]
fn protocol_version_two_is_required_before_any_u32_payload_decode() {
    assert_eq!(VPN_CMD_VERSION, 2);
    assert_strict_version_guard(VPN_SERVER_SOURCE, "VpnCmdCode::GetVpnInfo as u8");
    assert_rejecting_version_guard_function(PN_CONTROL_SERVER_SOURCE);
    assert!(VPN_PROTOCOL_SOURCE.contains("VPN_CMD_VERSION: u8 = 2"));
    assert!(!VPN_PROTOCOL_SOURCE.contains("VPN_CMD_VERSION: u8 = 1"));
}
