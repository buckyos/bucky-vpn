use vpn_frame::{GetVpnInfoReq, GetVpnInfoResp};

pub fn new_request(network_version: u16, pn_modified_at_seconds: u32) -> GetVpnInfoReq {
    GetVpnInfoReq {
        seq: 1u32.into(),
        info_version: Some(network_version),
        pn_info_version: Some(pn_modified_at_seconds),
        client_version: None,
    }
}

pub fn read_versions(response: &GetVpnInfoResp) -> (u16, u32) {
    (response.info_version, response.pn_info_version)
}
