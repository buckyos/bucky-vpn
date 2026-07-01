use crate::errors::{VpnErrorCode, VpnResult, vpn_err};
use crate::server::{
    JoinedNode, NetworkGroupId, NetworkId, NetworkManager, NodeId, NodeManager, VpnStore,
    VpnStoreFactory,
};
use crate::{
    GetVpnInfoReq, GetVpnInfoResp, JoinNetworkGroupReq, JoinNetworkGroupResp, NodeVpnInfo,
    PnServerInfo, QueryNodeReq, QueryNodeResp, ReportPnTrafficStatsReq, ReportPnTrafficStatsResp,
    ValidatePnConnectionReq, ValidatePnConnectionResp, VpnCmdCode, VpnCmdHeader, VpnTunnelId,
};
use async_trait::async_trait;
use bucky_raw_codec::{RawConvertTo, RawFrom};
use chrono::{DateTime, TimeDelta, Utc};
use sfo_cmd_server::errors::{CmdErrorCode, into_cmd_err};
use sfo_cmd_server::server::CmdServer;
use sfo_cmd_server::{CmdBody, PeerId};
use std::collections::HashMap;
use std::net::IpAddr;
use std::ops::Add;
use std::sync::atomic::AtomicU16;
use std::sync::{Arc, Mutex};
use tokio::task::JoinHandle;

#[derive(Debug)]
pub struct OnlineNode {
    pub version: Option<String>,
    pub latest: DateTime<Utc>,
    pub change_version: AtomicU16,
}

impl OnlineNode {
    pub fn new(version: Option<String>, latest: DateTime<Utc>) -> Self {
        Self {
            version,
            latest,
            change_version: AtomicU16::new(0),
        }
    }

    pub fn change(&self) {
        self.change_version
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn get_change_version(&self) -> u16 {
        self.change_version
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn is_expire(&self) -> bool {
        self.latest.add(TimeDelta::seconds(120)).timestamp() < Utc::now().timestamp()
    }
}

#[async_trait]
pub trait VpnCmdServer: CmdServer<u16, u8> {
    async fn get_peer_wan_ip(&self, peer_id: &PeerId) -> VpnResult<Vec<IpAddr>>;
}

#[async_trait]
pub trait PnServerSelector: Send + Sync + 'static {
    async fn is_valid(&self, pn_server: &PnServerInfo) -> VpnResult<bool>;
    async fn select(&self, network_id: NetworkId) -> VpnResult<Option<PnServerInfo>>;

    async fn matches_pn_node(
        &self,
        pn_server: &PnServerInfo,
        pn_node_id: &NodeId,
    ) -> VpnResult<bool> {
        Ok(pn_server.id == pn_node_id.to_base36())
    }

    async fn can_accept_connections_from(&self, _pn_node_id: &NodeId) -> VpnResult<bool> {
        Ok(true)
    }

    async fn report_heartbeat(&self, _pn_server: &PnServerInfo) -> VpnResult<()> {
        Ok(())
    }
}

struct OnlineNodesState {
    online_nodes1: HashMap<NodeId, OnlineNode>,
    online_nodes2: HashMap<NodeId, OnlineNode>,
    effect_cache: u8,
}

impl OnlineNodesState {
    pub fn new() -> Self {
        Self {
            online_nodes1: HashMap::new(),
            online_nodes2: HashMap::new(),
            effect_cache: 0,
        }
    }

    pub fn update_online_node(&mut self, node: &NodeId, version: Option<String>) -> bool {
        let (cur_online_nodes, prev_online_nodes) = if self.effect_cache == 0 {
            (&mut self.online_nodes1, &mut self.online_nodes2)
        } else {
            (&mut self.online_nodes2, &mut self.online_nodes1)
        };
        if let Some(mut prev_node) = prev_online_nodes.remove(&node) {
            prev_node.version = version;
            prev_node.latest = Utc::now();
            cur_online_nodes.insert(node.clone(), prev_node);
            false
        } else {
            if let Some(node) = cur_online_nodes.get_mut(node) {
                node.version = version;
                node.latest = Utc::now();
                false
            } else {
                cur_online_nodes.insert(node.clone(), OnlineNode::new(version, Utc::now()));
                log::info!("node {} is online", node.to_base36());
                true
            }
        }
    }

    pub fn get_offline_nodes(&mut self) -> Vec<NodeId> {
        let (cur_online_nodes, prev_online_nodes) = if self.effect_cache == 0 {
            (&mut self.online_nodes1, &mut self.online_nodes2)
        } else {
            (&mut self.online_nodes2, &mut self.online_nodes1)
        };

        let mut offline_nodes = Vec::new();
        for (node_id, online) in prev_online_nodes.drain() {
            if online.is_expire() {
                log::info!("node {} is offline", node_id.to_base36());
                offline_nodes.push(node_id);
            } else {
                cur_online_nodes.insert(node_id, online);
            }
        }
        self.effect_cache = 1 - self.effect_cache;
        offline_nodes
    }

    pub fn get_node(&self, node_id: &NodeId) -> Option<&OnlineNode> {
        if self.effect_cache == 0 {
            if let Some(node) = self.online_nodes1.get(node_id) {
                Some(node)
            } else {
                self.online_nodes2.get(node_id)
            }
        } else {
            if let Some(node) = self.online_nodes2.get(node_id) {
                Some(node)
            } else {
                self.online_nodes1.get(node_id)
            }
        }
    }
}

pub struct VpnServer<T: VpnCmdServer, S: VpnStore, F: VpnStoreFactory<S>> {
    store_factory: Arc<F>,
    network_manager: Arc<NetworkManager<S, F>>,
    node_manager: Arc<NodeManager<S, F>>,
    cmd_server: Arc<T>,
    pn_server_selector: Option<Arc<dyn PnServerSelector>>,
    online_nodes: Mutex<OnlineNodesState>,
    offline_monitor_handle: Mutex<Option<JoinHandle<()>>>,
}
pub type VpnServerRef<T, S, F> = Arc<VpnServer<T, S, F>>;

impl<T: VpnCmdServer, S: VpnStore, F: VpnStoreFactory<S>> VpnServer<T, S, F> {
    pub fn new(cmd_server: Arc<T>, factory: Arc<F>) -> Arc<Self> {
        Self::new_with_optional_pn_server_selector(cmd_server, factory, None)
    }

    pub fn new_with_pn_server_selector(
        cmd_server: Arc<T>,
        factory: Arc<F>,
        pn_server_selector: Arc<dyn PnServerSelector>,
    ) -> Arc<Self> {
        Self::new_with_optional_pn_server_selector(cmd_server, factory, Some(pn_server_selector))
    }

    fn new_with_optional_pn_server_selector(
        cmd_server: Arc<T>,
        factory: Arc<F>,
        pn_server_selector: Option<Arc<dyn PnServerSelector>>,
    ) -> Arc<Self> {
        let node_manager = NodeManager::new(factory.clone());
        Arc::new(Self {
            store_factory: factory.clone(),
            network_manager: NetworkManager::new(factory.clone(), node_manager.clone()),
            node_manager,
            cmd_server,
            pn_server_selector,
            online_nodes: Mutex::new(OnlineNodesState::new()),
            offline_monitor_handle: Mutex::new(None),
        })
    }

    pub fn network_manager(&self) -> &Arc<NetworkManager<S, F>> {
        &self.network_manager
    }

    pub fn node_manager(&self) -> &Arc<NodeManager<S, F>> {
        &self.node_manager
    }

    pub async fn select_pn_server(&self, network_id: NetworkId) -> VpnResult<Option<PnServerInfo>> {
        if let Some(selector) = &self.pn_server_selector {
            selector.select(network_id).await
        } else {
            Ok(None)
        }
    }

    pub fn start(self: &Arc<Self>) {
        self.register_cmd_handler();
        let this = self.clone();
        let handle = tokio::spawn(async move {
            this.monitor_offline_nodes().await;
        });
        let mut handle_lock = self.offline_monitor_handle.lock().unwrap();
        *handle_lock = Some(handle);
    }

    async fn monitor_offline_nodes(self: &Arc<Self>) {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(65)).await;
            let offline_nodes = {
                let mut online_nodes = self.online_nodes.lock().unwrap();
                online_nodes.get_offline_nodes()
            };
            if let Err(e) = self.network_manager().node_offline(&offline_nodes).await {
                log::error!("node_offline failed: {:?}", e);
            }
        }
    }

    fn register_cmd_handler(self: &Arc<Self>) {
        let this = self.clone();
        self.cmd_server.register_cmd_handler(
            VpnCmdCode::GetVpnInfo as u8,
            move |_local_id: PeerId,
                  peer_id: PeerId,
                  _tunnel_id: VpnTunnelId,
                  _header: VpnCmdHeader,
                  mut body: CmdBody| {
                let this = this.clone();
                async move {
                    let data = body.read_all().await?;
                    let req = GetVpnInfoReq::clone_from_slice(data.as_slice())
                        .map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?;
                    {
                        let node_id = NodeId::from(peer_id.as_slice());
                        let is_new = {
                            let mut online_nodes = this.online_nodes.lock().unwrap();
                            online_nodes.update_online_node(&node_id, req.client_version.clone())
                        };
                        if is_new {
                            if let Err(e) = this.network_manager().node_online(&vec![node_id]).await
                            {
                                log::error!("node_online failed: {:?}", e);
                            }
                        }
                    }
                    let seq = req.seq;
                    let resp = match this
                        .handle_get_vpn_info_req(peer_id.clone(), req.info_version)
                        .await
                    {
                        Ok((version, result)) => GetVpnInfoResp {
                            seq,
                            result: 0,
                            info_version: version,
                            vpn_list: result,
                        },
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
                    Ok(Some(CmdBody::from_bytes(
                        resp.to_vec()
                            .map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?,
                    )))
                }
            },
        );

        let this = self.clone();
        self.cmd_server.register_cmd_handler(
            VpnCmdCode::JoinNetworkGroup as u8,
            move |_local_id: PeerId,
                  peer_id: PeerId,
                  _tunnel_id: VpnTunnelId,
                  _header: VpnCmdHeader,
                  mut body: CmdBody| {
                let this = this.clone();
                async move {
                    let data = body.read_all().await?;
                    let req = JoinNetworkGroupReq::clone_from_slice(data.as_slice())
                        .map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?;
                    let seq = req.seq;
                    let resp = if let Err(e) = this
                        .handle_join_network_group_req(peer_id.clone(), req)
                        .await
                    {
                        log::error!("handle_join_network_group_req failed: {:?}", e);
                        JoinNetworkGroupResp {
                            seq,
                            result: e.code() as u8,
                        }
                    } else {
                        JoinNetworkGroupResp { seq, result: 0 }
                    };
                    Ok(Some(CmdBody::from_bytes(
                        resp.to_vec()
                            .map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?,
                    )))
                }
            },
        );

        let this = self.clone();
        self.cmd_server.register_cmd_handler(
            VpnCmdCode::QueryNode as u8,
            move |_local_id: PeerId,
                  _peer_id: PeerId,
                  _tunnel_id: VpnTunnelId,
                  _header: VpnCmdHeader,
                  mut body: CmdBody| {
                let this = this.clone();
                async move {
                    let data = body.read_all().await?;
                    let req = QueryNodeReq::clone_from_slice(data.as_slice())
                        .map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?;
                    let seq = req.seq;
                    let resp = match this
                        .handle_query_node_req(req.group_id, req.network_id, req.ip)
                        .await
                    {
                        Ok(result) => QueryNodeResp {
                            seq,
                            node_id: result,
                        },
                        Err(e) => {
                            log::error!("handle_query_node_req failed: {:?}", e);
                            QueryNodeResp { seq, node_id: None }
                        }
                    };
                    Ok(Some(CmdBody::from_bytes(
                        resp.to_vec()
                            .map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?,
                    )))
                }
            },
        );

        let this = self.clone();
        self.cmd_server.register_cmd_handler(
            VpnCmdCode::ReportPnTrafficStats as u8,
            move |_local_id: PeerId,
                  _peer_id: PeerId,
                  _tunnel_id: VpnTunnelId,
                  _header: VpnCmdHeader,
                  mut body: CmdBody| {
                let this = this.clone();
                async move {
                    let data = body.read_all().await?;
                    let req = ReportPnTrafficStatsReq::clone_from_slice(data.as_slice())
                        .map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?;
                    let seq = req.seq;
                    let resp = match this
                        .handle_report_pn_traffic_stats_req(
                            &req.node_id,
                            req.pn_server.as_ref(),
                            req.tx_bytes,
                            req.rx_bytes,
                        )
                        .await
                    {
                        Ok(()) => ReportPnTrafficStatsResp { seq, result: 0 },
                        Err(e) => {
                            log::error!("handle_report_pn_traffic_stats_req failed: {:?}", e);
                            ReportPnTrafficStatsResp {
                                seq,
                                result: e.code() as u8,
                            }
                        }
                    };
                    Ok(Some(CmdBody::from_bytes(
                        resp.to_vec()
                            .map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?,
                    )))
                }
            },
        );

        let this = self.clone();
        self.cmd_server.register_cmd_handler(
            VpnCmdCode::ValidatePnConnection as u8,
            move |_local_id: PeerId,
                  peer_id: PeerId,
                  _tunnel_id: VpnTunnelId,
                  _header: VpnCmdHeader,
                  mut body: CmdBody| {
                let this = this.clone();
                async move {
                    let data = body.read_all().await?;
                    let req = ValidatePnConnectionReq::clone_from_slice(data.as_slice())
                        .map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?;
                    let seq = req.seq;
                    let pn_node_id = NodeId::from(peer_id.as_slice());
                    let resp = match this
                        .validate_pn_connection_from_pn_node(&pn_node_id, &req.from, &req.to)
                        .await
                    {
                        Ok(allowed) => ValidatePnConnectionResp {
                            seq,
                            result: 0,
                            allowed,
                        },
                        Err(e) => {
                            log::error!("handle_validate_pn_connection_req failed: {:?}", e);
                            ValidatePnConnectionResp {
                                seq,
                                result: e.code() as u8,
                                allowed: false,
                            }
                        }
                    };
                    Ok(Some(CmdBody::from_bytes(
                        resp.to_vec()
                            .map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?,
                    )))
                }
            },
        );
    }

    async fn handle_join_network_group_req(
        &self,
        peer_id: PeerId,
        req: JoinNetworkGroupReq,
    ) -> VpnResult<()> {
        log::info!(
            "join peer {} to group {}",
            peer_id.to_base36(),
            req.group_id.to_string()
        );
        let node_id = NodeId::from(peer_id.as_slice());
        let exist = self
            .network_manager
            .exist_network_group(&req.group_id)
            .await?;
        if !exist {
            Err(vpn_err!(VpnErrorCode::NetworkGroupNotExist))
        } else {
            if !self
                .network_manager
                .has_joined(&req.group_id, &node_id)
                .await?
            {
                self.network_manager
                    .add_joined_node(&req.group_id, &node_id, req.name.clone())
                    .await?;
            }
            Ok(())
        }
    }

    async fn handle_get_vpn_info_req(
        &self,
        peer_id: PeerId,
        info_version: Option<u16>,
    ) -> VpnResult<(u16, Vec<NodeVpnInfo>)> {
        let node_id = NodeId::from(peer_id.as_slice());
        let node = self.node_manager.get_node(&node_id).await?;
        if node.is_none() {
            return Err(vpn_err!(VpnErrorCode::NotFoundNode));
        }
        if info_version.is_some() && node.as_ref().unwrap().info_version == info_version.unwrap() {
            return Ok((info_version.unwrap(), vec![]));
        }
        let mut info_list = vec![];
        let node_networks = self.network_manager.get_networks_of_node(&node_id).await?;
        for node_network in node_networks {
            let mut node_network = node_network;
            if node_network.ip.is_none() {
                continue;
            }
            self.ensure_node_network_pn_server(&mut node_network)
                .await?;

            let members = self
                .network_manager
                .get_allowed_network_member(&node_network.id)
                .await?;
            let members = {
                let online_nodes = self.online_nodes.lock().unwrap();
                members
                    .iter()
                    .filter(|member| member.id != node_id)
                    .filter(|member| {
                        if let Some(online_node) = online_nodes.get_node(&member.id) {
                            if !online_node.is_expire() {
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    })
                    .map(|member| member.clone())
                    .collect()
            };
            info_list.push(NodeVpnInfo {
                node_info: node_network,
                members,
            });
        }
        Ok((node.as_ref().unwrap().info_version, info_list))
    }

    async fn ensure_node_network_pn_server(
        &self,
        node_network: &mut crate::NodeNetwork,
    ) -> VpnResult<()> {
        let Some(selector) = &self.pn_server_selector else {
            return Ok(());
        };

        let current_pn_server = node_network.pn_server.as_ref();
        let need_reassign = match current_pn_server {
            Some(pn_server) => !selector.is_valid(pn_server).await?,
            None => true,
        };
        if !need_reassign {
            return Ok(());
        }

        let selected = selector.select(node_network.id).await?;
        if selected.as_ref() != current_pn_server {
            self.network_manager
                .update_network_pn_server(&node_network.id, selected.clone())
                .await?;
            node_network.pn_server = selected;
        }
        Ok(())
    }

    async fn handle_query_node_req(
        &self,
        group_id: NetworkGroupId,
        network_id: NetworkId,
        dest_ip: IpAddr,
    ) -> VpnResult<Option<NodeId>> {
        self.network_manager
            .get_member_of_ip(&group_id, &network_id, &dest_ip)
            .await
    }

    async fn handle_report_pn_traffic_stats_req(
        &self,
        node_id: &NodeId,
        pn_server: Option<&PnServerInfo>,
        tx_bytes: u64,
        rx_bytes: u64,
    ) -> VpnResult<()> {
        if let (Some(selector), Some(pn_server)) = (&self.pn_server_selector, pn_server) {
            selector.report_heartbeat(pn_server).await?;
        }
        let mut store = self.store_factory.get_vpn_store().await?;
        store
            .add_pn_traffic_delta(node_id, tx_bytes, rx_bytes)
            .await
    }

    pub async fn validate_pn_connection(
        &self,
        source_node_id: &NodeId,
        target_node_id: &NodeId,
    ) -> VpnResult<bool> {
        let mut store = self.store_factory.get_vpn_store().await?;
        let source_groups = store.get_joined_network_group(source_node_id).await?;
        let target_groups = store.get_joined_network_group(target_node_id).await?;
        let allowed_target_groups = target_groups
            .iter()
            .filter(|joined| joined.allow_join)
            .map(|joined| joined.group_id)
            .collect::<std::collections::HashSet<_>>();

        let allowed = source_groups
            .iter()
            .any(|joined| joined.allow_join && allowed_target_groups.contains(&joined.group_id));
        if !allowed {
            log::warn!(
                "pn connection rejected by group policy source={} source_groups=[{}] target={} target_groups=[{}]",
                source_node_id.to_base36(),
                format_joined_groups(&source_groups),
                target_node_id.to_base36(),
                format_joined_groups(&target_groups)
            );
        }
        Ok(allowed)
    }

    pub async fn validate_pn_connection_from_pn_node(
        &self,
        pn_node_id: &NodeId,
        source_node_id: &NodeId,
        target_node_id: &NodeId,
    ) -> VpnResult<bool> {
        let Some(selector) = &self.pn_server_selector else {
            return self
                .validate_pn_connection(source_node_id, target_node_id)
                .await;
        };
        if !selector.can_accept_connections_from(pn_node_id).await? {
            log::warn!(
                "pn connection rejected because pn node is not authorized pn_node={} source={} target={}",
                pn_node_id.to_base36(),
                source_node_id.to_base36(),
                target_node_id.to_base36()
            );
            return Ok(false);
        }
        if !self
            .source_client_is_assigned_to_pn_node(selector.as_ref(), pn_node_id, source_node_id)
            .await?
        {
            log::warn!(
                "pn connection rejected because source client is not assigned to pn node pn_node={} source={} target={}",
                pn_node_id.to_base36(),
                source_node_id.to_base36(),
                target_node_id.to_base36()
            );
            return Ok(false);
        }
        self.validate_pn_connection(source_node_id, target_node_id)
            .await
    }

    async fn source_client_is_assigned_to_pn_node(
        &self,
        selector: &dyn PnServerSelector,
        pn_node_id: &NodeId,
        source_node_id: &NodeId,
    ) -> VpnResult<bool> {
        let source_networks = self
            .network_manager
            .get_networks_of_node(source_node_id)
            .await?;
        for network in &source_networks {
            let Some(pn_server) = network.pn_server.as_ref() else {
                continue;
            };
            if selector.matches_pn_node(pn_server, pn_node_id).await? {
                return Ok(true);
            }
        }
        log::warn!(
            "source client has no network assigned to pn node pn_node={} source={} source_networks=[{}]",
            pn_node_id.to_base36(),
            source_node_id.to_base36(),
            format_node_network_pn_assignments(&source_networks)
        );
        Ok(false)
    }

    pub async fn get_peer_ip_list(&self, peer_id: &PeerId) -> VpnResult<Vec<IpAddr>> {
        self.cmd_server.get_peer_wan_ip(peer_id).await
    }

    pub async fn get_node_online_state(
        &self,
        node_id: &NodeId,
    ) -> Option<(Option<String>, Vec<IpAddr>)> {
        let version = {
            let online_nodes = self.online_nodes.lock().unwrap();
            if let Some(node) = online_nodes.get_node(node_id) {
                if node.is_expire() {
                    return None;
                }
                node.version.clone()
            } else {
                return None;
            }
        };

        let ips = self
            .cmd_server
            .get_peer_wan_ip(&PeerId::from(node_id.as_slice()))
            .await
            .unwrap_or(vec![]);
        if ips.is_empty() {
            None
        } else {
            Some((version, ips))
        }
    }
}

fn format_joined_groups(groups: &[JoinedNode]) -> String {
    groups
        .iter()
        .map(|joined| format!("{}:allow_join={}", joined.group_id, joined.allow_join))
        .collect::<Vec<_>>()
        .join(",")
}

fn format_node_network_pn_assignments(networks: &[crate::NodeNetwork]) -> String {
    networks
        .iter()
        .map(|network| {
            let pn_server = network
                .pn_server
                .as_ref()
                .map(|pn_server| pn_server.id.as_str())
                .unwrap_or("none");
            format!("{}:group={}:pn={}", network.id, network.group_id, pn_server)
        })
        .collect::<Vec<_>>()
        .join(",")
}

impl<T: VpnCmdServer, S: VpnStore, F: VpnStoreFactory<S>> Drop for VpnServer<T, S, F> {
    fn drop(&mut self) {
        let mut handle_lock = self.offline_monitor_handle.lock().unwrap();
        if let Some(handle) = handle_lock.take() {
            handle.abort();
        }
    }
}
