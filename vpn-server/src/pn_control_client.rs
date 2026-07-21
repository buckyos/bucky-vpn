use crate::pn_control_server::proxy_control_purpose;
use crate::pn_traffic_service::PnTrafficReporter;
use crate::server_config::PnControlServerConfig;
use crate::sqlite_store_factory::VpnServerRef;
use p2p_frame::endpoint::Endpoint;
use p2p_frame::networks::{IncomingTunnelValidateContext, IncomingTunnelValidator, ValidateResult};
use p2p_frame::p2p_identity::P2pId;
use p2p_frame::pn::{PnConnectionValidateContext, PnConnectionValidator};
use p2p_frame::sn::types::{SnTunnelClassification, SnTunnelRead, SnTunnelWrite};
use p2p_frame::ttp::{TtpClientRef, TtpConnector, TtpTarget};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use vpn_frame::{PnServerInfo, VpnCmdPkgLen};
use vpn_frame::client::VpnServerClient;
use vpn_frame::cmd_server::client::{
    ClassifiedClientSendGuard, ClassifiedCmdSend, ClassifiedCmdTunnel, ClassifiedCmdTunnelFactory,
    DefaultClassifiedCmdClient,
};
use vpn_frame::cmd_server::errors::{CmdErrorCode, CmdResult, into_cmd_err};
use vpn_frame::control_channel::{
    VpnCmdPnConnectionValidatorCore, VpnCmdPnTrafficReporter as CoreVpnCmdPnTrafficReporter,
    VpnControlClientOpsRef, into_control_client_ops,
};
use vpn_frame::errors::{VpnErrorCode, VpnResult, into_vpn_err};
use vpn_frame::server::NodeId;

pub type P2pControlCmdSend =
    ClassifiedCmdSend<SnTunnelClassification, (), SnTunnelRead, SnTunnelWrite, VpnCmdPkgLen, u8>;
pub type P2pControlCmdSendGuard = ClassifiedClientSendGuard<
    SnTunnelClassification,
    (),
    SnTunnelRead,
    SnTunnelWrite,
    ControlCmdTunnelFactory,
    VpnCmdPkgLen,
    u8,
>;
pub type ControlCmdClient = DefaultClassifiedCmdClient<
    SnTunnelClassification,
    (),
    SnTunnelRead,
    SnTunnelWrite,
    ControlCmdTunnelFactory,
    VpnCmdPkgLen,
    u8,
>;
pub type VpnControlClient =
    VpnServerClient<(), P2pControlCmdSend, P2pControlCmdSendGuard, ControlCmdClient>;
pub type VpnControlClientRef = Arc<VpnControlClient>;

pub struct ControlCmdTunnelFactory {
    ttp_client: TtpClientRef,
    control_id: P2pId,
    control_name: Option<String>,
    control_endpoint: Endpoint,
}

impl ControlCmdTunnelFactory {
    fn new(
        ttp_client: TtpClientRef,
        control_id: P2pId,
        control_name: Option<String>,
        control_endpoint: Endpoint,
    ) -> Self {
        Self {
            ttp_client,
            control_id,
            control_name,
            control_endpoint,
        }
    }

    async fn open_cmd_tunnel(
        &self,
        local_ep: Option<&Endpoint>,
    ) -> CmdResult<ClassifiedCmdTunnel<SnTunnelRead, SnTunnelWrite>> {
        let purpose = proxy_control_purpose().map_err(into_cmd_err!(
            CmdErrorCode::Failed,
            "encode control cmd purpose failed"
        ))?;
        let target = TtpTarget {
            local_ep: local_ep.copied(),
            remote_ep: self.control_endpoint,
            remote_id: self.control_id.clone(),
            remote_name: self
                .control_name
                .clone()
                .or_else(|| Some(self.control_id.to_string())),
        };
        self.ttp_client
            .connect_server(target.clone())
            .await
            .map_err(into_cmd_err!(
                CmdErrorCode::Failed,
                "connect control ttp server failed"
            ))?;
        let (meta, read, write) = self
            .ttp_client
            .open_control_stream(&target, purpose)
            .await
            .map_err(into_cmd_err!(
                CmdErrorCode::Failed,
                "open control cmd stream failed"
            ))?;
        let local = meta
            .local_ep
            .unwrap_or(local_ep.copied().unwrap_or_default());
        let remote = meta.remote_ep.unwrap_or(self.control_endpoint);
        let local_id = meta.local_id;
        let remote_id = meta.remote_id;
        Ok(ClassifiedCmdTunnel::new(
            SnTunnelRead::new(read, local, remote, local_id.clone(), remote_id.clone()),
            SnTunnelWrite::new(write, local, remote, local_id, remote_id),
        ))
    }
}

#[async_trait::async_trait]
impl ClassifiedCmdTunnelFactory<SnTunnelClassification, (), SnTunnelRead, SnTunnelWrite>
    for ControlCmdTunnelFactory
{
    async fn create_tunnel(
        &self,
        classification: Option<SnTunnelClassification>,
    ) -> CmdResult<ClassifiedCmdTunnel<SnTunnelRead, SnTunnelWrite>> {
        self.open_cmd_tunnel(
            classification
                .as_ref()
                .and_then(|classification| classification.local_ep.as_ref()),
        )
        .await
    }
}

pub async fn create_vpn_control_client(
    ttp_client: TtpClientRef,
    control_server: &PnControlServerConfig,
    conn_timeout: Duration,
) -> VpnResult<VpnControlClientRef> {
    let control_id =
        P2pId::from_str(&control_server.id).map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
    let factory = ControlCmdTunnelFactory::new(
        ttp_client,
        control_id,
        control_server.name.clone(),
        control_server.endpoint.clone(),
    );
    Ok(VpnServerClient::new(
        ControlCmdClient::new(factory, 1),
        conn_timeout,
    ))
}

pub struct VpnCmdPnTrafficReporter {
    core: Arc<CoreVpnCmdPnTrafficReporter>,
}

impl VpnCmdPnTrafficReporter {
    pub fn new(client: VpnControlClientRef, pn_server: PnServerInfo) -> Arc<Self> {
        Arc::new(Self {
            core: CoreVpnCmdPnTrafficReporter::new(into_control_client_ops(client), pn_server),
        })
    }
}

#[async_trait::async_trait]
impl PnTrafficReporter for VpnCmdPnTrafficReporter {
    async fn report_heartbeat(&self) -> VpnResult<()> {
        self.core.report_heartbeat().await
    }

    async fn report_node_traffic(
        &self,
        reports: Vec<vpn_frame::NodeTrafficReport>,
    ) -> VpnResult<Vec<vpn_frame::NodeTrafficReportResp>> {
        self.core.report_node_traffic(reports).await
    }
}

pub struct LocalPnTrafficReporter {
    vpn_server: VpnServerRef,
    pn_node_id: NodeId,
}

impl LocalPnTrafficReporter {
    pub fn new(vpn_server: VpnServerRef, pn_node_id: NodeId) -> Arc<Self> {
        Arc::new(Self {
            vpn_server,
            pn_node_id,
        })
    }
}

#[async_trait::async_trait]
impl PnTrafficReporter for LocalPnTrafficReporter {
    async fn report_heartbeat(&self) -> VpnResult<()> {
        Ok(())
    }

    async fn report_node_traffic(
        &self,
        reports: Vec<vpn_frame::NodeTrafficReport>,
    ) -> VpnResult<Vec<vpn_frame::NodeTrafficReportResp>> {
        self.vpn_server
            .report_node_traffic_from_pn_node(&self.pn_node_id, reports)
            .await
    }
}

pub struct VpnCmdPnConnectionValidator {
    core: Arc<VpnCmdPnConnectionValidatorCore>,
}

pub struct DeferredVpnCmdIncomingTunnelValidator {
    client: Mutex<Option<VpnControlClientOpsRef>>,
}

impl VpnCmdPnConnectionValidator {
    pub fn new(client: VpnControlClientRef) -> Arc<Self> {
        let client = into_control_client_ops(client);
        Arc::new(Self {
            core: VpnCmdPnConnectionValidatorCore::new(client),
        })
    }
}

#[cfg(test)]
#[path = "pn_control_client_tests.rs"]
mod pn_control_client_tests;

impl DeferredVpnCmdIncomingTunnelValidator {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            client: Mutex::new(None),
        })
    }

    pub fn set_client(&self, client: VpnControlClientRef) {
        *self.client.lock().unwrap() = Some(into_control_client_ops(client));
    }
}

#[async_trait::async_trait]
impl IncomingTunnelValidator for DeferredVpnCmdIncomingTunnelValidator {
    async fn validate(
        &self,
        ctx: &IncomingTunnelValidateContext,
    ) -> p2p_frame::error::P2pResult<ValidateResult> {
        let client = self.client.lock().unwrap().clone();
        let Some(client) = client else {
            return Ok(ValidateResult::Reject(format!(
                "incoming tunnel rejected because remote validation is unavailable remote={} local={}",
                ctx.remote_id, ctx.local_id
            )));
        };
        validate_incoming_tunnel_with_control_client(&client, ctx).await
    }
}

async fn validate_incoming_tunnel_with_control_client(
    client: &VpnControlClientOpsRef,
    ctx: &IncomingTunnelValidateContext,
) -> p2p_frame::error::P2pResult<ValidateResult> {
    let remote_node_id = NodeId::from(ctx.remote_id.as_slice());
    let network_id = client
        .validate_pn_connection(remote_node_id.clone(), remote_node_id)
        .await
        .map_err(|err| {
            p2p_frame::error::p2p_err!(
                p2p_frame::error::P2pErrorCode::InternalError,
                "validate incoming tunnel by vpn server failed: code={:?} msg={}",
                err.code(),
                err.msg()
            )
        })?;
    if network_id.is_some() {
        Ok(ValidateResult::Accept)
    } else {
        Ok(ValidateResult::Reject(format!(
            "incoming tunnel rejected by vpn server remote={} local={}",
            ctx.remote_id, ctx.local_id
        )))
    }
}

#[async_trait::async_trait]
impl PnConnectionValidator for VpnCmdPnConnectionValidator {
    async fn validate(
        &self,
        ctx: &PnConnectionValidateContext,
    ) -> p2p_frame::error::P2pResult<ValidateResult> {
        let network_id = self.validate_network(ctx).await?;
        Ok(if network_id.is_some() {
            ValidateResult::Accept
        } else {
            ValidateResult::Reject(format!(
                "pn connection rejected by vpn server from={} to={}",
                ctx.from, ctx.to
            ))
        })
    }
}

impl VpnCmdPnConnectionValidator {
    async fn validate_network(
        &self,
        ctx: &PnConnectionValidateContext,
    ) -> p2p_frame::error::P2pResult<Option<u64>> {
        let validation = self
            .core
            .validate(NodeId::from(ctx.from.as_slice()), NodeId::from(ctx.to.as_slice()))
            .await
            .map_err(|err| {
                p2p_frame::error::p2p_err!(
                    p2p_frame::error::P2pErrorCode::InternalError,
                    "validate pn connection by vpn server failed: code={:?} msg={}",
                    err.code(),
                    err.msg()
                )
            })?;
        Ok(validation.map(|validation| validation.network_id))
    }
}
