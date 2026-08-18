use crate::pn_server_info::{
    PnServerEndpoint as VpnEndpoint, PnServerInfoPayload, encode_pn_server_info,
};
use p2p_frame::cmd_server::server::{CmdTunnelService, DefaultCmdServerService};
use p2p_frame::cmd_server::{CmdTunnel, PeerId, TunnelId};
use p2p_frame::endpoint::{Endpoint, Protocol};
use p2p_frame::networks::TunnelPurpose;
use p2p_frame::sn::types::{SnTunnelRead, SnTunnelWrite};
use p2p_frame::ttp::{TtpPortListener, TtpServerRef, TtpStreamMeta};
use std::sync::Arc;
use std::time::Duration;
use vpn_frame::PnServerInfo;
use vpn_frame::errors::{VpnErrorCode, VpnResult, into_vpn_err};
use vpn_frame::server::{NodeId, PnControlTunnelObserver, VpnControlCmdPkgLen};

const PROXY_CONTROL_SERVICE: &str = "vpn_proxy_control";
const PROXY_CONTROL_TUNNEL_LOOKUP_ATTEMPTS: usize = 20;
const PROXY_CONTROL_TUNNEL_LOOKUP_INTERVAL: Duration = Duration::from_millis(5);

pub type ProxyControlCmdService =
    DefaultCmdServerService<(), SnTunnelRead, SnTunnelWrite, VpnControlCmdPkgLen, u8>;
pub type ProxyControlCmdServiceRef = Arc<ProxyControlCmdService>;

pub fn proxy_control_purpose() -> p2p_frame::error::P2pResult<TunnelPurpose> {
    TunnelPurpose::from_value(&PROXY_CONTROL_SERVICE.to_string())
}

pub fn create_proxy_control_cmd_service() -> ProxyControlCmdServiceRef {
    ProxyControlCmdService::new()
}

struct ProxyControlTunnelObserver {
    cmd_service: ProxyControlCmdServiceRef,
}

#[async_trait::async_trait]
trait ProxyControlTunnelLookup: Send + Sync {
    async fn remote_endpoint(&self, peer_id: &PeerId, tunnel_id: TunnelId) -> Option<Endpoint>;
}

#[async_trait::async_trait]
impl ProxyControlTunnelLookup for ProxyControlCmdService {
    async fn remote_endpoint(&self, peer_id: &PeerId, tunnel_id: TunnelId) -> Option<Endpoint> {
        let connection = self
            .get_peer_tunnels(peer_id)
            .await
            .into_iter()
            .find(|connection| connection.conn_id == tunnel_id)?;
        let remote_ep = {
            let writer = connection.send.get().await;
            writer.remote()
        };
        Some(remote_ep)
    }
}

async fn observe_proxy_control_tunnel_with_retry<L>(
    lookup: &L,
    pn_node_id: &NodeId,
    tunnel_id: TunnelId,
    max_attempts: usize,
    retry_interval: Duration,
) -> VpnResult<Option<PnServerInfo>>
where
    L: ProxyControlTunnelLookup + ?Sized,
{
    let peer_id = PeerId::from(pn_node_id.as_slice());
    let max_attempts = max_attempts.max(1);
    for attempt in 0..max_attempts {
        if let Some(remote_ep) = lookup.remote_endpoint(&peer_id, tunnel_id).await {
            let pn_server = encode_pn_server_info(
                pn_node_id.to_base36(),
                PnServerInfoPayload::new_with_primary_address(
                    pn_endpoint_from_p2p_endpoint(remote_ep),
                    Vec::new(),
                ),
            )?;
            return Ok(Some(pn_server));
        }

        if attempt + 1 < max_attempts {
            if retry_interval.is_zero() {
                tokio::task::yield_now().await;
            } else {
                tokio::time::sleep(retry_interval).await;
            }
        }
    }

    log::debug!(
        "proxy control tunnel observation unavailable peer={} tunnel={:?} attempts={}",
        pn_node_id.to_base36(),
        tunnel_id,
        max_attempts
    );
    Ok(None)
}

#[async_trait::async_trait]
impl PnControlTunnelObserver for ProxyControlTunnelObserver {
    async fn observe(
        &self,
        pn_node_id: &NodeId,
        tunnel_id: TunnelId,
    ) -> VpnResult<Option<PnServerInfo>> {
        observe_proxy_control_tunnel_with_retry(
            self.cmd_service.as_ref(),
            pn_node_id,
            tunnel_id,
            PROXY_CONTROL_TUNNEL_LOOKUP_ATTEMPTS,
            PROXY_CONTROL_TUNNEL_LOOKUP_INTERVAL,
        )
        .await
    }
}

pub fn create_proxy_control_tunnel_observer(
    cmd_service: ProxyControlCmdServiceRef,
) -> Arc<dyn PnControlTunnelObserver> {
    Arc::new(ProxyControlTunnelObserver { cmd_service })
}

pub async fn register_proxy_control_cmd_listener(
    ttp_server: TtpServerRef,
    cmd_service: ProxyControlCmdServiceRef,
) -> VpnResult<()> {
    let purpose = proxy_control_purpose().map_err(into_vpn_err!(VpnErrorCode::Failed))?;
    ttp_server
        .listen_control_stream(
            purpose,
            Arc::new(move |accepted| {
                let cmd_service = cmd_service.clone();
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
                    let tunnel = into_cmd_tunnel(meta, read, write, remote_ep);
                    if let Err(err) = cmd_service.handle_tunnel(tunnel).await {
                        log::error!("proxy control command tunnel failed: {:?}", err);
                    }
                })
            }),
        )
        .await
        .map_err(into_vpn_err!(VpnErrorCode::Failed))
}

fn pn_endpoint_from_p2p_endpoint(endpoint: Endpoint) -> VpnEndpoint {
    let protocol = match endpoint.protocol() {
        Protocol::Quic => VpnEndpoint::PROTOCOL_QUIC,
        Protocol::Tcp => VpnEndpoint::PROTOCOL_TCP,
        Protocol::Ext(_) => VpnEndpoint::PROTOCOL_QUIC,
    };
    VpnEndpoint::new_with_protocol(protocol, endpoint.addr().ip(), endpoint.addr().port())
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

#[cfg(test)]
#[path = "../tests/unit/pn_control_tunnel_observer_tests.rs"]
mod pn_control_tunnel_observer_tests;
