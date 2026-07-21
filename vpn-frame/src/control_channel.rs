use crate::errors::{VpnErrorCode, VpnResult, into_vpn_err, vpn_err};
use crate::server::NodeId;
use crate::{
    NodeTrafficReport, NodeTrafficReportResp, PnServerInfo, ProxyNodeHeartbeat, ProxyNodeHeartbeatId,
    ProxyTrafficReport, ProxyTrafficReportResp, ValidatedPnConnection,
};
use async_trait::async_trait;
use sfo_cmd_server::errors::CmdResult;
use std::sync::Arc;

#[async_trait]
pub trait VpnControlClientOps: Send + Sync + 'static {
    async fn report_pn_traffic_stats(
        &self,
        reports: Vec<NodeTrafficReport>,
    ) -> VpnResult<Vec<NodeTrafficReportResp>>;

    async fn report_proxy_heartbeat(&self, heartbeat: ProxyNodeHeartbeat) -> VpnResult<()>;

    async fn report_proxy_traffic(
        &self,
        reports: Vec<ProxyTrafficReport>,
    ) -> VpnResult<Vec<ProxyTrafficReportResp>>;

    async fn validate_pn_connection(
        &self,
        from: NodeId,
        to: NodeId,
    ) -> VpnResult<Option<ValidatedPnConnection>>;
}

pub type VpnControlClientOpsRef = Arc<dyn VpnControlClientOps>;

pub struct VpnCmdPnTrafficReporter {
    client: VpnControlClientOpsRef,
    pn_server: PnServerInfo,
    heartbeat_seq: std::sync::atomic::AtomicU64,
}

impl VpnCmdPnTrafficReporter {
    pub fn new(client: VpnControlClientOpsRef, pn_server: PnServerInfo) -> Arc<Self> {
        Arc::new(Self {
            client,
            pn_server,
            heartbeat_seq: std::sync::atomic::AtomicU64::new(0),
        })
    }

    pub async fn report_node_traffic(
        &self,
        reports: Vec<NodeTrafficReport>,
    ) -> VpnResult<Vec<NodeTrafficReportResp>> {
        self.client.report_pn_traffic_stats(reports).await
    }

    pub async fn report_heartbeat(&self) -> VpnResult<()> {
        let seq = self
            .heartbeat_seq
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.client
            .report_proxy_heartbeat(ProxyNodeHeartbeat {
                heartbeat_id: ProxyNodeHeartbeatId(format!("{}:{}", self.pn_server.id, seq)),
                pn_server: Some(self.pn_server.clone()),
            })
            .await
    }

    pub async fn report_proxy_traffic(
        &self,
        reports: Vec<ProxyTrafficReport>,
    ) -> VpnResult<Vec<ProxyTrafficReportResp>> {
        self.client.report_proxy_traffic(reports).await
    }
}

pub struct VpnCmdPnConnectionValidatorCore {
    client: VpnControlClientOpsRef,
}

impl VpnCmdPnConnectionValidatorCore {
    pub fn new(client: VpnControlClientOpsRef) -> Arc<Self> {
        Arc::new(Self { client })
    }

    pub async fn validate(
        &self,
        from: NodeId,
        to: NodeId,
    ) -> VpnResult<Option<ValidatedPnConnection>> {
        self.client.validate_pn_connection(from, to).await
    }
}

pub async fn validate_remote_pn_connection(
    client: Option<VpnControlClientOpsRef>,
    from: NodeId,
    to: NodeId,
) -> VpnResult<Option<ValidatedPnConnection>> {
    let client = client.ok_or_else(|| {
        vpn_err!(
            VpnErrorCode::Failed,
            "remote vpn control validation is unavailable"
        )
    })?;
    client.validate_pn_connection(from, to).await
}

pub fn into_control_client_ops<C>(client: Arc<C>) -> VpnControlClientOpsRef
where
    C: VpnControlClientOps,
{
    client
}

pub fn into_vpn_result<T>(result: CmdResult<T>) -> VpnResult<T> {
    result.map_err(into_vpn_err!(VpnErrorCode::IoError))
}
