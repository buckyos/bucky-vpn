use std::collections::HashMap;
use std::net::IpAddr;
use std::ops::Add;
use std::sync::{Arc, Mutex};
use async_trait::async_trait;
use bucky_raw_codec::{RawConvertTo, RawFrom};
use chrono::{DateTime, TimeDelta, Utc};
use mini_moka::sync::Cache;
use sfo_cmd_server::{CmdBodyRead, PeerId};
use sfo_cmd_server::errors::{into_cmd_err, CmdErrorCode};
use sfo_cmd_server::server::CmdServer;
use crate::errors::{vpn_err, VpnErrorCode, VpnResult};
use crate::{GetVpnInfoReq, GetVpnInfoResp, JoinNetworkGroupReq, JoinNetworkGroupResp, NodeNetwork, NodeVpnInfo, QueryNodeReq, QueryNodeResp, VpnTunnelId, VpnCmdCode, VpnCmdHeader};
use crate::server::{Network, NetworkGroupId, NetworkId, NetworkManager, NodeId, NodeManager, VpnStore, VpnStoreFactory};

#[derive(Debug, Clone)]
pub struct OnlineNode {
    pub version: String,
    pub latest: DateTime<Utc>,
}

impl OnlineNode {
    pub fn new(version: String, latest: DateTime<Utc>) -> Self {
        Self {
            version,
            latest,
        }
    }

    pub fn is_expire(&self) -> bool {
        self.latest.add(TimeDelta::seconds(120)).timestamp() < Utc::now().timestamp()
    }
}

#[async_trait]
pub trait VpnCmdServer: CmdServer<u16, u8> {
    async fn get_peer_wan_ip(&self, peer_id: &PeerId) -> VpnResult<Vec<IpAddr>>;
}

pub struct VpnServer<T: VpnCmdServer, S: VpnStore, F: VpnStoreFactory<S>> {
    network_manager: Arc<NetworkManager<S, F>>,
    node_manager: Arc<NodeManager<S, F>>,
    cmd_server: Arc<T>,
    version: u8,
    online_nodes: Mutex<HashMap<NodeId, OnlineNode>>,
}
pub type VpnServerRef<T, S, F> = Arc<VpnServer<T, S, F>>;

impl<T: VpnCmdServer, S: VpnStore, F: VpnStoreFactory<S>> VpnServer<T, S, F> {
    pub fn new(cmd_server: Arc<T>,
               factory: Arc<F>) -> Arc<Self> {
        Arc::new(Self {
            network_manager: NetworkManager::new(factory.clone()),
            node_manager: NodeManager::new(factory),
            cmd_server,
            version: 0,
            online_nodes: Mutex::new(HashMap::new()),
        })
    }

    pub fn network_manager(&self) -> &Arc<NetworkManager<S, F>> {
        &self.network_manager
    }

    pub fn node_manager(&self) -> &Arc<NodeManager<S, F>> {
        &self.node_manager
    }

    pub fn start(self: &Arc<Self>) {
        self.register_cmd_handler();
    }

    fn register_cmd_handler(self: &Arc<Self>) {
        let this = self.clone();
        self.cmd_server.register_cmd_handler(VpnCmdCode::GetVpnInfo as u8, move |peer_id: PeerId, _tunnel_id: VpnTunnelId, _header: VpnCmdHeader, mut body: CmdBodyRead| {
            let this = this.clone();
            async move {
                let data = body.read_all().await?;
                let req = GetVpnInfoReq::clone_from_slice(data.as_slice()).map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?;
                {
                    let mut online_nodes = this.online_nodes.lock().unwrap();
                    if let Some(node) = online_nodes.get_mut(&NodeId::from(peer_id.as_slice())) {
                        node.latest = Utc::now();
                    } else {
                        online_nodes.insert(NodeId::from(peer_id.as_slice()), OnlineNode::new(req.client_version.clone(), Utc::now()));
                    }
                }
                let seq = req.seq;
                let resp = match this.handle_get_vpn_info_req(peer_id.clone(), req.info_version).await {
                    Ok((version, result)) => {
                        GetVpnInfoResp {
                            seq,
                            result: 0,
                            info_version: version,
                            vpn_list: result,
                        }
                    }
                    Err(e) => {
                        log::error!("handle_get_vpn_info_req failed: {:?}", e);
                        GetVpnInfoResp {
                            seq,
                            result: e.code() as u8,
                            info_version: 0,
                            vpn_list: vec![],
                        }
                    }
                };
                this.cmd_server.send(&peer_id,
                                     VpnCmdCode::GetVpnInfoResp as u8,
                                     this.version,
                                     resp.to_vec().map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?.as_slice()).await?;
                Ok(())
            }
        });


        let this = self.clone();
        self.cmd_server.register_cmd_handler(VpnCmdCode::JoinNetworkGroup as u8, move |peer_id: PeerId, _tunnel_id: VpnTunnelId, _header: VpnCmdHeader, mut body: CmdBodyRead| {
            let this = this.clone();
            async move {
                let data = body.read_all().await?;
                let req = JoinNetworkGroupReq::clone_from_slice(data.as_slice()).map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?;
                let seq = req.seq;
                let resp = if let Err(e) = this.handle_join_network_group_req(peer_id.clone(), req).await {
                    log::error!("handle_join_network_group_req failed: {:?}", e);
                    JoinNetworkGroupResp {
                        seq,
                        result: e.code() as u8,
                    }
                } else {
                    JoinNetworkGroupResp {
                        seq,
                        result: 0,
                    }
                };
                this.cmd_server.send(&peer_id,
                                     VpnCmdCode::JoinNetworkGroupResp as u8,
                                     this.version,
                                     resp.to_vec().map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?.as_slice()).await?;
                Ok(())
            }
        });

        let this = self.clone();
        self.cmd_server.register_cmd_handler(VpnCmdCode::QueryNode as u8, move |peer_id: PeerId, _tunnel_id: VpnTunnelId, _header: VpnCmdHeader, mut body: CmdBodyRead| {
            let this = this.clone();
            async move {
                let data = body.read_all().await?;
                let req = QueryNodeReq::clone_from_slice(data.as_slice()).map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?;
                let seq = req.seq;
                let resp = match this.handle_query_node_req(req.group_id, req.network_id, req.ip).await {
                    Ok(result) => QueryNodeResp {
                        seq,
                        node_id: result,
                    },
                    Err(e) => {
                        log::error!("handle_query_node_req failed: {:?}", e);
                        QueryNodeResp {
                            seq,
                            node_id: None,
                        }
                    }
                };
                this.cmd_server.send(&peer_id,
                                     VpnCmdCode::QueryNodeResp as u8,
                                     this.version,
                                     resp.to_vec().map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?.as_slice()).await?;
                Ok(())
            }
        });
    }

    async fn handle_join_network_group_req(&self, peer_id: PeerId, req: JoinNetworkGroupReq) -> VpnResult<()> {
        log::info!("join peer base58 {} base36 {} to group {}", peer_id.to_base58(), peer_id.to_base36(), req.group_id.to_string());
        let node_id = NodeId::from(peer_id.as_slice());
        let exist = self.network_manager.exist_network_group(&req.group_id).await?;
        if !exist {
            Err(vpn_err!(VpnErrorCode::NetworkGroupNotExist))
        } else {
            if !self.network_manager.has_joined(&req.group_id, &node_id).await? {
                self.network_manager.add_joined_node(&req.group_id, &node_id, req.name.clone()).await?;
            }
            Ok(())
        }
    }

    async fn handle_get_vpn_info_req(&self, peer_id: PeerId, info_version: u64) -> VpnResult<(u64, Vec<NodeVpnInfo>)> {
        let node_id = NodeId::from(peer_id.as_slice());
        let node =  self.node_manager.get_node(&node_id).await?;
        if node.is_none() {
            return Err(vpn_err!(VpnErrorCode::NotFoundNode));
        }
        if node.as_ref().unwrap().info_version == info_version {
            return Ok((info_version, vec![]));
        }
        let mut info_list = vec![];
        let node_networks = self.network_manager.get_networks_of_node(&node_id).await?;
        for node_network in node_networks.iter() {
            if node_network.ip.is_none() {
                continue;
            }

            let members = self.network_manager.get_network_member(&node_network.id).await?;
            info_list.push(NodeVpnInfo {
                node_info: node_network.clone(),
                members,
            });
        }
        Ok((node.as_ref().unwrap().info_version, info_list))
    }

    async fn handle_query_node_req(&self, group_id: NetworkGroupId, network_id: NetworkId, dest_ip: IpAddr) -> VpnResult<Option<NodeId>> {
        self.network_manager.get_member_of_ip(&group_id, &network_id, &dest_ip).await
    }

    pub async fn get_peer_ip_list(&self, peer_id: &PeerId) -> VpnResult<Vec<IpAddr>> {
        self.cmd_server.get_peer_wan_ip(peer_id).await
    }
}
