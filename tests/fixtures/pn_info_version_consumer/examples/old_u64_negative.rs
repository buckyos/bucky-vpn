use vpn_frame::{GetVpnInfoReq, GetVpnInfoResp};

fn unfinished_request(pn_modified_at_ms: u64) -> GetVpnInfoReq {
    GetVpnInfoReq {
        seq: 1u32.into(),
        info_version: Some(1u16),
        pn_info_version: Some(pn_modified_at_ms),
        client_version: None,
    }
}

fn unfinished_response_consumer(response: &GetVpnInfoResp) -> u64 {
    response.pn_info_version
}

fn main() {
    let _ = unfinished_request(1_723_456_789_012);
    let _ = unfinished_response_consumer;
}
