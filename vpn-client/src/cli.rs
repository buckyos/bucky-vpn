#![allow(unused)]

use sfo_http::http_server::HttpServerResult;
use sfo_http::http_util::{HttpClient};
use vpn_frame::errors::{into_vpn_err, vpn_err, VpnErrorCode, VpnResult};
use vpn_frame::server::NetworkGroupId;
use crate::api::Join;

pub struct Cli;

impl Cli {
    pub async fn join(server: String, server_port: u16, server_id: String, group_id: NetworkGroupId, name: Option<String>) -> VpnResult<()> {
        let http_client = HttpClient::new(5, Some("http://127.0.0.1:4536"));
        let result: HttpServerResult<()> = http_client.post_json("/join", &Join {
            server,
            server_port,
            server_id,
            group_id,
            name
        }).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        if result.err != 0 {
            Err(vpn_err!(VpnErrorCode::Failed, "err: {}, msg: {}", result.err, result.msg))
        } else {
            Ok(())
        }
    }

    pub async fn get_state(_server: String) -> VpnResult<()> {
        let http_client = HttpClient::new(5, Some("http://127.0.0.1:4536"));
        let result: HttpServerResult<()> = http_client.get_json("/state").await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        if result.err != 0 {
            Err(vpn_err!(VpnErrorCode::Failed, "err: {}, msg: {}", result.err, result.msg))
        } else {
            Ok(())
        }
    }
}
