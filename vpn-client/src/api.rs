use crate::p2p_vpn::{JoinRecord, vpn_client_manager};
use crate::setting::Setting;
use serde::{Deserialize, Serialize};
use sfo_http::http_server::{HttpMethod, HttpServer, Request, Response};
use sfo_http::openapi::utoipa::ToSchema;
use sfo_http::openapi::{OpenApiServer, OpenapiNoValueResult, utoipa};
use sfo_http::{add_openapi_item, def_openapi};
use std::sync::Arc;
use vpn_frame::errors::{VpnErrorCode, VpnResult, into_vpn_err};
use vpn_frame::server::NetworkGroupId;

#[derive(Serialize, Deserialize, ToSchema, Eq, PartialEq)]
pub struct Join {
    pub server: String,
    pub server_port: u16,
    pub server_id: String,
    pub group_id: NetworkGroupId,
    pub name: Option<String>,
}
pub struct Api;

impl Api {
    pub fn register_api<Req: Request, Resp: Response, S: HttpServer<Req, Resp> + OpenApiServer>(
        server: &mut S,
        setting: Arc<Setting>,
    ) {
        def_openapi! {
            [login]
            #[utoipa::path(
                post,
                path = "/account/login",
                summary = "Login",
                responses (
                    (status = 200,
                        body = inline(Join))
                ),
                request_body = inline(OpenapiNoValueResult),
                tag = "account"
            )]
        }
        add_openapi_item!(server, login);
        let set = setting.clone();
        server.serve("/join", HttpMethod::POST, move |mut req: Req| {
            let setting = set.clone();
            async move {
                let result: VpnResult<()> = async move {
                    let join = req
                        .body_json::<Join>()
                        .await
                        .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let vpn_client = vpn_client_manager()
                        .get_client(
                            format!(
                                "{}_{}:{}",
                                join.server_id.as_str(),
                                join.server.as_str(),
                                join.server_port
                            )
                            .as_str(),
                        )
                        .await?;
                    vpn_client.join(join.group_id, join.name.clone()).await?;
                    let mut joined_networks: Vec<JoinRecord> =
                        setting.get("joined_networks").unwrap_or(vec![]);
                    let record = JoinRecord {
                        server_ip: join.server,
                        server_port: join.server_port,
                        server_id: join.server_id,
                        network_id: join.group_id,
                    };
                    if !joined_networks.contains(&record) {
                        joined_networks.push(record);
                        setting.set("joined_networks", joined_networks)?;
                        setting.save().await?;
                    }
                    vpn_client.run();
                    Ok(())
                }
                .await;
                Ok(Resp::from_result(result))
            }
        });

        server.serve("/state", HttpMethod::GET, |_req: Req| async move {
            let result: VpnResult<()> = async move { Ok(()) }.await;
            Ok(Resp::from_result(result))
        });
    }
}
