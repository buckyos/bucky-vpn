use crate::pn_server_info::{
    PnServerEndpoint as VpnEndpoint, PnServerInfoPayload, encode_pn_server_info,
};
use crate::pn_server_manager::PnServerManagerRef;
use p2p_frame::cmd_server::CmdTunnel;
use p2p_frame::cmd_server::server::{CmdTunnelService, DefaultCmdServerService};
use p2p_frame::endpoint::{Endpoint, Protocol};
use p2p_frame::networks::TunnelPurpose;
use p2p_frame::sn::types::{SnTunnelRead, SnTunnelWrite};
use p2p_frame::ttp::{TtpPortListener, TtpServerRef, TtpStreamMeta};
use std::sync::Arc;
use vpn_frame::errors::{VpnErrorCode, VpnResult, into_vpn_err};
use vpn_frame::VpnCmdPkgLen;

const PROXY_CONTROL_SERVICE: &str = "vpn_proxy_control";

pub type ProxyControlCmdService =
    DefaultCmdServerService<(), SnTunnelRead, SnTunnelWrite, VpnCmdPkgLen, u8>;
pub type ProxyControlCmdServiceRef = Arc<ProxyControlCmdService>;

pub fn proxy_control_purpose() -> p2p_frame::error::P2pResult<TunnelPurpose> {
    TunnelPurpose::from_value(&PROXY_CONTROL_SERVICE.to_string())
}

pub fn create_proxy_control_cmd_service() -> ProxyControlCmdServiceRef {
    ProxyControlCmdService::new()
}

pub async fn register_proxy_control_cmd_listener(
    ttp_server: TtpServerRef,
    cmd_service: ProxyControlCmdServiceRef,
    pn_server_selector: PnServerManagerRef,
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
                    let pn_server = encode_pn_server_info(
                        meta.remote_id.to_string(),
                        PnServerInfoPayload::new_with_primary_address(
                            pn_endpoint_from_p2p_endpoint(remote_ep),
                            Vec::new(),
                        ),
                    )
                    .unwrap();
                    if let Err(err) = pn_server_selector.report_observed_heartbeat(&pn_server).await
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
