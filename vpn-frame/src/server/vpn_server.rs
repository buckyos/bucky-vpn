use crate::errors::{VpnErrorCode, VpnResult, vpn_err};
use crate::server::{
    NetworkGroupId, NetworkId, NetworkManager, NodeId, NodeManager, PnControlServer, PnStore,
    VpnStore, VpnStoreFactory,
};
use crate::{
    ClientProxyNodeInfo, GetVpnInfoReq, GetVpnInfoResp, JoinNetworkGroupReq, JoinNetworkGroupResp,
    NodeNetworkPnInfo, NodeVpnInfo, PnServerInfo, QueryNodeReq, QueryNodeResp, VpnCmdCode,
    VPN_CMD_VERSION, VpnCmdHeader, VpnTunnelId, decode_pn_server_info,
};
use async_trait::async_trait;
use bucky_raw_codec::{RawConvertTo, RawFrom};
use chrono::{DateTime, TimeDelta, Utc};
use sfo_cmd_server::errors::{CmdErrorCode, cmd_err, into_cmd_err};
use sfo_cmd_server::server::CmdServer;
use sfo_cmd_server::{CmdBody, PeerId};
use std::collections::HashMap;
use std::net::IpAddr;
use std::ops::Add;
use std::sync::atomic::AtomicU16;
use std::sync::{Arc, Mutex, Once};
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

    async fn resolve(&self, pn_server: &PnServerInfo) -> VpnResult<Option<PnServerInfo>> {
        if self.is_valid(pn_server).await? {
            Ok(Some(pn_server.clone()))
        } else {
            Ok(None)
        }
    }

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

    async fn report_heartbeat(
        &self,
        _pn_node_id: &NodeId,
        _heartbeat: &crate::ProxyNodeHeartbeat,
    ) -> VpnResult<()> {
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

pub struct VpnServer<
    T: VpnCmdServer,
    S: VpnStore + PnStore,
    F: VpnStoreFactory<S>,
    P: CmdServer<u16, u8> = T,
> {
    network_manager: Arc<NetworkManager<S, F>>,
    node_manager: Arc<NodeManager<S, F>>,
    cmd_server: Arc<T>,
    pn_control_server: Arc<PnControlServer<P, S, F>>,
    start_once: Once,
    online_nodes: Mutex<OnlineNodesState>,
    offline_monitor_handle: Mutex<Option<JoinHandle<()>>>,
}
pub type VpnServerRef<T, S, F, P = T> = Arc<VpnServer<T, S, F, P>>;

impl<T: VpnCmdServer, S: VpnStore + PnStore, F: VpnStoreFactory<S>> VpnServer<T, S, F> {
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
        Self::new_with_optional_pn_control_cmd_server(
            cmd_server.clone(),
            cmd_server,
            factory,
            pn_server_selector,
        )
    }
}

impl<T, S, F, P> VpnServer<T, S, F, P>
where
    T: VpnCmdServer,
    S: VpnStore + PnStore,
    F: VpnStoreFactory<S>,
    P: CmdServer<u16, u8>,
{
    pub fn new_with_pn_control_cmd_server(
        cmd_server: Arc<T>,
        pn_cmd_server: Arc<P>,
        factory: Arc<F>,
        pn_server_selector: Arc<dyn PnServerSelector>,
    ) -> Arc<Self> {
        Self::new_with_optional_pn_control_cmd_server(
            cmd_server,
            pn_cmd_server,
            factory,
            Some(pn_server_selector),
        )
    }

    fn new_with_optional_pn_control_cmd_server(
        cmd_server: Arc<T>,
        pn_cmd_server: Arc<P>,
        factory: Arc<F>,
        pn_server_selector: Option<Arc<dyn PnServerSelector>>,
    ) -> Arc<Self> {
        let node_manager = NodeManager::new(factory.clone());
        let network_manager = NetworkManager::new(factory.clone(), node_manager.clone());
        let pn_control_server = PnControlServer::new(
            pn_cmd_server,
            factory.clone(),
            network_manager.clone(),
            pn_server_selector,
        );
        Arc::new(Self {
            network_manager,
            node_manager,
            cmd_server,
            pn_control_server,
            start_once: Once::new(),
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
        self.pn_control_server.select_pn_server(network_id).await
    }

    pub fn start(self: &Arc<Self>) {
        let this = self.clone();
        self.start_once.call_once(move || {
            this.register_cmd_handler();
            this.pn_control_server.start();
            let monitor = this.clone();
            let handle = tokio::spawn(async move {
                monitor.monitor_offline_nodes().await;
            });
            let mut handle_lock = this.offline_monitor_handle.lock().unwrap();
            *handle_lock = Some(handle);
        });
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
                  header: VpnCmdHeader,
                  mut body: CmdBody| {
                let this = this.clone();
                async move {
                    if header.version() != VPN_CMD_VERSION {
                        return Err(cmd_err!(
                            CmdErrorCode::InvalidParam,
                            "unsupported vpn command version {} expected {}",
                            header.version(),
                            VPN_CMD_VERSION
                        ));
                    }
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
                        .handle_get_vpn_info_req(
                            peer_id.clone(),
                            req.info_version,
                            req.pn_info_version,
                        )
                        .await
                    {
                        Ok((version, result)) => GetVpnInfoResp {
                            seq,
                            result: 0,
                            info_version: version.0,
                            pn_info_version: version.1,
                            vpn_list: result,
                        },
                        Err(e) => {
                            log::error!("handle_get_vpn_info_req failed: {:?}", e);
                            GetVpnInfoResp {
                                seq,
                                result: e.code() as u8,
                                info_version: 0,
                                pn_info_version: 0,
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
        pn_info_version: Option<u16>,
    ) -> VpnResult<((u16, u16), Vec<NodeVpnInfo>)> {
        let node_id = NodeId::from(peer_id.as_slice());
        let node = self.node_manager.get_node(&node_id).await?;
        if node.is_none() {
            return Err(vpn_err!(VpnErrorCode::NotFoundNode));
        }
        let info_version_current = node.as_ref().unwrap().info_version;
        let node_networks = self.network_manager.get_networks_of_node(&node_id).await?;

        let mut node_pn_networks = Vec::new();
        let mut node_networks_with_pn = Vec::new();
        for mut node_network in node_networks {
            if node_network.ip.is_none() {
                continue;
            }
            let persisted_pn_server = node_network
                .pn_server
                .as_ref()
                .map(|proxy| PnServerInfo::new(proxy.proxy_id.to_p2p_base36(), Vec::new()));
            let resolved_pn_server = self
                .pn_control_server
                .resolve_node_network_pn_server(node_network.id, persisted_pn_server.as_ref())
                .await?;
            let client_pn_server = resolved_pn_server
                .as_ref()
                .map(client_proxy_node_info_from_pn_server)
                .transpose()?;
            node_pn_networks.push(NodeNetworkPnInfo {
                network_id: node_network.id,
                proxy: client_pn_server.clone(),
            });
            node_network.pn_server = client_pn_server;
            node_networks_with_pn.push(node_network);
        }

        let (pn_info_version_current, pn_changed_now) = self
            .pn_control_server
            .update_node_pn_info(&node_id, node_pn_networks);
        let info_changed = info_version != Some(info_version_current);
        let should_return_pn = pn_info_version != Some(pn_info_version_current) || pn_changed_now;

        if !info_changed && !should_return_pn {
            return Ok(((info_version_current, pn_info_version_current), vec![]));
        }

        let mut info_list = vec![];
        for node_network in node_networks_with_pn {
            let mut node_network = node_network;
            if !should_return_pn {
                node_network.pn_server = None;
            }

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
                pn_server_changed: should_return_pn,
                members,
            });
        }
        Ok(((info_version_current, pn_info_version_current), info_list))
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

    pub async fn validate_pn_connection(
        &self,
        source_node_id: &NodeId,
        target_node_id: &NodeId,
    ) -> VpnResult<Option<crate::ValidatedPnConnection>> {
        self.pn_control_server
            .validate_pn_connection(source_node_id, target_node_id)
            .await
    }

    pub async fn validate_pn_connection_from_pn_node(
        &self,
        pn_node_id: &NodeId,
        source_node_id: &NodeId,
        target_node_id: &NodeId,
    ) -> VpnResult<Option<crate::ValidatedPnConnection>> {
        self.pn_control_server
            .validate_pn_connection_from_pn_node(pn_node_id, source_node_id, target_node_id)
            .await
    }

    /// Applies proxy connection traffic reported by the identified proxy node.
    ///
    /// Process-local reporters use the same authorization, assignment, record
    /// validation, and persistence path as reports received over the control
    /// channel.
    pub async fn report_proxy_traffic_from_pn_node(
        &self,
        pn_node_id: &NodeId,
        reports: Vec<crate::ProxyTrafficReport>,
    ) -> VpnResult<Vec<crate::ProxyTrafficReportResp>> {
        self.pn_control_server
            .report_proxy_traffic(pn_node_id, reports)
            .await
    }

    /// Applies per-node traffic reported by the identified proxy node.
    ///
    /// Process-local reporters use the same authorization, assignment, record
    /// validation, and persistence path as reports received over the control
    /// channel.
    pub async fn report_node_traffic_from_pn_node(
        &self,
        pn_node_id: &NodeId,
        reports: Vec<crate::NodeTrafficReport>,
    ) -> VpnResult<Vec<crate::NodeTrafficReportResp>> {
        self.pn_control_server
            .report_node_traffic(pn_node_id, reports)
            .await
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

fn client_proxy_node_info_from_pn_server(
    pn_server: &PnServerInfo,
) -> VpnResult<ClientProxyNodeInfo> {
    let payload = decode_pn_server_info(pn_server)?;
    let proxy_id = NodeId::from_p2p_base36(&pn_server.id).map_err(|_| {
        vpn_err!(
            VpnErrorCode::InvalidParam,
            "invalid proxy node id {}",
            pn_server.id
        )
    })?;
    Ok(ClientProxyNodeInfo {
        proxy_id,
        name: payload.name,
        endpoints: payload.endpoints,
    })
}

impl<T, S, F, P> Drop for VpnServer<T, S, F, P>
where
    T: VpnCmdServer,
    S: VpnStore + PnStore,
    F: VpnStoreFactory<S>,
    P: CmdServer<u16, u8>,
{
    fn drop(&mut self) {
        let mut handle_lock = self.offline_monitor_handle.lock().unwrap();
        if let Some(handle) = handle_lock.take() {
            handle.abort();
        }
    }
}
