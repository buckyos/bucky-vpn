use std::net::{IpAddr, Ipv4Addr};
use serde::{Deserialize, Serialize};
use sfo_account::{AccountManager};
use sfo_http::http::header::AUTHORIZATION;
use sfo_http::http_server::{HttpMethod, HttpServer, Request, Response};
use sfo_http::openapi::OpenApiServer;
use sfo_http::openapi::utoipa;
use vpn_frame::errors::{into_vpn_err, vpn_err, VpnErrorCode, VpnResult};
use vpn_frame::server::{NetworkGroupId, NetworkId, NodeId};
use crate::sqlite_store_factory::{NetworkManagerRef};
use crate::user_store::UserManagerRef;
use vpn_frame::serialize_u64_as_string;
use vpn_frame::deserialize_u64_from_string;

pub struct Api;

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
struct JsonJoinedNode {
    #[serde(serialize_with = "serialize_u64_as_string", deserialize_with = "deserialize_u64_from_string")]
    pub group_id: NetworkGroupId,
    pub node_id: String,
    pub allow_join: bool,
    pub name: String,
    pub comment: String,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
struct JsonNetworkMember {
    pub id: String,
    pub ip: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv6: Option<String>,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
struct AllowJoinReq {
    pub node_id: String,
    pub allow_join: bool,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
struct AddNetworkReq {
    pub name: String,
    pub ip_addr: String,
    pub mask: u8,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
struct AddNetworkMemberReq {
    #[serde(serialize_with = "serialize_u64_as_string", deserialize_with = "deserialize_u64_from_string")]
    pub network_id: NetworkId,
    pub node_id: String,
    pub ip_addr: String,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
struct UpdateNetworkMemberReq {
    #[serde(serialize_with = "serialize_u64_as_string", deserialize_with = "deserialize_u64_from_string")]
    pub network_id: NetworkId,
    pub node_id: String,
    pub ip_addr: String,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
struct DeleteNetworkMemberReq {
    #[serde(serialize_with = "serialize_u64_as_string", deserialize_with = "deserialize_u64_from_string")]
    pub network_id: NetworkId,
    pub node_id: String,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
struct GetNetworkMemberReq {
    #[serde(serialize_with = "serialize_u64_as_string", deserialize_with = "deserialize_u64_from_string")]
    pub network_id: NetworkId,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
struct UpdateNetworkReq {
    #[serde(serialize_with = "serialize_u64_as_string", deserialize_with = "deserialize_u64_from_string")]
    pub network_id: NetworkId,
    pub name: String,
    pub ip_addr: String,
    pub mask: u8,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct JsonNetwork {
    #[serde(serialize_with = "serialize_u64_as_string", deserialize_with = "deserialize_u64_from_string")]
    pub id: NetworkId,
    #[serde(serialize_with = "serialize_u64_as_string", deserialize_with = "deserialize_u64_from_string")]
    pub group_id: NetworkGroupId,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_seg: Option<String>,
    pub mask: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv6_seg: Option<String>,
    pub ipv6_mask: u8,
}

impl Api {
    pub fn register_api<
        Req: Request,
        Resp: Response,
        S: HttpServer<Req, Resp> + OpenApiServer,
    >(server: &mut S,
      user_manager: UserManagerRef,
      network_manager: NetworkManagerRef) {
        let tmp_user_manager = user_manager.clone();
        let tmp_network_manager = network_manager.clone();
        server.serve("/get_joined_nodes", HttpMethod::GET, move | req: Req| {
            let user_manager = tmp_user_manager.clone();
            let network_manager = tmp_network_manager.clone();
            async move {
                let result: VpnResult<Vec<JsonJoinedNode>> = async move {
                    let session = req
                        .header(AUTHORIZATION)
                        .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam))?
                        .to_str().map_err(|_| vpn_err!(VpnErrorCode::InvalidParam))?.to_string();
                    if !session.to_lowercase().starts_with("bearer ") {
                        return Err(vpn_err!(VpnErrorCode::InvalidParam));
                    }
                    let session = session.split_at("Bearer ".len()).1;
                    let user = user_manager.decode_session(session).await.map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let nodes = network_manager.get_joint_nodes(&user.account.network_id).await?.iter().map(|v| JsonJoinedNode {
                        group_id: v.group_id,
                        node_id: v.node_id.to_base58(),
                        allow_join: v.allow_join,
                        name: v.name.clone(),
                        comment: v.comment.clone(),
                    }).collect::<Vec<_>>();
                    Ok(nodes)
                }.await;
                Ok(Resp::from_result(result))
            }
        });

        let tmp_user_manager = user_manager.clone();
        let tmp_network_manager = network_manager.clone();
        server.serve("/allow_join", HttpMethod::POST, move | mut req: Req| {
            let user_manager = tmp_user_manager.clone();
            let network_manager = tmp_network_manager.clone();
            async move {
                let result: VpnResult<()> = async move {
                    let session = req
                        .header(AUTHORIZATION)
                        .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam))?
                        .to_str().map_err(|_| vpn_err!(VpnErrorCode::InvalidParam))?.to_string();
                    if !session.to_lowercase().starts_with("bearer ") {
                        return Err(vpn_err!(VpnErrorCode::InvalidParam));
                    }
                    let session = session.split_at("Bearer ".len()).1;
                    let user = user_manager.decode_session(session).await.map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let req = req.body_json::<AllowJoinReq>().await.map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let node_id = NodeId::from_base58(&req.node_id).map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    network_manager.update_allow_join(&user.account.network_id, &node_id, req.allow_join).await
                }.await;
                Ok(Resp::from_result(result))
            }
        });

        let tmp_user_manager = user_manager.clone();
        let tmp_network_manager = network_manager.clone();
        server.serve("/add_network", HttpMethod::POST, move | mut req: Req| {
            let user_manager = tmp_user_manager.clone();
            let network_manager = tmp_network_manager.clone();
            async move {
                let result: VpnResult<()> = async move {
                    let session = req
                        .header(AUTHORIZATION)
                        .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam))?
                        .to_str().map_err(|_| vpn_err!(VpnErrorCode::InvalidParam))?.to_string();
                    if !session.to_lowercase().starts_with("bearer ") {
                        return Err(vpn_err!(VpnErrorCode::InvalidParam));
                    }
                    let session = session.split_at("Bearer ".len()).1;
                    let user = user_manager.decode_session(session).await.map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let req = req.body_json::<AddNetworkReq>().await.map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let mut network = network_manager.new_network(&user.account.network_id).await?;
                    network.name = req.name.clone();
                    network.ip_seg = Some(req.ip_addr.parse().map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?);
                    network.mask = req.mask;

                    network_manager.update_network(&network).await
                }.await;
                Ok(Resp::from_result(result))
            }
        });

        let tmp_user_manager = user_manager.clone();
        let tmp_network_manager = network_manager.clone();
        server.serve("/update_network", HttpMethod::POST, move | mut req: Req| {
            let user_manager = tmp_user_manager.clone();
            let network_manager = tmp_network_manager.clone();
            async move {
                let result: VpnResult<()> = async move {
                    let session = req
                        .header(AUTHORIZATION)
                        .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam))?
                        .to_str().map_err(|_| vpn_err!(VpnErrorCode::InvalidParam))?.to_string();
                    if !session.to_lowercase().starts_with("bearer ") {
                        return Err(vpn_err!(VpnErrorCode::InvalidParam));
                    }
                    let session = session.split_at("Bearer ".len()).1;
                    let user = user_manager.decode_session(session).await.map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let req = req.body_json::<UpdateNetworkReq>().await.map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let network = network_manager.get_network(&req.network_id).await?;
                    if network.is_none() {
                        return Err(vpn_err!(VpnErrorCode::InvalidParam));
                    }
                    let mut network = network.unwrap();
                    if network.group_id != user.account.network_id {
                        return Err(vpn_err!(VpnErrorCode::NoPermission, "No permission"));
                    }

                    network.name = req.name.clone();
                    network.ip_seg = Some(req.ip_addr.parse().map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?);
                    network.mask = req.mask;

                    network_manager.update_network(&network).await
                }.await;
                Ok(Resp::from_result(result))
            }
        });

        let tmp_user_manager = user_manager.clone();
        let tmp_network_manager = network_manager.clone();
        server.serve("/get_networks", HttpMethod::GET, move | req: Req| {
            let user_manager = tmp_user_manager.clone();
            let network_manager = tmp_network_manager.clone();
            async move {
                let result: VpnResult<Vec<JsonNetwork>> = async move {
                    let session = req
                        .header(AUTHORIZATION)
                        .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam))?
                        .to_str().map_err(|_| vpn_err!(VpnErrorCode::InvalidParam))?.to_string();
                    if !session.to_lowercase().starts_with("bearer ") {
                        return Err(vpn_err!(VpnErrorCode::InvalidParam));
                    }
                    let session = session.split_at("Bearer ".len()).1;
                    let user = user_manager.decode_session(session).await.map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;

                    let list = network_manager.get_networks_of_group(&user.account.network_id).await?.iter().map(|v| {
                        JsonNetwork {
                            id: v.id,
                            group_id: v.group_id,
                            name: v.name.clone(),
                            ip_seg: v.ip_seg.as_ref().map(|v| v.to_string()),
                            mask: v.mask,
                            ipv6_seg: v.ipv6_seg.as_ref().map(|v| v.to_string()),
                            ipv6_mask: v.ipv6_mask,
                        }
                    }).collect();
                    Ok(list)
                }.await;
                Ok(Resp::from_result(result))
            }
        });

        let tmp_user_manager = user_manager.clone();
        let tmp_network_manager = network_manager.clone();
        server.serve("/add_network_member", HttpMethod::POST, move | mut req: Req| {
            let user_manager = tmp_user_manager.clone();
            let network_manager = tmp_network_manager.clone();
            async move {
                let result: VpnResult<()> = async move {
                    let session = req
                        .header(AUTHORIZATION)
                        .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam))?
                        .to_str().map_err(|_| vpn_err!(VpnErrorCode::InvalidParam))?.to_string();
                    if !session.to_lowercase().starts_with("bearer ") {
                        return Err(vpn_err!(VpnErrorCode::InvalidParam));
                    }
                    let session = session.split_at("Bearer ".len()).1;
                    let user = user_manager.decode_session(session).await.map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let req = req.body_json::<AddNetworkMemberReq>().await.map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let network = network_manager.get_network(&req.network_id).await?;
                    if network.is_none() {
                        return Err(vpn_err!(VpnErrorCode::InvalidParam));
                    }
                    let network = network.unwrap();
                    if network.group_id != user.account.network_id {
                        return Err(vpn_err!(VpnErrorCode::NoPermission, "No permission"));
                    }
                    //比较添加成员的ip是否满足network中的ip_seg和网络mask是否匹配，ip_seg是网络段，mask是掩码位数，不匹配添加失败
                    let ip: Ipv4Addr = req.ip_addr.parse().map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    if network.ip_seg.is_none() || !network.is_in_seg(&IpAddr::V4(ip)) {
                        return Err(vpn_err!(VpnErrorCode::InvalidIp));
                    }

                    let node_id = NodeId::from_base58(req.node_id.as_str()).map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    network_manager.add_network_member(&network.id, &node_id, req.ip_addr.clone(), None).await
                }.await;
                Ok(Resp::from_result(result))
            }
        });

        let tmp_user_manager = user_manager.clone();
        let tmp_network_manager = network_manager.clone();
        server.serve("/update_network_member", HttpMethod::POST, move | mut req: Req| {
            let user_manager = tmp_user_manager.clone();
            let network_manager = tmp_network_manager.clone();
            async move {
                let result: VpnResult<()> = async move {
                    let session = req
                        .header(AUTHORIZATION)
                        .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam))?
                        .to_str().map_err(|_| vpn_err!(VpnErrorCode::InvalidParam))?.to_string();
                    if !session.to_lowercase().starts_with("bearer ") {
                        return Err(vpn_err!(VpnErrorCode::InvalidParam));
                    }
                    let session = session.split_at("Bearer ".len()).1;
                    let user = user_manager.decode_session(session).await.map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let req = req.body_json::<UpdateNetworkMemberReq>().await.map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let network = network_manager.get_network(&req.network_id).await?;
                    if network.is_none() {
                        return Err(vpn_err!(VpnErrorCode::InvalidParam));
                    }
                    let network = network.unwrap();
                    if network.group_id != user.account.network_id {
                        return Err(vpn_err!(VpnErrorCode::NoPermission, "No permission"));
                    }
                    //比较添加成员的ip是否满足network中的ip_seg和网络mask是否匹配，ip_seg是网络段，mask是掩码位数，不匹配添加失败
                    let ip: Ipv4Addr = req.ip_addr.parse().map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    if network.ip_seg.is_none() || !network.is_in_seg(&IpAddr::V4(ip)) {
                        return Err(vpn_err!(VpnErrorCode::InvalidIp));
                    }

                    let node_id = NodeId::from_base58(req.node_id.as_str()).map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    network_manager.update_network_member(&network.id, node_id, req.ip_addr.clone(), None).await
                }.await;
                Ok(Resp::from_result(result))
            }
        });

        let tmp_user_manager = user_manager.clone();
        let tmp_network_manager = network_manager.clone();
        server.serve("/delete_network_member", HttpMethod::POST, move | mut req: Req| {
            let user_manager = tmp_user_manager.clone();
            let network_manager = tmp_network_manager.clone();
            async move {
                let result: VpnResult<()> = async move {
                    let session = req
                        .header(AUTHORIZATION)
                        .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam))?
                        .to_str().map_err(|_| vpn_err!(VpnErrorCode::InvalidParam))?.to_string();
                    if !session.to_lowercase().starts_with("bearer ") {
                        return Err(vpn_err!(VpnErrorCode::InvalidParam));
                    }
                    let session = session.split_at("Bearer ".len()).1;
                    let user = user_manager.decode_session(session).await.map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let req = req.body_json::<DeleteNetworkMemberReq>().await.map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let network = network_manager.get_network(&req.network_id).await?;
                    if network.is_none() {
                        return Err(vpn_err!(VpnErrorCode::InvalidParam));
                    }
                    let network = network.unwrap();
                    if network.group_id != user.account.network_id {
                        return Err(vpn_err!(VpnErrorCode::NoPermission, "No permission"));
                    }

                    let node_id = NodeId::from_base58(req.node_id.as_str()).map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    network_manager.del_network_member(&network.id, &node_id).await
                }.await;
                Ok(Resp::from_result(result))
            }
        });

        let tmp_user_manager = user_manager.clone();
        let tmp_network_manager = network_manager.clone();
        server.serve("/get_network_member", HttpMethod::POST, move | mut req: Req| {
            let user_manager = tmp_user_manager.clone();
            let network_manager = tmp_network_manager.clone();
            async move {
                let result: VpnResult<Vec<JsonNetworkMember>> = async move {
                    let session = req
                        .header(AUTHORIZATION)
                        .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam))?
                        .to_str().map_err(|_| vpn_err!(VpnErrorCode::InvalidParam))?.to_string();
                    if !session.to_lowercase().starts_with("bearer ") {
                        return Err(vpn_err!(VpnErrorCode::InvalidParam));
                    }
                    let session = session.split_at("Bearer ".len()).1;
                    let user = user_manager.decode_session(session).await.map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let req = req.body_json::<GetNetworkMemberReq>().await.map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let network = network_manager.get_network(&req.network_id).await?;
                    if network.is_none() {
                        return Err(vpn_err!(VpnErrorCode::InvalidParam));
                    }
                    let network = network.unwrap();
                    if network.group_id != user.account.network_id {
                        return Err(vpn_err!(VpnErrorCode::NoPermission, "No permission"));
                    }

                    let list = network_manager.get_network_member(&req.network_id).await?.iter().map(|v| {
                        JsonNetworkMember {
                            id: v.id.to_base58(),
                            ip: v.ip.clone(),
                            ipv6: v.ipv6.clone(),
                        }
                    }).collect();
                    Ok(list)
                }.await;
                Ok(Resp::from_result(result))
            }
        });
    }
}
