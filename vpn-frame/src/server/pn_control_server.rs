use crate::errors::{VpnErrorCode, VpnResult, vpn_err};
use crate::server::{
    JoinedNode, NetworkId, NetworkManager, NodeId, NodePnManager, PnServerSelector, PnStore, VpnStore,
    VpnStoreFactory,
};
use crate::{
    NodeNetworkPnInfo, NodeTrafficReport, NodeTrafficReportResp, PnServerInfo, ProxyNodeHeartbeat,
    ProxyTrafficReportApplyResult, ReportPnTrafficStatsReq, ReportPnTrafficStatsResp,
    ReportProxyHeartbeatReq, ReportProxyHeartbeatResp, ReportProxyTrafficReq,
    ReportProxyTrafficResp, VPN_CMD_VERSION, ValidatePnConnectionReq, ValidatePnConnectionResp,
    VpnCmdCode, VpnCmdPkgLen,
};
use bucky_raw_codec::{RawConvertTo, RawEncode, RawFrom};
use sfo_cmd_server::errors::{CmdErrorCode, CmdResult, cmd_err, into_cmd_err};
use sfo_cmd_server::server::CmdServer;
use sfo_cmd_server::{CmdBody, PeerId};
use std::sync::{Arc, Once};

const MAX_TRAFFIC_COMMAND_BYTES: usize = 1024 * 1024;
const MAX_TRAFFIC_REPORT_ID_LEN: usize = 256;

pub struct PnControlServer<P, S, F>
where
    P: CmdServer<VpnCmdPkgLen, u8>,
    S: VpnStore + PnStore,
    F: VpnStoreFactory<S>,
{
    pn_cmd_server: Arc<P>,
    store_factory: Arc<F>,
    network_manager: Arc<NetworkManager<S, F>>,
    pn_server_selector: Option<Arc<dyn PnServerSelector>>,
    node_pn_manager: Arc<NodePnManager>,
    start_once: Once,
}

impl<P, S, F> PnControlServer<P, S, F>
where
    P: CmdServer<VpnCmdPkgLen, u8>,
    S: VpnStore + PnStore,
    F: VpnStoreFactory<S>,
{
    pub fn new(
        pn_cmd_server: Arc<P>,
        store_factory: Arc<F>,
        network_manager: Arc<NetworkManager<S, F>>,
        pn_server_selector: Option<Arc<dyn PnServerSelector>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            pn_cmd_server,
            store_factory,
            network_manager,
            pn_server_selector,
            node_pn_manager: NodePnManager::new(),
            start_once: Once::new(),
        })
    }

    pub(crate) fn start(self: &Arc<Self>) {
        let this = self.clone();
        self.start_once
            .call_once(move || this.register_cmd_handler());
    }

    fn register_cmd_handler(self: &Arc<Self>) {
        let this = self.clone();
        self.pn_cmd_server.register_cmd_handler(
            VpnCmdCode::ReportPnTrafficStats as u8,
            move |_local_id: PeerId, peer_id: PeerId, _tunnel_id, header, mut body: CmdBody| {
                let this = this.clone();
                async move {
                    require_version(&header)?;
                    let data = body.read_all().await?;
                    require_traffic_command_size(data.len())?;
                    let req = ReportPnTrafficStatsReq::clone_from_slice(data.as_slice())
                        .map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?;
                    let seq = req.seq;
                    let peer_node_id = NodeId::from(peer_id.as_slice());
                    let resp = match this
                        .report_node_traffic(&peer_node_id, req.reports)
                        .await
                    {
                        Ok(reports) => ReportPnTrafficStatsResp {
                            seq,
                            result: 0,
                            reports,
                        },
                        Err(err) => {
                            log::error!(
                                "handle proxy control traffic report failed: code={:?} msg={}",
                                err.code(),
                                err.msg()
                            );
                            ReportPnTrafficStatsResp {
                                seq,
                                result: err.code() as u8,
                                reports: Vec::new(),
                            }
                        }
                    };
                    response_body(resp)
                }
            },
        );

        let this = self.clone();
        self.pn_cmd_server.register_cmd_handler(
            VpnCmdCode::ReportProxyHeartbeat as u8,
            move |_local_id: PeerId, peer_id: PeerId, _tunnel_id, header, mut body: CmdBody| {
                let this = this.clone();
                async move {
                    require_version(&header)?;
                    let data = body.read_all().await?;
                    require_traffic_command_size(data.len())?;
                    let req = ReportProxyHeartbeatReq::clone_from_slice(data.as_slice())
                        .map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?;
                    let seq = req.seq;
                    let peer_node_id = NodeId::from(peer_id.as_slice());
                    let resp = match this
                        .report_proxy_heartbeat(&peer_node_id, &req.heartbeat)
                        .await
                    {
                        Ok(()) => ReportProxyHeartbeatResp { seq, result: 0 },
                        Err(err) => ReportProxyHeartbeatResp {
                            seq,
                            result: err.code() as u8,
                        },
                    };
                    response_body(resp)
                }
            },
        );

        let this = self.clone();
        self.pn_cmd_server.register_cmd_handler(
            VpnCmdCode::ReportProxyTraffic as u8,
            move |_local_id: PeerId, peer_id: PeerId, _tunnel_id, header, mut body: CmdBody| {
                let this = this.clone();
                async move {
                    require_version(&header)?;
                    let data = body.read_all().await?;
                    require_traffic_command_size(data.len())?;
                    let req = ReportProxyTrafficReq::clone_from_slice(data.as_slice())
                        .map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?;
                    let seq = req.seq;
                    let peer_node_id = NodeId::from(peer_id.as_slice());
                    let resp = match this.report_proxy_traffic(&peer_node_id, req.reports).await {
                        Ok(reports) => ReportProxyTrafficResp {
                            seq,
                            result: 0,
                            reports,
                        },
                        Err(err) => {
                            log::error!(
                                "handle proxy control proxy traffic report failed: code={:?} msg={}",
                                err.code(),
                                err.msg()
                            );
                            ReportProxyTrafficResp {
                                seq,
                                result: err.code() as u8,
                                reports: Vec::new(),
                            }
                        }
                    };
                    response_body(resp)
                }
            },
        );

        let this = self.clone();
        self.pn_cmd_server.register_cmd_handler(
            VpnCmdCode::ValidatePnConnection as u8,
            move |_local_id: PeerId, peer_id: PeerId, _tunnel_id, header, mut body: CmdBody| {
                let this = this.clone();
                async move {
                    require_version(&header)?;
                    let data = body.read_all().await?;
                    let req = ValidatePnConnectionReq::clone_from_slice(data.as_slice())
                        .map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?;
                    let seq = req.seq;
                    let peer_node_id = NodeId::from(peer_id.as_slice());
                    let resp = match this
                        .validate_pn_connection_from_pn_node(&peer_node_id, &req.from, &req.to)
                        .await
                    {
                        Ok(validation) => ValidatePnConnectionResp {
                            seq,
                            result: 0,
                            allowed: validation.is_some(),
                            network_id: validation.map(|validation| validation.network_id),
                        },
                        Err(err) => {
                            log::error!(
                                "handle proxy control pn connection validation failed: code={:?} msg={}",
                                err.code(),
                                err.msg()
                            );
                            ValidatePnConnectionResp {
                                seq,
                                result: err.code() as u8,
                                allowed: false,
                                network_id: None,
                            }
                        }
                    };
                    response_body(resp)
                }
            },
        );
    }

    pub(crate) async fn select_pn_server(
        &self,
        network_id: NetworkId,
    ) -> VpnResult<Option<PnServerInfo>> {
        if let Some(selector) = &self.pn_server_selector {
            selector.select(network_id).await
        } else {
            Ok(None)
        }
    }

    pub(crate) async fn resolve_node_network_pn_server(
        &self,
        network_id: NetworkId,
        current_pn_server: Option<&PnServerInfo>,
    ) -> VpnResult<Option<PnServerInfo>> {
        let Some(selector) = &self.pn_server_selector else {
            return Ok(None);
        };

        if let Some(pn_server) = current_pn_server {
            if let Some(resolved) = selector.resolve(pn_server).await? {
                return Ok(Some(resolved));
            }
        }

        let selected = selector.select(network_id).await?;
        self.network_manager
            .assign_network_pn_server(&network_id, selected.clone())
            .await?;
        Ok(selected)
    }

    pub(crate) fn update_node_pn_info(
        &self,
        node_id: &NodeId,
        networks: Vec<NodeNetworkPnInfo>,
    ) -> (u16, bool) {
        self.node_pn_manager
            .update_node_pn_info(node_id, networks)
    }

    /// Shared application path for wire and process-local node traffic reports.
    pub(crate) async fn report_node_traffic(
        &self,
        peer_id: &NodeId,
        reports: Vec<NodeTrafficReport>,
    ) -> VpnResult<Vec<NodeTrafficReportResp>> {
        let selector = self.pn_server_selector.as_ref().ok_or_else(|| {
            vpn_err!(
                VpnErrorCode::NoPermission,
                "proxy control traffic report requires a pn server selector"
            )
        })?;
        require_record_count(reports.len())?;
        if !selector.can_accept_connections_from(peer_id).await? {
            return Err(vpn_err!(
                VpnErrorCode::NoPermission,
                "proxy node {} is not allowed to report traffic",
                peer_id.to_base36()
            ));
        }
        let mut store = self.store_factory.get_vpn_store().await?;
        let mut responses = Vec::with_capacity(reports.len());
        for report in reports {
            let result = async {
                validate_node_traffic_record(&report)?;
                if !self
                    .source_client_is_assigned_to_pn_node(
                        selector.as_ref(),
                        peer_id,
                        &report.delta.node_id,
                    )
                    .await?
                {
                    return Err(vpn_err!(
                        VpnErrorCode::NoPermission,
                        "node {} is not assigned to proxy node {}",
                        report.delta.node_id.to_base36(),
                        peer_id.to_base36()
                    ));
                }
                store.apply_node_traffic_report(peer_id, &report).await
            }
            .await;
            responses.push(match result {
                Ok(result) => NodeTrafficReportResp {
                    report_id: report.report_id,
                    result,
                    error_code: None,
                },
                Err(err) => NodeTrafficReportResp {
                    report_id: report.report_id,
                    result: proxy_traffic_error_result(err.code()),
                    error_code: Some(err.code() as u8),
                },
            });
        }
        Ok(responses)
    }

    async fn report_proxy_heartbeat(
        &self,
        peer_id: &NodeId,
        heartbeat: &ProxyNodeHeartbeat,
    ) -> VpnResult<()> {
        let selector = self.pn_server_selector.as_ref().ok_or_else(|| {
            vpn_err!(
                VpnErrorCode::NoPermission,
                "proxy heartbeat requires a pn server selector"
            )
        })?;
        if let Some(pn_server) = heartbeat.pn_server.as_ref() {
            if !selector.matches_pn_node(pn_server, peer_id).await? {
                return Err(vpn_err!(
                    VpnErrorCode::InvalidParam,
                    "proxy heartbeat peer {} does not match reported proxy {}",
                    peer_id.to_base36(),
                    pn_server.id
                ));
            }
        }
        selector.report_heartbeat(peer_id, heartbeat).await
    }

    /// Shared application path for wire and process-local proxy traffic reports.
    pub(crate) async fn report_proxy_traffic(
        &self,
        pn_node_id: &NodeId,
        reports: Vec<crate::ProxyTrafficReport>,
    ) -> VpnResult<Vec<crate::ProxyTrafficReportResp>> {
        let selector = self.pn_server_selector.as_ref().ok_or_else(|| {
            vpn_err!(
                VpnErrorCode::NoPermission,
                "proxy traffic report requires a pn server selector"
            )
        })?;
        require_record_count(reports.len())?;
        if !selector.can_accept_connections_from(pn_node_id).await? {
            log::warn!(
                "proxy node {} is not approved to report traffic",
                pn_node_id.to_base36()
            );
            return Ok(reports
                .into_iter()
                .map(|report| crate::ProxyTrafficReportResp {
                    report_id: report.report_id,
                    result: ProxyTrafficReportApplyResult::Rejected,
                    error_code: Some(VpnErrorCode::NoPermission as u8),
                    remaining: Vec::new(),
                })
                .collect());
        }

        let mut store = self.store_factory.get_vpn_store().await?;
        let mut results = Vec::with_capacity(reports.len());
        for report in reports {
            let result = async {
                validate_proxy_traffic_record(&report)?;
                self.validate_proxy_traffic_assignments(pn_node_id, std::slice::from_ref(&report))
                    .await?;
                store.apply_proxy_traffic_report(pn_node_id, &report).await
            }
            .await;
            results.push(match result {
                Ok(response) => response,
                Err(err) => crate::ProxyTrafficReportResp {
                    report_id: report.report_id,
                    result: proxy_traffic_error_result(err.code()),
                    error_code: Some(err.code() as u8),
                    remaining: Vec::new(),
                },
            });
        }
        Ok(results)
    }

    async fn validate_proxy_traffic_assignments(
        &self,
        pn_node_id: &NodeId,
        reports: &[crate::ProxyTrafficReport],
    ) -> VpnResult<()> {
        let selector = self.pn_server_selector.as_ref().ok_or_else(|| {
            vpn_err!(
                VpnErrorCode::NoPermission,
                "proxy traffic report requires a pn server selector"
            )
        })?;
        for report in reports {
            for sample in std::slice::from_ref(&report.traffic_sample) {
                let network = self
                    .network_manager
                    .get_network(&sample.network_id)
                    .await?
                    .ok_or_else(|| {
                        vpn_err!(
                            VpnErrorCode::InvalidParam,
                            "proxy traffic report {} references missing network {}",
                            report.report_id.0,
                            sample.network_id
                        )
                    })?;
                let assigned = match network.pn_server.as_ref() {
                    Some(pn_server) => {
                        selector.matches_pn_node(pn_server, pn_node_id).await?
                    }
                    None => false,
                };
                if !assigned {
                    return Err(vpn_err!(
                        VpnErrorCode::NoPermission,
                        "network {} is not assigned to proxy node {}",
                        sample.network_id,
                        pn_node_id.to_base36()
                    ));
                }

                let members = self
                    .network_manager
                    .get_network_member(&sample.network_id)
                    .await?;
                let source_is_member = members
                    .iter()
                    .any(|member| member.id == sample.source_id);
                let dest_is_member = members.iter().any(|member| member.id == sample.dest_id);
                if !source_is_member || !dest_is_member {
                    return Err(vpn_err!(
                        VpnErrorCode::NoPermission,
                        "proxy traffic report {} contains nodes outside network {}",
                        report.report_id.0,
                        sample.network_id
                    ));
                }
            }
        }
        Ok(())
    }

    pub(crate) async fn validate_pn_connection(
        &self,
        source_node_id: &NodeId,
        target_node_id: &NodeId,
    ) -> VpnResult<Option<crate::ValidatedPnConnection>> {
        self.select_validated_network(source_node_id, target_node_id, None)
            .await
            .map(|network_id| network_id.map(|network_id| crate::ValidatedPnConnection { network_id }))
    }

    async fn select_validated_network(
        &self,
        source_node_id: &NodeId,
        target_node_id: &NodeId,
        pn_node_id: Option<&NodeId>,
    ) -> VpnResult<Option<NetworkId>> {
        let mut store = self.store_factory.get_vpn_store().await?;
        let (source_groups, target_groups, source_networks, target_networks) = store
            .with_transaction(async |store| {
                Ok((
                    store.get_joined_network_group(source_node_id).await?,
                    store.get_joined_network_group(target_node_id).await?,
                    store.get_networks_of_node(source_node_id).await?,
                    store.get_networks_of_node(target_node_id).await?,
                ))
            })
            .await?;
        let allowed_target_groups = target_groups
            .iter()
            .filter(|joined| joined.allow_join)
            .map(|joined| joined.group_id)
            .collect::<std::collections::HashSet<_>>();

        let allowed_groups = source_groups
            .iter()
            .filter(|joined| joined.allow_join && allowed_target_groups.contains(&joined.group_id))
            .map(|joined| joined.group_id)
            .collect::<std::collections::HashSet<_>>();
        let target_network_ids = target_networks
            .into_iter()
            .map(|network| network.id)
            .collect::<std::collections::HashSet<_>>();
        let selected = crate::select_first_eligible_pn_network(
            &source_networks,
            &target_network_ids,
            &allowed_groups,
            pn_node_id,
        );
        if selected.is_none() {
            log::warn!(
                "pn connection rejected by group policy source={} source_groups=[{}] target={} target_groups=[{}]",
                source_node_id.to_base36(),
                format_joined_groups(&source_groups),
                target_node_id.to_base36(),
                format_joined_groups(&target_groups)
            );
        }
        Ok(selected)
    }

    pub(crate) async fn validate_pn_connection_from_pn_node(
        &self,
        pn_node_id: &NodeId,
        source_node_id: &NodeId,
        target_node_id: &NodeId,
    ) -> VpnResult<Option<crate::ValidatedPnConnection>> {
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
            return Ok(None);
        }
        self.select_validated_network(source_node_id, target_node_id, Some(pn_node_id))
            .await
            .map(|network_id| network_id.map(|network_id| crate::ValidatedPnConnection { network_id }))
    }

    async fn source_client_is_assigned_to_pn_node(
        &self,
        _selector: &dyn PnServerSelector,
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
            if pn_server.proxy_id == *pn_node_id {
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
}

fn require_version(header: &crate::VpnCmdHeader) -> CmdResult<()> {
    if header.version() != VPN_CMD_VERSION {
        return Err(cmd_err!(
            CmdErrorCode::InvalidParam,
            "unsupported vpn command version {} expected {}",
            header.version(),
            VPN_CMD_VERSION
        ));
    }
    Ok(())
}

fn proxy_traffic_error_result(error_code: VpnErrorCode) -> ProxyTrafficReportApplyResult {
    match error_code {
        VpnErrorCode::Failed | VpnErrorCode::IoError | VpnErrorCode::Timeout => {
            ProxyTrafficReportApplyResult::Retryable
        }
        _ => ProxyTrafficReportApplyResult::Rejected,
    }
}

fn require_traffic_command_size(size: usize) -> CmdResult<()> {
    if size > MAX_TRAFFIC_COMMAND_BYTES {
        return Err(cmd_err!(
            CmdErrorCode::InvalidParam,
            "traffic command body {} exceeds limit {}",
            size,
            MAX_TRAFFIC_COMMAND_BYTES
        ));
    }
    Ok(())
}

fn require_record_count(count: usize) -> VpnResult<()> {
    if count == 0 || count > crate::MAX_TRAFFIC_RECORDS_PER_COMMAND {
        return Err(vpn_err!(
            VpnErrorCode::InvalidParam,
            "traffic record count {} is outside 1..={}",
            count,
            crate::MAX_TRAFFIC_RECORDS_PER_COMMAND
        ));
    }
    Ok(())
}

fn validate_node_traffic_record(report: &NodeTrafficReport) -> VpnResult<()> {
    if report.started_at_ms > report.ended_at_ms
        || report.report_id.0.is_empty()
        || report.report_id.0.len() > MAX_TRAFFIC_REPORT_ID_LEN
        || report.delta.tx_bytes > i64::MAX as u64
        || report.delta.rx_bytes > i64::MAX as u64
        || report.delta.tx_speed > i64::MAX as u64
        || report.delta.rx_speed > i64::MAX as u64
    {
        return Err(vpn_err!(
            VpnErrorCode::InvalidParam,
            "node traffic record {} is invalid",
            report.report_id.0
        ));
    }
    Ok(())
}

fn validate_proxy_traffic_record(report: &crate::ProxyTrafficReport) -> VpnResult<()> {
    let sample = &report.traffic_sample;
    if report.started_at_ms > report.ended_at_ms
        || report.report_id.0.is_empty()
        || report.report_id.0.len() > MAX_TRAFFIC_REPORT_ID_LEN
        || sample.source_to_dest.bytes > i64::MAX as u64
        || sample.dest_to_source.bytes > i64::MAX as u64
    {
        return Err(vpn_err!(
            VpnErrorCode::InvalidParam,
            "proxy traffic record {} is invalid",
            report.report_id.0
        ));
    }
    Ok(())
}

fn response_body<T>(resp: T) -> CmdResult<Option<CmdBody>>
where
    T: RawConvertTo<T> + RawEncode,
{
    Ok(Some(CmdBody::from_bytes(
        resp.to_vec()
            .map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?,
    )))
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
                .map(|pn_server| pn_server.proxy_id.to_base36())
                .unwrap_or_else(|| "none".to_string());
            format!("{}:group={}:pn={}", network.id, network.group_id, pn_server)
        })
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{
        Network, NetworkGroupId, NetworkMember, NetworkStore, Node, NodeStore, VpnStoreGuard,
    };
    use crate::{
        ClientProxyNodeInfo, NodeTrafficDelta, NodeTrafficReport, NodeTrafficReportId,
        PnTrafficDirectionSample, PnTrafficSample, ProxyNodeHeartbeat, ProxyNodeHeartbeatId,
        ProxyTrafficReport, ProxyTrafficReportApplyResult, ProxyTrafficReportId,
        ProxyTrafficReportResp,
    };
    use sfo_cmd_server::server::DefaultCmdServerService;
    use sfo_cmd_server::{CmdTunnelRead, CmdTunnelWrite};
    use std::collections::HashMap;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::task::{Context, Poll};
    use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

    struct TestRead;
    struct TestWrite;

    impl AsyncRead for TestRead {
        fn poll_read(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            _buf: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    impl AsyncWrite for TestWrite {
        fn poll_write(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<Result<usize, std::io::Error>> {
            Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), std::io::Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(
            self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
        ) -> Poll<Result<(), std::io::Error>> {
            Poll::Ready(Ok(()))
        }
    }

    fn test_peer(byte: u8) -> PeerId {
        PeerId::from(vec![byte; 32].as_slice())
    }

    impl CmdTunnelRead<()> for TestRead {
        fn get_local_peer_id(&self) -> PeerId {
            test_peer(1)
        }

        fn get_remote_peer_id(&self) -> PeerId {
            test_peer(2)
        }
    }

    impl CmdTunnelWrite<()> for TestWrite {
        fn get_local_peer_id(&self) -> PeerId {
            test_peer(1)
        }

        fn get_remote_peer_id(&self) -> PeerId {
            test_peer(2)
        }
    }

    type TestCmdServer = DefaultCmdServerService<(), TestRead, TestWrite, VpnCmdPkgLen, u8>;

    #[derive(Default)]
    struct TestStoreState {
        assignments: HashMap<NetworkId, Option<PnServerInfo>>,
        node_networks: HashMap<NodeId, Vec<crate::NodeNetwork>>,
        members: HashMap<NetworkId, Vec<NetworkMember>>,
        node_traffic_writes: usize,
        proxy_report_writes: usize,
    }

    #[derive(Clone, Default)]
    struct TestStore {
        state: Arc<Mutex<TestStoreState>>,
    }

    #[derive(Clone)]
    struct TestStoreFactory {
        store: TestStore,
    }

    #[async_trait::async_trait]
    impl VpnStoreFactory<TestStore> for TestStoreFactory {
        async fn get_vpn_store(&self) -> VpnResult<VpnStoreGuard<TestStore>> {
            Ok(VpnStoreGuard::new(self.store.clone()))
        }
    }

    fn unsupported<T>() -> VpnResult<T> {
        Err(vpn_err!(VpnErrorCode::Failed, "unsupported test store operation"))
    }

    #[async_trait::async_trait]
    impl NodeStore for TestStore {
        async fn add_node(&mut self, _node: &Node) -> VpnResult<()> { unsupported() }
        async fn remove_node(&mut self, _id: &NodeId) -> VpnResult<()> { unsupported() }
        async fn get_node(&mut self, _id: &NodeId) -> VpnResult<Option<Node>> { unsupported() }
        async fn exist_node(&mut self, _id: &NodeId) -> VpnResult<bool> { unsupported() }
        async fn inc_info_version(&mut self, _id: &NodeId) -> VpnResult<()> { Ok(()) }
    }

    #[async_trait::async_trait]
    impl NetworkStore for TestStore {
        async fn add_network_group(&mut self, _group_id: &NetworkGroupId) -> VpnResult<()> { unsupported() }
        async fn exist_network_group(&mut self, _group_id: &NetworkGroupId) -> VpnResult<bool> { unsupported() }
        async fn has_joined(&mut self, _group_id: &NetworkGroupId, _node_id: &NodeId) -> VpnResult<bool> { unsupported() }
        async fn add_joined_node(&mut self, _node: &JoinedNode) -> VpnResult<()> { unsupported() }
        async fn del_joined_node(&mut self, _group_id: &NetworkGroupId, _node_id: &NodeId) -> VpnResult<()> { unsupported() }
        async fn get_joined_node(&mut self, _group_id: &NetworkGroupId, _node_id: &NodeId) -> VpnResult<Option<JoinedNode>> { unsupported() }
        async fn get_joined_nodes(&mut self, _group_id: &NetworkGroupId) -> VpnResult<Vec<JoinedNode>> { unsupported() }
        async fn update_joined_node(&mut self, _node: &JoinedNode) -> VpnResult<()> { unsupported() }
        async fn get_joined_network_group(&mut self, _node_id: &NodeId) -> VpnResult<Vec<JoinedNode>> { Ok(Vec::new()) }
        async fn get_networks(&mut self, _group_id: &NetworkGroupId) -> VpnResult<Vec<Network>> { unsupported() }
        async fn add_network(&mut self, _network: &Network) -> VpnResult<()> { unsupported() }
        async fn del_network(&mut self, _network_id: &NetworkId) -> VpnResult<()> { unsupported() }

        async fn get_network(&mut self, network_id: &NetworkId) -> VpnResult<Option<Network>> {
            let state = self.state.lock().unwrap();
            Ok(state.assignments.get(network_id).map(|pn_server| Network {
                id: *network_id,
                group_id: 1,
                name: String::new(),
                ip_seg: None,
                mask: 0,
                ipv6_seg: None,
                ipv6_mask: 0,
                pn_server: pn_server.clone(),
            }))
        }

        async fn update_network(&mut self, network: &Network) -> VpnResult<()> {
            self.state
                .lock()
                .unwrap()
                .assignments
                .insert(network.id, network.pn_server.clone());
            Ok(())
        }

        async fn exist_network(&mut self, network_id: &NetworkId) -> VpnResult<bool> {
            Ok(self.state.lock().unwrap().assignments.contains_key(network_id))
        }

        async fn add_member(&mut self, _network_id: &NetworkId, _member: &NetworkMember) -> VpnResult<()> { unsupported() }
        async fn del_member(&mut self, _network_id: &NetworkId, _member: &NodeId) -> VpnResult<()> { unsupported() }
        async fn has_member(&mut self, network_id: &NetworkId, member: &NodeId) -> VpnResult<bool> {
            Ok(self.state.lock().unwrap().members.get(network_id).is_some_and(|members| members.iter().any(|item| item.id == *member)))
        }
        async fn update_member(&mut self, _network_id: &NetworkId, _member: &NetworkMember) -> VpnResult<()> { unsupported() }
        async fn get_members(&mut self, network_id: &NetworkId) -> VpnResult<Vec<NetworkMember>> {
            Ok(self.state.lock().unwrap().members.get(network_id).cloned().unwrap_or_default())
        }
        async fn get_allowed_members(&mut self, network_id: &NetworkId) -> VpnResult<Vec<NetworkMember>> {
            self.get_members(network_id).await
        }
        async fn get_member(&mut self, _network_id: &NetworkId, _ip_addr: &std::net::IpAddr) -> VpnResult<Option<NetworkMember>> { unsupported() }
        async fn get_networks_of_node(&mut self, node_id: &NodeId) -> VpnResult<Vec<crate::NodeNetwork>> {
            Ok(self.state.lock().unwrap().node_networks.get(node_id).cloned().unwrap_or_default())
        }
    }

    #[async_trait::async_trait]
    impl PnStore for TestStore {
        async fn add_pn_traffic_delta(
            &mut self,
            _node_id: &NodeId,
            _tx_bytes: u64,
            _rx_bytes: u64,
        ) -> VpnResult<()> {
            self.state.lock().unwrap().node_traffic_writes += 1;
            Ok(())
        }

        async fn apply_proxy_traffic_report(
            &mut self,
            _pn_node_id: &NodeId,
            report: &ProxyTrafficReport,
        ) -> VpnResult<ProxyTrafficReportResp> {
            self.state.lock().unwrap().proxy_report_writes += 1;
            Ok(ProxyTrafficReportResp {
                report_id: report.report_id.clone(),
                result: ProxyTrafficReportApplyResult::Applied,
                error_code: None,
                remaining: Vec::new(),
            })
        }

        async fn apply_node_traffic_report(
            &mut self,
            _pn_node_id: &NodeId,
            _report: &NodeTrafficReport,
        ) -> VpnResult<ProxyTrafficReportApplyResult> {
            self.state.lock().unwrap().node_traffic_writes += 1;
            Ok(ProxyTrafficReportApplyResult::Applied)
        }
    }

    #[async_trait::async_trait]
    impl VpnStore for TestStore {
        async fn begin_transaction(&mut self) -> VpnResult<()> { Ok(()) }
        async fn commit_transaction(&mut self) -> VpnResult<()> { Ok(()) }
        async fn rollback_transaction(&mut self) -> VpnResult<()> { Ok(()) }
    }

    struct TestSelector {
        allowed: AtomicBool,
        selected: Mutex<Option<PnServerInfo>>,
        heartbeat_count: AtomicUsize,
    }

    impl TestSelector {
        fn new(allowed: bool, selected: Option<PnServerInfo>) -> Arc<Self> {
            Arc::new(Self {
                allowed: AtomicBool::new(allowed),
                selected: Mutex::new(selected),
                heartbeat_count: AtomicUsize::new(0),
            })
        }
    }

    #[async_trait::async_trait]
    impl PnServerSelector for TestSelector {
        async fn is_valid(&self, pn_server: &PnServerInfo) -> VpnResult<bool> {
            Ok(self.selected.lock().unwrap().as_ref() == Some(pn_server))
        }

        async fn select(&self, _network_id: NetworkId) -> VpnResult<Option<PnServerInfo>> {
            Ok(self.selected.lock().unwrap().clone())
        }

        async fn resolve(&self, pn_server: &PnServerInfo) -> VpnResult<Option<PnServerInfo>> {
            Ok(self
                .selected
                .lock()
                .unwrap()
                .as_ref()
                .filter(|selected| selected.id == pn_server.id)
                .cloned())
        }

        async fn can_accept_connections_from(&self, _pn_node_id: &NodeId) -> VpnResult<bool> {
            Ok(self.allowed.load(Ordering::SeqCst))
        }

        async fn report_heartbeat(
            &self,
            _pn_node_id: &NodeId,
            _heartbeat: &ProxyNodeHeartbeat,
        ) -> VpnResult<()> {
            self.heartbeat_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    type TestControlServer = PnControlServer<TestCmdServer, TestStore, TestStoreFactory>;

    fn test_control(
        store: TestStore,
        selector: Arc<TestSelector>,
    ) -> Arc<TestControlServer> {
        let factory = Arc::new(TestStoreFactory { store });
        let node_manager = crate::server::NodeManager::new(factory.clone());
        let network_manager = NetworkManager::new(factory.clone(), node_manager);
        PnControlServer::new(TestCmdServer::new(), factory, network_manager, Some(selector))
    }

    fn node(byte: u8) -> NodeId {
        NodeId::from(vec![byte; 32].as_slice())
    }

    fn pn_info(node_id: &NodeId) -> PnServerInfo {
        PnServerInfo::new(node_id.to_base36(), Vec::new())
    }

    fn client_proxy(node_id: &NodeId) -> ClientProxyNodeInfo {
        ClientProxyNodeInfo {
            proxy_id: node_id.clone(),
            name: None,
            endpoints: Vec::new(),
        }
    }

    fn node_network(
        network_id: NetworkId,
        pn_server: Option<ClientProxyNodeInfo>,
    ) -> crate::NodeNetwork {
        crate::NodeNetwork {
            id: network_id,
            group_id: 1,
            name: String::new(),
            ip: Some("10.0.0.1".parse().unwrap()),
            mask: 24,
            ipv6: None,
            ipv6_mask: 0,
            pn_server,
        }
    }

    fn member(node_id: NodeId) -> NetworkMember {
        NetworkMember {
            id: node_id,
            ip: String::new(),
            ipv6: None,
        }
    }

    #[test]
    fn changed_control_commands_reject_version_zero_before_decode() {
        let old = crate::VpnCmdHeader::new(
            0,
            false,
            None,
            VpnCmdCode::ReportPnTrafficStats as u8,
            VpnCmdPkgLen::new(0).unwrap(),
        );
        let current = crate::VpnCmdHeader::new(
            VPN_CMD_VERSION,
            false,
            None,
            VpnCmdCode::ReportPnTrafficStats as u8,
            VpnCmdPkgLen::new(0).unwrap(),
        );

        assert!(require_version(&old).is_err());
        assert!(require_version(&current).is_ok());
    }

    #[tokio::test]
    async fn dedicated_heartbeat_does_not_write_traffic() {
        let proxy = node(9);
        let proxy_info = pn_info(&proxy);
        let store = TestStore::default();
        let selector = TestSelector::new(false, Some(proxy_info.clone()));
        let control = test_control(store.clone(), selector.clone());
        let heartbeat = ProxyNodeHeartbeat {
            heartbeat_id: ProxyNodeHeartbeatId("heartbeat-1".to_string()),
            pn_server: Some(proxy_info),
        };

        control
            .report_proxy_heartbeat(&proxy, &heartbeat)
            .await
            .unwrap();

        assert_eq!(store.state.lock().unwrap().node_traffic_writes, 0);
        assert_eq!(selector.heartbeat_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn node_reports_return_independent_results() {
        let proxy = node(9);
        let first = node(1);
        let second = node(2);
        let proxy_info = pn_info(&proxy);
        let store = TestStore::default();
        store.state.lock().unwrap().node_networks.insert(
            first.clone(),
            vec![node_network(7, Some(client_proxy(&proxy)))],
        );
        store.state.lock().unwrap().node_networks.insert(
            second.clone(),
            vec![node_network(8, Some(client_proxy(&node(8))))],
        );
        let selector = TestSelector::new(false, Some(proxy_info.clone()));
        let control = test_control(store.clone(), selector.clone());
        let reports = vec![
            NodeTrafficReport {
                report_id: NodeTrafficReportId("record-1".to_string()),
                started_at_ms: 1,
                ended_at_ms: 2,
                delta: NodeTrafficDelta {
                    node_id: first.clone(),
                    tx_bytes: 10,
                    rx_bytes: 20,
                    tx_speed: 3,
                    rx_speed: 4,
                },
            },
            NodeTrafficReport {
                report_id: NodeTrafficReportId("record-2".to_string()),
                started_at_ms: 1,
                ended_at_ms: 2,
                delta: NodeTrafficDelta {
                    node_id: second.clone(),
                    tx_bytes: 30,
                    rx_bytes: 40,
                    tx_speed: 5,
                    rx_speed: 6,
                },
            },
        ];

        assert!(control.report_node_traffic(&proxy, reports.clone()).await.is_err());
        assert_eq!(store.state.lock().unwrap().node_traffic_writes, 0);
        assert_eq!(selector.heartbeat_count.load(Ordering::SeqCst), 0);

        selector.allowed.store(true, Ordering::SeqCst);
        let responses = control
            .report_node_traffic(&proxy, reports.clone())
            .await
            .unwrap();
        assert_eq!(responses[0].result, ProxyTrafficReportApplyResult::Applied);
        assert_eq!(responses[1].result, ProxyTrafficReportApplyResult::Rejected);
        assert_eq!(store.state.lock().unwrap().node_traffic_writes, 1);
        assert_eq!(selector.heartbeat_count.load(Ordering::SeqCst), 0);

        store.state.lock().unwrap().node_networks.insert(
            second,
            vec![node_network(8, Some(client_proxy(&proxy)))],
        );
        let responses = control.report_node_traffic(&proxy, reports).await.unwrap();
        assert!(responses
            .iter()
            .all(|response| response.result == ProxyTrafficReportApplyResult::Applied));
        assert_eq!(store.state.lock().unwrap().node_traffic_writes, 3);
        assert_eq!(selector.heartbeat_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn pair_reports_return_independent_results() {
        let proxy = node(9);
        let source = node(1);
        let dest = node(2);
        let proxy_info = pn_info(&proxy);
        let store = TestStore::default();
        {
            let mut state = store.state.lock().unwrap();
            state.assignments.insert(7, Some(proxy_info.clone()));
            state.assignments.insert(8, Some(pn_info(&node(8))));
            state.members.insert(7, vec![member(source.clone()), member(dest.clone())]);
            state.members.insert(8, vec![member(source.clone()), member(dest.clone())]);
        }
        let selector = TestSelector::new(true, Some(proxy_info));
        let control = test_control(store.clone(), selector);
        let sample = |network_id| PnTrafficSample {
            network_id,
            source_id: source.clone(),
            dest_id: dest.clone(),
            source_to_dest: PnTrafficDirectionSample { bytes: 1, speed_bytes_per_sec: 1 },
            dest_to_source: PnTrafficDirectionSample { bytes: 1, speed_bytes_per_sec: 1 },
        };
        let reports = vec![
            ProxyTrafficReport {
                report_id: ProxyTrafficReportId("valid".to_string()),
                started_at_ms: 1,
                ended_at_ms: 2,
                traffic_sample: sample(7),
            },
            ProxyTrafficReport {
                report_id: ProxyTrafficReportId("invalid".to_string()),
                started_at_ms: 2,
                ended_at_ms: 3,
                traffic_sample: sample(8),
            },
        ];

        let responses = control.report_proxy_traffic(&proxy, reports).await.unwrap();
        assert_eq!(responses[0].result, ProxyTrafficReportApplyResult::Applied);
        assert_eq!(responses[1].result, ProxyTrafficReportApplyResult::Rejected);
        assert_eq!(store.state.lock().unwrap().proxy_report_writes, 1);
    }

    #[tokio::test]
    async fn invalid_assignment_is_replaced_or_cleared() {
        let old = pn_info(&node(8));
        let replacement = pn_info(&node(9));
        let store = TestStore::default();
        store.state.lock().unwrap().assignments.insert(7, Some(old.clone()));
        let selector = TestSelector::new(true, Some(replacement.clone()));
        let control = test_control(store.clone(), selector.clone());

        let selected = control
            .resolve_node_network_pn_server(7, Some(&old))
            .await
            .unwrap();
        assert_eq!(selected, Some(replacement.clone()));
        assert_eq!(
            store.state.lock().unwrap().assignments.get(&7),
            Some(&Some(replacement))
        );

        *selector.selected.lock().unwrap() = None;
        let old = pn_info(&node(8));
        let selected = control
            .resolve_node_network_pn_server(7, Some(&old))
            .await
            .unwrap();
        assert_eq!(selected, None);
        assert_eq!(store.state.lock().unwrap().assignments.get(&7), Some(&None));
    }
}
