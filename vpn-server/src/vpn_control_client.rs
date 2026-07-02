use crate::pn_traffic_service::{PnTrafficNodeTrackerRef, PnTrafficReporter};
use crate::server_config::{ConfigPnServerSelectorRef, PnControlServerConfig};
use crate::sqlite_store_factory::{PersistedTrafficStats, SqliteStoreFactory, VpnServerRef};
use bucky_raw_codec::{RawConvertTo, RawFrom};
use p2p_frame::cmd_server::server::{CmdServer, CmdTunnelService, DefaultCmdServerService};
use p2p_frame::cmd_server::{CmdBody, CmdTunnel, PeerId};
use p2p_frame::endpoint::{Endpoint, Protocol};
use p2p_frame::networks::{
    IncomingTunnelValidateContext, IncomingTunnelValidator, TunnelPurpose, ValidateResult,
};
use p2p_frame::p2p_identity::{P2pId, P2pIdentityRef};
use p2p_frame::pn::{PnConnectionValidateContext, PnConnectionValidator};
use p2p_frame::sn::types::{SnTunnelClassification, SnTunnelRead, SnTunnelWrite};
use p2p_frame::stack::{P2pConfig, create_p2p_env};
use p2p_frame::ttp::{
    TtpClient, TtpClientRef, TtpConnector, TtpPortListener, TtpServerRef, TtpStreamMeta, TtpTarget,
};
use p2p_frame::x509::{X509IdentityCertFactory, X509IdentityFactory};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use vpn_frame::client::VpnServerClient;
use vpn_frame::cmd_server::client::{
    ClassifiedClientSendGuard, ClassifiedCmdSend, ClassifiedCmdTunnel, ClassifiedCmdTunnelFactory,
    DefaultClassifiedCmdClient,
};
use vpn_frame::cmd_server::errors::{CmdErrorCode, CmdResult, into_cmd_err};
use vpn_frame::errors::{VpnErrorCode, VpnResult, into_vpn_err, vpn_err};
use vpn_frame::server::{NodeId, PnServerSelector, VpnStore, VpnStoreFactory};
use vpn_frame::{
    PnServerInfo, ReportPnTrafficStatsReq, ReportPnTrafficStatsResp, ValidatePnConnectionReq,
    ValidatePnConnectionResp, VpnCmdCode, VpnCmdHeader, VpnTunnelId,
};

const PROXY_CONTROL_SERVICE: &str = "vpn_proxy_control";

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
pub type ProxyControlCmdService = DefaultCmdServerService<(), SnTunnelRead, SnTunnelWrite, u16, u8>;
pub type ProxyControlCmdServiceRef = Arc<ProxyControlCmdService>;

pub fn proxy_control_purpose() -> p2p_frame::error::P2pResult<TunnelPurpose> {
    TunnelPurpose::from_value(&PROXY_CONTROL_SERVICE.to_string())
}

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
        let purpose = proxy_control_purpose().map_err(into_cmd_err!(
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

pub fn create_proxy_control_cmd_service(
    vpn_server: VpnServerRef,
    store_factory: Arc<SqliteStoreFactory>,
    pn_server_selector: ConfigPnServerSelectorRef,
) -> ProxyControlCmdServiceRef {
    let service = ProxyControlCmdService::new();
    register_proxy_control_traffic_handler(
        &service,
        store_factory.clone(),
        pn_server_selector.clone(),
    );
    register_proxy_control_connection_handler(&service, vpn_server);
    service
}

fn register_proxy_control_traffic_handler(
    service: &ProxyControlCmdServiceRef,
    store_factory: Arc<SqliteStoreFactory>,
    pn_server_selector: ConfigPnServerSelectorRef,
) {
    service.register_cmd_handler(
        VpnCmdCode::ReportPnTrafficStats as u8,
        move |_local_id: PeerId,
              peer_id: PeerId,
              _tunnel_id: VpnTunnelId,
              _header: VpnCmdHeader,
              mut body: CmdBody| {
            let store_factory = store_factory.clone();
            let pn_server_selector = pn_server_selector.clone();
            async move {
                let data = body.read_all().await?;
                let req = ReportPnTrafficStatsReq::clone_from_slice(data.as_slice())
                    .map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?;
                let seq = req.seq;
                let resp = match handle_proxy_control_traffic_report(
                    store_factory,
                    pn_server_selector,
                    peer_id,
                    req,
                )
                .await
                {
                    Ok(()) => ReportPnTrafficStatsResp { seq, result: 0 },
                    Err(err) => {
                        log::error!(
                            "handle proxy control traffic report failed: code={:?} msg={}",
                            err.code(),
                            err.msg()
                        );
                        ReportPnTrafficStatsResp {
                            seq,
                            result: err.code() as u8,
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

fn register_proxy_control_connection_handler(
    service: &ProxyControlCmdServiceRef,
    vpn_server: VpnServerRef,
) {
    service.register_cmd_handler(
        VpnCmdCode::ValidatePnConnection as u8,
        move |_local_id: PeerId,
              peer_id: PeerId,
              _tunnel_id: VpnTunnelId,
              _header: VpnCmdHeader,
              mut body: CmdBody| {
            let vpn_server = vpn_server.clone();
            async move {
                let data = body.read_all().await?;
                let req = ValidatePnConnectionReq::clone_from_slice(data.as_slice())
                    .map_err(into_cmd_err!(CmdErrorCode::RawCodecError))?;
                let seq = req.seq;
                let pn_node_id = NodeId::from(peer_id.as_slice());
                let resp = match vpn_server
                    .validate_pn_connection_from_pn_node(&pn_node_id, &req.from, &req.to)
                    .await
                {
                    Ok(allowed) => ValidatePnConnectionResp {
                        seq,
                        result: 0,
                        allowed,
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

async fn handle_proxy_control_traffic_report(
    store_factory: Arc<SqliteStoreFactory>,
    pn_server_selector: ConfigPnServerSelectorRef,
    peer_id: PeerId,
    req: ReportPnTrafficStatsReq,
) -> VpnResult<()> {
    let pn_node_id = NodeId::from(peer_id.as_slice());
    let pn_server = req.pn_server.as_ref().ok_or_else(|| {
        vpn_err!(
            VpnErrorCode::InvalidParam,
            "proxy control traffic report missing pn_server"
        )
    })?;
    if !pn_server_selector
        .matches_pn_node(pn_server, &pn_node_id)
        .await?
    {
        return Err(vpn_err!(
            VpnErrorCode::InvalidParam,
            "proxy control traffic report peer {} does not match pn_server {}",
            pn_node_id.to_base36(),
            pn_server.id
        ));
    }
    pn_server_selector.report_heartbeat(pn_server).await?;
    let mut store = store_factory.get_vpn_store().await?;
    store
        .add_pn_traffic_delta(&req.node_id, req.tx_bytes, req.rx_bytes)
        .await
}

pub async fn register_proxy_control_cmd_listener(
    ttp_server: TtpServerRef,
    cmd_service: ProxyControlCmdServiceRef,
    pn_server_selector: ConfigPnServerSelectorRef,
) -> VpnResult<()> {
    let purpose = proxy_control_purpose().map_err(into_vpn_err!(VpnErrorCode::Failed))?;
    ttp_server
        .listen_control_stream(
            purpose,
            Arc::new(move |accepted| {
                let cmd_service = cmd_service.clone();
                let pn_server_selector = pn_server_selector.clone();
                Box::pin(async move {
                    let accepted = match accepted {
                        Ok(accepted) => accepted,
                        Err(err) => {
                            log::warn!("proxy control accept stopped: {:?}", err);
                            return;
                        }
                    };
                    let (meta, read, write) = accepted;
                    let Some(remote_ep) = meta.remote_ep else {
                        log::warn!(
                            "proxy control connection rejected because remote endpoint is missing remote_id={}",
                            meta.remote_id
                        );
                        return;
                    };
                    let pn_server = PnServerInfo::new(
                        meta.remote_id.to_string(),
                        remote_ep.addr().ip(),
                        remote_ep.addr().port(),
                    );
                    if let Err(err) = PnServerSelector::report_heartbeat(
                        pn_server_selector.as_ref(),
                        &pn_server,
                    )
                    .await
                    {
                        log::error!(
                            "proxy control connection rejected because proxy heartbeat registration failed remote_id={} code={:?} msg={}",
                            meta.remote_id,
                            err.code(),
                            err.msg()
                        );
                        return;
                    }
                    let tunnel = into_cmd_tunnel(meta, read, write, remote_ep);
                    tokio::spawn(async move {
                        if let Err(err) = cmd_service.handle_tunnel(tunnel).await {
                            log::error!("proxy control command tunnel failed: {:?}", err);
                        }
                    });
                })
            }),
        )
        .await
        .map_err(into_vpn_err!(VpnErrorCode::Failed))
}

fn into_cmd_tunnel(
    meta: TtpStreamMeta,
    read: p2p_frame::networks::TunnelStreamRead,
    write: p2p_frame::networks::TunnelStreamWrite,
    remote_ep: Endpoint,
) -> CmdTunnel<SnTunnelRead, SnTunnelWrite> {
    let local = meta.local_ep.unwrap_or_default();
    let remote = remote_ep;
    let local_id = meta.local_id;
    let remote_id = meta.remote_id;
    CmdTunnel::new(
        SnTunnelRead::new(read, local, remote, local_id.clone(), remote_id.clone()),
        SnTunnelWrite::new(write, local, remote, local_id, remote_id),
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_control_purpose_is_dedicated() {
        let proxy_control = proxy_control_purpose().unwrap();
        let sn_command = TunnelPurpose::from_value(&"sn_service".to_string()).unwrap();

        assert_ne!(proxy_control, sn_command);
        assert_eq!(
            proxy_control,
            TunnelPurpose::from_value(&PROXY_CONTROL_SERVICE.to_string()).unwrap()
        );
    }

    #[test]
    fn proxy_control_cmd_service_is_independent_from_sn_service() {
        let service = ProxyControlCmdService::new();
        let service_type = std::any::type_name::<ProxyControlCmdService>();

        assert_eq!(Arc::strong_count(&service), 1);
        assert!(service_type.contains("DefaultCmdServerService"));
        assert!(!service_type.contains("SnService"));
    }
}
