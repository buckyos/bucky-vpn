use crate::pn_traffic_service::PnTrafficServiceRef;
use crate::server_config::ConfigPnServerSelectorRef;
use crate::sqlite_store_factory::VpnServerRef;
use crate::user_store::UserManagerRef;
use p2p_frame::p2p_identity::P2pId;
use p2p_frame::pn::PnUserTrafficSnapshot;
use serde::{Deserialize, Serialize};
use sfo_account::AccountManager;
use sfo_http::http::header::AUTHORIZATION;
use sfo_http::http_server::{HttpMethod, HttpServer, Request, Response};
use sfo_http::openapi::OpenApiServer;
use sfo_http::openapi::utoipa;
use std::net::{IpAddr, Ipv4Addr};
use vpn_frame::cmd_server::PeerId;
use vpn_frame::deserialize_u64_from_string;
use vpn_frame::errors::{VpnErrorCode, VpnResult, into_vpn_err, vpn_err};
use vpn_frame::serialize_u64_as_string;
use vpn_frame::server::{NetworkGroupId, NetworkId, NodeId};
use vpn_frame::{PnServerAddress, PnServerInfo};

pub struct Api;

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
struct JsonJoinedNode {
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string"
    )]
    pub group_id: NetworkGroupId,
    pub node_id: String,
    pub allow_join: bool,
    pub name: String,
    pub comment: String,
    pub online: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_list: Option<Vec<String>>,
    pub tx_bytes: String,
    pub tx_speed: String,
    pub rx_bytes: String,
    pub rx_speed: String,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
struct JsonNetworkMember {
    pub id: String,
    pub ip: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv6: Option<String>,
    pub online: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_list: Option<Vec<String>>,
    pub tx_bytes: String,
    pub tx_speed: String,
    pub rx_bytes: String,
    pub rx_speed: String,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
struct JsonUserTrafficStats {
    pub tx_bytes: String,
    pub tx_speed: String,
    pub rx_bytes: String,
    pub rx_speed: String,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
struct AllowJoinReq {
    pub node_id: String,
    pub allow_join: bool,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
struct UpdateJoinCommentReq {
    pub node_id: String,
    pub comment: String,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
struct DeleteJoinedNodeReq {
    pub node_id: String,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct JsonPnServerAddress {
    #[serde(default = "default_pn_server_address_protocol")]
    pub protocol: String,
    pub ip: String,
    pub port: u16,
}

fn default_pn_server_address_protocol() -> String {
    PnServerAddress::PROTOCOL_QUIC.to_string()
}

impl TryFrom<JsonPnServerAddress> for PnServerAddress {
    type Error = vpn_frame::errors::VpnError;

    fn try_from(value: JsonPnServerAddress) -> VpnResult<Self> {
        Ok(Self::new_with_protocol(
            value.protocol,
            value
                .ip
                .parse()
                .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?,
            value.port,
        ))
    }
}

impl From<PnServerAddress> for JsonPnServerAddress {
    fn from(value: PnServerAddress) -> Self {
        Self {
            protocol: value.protocol,
            ip: value.ip.to_string(),
            port: value.port,
        }
    }
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct JsonPnServerInfo {
    pub id: String,
    pub ip: String,
    pub port: u16,
    #[serde(default)]
    pub addresses: Vec<JsonPnServerAddress>,
}

impl JsonPnServerInfo {
    fn into_pn_server_info(self) -> VpnResult<PnServerInfo> {
        let ip = self
            .ip
            .parse()
            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
        let addresses = self
            .addresses
            .into_iter()
            .map(TryInto::try_into)
            .collect::<VpnResult<Vec<_>>>()?;
        Ok(PnServerInfo::new_with_addresses(
            self.id, ip, self.port, addresses,
        ))
    }
}

impl From<PnServerInfo> for JsonPnServerInfo {
    fn from(value: PnServerInfo) -> Self {
        Self {
            id: value.id,
            ip: value.ip.to_string(),
            port: value.port,
            addresses: value.addresses.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
struct ProxyNodeApprovalReq {
    pub pn_server: JsonPnServerInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
struct JsonProxyNode {
    pub pn_server: JsonPnServerInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_addr: Option<String>,
    pub status: String,
    pub live: bool,
    pub updated_at: String,
    pub comment: String,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
struct AddNetworkReq {
    pub name: String,
    pub ip_addr: String,
    pub mask: u8,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
struct AddNetworkMemberReq {
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string"
    )]
    pub network_id: NetworkId,
    pub node_id: String,
    pub ip_addr: String,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
struct UpdateNetworkMemberReq {
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string"
    )]
    pub network_id: NetworkId,
    pub node_id: String,
    pub ip_addr: String,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
struct DeleteNetworkMemberReq {
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string"
    )]
    pub network_id: NetworkId,
    pub node_id: String,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
struct GetNetworkMemberReq {
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string"
    )]
    pub network_id: NetworkId,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
struct DeleteNetworkReq {
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string"
    )]
    pub network_id: NetworkId,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
struct UpdateNetworkReq {
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string"
    )]
    pub network_id: NetworkId,
    pub name: String,
    pub ip_addr: String,
    pub mask: u8,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct JsonNetwork {
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string"
    )]
    pub id: NetworkId,
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string"
    )]
    pub group_id: NetworkGroupId,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_seg: Option<String>,
    pub mask: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv6_seg: Option<String>,
    pub ipv6_mask: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pn_server: Option<JsonPnServerInfo>,
}

impl JsonUserTrafficStats {
    fn from_snapshot(snapshot: PnUserTrafficSnapshot) -> Self {
        Self {
            tx_bytes: snapshot.tx_bytes.to_string(),
            tx_speed: snapshot.tx_speed.to_string(),
            rx_bytes: snapshot.rx_bytes.to_string(),
            rx_speed: snapshot.rx_speed.to_string(),
        }
    }
}

#[cfg(test)]
fn accumulate_traffic_stats(snapshot: &mut PnUserTrafficSnapshot, item: &PnUserTrafficSnapshot) {
    snapshot.tx_bytes = snapshot.tx_bytes.saturating_add(item.tx_bytes);
    snapshot.tx_speed = snapshot.tx_speed.saturating_add(item.tx_speed);
    snapshot.rx_bytes = snapshot.rx_bytes.saturating_add(item.rx_bytes);
    snapshot.rx_speed = snapshot.rx_speed.saturating_add(item.rx_speed);
}

async fn observed_proxy_addr(
    vpn_server: &VpnServerRef,
    pn_server: &PnServerInfo,
) -> Option<String> {
    let peer_id = peer_id_from_pn_server_id(&pn_server.id)?;
    let ips = vpn_server.get_peer_ip_list(&peer_id).await.ok()?;
    ips.first().map(ToString::to_string)
}

fn peer_id_from_pn_server_id(id: &str) -> Option<PeerId> {
    if let Ok(node_id) = NodeId::from_base36_or_base58(id) {
        return Some(PeerId::from(node_id.as_slice()));
    }
    id.parse::<P2pId>()
        .ok()
        .map(|p2p_id| PeerId::from(p2p_id.as_slice()))
}

impl Api {
    pub fn register_api<Req: Request, Resp: Response, S: HttpServer<Req, Resp> + OpenApiServer>(
        server: &mut S,
        user_manager: UserManagerRef,
        vpn_server: VpnServerRef,
        traffic_service: PnTrafficServiceRef,
        pn_server_selector: ConfigPnServerSelectorRef,
    ) {
        let tmp_user_manager = user_manager.clone();
        let tmp_pn_server_selector = pn_server_selector.clone();
        let tmp_vpn_server = vpn_server.clone();
        server.serve("/pn_proxy_nodes", HttpMethod::GET, move |req: Req| {
            let user_manager = tmp_user_manager.clone();
            let pn_server_selector = tmp_pn_server_selector.clone();
            let vpn_server = tmp_vpn_server.clone();
            async move {
                let result: VpnResult<Vec<JsonProxyNode>> = async move {
                    let session = req
                        .header(AUTHORIZATION)
                        .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam))?
                        .to_str()
                        .map_err(|_| vpn_err!(VpnErrorCode::InvalidParam))?
                        .to_string();
                    if !session.to_lowercase().starts_with("bearer ") {
                        return Err(vpn_err!(VpnErrorCode::InvalidParam));
                    }
                    let session = session.split_at("Bearer ".len()).1;
                    let _user = user_manager
                        .decode_session(session)
                        .await
                        .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let nodes = pn_server_selector.list_proxy_nodes().await?;
                    let mut json_nodes = Vec::with_capacity(nodes.len());
                    for node in nodes {
                        let observed_addr = observed_proxy_addr(&vpn_server, &node.pn_server).await;
                        json_nodes.push(JsonProxyNode {
                            pn_server: node.pn_server.into(),
                            observed_addr,
                            status: node.status.as_str().to_string(),
                            live: node.live,
                            updated_at: node.updated_at.to_string(),
                            comment: node.comment,
                        });
                    }
                    Ok(json_nodes)
                }
                .await;
                Ok(Resp::from_result(result))
            }
        });

        let tmp_user_manager = user_manager.clone();
        let tmp_pn_server_selector = pn_server_selector.clone();
        server.serve(
            "/approve_pn_proxy_node",
            HttpMethod::POST,
            move |mut req: Req| {
                let user_manager = tmp_user_manager.clone();
                let pn_server_selector = tmp_pn_server_selector.clone();
                async move {
                    let result: VpnResult<()> = async move {
                        let session = req
                            .header(AUTHORIZATION)
                            .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam))?
                            .to_str()
                            .map_err(|_| vpn_err!(VpnErrorCode::InvalidParam))?
                            .to_string();
                        if !session.to_lowercase().starts_with("bearer ") {
                            return Err(vpn_err!(VpnErrorCode::InvalidParam));
                        }
                        let session = session.split_at("Bearer ".len()).1;
                        let _user = user_manager
                            .decode_session(session)
                            .await
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                        let req = req
                            .body_json::<ProxyNodeApprovalReq>()
                            .await
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                        let pn_server = req.pn_server.into_pn_server_info()?;
                        pn_server_selector
                            .approve_proxy_node(&pn_server, req.comment.as_deref())
                            .await
                    }
                    .await;
                    Ok(Resp::from_result(result))
                }
            },
        );

        let tmp_user_manager = user_manager.clone();
        let tmp_pn_server_selector = pn_server_selector.clone();
        server.serve(
            "/reject_pn_proxy_node",
            HttpMethod::POST,
            move |mut req: Req| {
                let user_manager = tmp_user_manager.clone();
                let pn_server_selector = tmp_pn_server_selector.clone();
                async move {
                    let result: VpnResult<()> = async move {
                        let session = req
                            .header(AUTHORIZATION)
                            .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam))?
                            .to_str()
                            .map_err(|_| vpn_err!(VpnErrorCode::InvalidParam))?
                            .to_string();
                        if !session.to_lowercase().starts_with("bearer ") {
                            return Err(vpn_err!(VpnErrorCode::InvalidParam));
                        }
                        let session = session.split_at("Bearer ".len()).1;
                        let _user = user_manager
                            .decode_session(session)
                            .await
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                        let req = req
                            .body_json::<ProxyNodeApprovalReq>()
                            .await
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                        let pn_server = req.pn_server.into_pn_server_info()?;
                        pn_server_selector
                            .reject_proxy_node(&pn_server, req.comment.as_deref())
                            .await
                    }
                    .await;
                    Ok(Resp::from_result(result))
                }
            },
        );

        let tmp_user_manager = user_manager.clone();
        let tmp_vpn_server = vpn_server.clone();
        let tmp_traffic_service = traffic_service.clone();
        server.serve("/get_joined_nodes", HttpMethod::GET, move |req: Req| {
            let user_manager = tmp_user_manager.clone();
            let vpn_server = tmp_vpn_server.clone();
            let traffic_service = tmp_traffic_service.clone();
            async move {
                let result: VpnResult<Vec<JsonJoinedNode>> = async move {
                    let session = req
                        .header(AUTHORIZATION)
                        .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam))?
                        .to_str()
                        .map_err(|_| vpn_err!(VpnErrorCode::InvalidParam))?
                        .to_string();
                    if !session.to_lowercase().starts_with("bearer ") {
                        return Err(vpn_err!(VpnErrorCode::InvalidParam));
                    }
                    let session = session.split_at("Bearer ".len()).1;
                    let user = user_manager
                        .decode_session(session)
                        .await
                        .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let nodes = vpn_server
                        .network_manager()
                        .get_joint_nodes(&user.account.network_id)
                        .await?;
                    let mut ret_nodes = Vec::new();
                    for node in nodes.iter() {
                        let traffic = traffic_service.get_node_snapshot(&node.node_id).await?;
                        let online = vpn_server.get_node_online_state(&node.node_id).await;
                        if online.is_some() {
                            let (client_version, ip_list) = online.unwrap();
                            ret_nodes.push(JsonJoinedNode {
                                group_id: node.group_id,
                                node_id: node.node_id.to_base36(),
                                allow_join: node.allow_join,
                                name: node.name.clone(),
                                comment: node.comment.clone(),
                                online: true,
                                client_version,
                                ip_list: Some(ip_list.iter().map(|ip| ip.to_string()).collect()),
                                tx_bytes: traffic.tx_bytes.to_string(),
                                tx_speed: traffic.tx_speed.to_string(),
                                rx_bytes: traffic.rx_bytes.to_string(),
                                rx_speed: traffic.rx_speed.to_string(),
                            });
                        } else {
                            ret_nodes.push(JsonJoinedNode {
                                group_id: node.group_id,
                                node_id: node.node_id.to_base36(),
                                allow_join: node.allow_join,
                                name: node.name.clone(),
                                comment: node.comment.clone(),
                                online: false,
                                client_version: None,
                                ip_list: None,
                                tx_bytes: traffic.tx_bytes.to_string(),
                                tx_speed: traffic.tx_speed.to_string(),
                                rx_bytes: traffic.rx_bytes.to_string(),
                                rx_speed: traffic.rx_speed.to_string(),
                            });
                        }
                    }
                    Ok(ret_nodes)
                }
                .await;
                Ok(Resp::from_result(result))
            }
        });

        let tmp_user_manager = user_manager.clone();
        let tmp_traffic_service = traffic_service.clone();
        server.serve(
            "/get_user_traffic_stats",
            HttpMethod::GET,
            move |req: Req| {
                let user_manager = tmp_user_manager.clone();
                let traffic_service = tmp_traffic_service.clone();
                async move {
                    let result: VpnResult<JsonUserTrafficStats> = async move {
                        let session = req
                            .header(AUTHORIZATION)
                            .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam))?
                            .to_str()
                            .map_err(|_| vpn_err!(VpnErrorCode::InvalidParam))?
                            .to_string();
                        if !session.to_lowercase().starts_with("bearer ") {
                            return Err(vpn_err!(VpnErrorCode::InvalidParam));
                        }
                        let session = session.split_at("Bearer ".len()).1;
                        let user = user_manager
                            .decode_session(session)
                            .await
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                        let snapshot = traffic_service
                            .get_group_snapshot(&user.account.network_id)
                            .await?;
                        Ok(JsonUserTrafficStats::from_snapshot(snapshot))
                    }
                    .await;
                    Ok(Resp::from_result(result))
                }
            },
        );

        let tmp_user_manager = user_manager.clone();
        let tmp_vpn_server = vpn_server.clone();
        let tmp_traffic_service = traffic_service.clone();
        server.serve("/allow_join", HttpMethod::POST, move |mut req: Req| {
            let user_manager = tmp_user_manager.clone();
            let vpn_server = tmp_vpn_server.clone();
            let traffic_service = tmp_traffic_service.clone();
            async move {
                let result: VpnResult<()> = async move {
                    let session = req
                        .header(AUTHORIZATION)
                        .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam))?
                        .to_str()
                        .map_err(|_| vpn_err!(VpnErrorCode::InvalidParam))?
                        .to_string();
                    if !session.to_lowercase().starts_with("bearer ") {
                        return Err(vpn_err!(VpnErrorCode::InvalidParam));
                    }
                    let session = session.split_at("Bearer ".len()).1;
                    let user = user_manager
                        .decode_session(session)
                        .await
                        .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let req = req
                        .body_json::<AllowJoinReq>()
                        .await
                        .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let node_id = NodeId::from_base36_or_base58(&req.node_id)
                        .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let joined_nodes = vpn_server
                        .network_manager()
                        .get_joint_nodes(&user.account.network_id)
                        .await?;
                    if !joined_nodes
                        .iter()
                        .any(|node| node.node_id.as_slice() == node_id.as_slice())
                    {
                        return Err(vpn_err!(VpnErrorCode::NoPermission, "No permission"));
                    }
                    if !req.allow_join {
                        traffic_service.flush_node(&node_id).await?;
                    }
                    vpn_server
                        .network_manager()
                        .update_allow_join(&user.account.network_id, &node_id, req.allow_join)
                        .await
                }
                .await;
                Ok(Resp::from_result(result))
            }
        });

        let tmp_user_manager = user_manager.clone();
        let tmp_vpn_server = vpn_server.clone();
        server.serve(
            "/update_joined_comment",
            HttpMethod::POST,
            move |mut req: Req| {
                let user_manager = tmp_user_manager.clone();
                let vpn_server = tmp_vpn_server.clone();
                async move {
                    let result: VpnResult<()> = async move {
                        let session = req
                            .header(AUTHORIZATION)
                            .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam))?
                            .to_str()
                            .map_err(|_| vpn_err!(VpnErrorCode::InvalidParam))?
                            .to_string();
                        if !session.to_lowercase().starts_with("bearer ") {
                            return Err(vpn_err!(VpnErrorCode::InvalidParam));
                        }
                        let session = session.split_at("Bearer ".len()).1;
                        let user = user_manager
                            .decode_session(session)
                            .await
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                        let req = req
                            .body_json::<UpdateJoinCommentReq>()
                            .await
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                        let node_id = NodeId::from_base36_or_base58(&req.node_id)
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                        vpn_server
                            .network_manager()
                            .update_joined_node_comment(
                                &user.account.network_id,
                                &node_id,
                                req.comment,
                            )
                            .await
                    }
                    .await;
                    Ok(Resp::from_result(result))
                }
            },
        );

        let tmp_user_manager = user_manager.clone();
        let tmp_vpn_server = vpn_server.clone();
        let tmp_traffic_service = traffic_service.clone();
        server.serve(
            "/delete_joined_node",
            HttpMethod::POST,
            move |mut req: Req| {
                let user_manager = tmp_user_manager.clone();
                let vpn_server = tmp_vpn_server.clone();
                let traffic_service = tmp_traffic_service.clone();
                async move {
                    let result: VpnResult<()> = async move {
                        let session = req
                            .header(AUTHORIZATION)
                            .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam))?
                            .to_str()
                            .map_err(|_| vpn_err!(VpnErrorCode::InvalidParam))?
                            .to_string();
                        if !session.to_lowercase().starts_with("bearer ") {
                            return Err(vpn_err!(VpnErrorCode::InvalidParam));
                        }
                        let session = session.split_at("Bearer ".len()).1;
                        let user = user_manager
                            .decode_session(session)
                            .await
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                        let req = req
                            .body_json::<DeleteJoinedNodeReq>()
                            .await
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                        let node_id = NodeId::from_base36_or_base58(&req.node_id)
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                        let joined_nodes = vpn_server
                            .network_manager()
                            .get_joint_nodes(&user.account.network_id)
                            .await?;
                        if !joined_nodes
                            .iter()
                            .any(|node| node.node_id.as_slice() == node_id.as_slice())
                        {
                            return Err(vpn_err!(VpnErrorCode::NoPermission, "No permission"));
                        }
                        traffic_service.flush_node(&node_id).await?;
                        vpn_server
                            .network_manager()
                            .del_joined_node(&user.account.network_id, &node_id)
                            .await
                    }
                    .await;
                    Ok(Resp::from_result(result))
                }
            },
        );

        let tmp_user_manager = user_manager.clone();
        let tmp_vpn_server = vpn_server.clone();
        server.serve("/add_network", HttpMethod::POST, move |mut req: Req| {
            let user_manager = tmp_user_manager.clone();
            let vpn_server = tmp_vpn_server.clone();
            async move {
                let result: VpnResult<()> = async move {
                    let session = req
                        .header(AUTHORIZATION)
                        .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam))?
                        .to_str()
                        .map_err(|_| vpn_err!(VpnErrorCode::InvalidParam))?
                        .to_string();
                    if !session.to_lowercase().starts_with("bearer ") {
                        return Err(vpn_err!(VpnErrorCode::InvalidParam));
                    }
                    let session = session.split_at("Bearer ".len()).1;
                    let user = user_manager
                        .decode_session(session)
                        .await
                        .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let req = req
                        .body_json::<AddNetworkReq>()
                        .await
                        .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let mut network = vpn_server
                        .network_manager()
                        .new_network(&user.account.network_id)
                        .await?;
                    network.name = req.name.clone();
                    network.ip_seg = Some(
                        req.ip_addr
                            .parse()
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?,
                    );
                    network.mask = req.mask;
                    network.pn_server = vpn_server.select_pn_server(network.id).await?;

                    vpn_server.network_manager().update_network(&network).await
                }
                .await;
                Ok(Resp::from_result(result))
            }
        });

        let tmp_user_manager = user_manager.clone();
        let tmp_vpn_server = vpn_server.clone();
        server.serve("/update_network", HttpMethod::POST, move |mut req: Req| {
            let user_manager = tmp_user_manager.clone();
            let vpn_server = tmp_vpn_server.clone();
            async move {
                let result: VpnResult<()> = async move {
                    let session = req
                        .header(AUTHORIZATION)
                        .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam))?
                        .to_str()
                        .map_err(|_| vpn_err!(VpnErrorCode::InvalidParam))?
                        .to_string();
                    if !session.to_lowercase().starts_with("bearer ") {
                        return Err(vpn_err!(VpnErrorCode::InvalidParam));
                    }
                    let session = session.split_at("Bearer ".len()).1;
                    let user = user_manager
                        .decode_session(session)
                        .await
                        .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let req = req
                        .body_json::<UpdateNetworkReq>()
                        .await
                        .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let network = vpn_server
                        .network_manager()
                        .get_network(&req.network_id)
                        .await?;
                    if network.is_none() {
                        return Err(vpn_err!(VpnErrorCode::InvalidParam));
                    }
                    let mut network = network.unwrap();
                    if network.group_id != user.account.network_id {
                        return Err(vpn_err!(VpnErrorCode::NoPermission, "No permission"));
                    }

                    network.name = req.name.clone();
                    network.ip_seg = Some(
                        req.ip_addr
                            .parse()
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?,
                    );
                    network.mask = req.mask;

                    vpn_server.network_manager().update_network(&network).await
                }
                .await;
                Ok(Resp::from_result(result))
            }
        });

        let tmp_user_manager = user_manager.clone();
        let tmp_vpn_server = vpn_server.clone();
        server.serve("/delete_network", HttpMethod::POST, move |mut req: Req| {
            let user_manager = tmp_user_manager.clone();
            let vpn_server = tmp_vpn_server.clone();
            async move {
                let result: VpnResult<()> = async move {
                    let session = req
                        .header(AUTHORIZATION)
                        .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam))?
                        .to_str()
                        .map_err(|_| vpn_err!(VpnErrorCode::InvalidParam))?
                        .to_string();
                    if !session.to_lowercase().starts_with("bearer ") {
                        return Err(vpn_err!(VpnErrorCode::InvalidParam));
                    }
                    let session = session.split_at("Bearer ".len()).1;
                    let user = user_manager
                        .decode_session(session)
                        .await
                        .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let req = req
                        .body_json::<DeleteNetworkReq>()
                        .await
                        .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                    let network = vpn_server
                        .network_manager()
                        .get_network(&req.network_id)
                        .await?;
                    if network.is_none() {
                        return Err(vpn_err!(VpnErrorCode::InvalidParam));
                    }
                    let network = network.unwrap();
                    if network.group_id != user.account.network_id {
                        return Err(vpn_err!(VpnErrorCode::NoPermission, "No permission"));
                    }

                    vpn_server
                        .network_manager()
                        .del_network(&req.network_id)
                        .await
                }
                .await;
                Ok(Resp::from_result(result))
            }
        });

        let tmp_user_manager = user_manager.clone();
        let tmp_vpn_server = vpn_server.clone();
        server.serve("/get_networks", HttpMethod::GET, move |req: Req| {
            let user_manager = tmp_user_manager.clone();
            let vpn_server = tmp_vpn_server.clone();
            async move {
                let result: VpnResult<Vec<JsonNetwork>> = async move {
                    let session = req
                        .header(AUTHORIZATION)
                        .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam))?
                        .to_str()
                        .map_err(|_| vpn_err!(VpnErrorCode::InvalidParam))?
                        .to_string();
                    if !session.to_lowercase().starts_with("bearer ") {
                        return Err(vpn_err!(VpnErrorCode::InvalidParam));
                    }
                    let session = session.split_at("Bearer ".len()).1;
                    let user = user_manager
                        .decode_session(session)
                        .await
                        .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;

                    let list = vpn_server
                        .network_manager()
                        .get_networks_of_group(&user.account.network_id)
                        .await?
                        .iter()
                        .map(|v| JsonNetwork {
                            id: v.id,
                            group_id: v.group_id,
                            name: v.name.clone(),
                            ip_seg: v.ip_seg.as_ref().map(|v| v.to_string()),
                            mask: v.mask,
                            ipv6_seg: v.ipv6_seg.as_ref().map(|v| v.to_string()),
                            ipv6_mask: v.ipv6_mask,
                            pn_server: v.pn_server.clone().map(Into::into),
                        })
                        .collect();
                    Ok(list)
                }
                .await;
                Ok(Resp::from_result(result))
            }
        });

        let tmp_user_manager = user_manager.clone();
        let tmp_vpn_server = vpn_server.clone();
        server.serve(
            "/add_network_member",
            HttpMethod::POST,
            move |mut req: Req| {
                let user_manager = tmp_user_manager.clone();
                let vpn_server = tmp_vpn_server.clone();
                async move {
                    let result: VpnResult<()> = async move {
                        let session = req
                            .header(AUTHORIZATION)
                            .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam))?
                            .to_str()
                            .map_err(|_| vpn_err!(VpnErrorCode::InvalidParam))?
                            .to_string();
                        if !session.to_lowercase().starts_with("bearer ") {
                            return Err(vpn_err!(VpnErrorCode::InvalidParam));
                        }
                        let session = session.split_at("Bearer ".len()).1;
                        let user = user_manager
                            .decode_session(session)
                            .await
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                        let req = req
                            .body_json::<AddNetworkMemberReq>()
                            .await
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                        let network = vpn_server
                            .network_manager()
                            .get_network(&req.network_id)
                            .await?;
                        if network.is_none() {
                            return Err(vpn_err!(VpnErrorCode::InvalidParam));
                        }
                        let network = network.unwrap();
                        if network.group_id != user.account.network_id {
                            return Err(vpn_err!(VpnErrorCode::NoPermission, "No permission"));
                        }
                        //比较添加成员的ip是否满足network中的ip_seg和网络mask是否匹配，ip_seg是网络段，mask是掩码位数，不匹配添加失败
                        let ip: Ipv4Addr = req
                            .ip_addr
                            .parse()
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                        if network.ip_seg.is_none() || !network.is_in_seg(&IpAddr::V4(ip)) {
                            return Err(vpn_err!(VpnErrorCode::InvalidIp));
                        }

                        let node_id = NodeId::from_base36_or_base58(req.node_id.as_str())
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                        vpn_server
                            .network_manager()
                            .add_network_member(&network.id, &node_id, req.ip_addr.clone(), None)
                            .await
                    }
                    .await;
                    Ok(Resp::from_result(result))
                }
            },
        );

        let tmp_user_manager = user_manager.clone();
        let tmp_vpn_server = vpn_server.clone();
        server.serve(
            "/update_network_member",
            HttpMethod::POST,
            move |mut req: Req| {
                let user_manager = tmp_user_manager.clone();
                let vpn_server = tmp_vpn_server.clone();
                async move {
                    let result: VpnResult<()> = async move {
                        let session = req
                            .header(AUTHORIZATION)
                            .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam))?
                            .to_str()
                            .map_err(|_| vpn_err!(VpnErrorCode::InvalidParam))?
                            .to_string();
                        if !session.to_lowercase().starts_with("bearer ") {
                            return Err(vpn_err!(VpnErrorCode::InvalidParam));
                        }
                        let session = session.split_at("Bearer ".len()).1;
                        let user = user_manager
                            .decode_session(session)
                            .await
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                        let req = req
                            .body_json::<UpdateNetworkMemberReq>()
                            .await
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                        let network = vpn_server
                            .network_manager()
                            .get_network(&req.network_id)
                            .await?;
                        if network.is_none() {
                            return Err(vpn_err!(VpnErrorCode::InvalidParam));
                        }
                        let network = network.unwrap();
                        if network.group_id != user.account.network_id {
                            return Err(vpn_err!(VpnErrorCode::NoPermission, "No permission"));
                        }
                        //比较添加成员的ip是否满足network中的ip_seg和网络mask是否匹配，ip_seg是网络段，mask是掩码位数，不匹配添加失败
                        let ip: Ipv4Addr = req
                            .ip_addr
                            .parse()
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                        if network.ip_seg.is_none() || !network.is_in_seg(&IpAddr::V4(ip)) {
                            return Err(vpn_err!(VpnErrorCode::InvalidIp));
                        }

                        let node_id = NodeId::from_base36_or_base58(req.node_id.as_str())
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                        vpn_server
                            .network_manager()
                            .update_network_member(&network.id, node_id, req.ip_addr.clone(), None)
                            .await
                    }
                    .await;
                    Ok(Resp::from_result(result))
                }
            },
        );

        let tmp_user_manager = user_manager.clone();
        let tmp_vpn_server = vpn_server.clone();
        server.serve(
            "/delete_network_member",
            HttpMethod::POST,
            move |mut req: Req| {
                let user_manager = tmp_user_manager.clone();
                let vpn_server = tmp_vpn_server.clone();
                async move {
                    let result: VpnResult<()> = async move {
                        let session = req
                            .header(AUTHORIZATION)
                            .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam))?
                            .to_str()
                            .map_err(|_| vpn_err!(VpnErrorCode::InvalidParam))?
                            .to_string();
                        if !session.to_lowercase().starts_with("bearer ") {
                            return Err(vpn_err!(VpnErrorCode::InvalidParam));
                        }
                        let session = session.split_at("Bearer ".len()).1;
                        let user = user_manager
                            .decode_session(session)
                            .await
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                        let req = req
                            .body_json::<DeleteNetworkMemberReq>()
                            .await
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                        let network = vpn_server
                            .network_manager()
                            .get_network(&req.network_id)
                            .await?;
                        if network.is_none() {
                            return Err(vpn_err!(VpnErrorCode::InvalidParam));
                        }
                        let network = network.unwrap();
                        if network.group_id != user.account.network_id {
                            return Err(vpn_err!(VpnErrorCode::NoPermission, "No permission"));
                        }

                        let node_id = NodeId::from_base36_or_base58(req.node_id.as_str())
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                        vpn_server
                            .network_manager()
                            .del_network_member(&network.id, &node_id)
                            .await
                    }
                    .await;
                    Ok(Resp::from_result(result))
                }
            },
        );

        let tmp_user_manager = user_manager.clone();
        let tmp_vpn_server = vpn_server.clone();
        let tmp_traffic_service = traffic_service.clone();
        server.serve(
            "/get_network_member",
            HttpMethod::POST,
            move |mut req: Req| {
                let user_manager = tmp_user_manager.clone();
                let vpn_server = tmp_vpn_server.clone();
                let traffic_service = tmp_traffic_service.clone();
                async move {
                    let result: VpnResult<Vec<JsonNetworkMember>> = async move {
                        let session = req
                            .header(AUTHORIZATION)
                            .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam))?
                            .to_str()
                            .map_err(|_| vpn_err!(VpnErrorCode::InvalidParam))?
                            .to_string();
                        if !session.to_lowercase().starts_with("bearer ") {
                            return Err(vpn_err!(VpnErrorCode::InvalidParam));
                        }
                        let session = session.split_at("Bearer ".len()).1;
                        let user = user_manager
                            .decode_session(session)
                            .await
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                        let req = req
                            .body_json::<GetNetworkMemberReq>()
                            .await
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
                        let network = vpn_server
                            .network_manager()
                            .get_network(&req.network_id)
                            .await?;
                        if network.is_none() {
                            return Err(vpn_err!(VpnErrorCode::InvalidParam));
                        }
                        let network = network.unwrap();
                        if network.group_id != user.account.network_id {
                            return Err(vpn_err!(VpnErrorCode::NoPermission, "No permission"));
                        }

                        let members = vpn_server
                            .network_manager()
                            .get_network_member(&req.network_id)
                            .await?;
                        let mut list = Vec::new();
                        for member in members.iter() {
                            let traffic = traffic_service.get_node_snapshot(&member.id).await?;
                            let online = vpn_server.get_node_online_state(&member.id).await;
                            if online.is_some() {
                                let (client_version, ip_list) = online.unwrap();
                                list.push(JsonNetworkMember {
                                    id: member.id.to_base36(),
                                    ip: member.ip.clone(),
                                    ipv6: member.ipv6.clone(),
                                    online: true,
                                    client_version,
                                    ip_list: Some(
                                        ip_list.iter().map(|ip| ip.to_string()).collect(),
                                    ),
                                    tx_bytes: traffic.tx_bytes.to_string(),
                                    tx_speed: traffic.tx_speed.to_string(),
                                    rx_bytes: traffic.rx_bytes.to_string(),
                                    rx_speed: traffic.rx_speed.to_string(),
                                });
                            } else {
                                list.push(JsonNetworkMember {
                                    id: member.id.to_base36(),
                                    ip: member.ip.clone(),
                                    ipv6: member.ipv6.clone(),
                                    online: false,
                                    client_version: None,
                                    ip_list: None,
                                    tx_bytes: traffic.tx_bytes.to_string(),
                                    tx_speed: traffic.tx_speed.to_string(),
                                    rx_bytes: traffic.rx_bytes.to_string(),
                                    rx_speed: traffic.rx_speed.to_string(),
                                })
                            }
                        }
                        Ok(list)
                    }
                    .await;
                    Ok(Resp::from_result(result))
                }
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulate_traffic_stats_sums_all_fields() {
        let mut total = PnUserTrafficSnapshot {
            tx_bytes: 10,
            tx_speed: 20,
            rx_bytes: 30,
            rx_speed: 40,
        };
        let item = PnUserTrafficSnapshot {
            tx_bytes: 1,
            tx_speed: 2,
            rx_bytes: 3,
            rx_speed: 4,
        };

        accumulate_traffic_stats(&mut total, &item);

        assert_eq!(
            total,
            PnUserTrafficSnapshot {
                tx_bytes: 11,
                tx_speed: 22,
                rx_bytes: 33,
                rx_speed: 44,
            }
        );
    }

    #[test]
    fn json_user_traffic_stats_uses_decimal_strings() {
        let stats = JsonUserTrafficStats::from_snapshot(PnUserTrafficSnapshot {
            tx_bytes: 1024,
            tx_speed: 64,
            rx_bytes: 2048,
            rx_speed: 32,
        });

        assert_eq!(stats.tx_bytes, "1024");
        assert_eq!(stats.tx_speed, "64");
        assert_eq!(stats.rx_bytes, "2048");
        assert_eq!(stats.rx_speed, "32");
    }
}
