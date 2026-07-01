use crate::pn_traffic_service::{PnTrafficNodeTrackerRef, PnTrafficReporter};
use crate::server_config::PnControlServerConfig;
use crate::sqlite_store_factory::PersistedTrafficStats;
use p2p_frame::endpoint::{Endpoint, Protocol};
use p2p_frame::networks::{IncomingTunnelValidateContext, IncomingTunnelValidator, ValidateResult};
use p2p_frame::p2p_identity::{P2pId, P2pIdentityRef};
use p2p_frame::pn::{PnConnectionValidateContext, PnConnectionValidator};
use p2p_frame::sn::types::{SnTunnelClassification, SnTunnelRead, SnTunnelWrite, sn_cmd_purpose};
use p2p_frame::stack::{P2pConfig, create_p2p_env};
use p2p_frame::ttp::{TtpClient, TtpClientRef, TtpConnector, TtpTarget};
use p2p_frame::x509::{X509IdentityCertFactory, X509IdentityFactory};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use vpn_frame::PnServerInfo;
use vpn_frame::client::VpnServerClient;
use vpn_frame::cmd_server::client::{
    ClassifiedClientSendGuard, ClassifiedCmdSend, ClassifiedCmdTunnel, ClassifiedCmdTunnelFactory,
    DefaultClassifiedCmdClient,
};
use vpn_frame::cmd_server::errors::{CmdErrorCode, CmdResult, into_cmd_err};
use vpn_frame::errors::{VpnErrorCode, VpnResult, into_vpn_err};
use vpn_frame::server::NodeId;

pub type P2pControlCmdSend =
    ClassifiedCmdSend<SnTunnelClassification, (), SnTunnelRead, SnTunnelWrite, u16, u8>;
pub type P2pControlCmdSendGuard = ClassifiedClientSendGuard<
    SnTunnelClassification,
    (),
    SnTunnelRead,
    SnTunnelWrite,
    ControlCmdTunnelFactory,
    u16,
    u8,
>;
pub type ControlCmdClient = DefaultClassifiedCmdClient<
    SnTunnelClassification,
    (),
    SnTunnelRead,
    SnTunnelWrite,
    ControlCmdTunnelFactory,
    u16,
    u8,
>;
pub type VpnControlClient =
    VpnServerClient<(), P2pControlCmdSend, P2pControlCmdSendGuard, ControlCmdClient>;
pub type VpnControlClientRef = Arc<VpnControlClient>;

pub struct ControlCmdTunnelFactory {
    ttp_client: TtpClientRef,
    control_id: P2pId,
    control_endpoint: Endpoint,
}

impl ControlCmdTunnelFactory {
    fn new(ttp_client: TtpClientRef, control_id: P2pId, control_endpoint: Endpoint) -> Self {
        Self {
            ttp_client,
            control_id,
            control_endpoint,
        }
    }

    async fn open_cmd_tunnel(
        &self,
        local_ep: Option<&Endpoint>,
    ) -> CmdResult<ClassifiedCmdTunnel<SnTunnelRead, SnTunnelWrite>> {
        let purpose = sn_cmd_purpose().map_err(into_cmd_err!(
            CmdErrorCode::Failed,
            "encode control cmd purpose failed"
        ))?;
        let target = TtpTarget {
            local_ep: local_ep.copied(),
            remote_ep: self.control_endpoint,
            remote_id: self.control_id.clone(),
            remote_name: Some(self.control_id.to_string()),
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
    local_identity: P2pIdentityRef,
    control_server: &PnControlServerConfig,
    conn_timeout: Duration,
) -> VpnResult<VpnControlClientRef> {
    let control_endpoint = Endpoint::from((
        Protocol::Quic,
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)),
    ));
    let p2p_config = P2pConfig::new(
        Arc::new(X509IdentityFactory),
        Arc::new(X509IdentityCertFactory),
        vec![control_endpoint],
    );
    let p2p_env = create_p2p_env(p2p_config)
        .await
        .map_err(into_vpn_err!(VpnErrorCode::Failed))?;
    let control_id =
        P2pId::from_str(&control_server.id).map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?;
    p2p_env
        .net_manager()
        .add_listen_device(local_identity.clone())
        .await
        .map_err(into_vpn_err!(VpnErrorCode::Failed))?;
    p2p_env
        .net_manager()
        .listen(p2p_env.endpoints(), p2p_env.port_mapping().clone())
        .await
        .map_err(into_vpn_err!(VpnErrorCode::Failed))?;
    let ttp_client = TtpClient::new(local_identity, p2p_env.net_manager().clone());
    let factory =
        ControlCmdTunnelFactory::new(ttp_client, control_id, control_server.endpoint.clone());
    Ok(VpnServerClient::new(
        ControlCmdClient::new(factory, 1),
        conn_timeout,
    ))
}

pub struct VpnCmdPnTrafficReporter {
    client: VpnControlClientRef,
    pn_server: PnServerInfo,
}

impl VpnCmdPnTrafficReporter {
    pub fn new(client: VpnControlClientRef, pn_server: PnServerInfo) -> Arc<Self> {
        Arc::new(Self { client, pn_server })
    }
}

#[async_trait::async_trait]
impl PnTrafficReporter for VpnCmdPnTrafficReporter {
    async fn report_delta(&self, node_id: &NodeId, delta: PersistedTrafficStats) -> VpnResult<()> {
        self.client
            .report_pn_traffic_stats(
                node_id.clone(),
                Some(self.pn_server.clone()),
                delta.tx_bytes,
                delta.rx_bytes,
            )
            .await
    }
}

pub struct VpnCmdPnConnectionValidator {
    client: VpnControlClientRef,
    traffic_node_tracker: Option<PnTrafficNodeTrackerRef>,
}

pub struct VpnCmdIncomingTunnelValidator {
    client: VpnControlClientRef,
}

struct RejectAllIncomingTunnelValidator;

impl VpnCmdPnConnectionValidator {
    pub fn new_with_traffic_node_tracker(
        client: VpnControlClientRef,
        traffic_node_tracker: PnTrafficNodeTrackerRef,
    ) -> Arc<Self> {
        Arc::new(Self {
            client,
            traffic_node_tracker: Some(traffic_node_tracker),
        })
    }
}

impl VpnCmdIncomingTunnelValidator {
    pub fn new(client: VpnControlClientRef) -> Arc<Self> {
        Arc::new(Self { client })
    }
}

pub fn reject_all_incoming_tunnel_validator() -> p2p_frame::networks::IncomingTunnelValidatorRef {
    Arc::new(RejectAllIncomingTunnelValidator)
}

#[async_trait::async_trait]
impl IncomingTunnelValidator for RejectAllIncomingTunnelValidator {
    async fn validate(
        &self,
        ctx: &IncomingTunnelValidateContext,
    ) -> p2p_frame::error::P2pResult<ValidateResult> {
        Ok(ValidateResult::Reject(format!(
            "incoming tunnel rejected because remote validation is unavailable remote={} local={}",
            ctx.remote_id, ctx.local_id
        )))
    }
}

#[async_trait::async_trait]
impl IncomingTunnelValidator for VpnCmdIncomingTunnelValidator {
    async fn validate(
        &self,
        ctx: &IncomingTunnelValidateContext,
    ) -> p2p_frame::error::P2pResult<ValidateResult> {
        let remote_node_id = NodeId::from(ctx.remote_id.as_slice());
        let allowed = self
            .client
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
        if allowed {
            Ok(ValidateResult::Accept)
        } else {
            Ok(ValidateResult::Reject(format!(
                "incoming tunnel rejected by vpn server remote={} local={}",
                ctx.remote_id, ctx.local_id
            )))
        }
    }
}

#[async_trait::async_trait]
impl PnConnectionValidator for VpnCmdPnConnectionValidator {
    async fn validate(
        &self,
        ctx: &PnConnectionValidateContext,
    ) -> p2p_frame::error::P2pResult<ValidateResult> {
        let allowed = self
            .client
            .validate_pn_connection(
                NodeId::from(ctx.from.as_slice()),
                NodeId::from(ctx.to.as_slice()),
            )
            .await
            .map_err(|err| {
                p2p_frame::error::p2p_err!(
                    p2p_frame::error::P2pErrorCode::InternalError,
                    "validate pn connection by vpn server failed: code={:?} msg={}",
                    err.code(),
                    err.msg()
                )
            })?;
        if allowed {
            if let Some(tracker) = &self.traffic_node_tracker {
                tracker.track_node(NodeId::from(ctx.from.as_slice()));
                tracker.track_node(NodeId::from(ctx.to.as_slice()));
            }
            Ok(ValidateResult::Accept)
        } else {
            Ok(ValidateResult::Reject(format!(
                "pn connection rejected by vpn server from={} to={}",
                ctx.from, ctx.to
            )))
        }
    }
}
