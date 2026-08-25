use vpn_frame::{GetVpnInfoReq, GetVpnInfoResp};

fn old_request(pn_counter: u16) -> GetVpnInfoReq {
    GetVpnInfoReq {
        seq: 1u32.into(),
        info_version: Some(1u16),
        pn_info_version: Some(pn_counter),
        client_version: None,
    }
}

fn old_response_consumer(response: &GetVpnInfoResp) -> u16 {
    response.pn_info_version
}

fn main() {
    let _ = old_request(1);
    let _ = old_response_consumer;
}
