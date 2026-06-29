#![allow(unused)]

use base58::ToBase58;
use p2p_frame::endpoint::{Endpoint, Protocol};
use p2p_frame::error::{P2pErrorCode, P2pResult};
use p2p_frame::networks::TunnelPurpose;
use p2p_frame::p2p_identity::{P2pId, P2pIdentity, P2pIdentityFactory, P2pSn};
use p2p_frame::sn::client::{SnClientTunnelFactory, SnCmdClient};
use p2p_frame::sn::types::{SnTunnelClassification, SnTunnelRead, SnTunnelWrite};
use p2p_frame::stack::{
    P2pEnvRef, P2pStackConfig, P2pStackRef, PnProxyRouteResolver, create_p2p_stack,
};
use p2p_frame::stream::{StreamListenerGuard, StreamRead, StreamWrite};
use p2p_frame::ttp::TtpTarget;
use p2p_frame::x509;
use p2p_frame::x509::X509IdentityFactory;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Error;
use std::net::SocketAddr;
use std::ops::DerefMut;
use std::path::PathBuf;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::fs::create_dir_all;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::sync::Mutex;
use vpn_frame::client::{VpnClient, VpnClientFactory, VpnClientManager, VpnServerClient};
use vpn_frame::cmd_server::ClassifiedCmdNodeSendGuard;
use vpn_frame::cmd_server::client::{
    ClassifiedClientSendGuard, ClassifiedCmdSend, ClassifiedSendGuard,
};
use vpn_frame::deserialize_u64_from_string;
use vpn_frame::errors::{VpnErrorCode, VpnResult, into_vpn_err, vpn_err};
use vpn_frame::serialize_u64_as_string;
use vpn_frame::server::{NetworkGroupId, NetworkId, NodeId};
use vpn_frame::{
    NodeVpnInfo, PnServerInfo, VpnTunnelFactory, VpnTunnelListener, VpnTunnelRecv, VpnTunnelSend,
};

pub struct P2pVpnTunnelRecv {
    read: StreamRead,
}

impl P2pVpnTunnelRecv {
    pub fn new(read: StreamRead) -> P2pVpnTunnelRecv {
        P2pVpnTunnelRecv { read }
    }
}

impl VpnTunnelRecv for P2pVpnTunnelRecv {}

impl AsyncRead for P2pVpnTunnelRecv {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match Pin::new(self.read.deref_mut()).poll_read(cx, buf) {
            Poll::Ready(ret) => {
                if ret.is_ok() {
                    log::trace!(
                        "session {} read from {} len {} success",
                        self.read.session_id(),
                        self.read.remote_id().to_string(),
                        buf.filled().len()
                    );
                }
                Poll::Ready(ret)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

pub struct P2pVpnTunnelSend {
    write: StreamWrite,
}

impl P2pVpnTunnelSend {
    pub fn new(write: StreamWrite) -> P2pVpnTunnelSend {
        P2pVpnTunnelSend { write }
    }
}

impl VpnTunnelSend for P2pVpnTunnelSend {
    fn is_target_tunnel(&self, target: &NodeId) -> bool {
        if target == &NodeId::from(self.write.remote_id().as_slice()) {
            true
        } else {
            false
        }
    }

    fn is_closed(&self) -> bool {
        self.write.is_closed()
    }
}

impl AsyncWrite for P2pVpnTunnelSend {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, Error>> {
        match Pin::new(self.write.deref_mut()).poll_write(cx, buf) {
            Poll::Ready(ret) => {
                if ret.is_ok() {
                    log::trace!(
                        "session {} write to {} len {} success",
                        self.write.session_id(),
                        self.write.remote_id().to_string(),
                        buf.len()
                    );
                }
                Poll::Ready(ret)
            }
            Poll::Pending => Poll::Pending,
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Pin::new(self.write.deref_mut()).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Pin::new(self.write.deref_mut()).poll_shutdown(cx)
    }
}

pub struct P2pVpnPnProxyRouteResolver {
    routes: StdMutex<HashMap<P2pId, P2pId>>,
}

impl P2pVpnPnProxyRouteResolver {
    pub fn new() -> Self {
        Self {
            routes: StdMutex::new(HashMap::new()),
        }
    }

    pub fn update_routes(&self, vpn_infos: &[NodeVpnInfo]) -> VpnResult<()> {
        let mut routes = HashMap::new();
        for vpn_info in vpn_infos {
            let Some(pn_server) = vpn_info.node_info.pn_server.as_ref() else {
                continue;
            };
            let pn_server_id = P2pId::from_str(&pn_server.id).map_err(into_vpn_err!(
                VpnErrorCode::InvalidParam,
                "parse pn server id {} failed",
                pn_server.id
            ))?;
            for member in vpn_info.members.iter() {
                routes.insert(P2pId::from(member.id.as_slice()), pn_server_id.clone());
            }
        }
        *self.routes.lock().unwrap() = routes;
        Ok(())
    }
}

#[async_trait::async_trait]
impl PnProxyRouteResolver for P2pVpnPnProxyRouteResolver {
    async fn resolve_pn_server(&self, target: &P2pId) -> P2pResult<P2pId> {
        self.routes
            .lock()
            .unwrap()
            .get(target)
            .cloned()
            .ok_or_else(|| {
                p2p_frame::error::p2p_err!(
                    P2pErrorCode::InvalidParam,
                    "pn proxy route not found for target {}",
                    target
                )
            })
    }
}

pub struct P2pVpnTunnelFactory {
    stack: P2pStackRef,
    vpn_port: u16,
    server_id: P2pId,
    proxy_route_resolver: Arc<P2pVpnPnProxyRouteResolver>,
}

impl P2pVpnTunnelFactory {
    pub fn new(
        stack: P2pStackRef,
        vpn_port: u16,
        server_id: P2pId,
        proxy_route_resolver: Arc<P2pVpnPnProxyRouteResolver>,
    ) -> P2pVpnTunnelFactory {
        P2pVpnTunnelFactory {
            stack,
            vpn_port,
            server_id,
            proxy_route_resolver,
        }
    }

    async fn connect_pn_server(&self, pn_server: &PnServerInfo) -> VpnResult<()> {
        let remote_id = P2pId::from_str(&pn_server.id).map_err(into_vpn_err!(
            VpnErrorCode::InvalidParam,
            "parse pn server id {} failed",
            pn_server.id
        ))?;
        let endpoint = Endpoint::from((
            Protocol::Quic,
            SocketAddr::new(pn_server.ip, pn_server.port),
        ));
        self.stack
            .sn_client()
            .get_ttp_client()
            .connect_server(TtpTarget {
                local_ep: None,
                remote_ep: endpoint,
                remote_id: remote_id.clone(),
                remote_name: Some(remote_id.to_string()),
            })
            .await
            .map_err(into_vpn_err!(
                VpnErrorCode::Failed,
                "connect pn server {} {}:{} failed",
                pn_server.id,
                pn_server.ip,
                pn_server.port
            ))?;
        Ok(())
    }
}
#[async_trait::async_trait]
impl VpnTunnelFactory<P2pVpnTunnelRecv, P2pVpnTunnelSend> for P2pVpnTunnelFactory {
    async fn on_vpn_info_received(&self, vpn_infos: &[NodeVpnInfo]) -> VpnResult<()> {
        self.proxy_route_resolver.update_routes(vpn_infos)?;
        let mut connected = HashSet::new();
        for vpn_info in vpn_infos {
            let Some(pn_server) = vpn_info.node_info.pn_server.as_ref() else {
                continue;
            };
            if connected.insert(pn_server.clone()) {
                self.connect_pn_server(pn_server).await?;
            }
        }
        Ok(())
    }

    async fn create_tunnel(
        &self,
        network_group_id: NetworkGroupId,
        network_id: NetworkId,
        node_id: &NodeId,
    ) -> VpnResult<(P2pVpnTunnelRecv, P2pVpnTunnelSend)> {
        log::info!(
            "create p2p vpn tunnel group {} network {} target {}",
            network_group_id,
            network_id,
            node_id.as_slice().to_base58()
        );
        let (read, write) = self
            .stack
            .stream_manager()
            .connect_from_id(
                &P2pId::from(node_id.as_slice()),
                TunnelPurpose::from_value(&self.vpn_port).unwrap(),
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::Failed))?;
        Ok((P2pVpnTunnelRecv::new(read), P2pVpnTunnelSend::new(write)))
    }
}

pub struct P2pVpnTunnelListener {
    stack: P2pStackRef,
    vpn_port: u16,
    listener: Mutex<StreamListenerGuard>,
}

impl P2pVpnTunnelListener {
    pub async fn new(stack: P2pStackRef, vpn_port: u16) -> VpnResult<P2pVpnTunnelListener> {
        let listener = stack
            .stream_manager()
            .listen(TunnelPurpose::from_value(&vpn_port).unwrap())
            .await
            .map_err(into_vpn_err!(
                VpnErrorCode::Failed,
                "create listener failed"
            ))?;
        Ok(P2pVpnTunnelListener {
            stack,
            vpn_port,
            listener: Mutex::new(listener),
        })
    }
}

#[async_trait::async_trait]
impl VpnTunnelListener<P2pVpnTunnelRecv, P2pVpnTunnelSend> for P2pVpnTunnelListener {
    async fn accept(&self) -> VpnResult<(P2pVpnTunnelRecv, P2pVpnTunnelSend)> {
        let mut listener = self.listener.lock().await;
        let (read, write) = listener
            .accept()
            .await
            .map_err(into_vpn_err!(VpnErrorCode::Failed))?;
        log::info!(
            "accept a new connection {} remote_id {} remote_ep {} local_id {} local_ep {}",
            read.session_id(),
            read.remote_id().to_string(),
            read.remote().to_string(),
            read.local_id().to_string(),
            read.local().to_string()
        );
        Ok((P2pVpnTunnelRecv::new(read), P2pVpnTunnelSend::new(write)))
    }
}

pub type P2pCmdSend =
    ClassifiedCmdSend<SnTunnelClassification, (), SnTunnelRead, SnTunnelWrite, u16, u8>;
pub type P2pCmdSendGuard = ClassifiedClientSendGuard<
    SnTunnelClassification,
    (),
    SnTunnelRead,
    SnTunnelWrite,
    SnClientTunnelFactory,
    u16,
    u8,
>;
pub type P2pVpnClientRef = Arc<
    VpnClient<
        (),
        P2pCmdSend,
        P2pCmdSendGuard,
        SnCmdClient,
        P2pVpnTunnelRecv,
        P2pVpnTunnelSend,
        P2pVpnTunnelFactory,
        P2pVpnTunnelListener,
    >,
>;

pub struct P2pVpnClientFactory {
    p2p_env: P2pEnvRef,
    config_path: PathBuf,
    vpn_port: u16,
    client_version: String,
}

impl P2pVpnClientFactory {
    pub fn new(
        p2p_env: P2pEnvRef,
        config_path: PathBuf,
        vpn_port: u16,
        client_version: String,
    ) -> P2pVpnClientFactory {
        P2pVpnClientFactory {
            p2p_env,
            config_path,
            vpn_port,
            client_version,
        }
    }
}

#[async_trait::async_trait]
impl
    VpnClientFactory<
        (),
        P2pCmdSend,
        P2pCmdSendGuard,
        SnCmdClient,
        P2pVpnTunnelRecv,
        P2pVpnTunnelSend,
        P2pVpnTunnelFactory,
        P2pVpnTunnelListener,
    > for P2pVpnClientFactory
{
    async fn create_client(&self, key: &str) -> VpnResult<P2pVpnClientRef> {
        let list: Vec<_> = key.split('_').collect();
        if list.len() != 2 {
            return Err(vpn_err!(VpnErrorCode::Failed, "key {} is invalid", key));
        }
        let sn_id = list[0];
        let server = list[1];
        //判断ip是不是域名，如果是域名，需要解析ip
        let ip = if let Ok(addr) = server.parse::<SocketAddr>() {
            addr
        } else {
            // 解析域名
            tokio::net::lookup_host(server)
                .await
                .map_err(into_vpn_err!(
                    VpnErrorCode::Failed,
                    "resolve domain {} failed",
                    server
                ))?
                .next()
                .ok_or_else(|| {
                    vpn_err!(VpnErrorCode::Failed, "no IP found for domain {}", server)
                })?
        };
        let sn_port = ip.port();

        let server_config =
            self.config_path
                .join(format!("{}_{}_{}", sn_id, ip.ip().to_string(), sn_port));
        let identity_file = server_config.join("identity");
        let local_identity = if server_config.exists() && identity_file.exists() {
            let data = tokio::fs::read(identity_file.as_path())
                .await
                .map_err(into_vpn_err!(
                    VpnErrorCode::Failed,
                    "read {} failed",
                    identity_file.to_string_lossy().to_string()
                ))?;
            let local_identity = X509IdentityFactory
                .create(&data)
                .map_err(into_vpn_err!(VpnErrorCode::Failed))?;
            local_identity
        } else {
            create_dir_all(server_config.as_path())
                .await
                .map_err(into_vpn_err!(
                    VpnErrorCode::Failed,
                    "create {} failed",
                    server_config.to_string_lossy().to_string()
                ))?;
            let local_identity = x509::generate_rsa_x509_identity(None).map_err(into_vpn_err!(
                VpnErrorCode::Failed,
                "create identity failed"
            ))?;
            let data = local_identity
                .get_encoded_identity()
                .map_err(into_vpn_err!(VpnErrorCode::Failed))?;
            tokio::fs::write(identity_file.as_path(), data)
                .await
                .map_err(into_vpn_err!(
                    VpnErrorCode::Failed,
                    "write {} failed",
                    identity_file.to_string_lossy().to_string()
                ))?;
            Arc::new(local_identity)
        };
        let sn_id = P2pId::from_str(sn_id)
            .map_err(into_vpn_err!(VpnErrorCode::Failed, "parse sn_id failed"))?;
        let local_id = local_identity.get_id();
        log::info!(
            "create client base58:{} base36:{}",
            local_id.as_slice().to_base58(),
            local_id.to_string()
        );

        let sn_ep = Endpoint::from((Protocol::Quic, SocketAddr::new(ip.ip(), sn_port)));

        let conn_timeout = Duration::from_secs(5);
        let proxy_route_resolver = Arc::new(P2pVpnPnProxyRouteResolver::new());
        let stack_config = P2pStackConfig::new(self.p2p_env.clone(), local_identity)
            .set_conn_timeout(conn_timeout)
            .set_support_proxy(true)
            .set_proxy_route_resolver(proxy_route_resolver.clone())
            .add_sn(P2pSn::new(sn_id.clone(), sn_id.to_string(), vec![sn_ep]));
        let stack = create_p2p_stack(stack_config)
            .await
            .map_err(into_vpn_err!(VpnErrorCode::Failed, "create stack failed"))?;
        stack
            .wait_online(Some(Duration::from_secs(30)))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::Failed, "wait online timout"))?;

        let vpn_client =
            VpnServerClient::new(stack.sn_client().get_cmd_client().clone(), conn_timeout);
        let client = VpnClient::new(
            vpn_client.clone(),
            Arc::new(P2pVpnTunnelFactory::new(
                stack.clone(),
                self.vpn_port,
                sn_id,
                proxy_route_resolver,
            )),
            Arc::new(
                P2pVpnTunnelListener::new(stack.clone(), self.vpn_port)
                    .await
                    .map_err(into_vpn_err!(VpnErrorCode::Failed))?,
            ),
            self.client_version.clone(),
        );
        Ok(client)
    }
}

pub type P2pVpnClientManagerRef = Arc<
    VpnClientManager<
        (),
        P2pCmdSend,
        P2pCmdSendGuard,
        SnCmdClient,
        P2pVpnTunnelRecv,
        P2pVpnTunnelSend,
        P2pVpnTunnelFactory,
        P2pVpnTunnelListener,
        P2pVpnClientFactory,
    >,
>;

static VPN_CLIENT_MANAGER: OnceLock<P2pVpnClientManagerRef> = OnceLock::new();
pub fn init_p2p_vpn_client_manager(
    p2p_env: P2pEnvRef,
    config_path: PathBuf,
    vpn_port: u16,
    client_version: String,
) -> VpnResult<()> {
    VPN_CLIENT_MANAGER.get_or_init(|| {
        Arc::new(VpnClientManager::new(Arc::new(P2pVpnClientFactory::new(
            p2p_env,
            config_path,
            vpn_port,
            client_version,
        ))))
    });
    Ok(())
}

pub fn vpn_client_manager() -> P2pVpnClientManagerRef {
    VPN_CLIENT_MANAGER.get().unwrap().clone()
}

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct JoinRecord {
    pub server_ip: String,
    pub server_port: u16,
    pub server_id: String,
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string"
    )]
    pub network_id: NetworkGroupId,
}

#[cfg(test)]
mod tests {
    use super::*;
    use p2p_frame::error::P2pError;
    use p2p_frame::networks::{Tunnel, TunnelNetwork};
    use p2p_frame::p2p_identity::{P2pIdentityCertFactoryRef, P2pIdentityCertRef, P2pIdentityRef};
    use p2p_frame::pn::{PnClient, PnServer, PnServerRef};
    use p2p_frame::sn::client::{SNClientServiceRef, SnLocalIpProvider};
    use p2p_frame::sn::service::{SnServerRef, SnServiceConfig, create_sn_service};
    use p2p_frame::stack::{
        DeviceFinder, DeviceFinderRef, P2pConfig, P2pPn, PnServerAddress, create_p2p_env,
    };
    use p2p_frame::ttp::{TtpClient, TtpConnector, TtpTarget};
    use p2p_frame::x509::{X509IdentityCertFactory, X509IdentityFactory};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::task::JoinHandle;
    use tokio::time::timeout;
    use vpn_frame::NodeNetwork;
    use vpn_frame::server::NetworkMember;

    fn node_id(seed: u8) -> NodeId {
        NodeId::from(vec![seed; 32].as_slice())
    }

    fn p2p_id(seed: u8) -> P2pId {
        P2pId::from(vec![seed; 32].as_slice())
    }

    fn tunnel_purpose(value: u16) -> TunnelPurpose {
        TunnelPurpose::from_value(&value).unwrap()
    }

    fn vpn_info(pn_server: Option<PnServerInfo>, members: Vec<NetworkMember>) -> NodeVpnInfo {
        NodeVpnInfo {
            node_info: NodeNetwork {
                id: 100,
                group_id: 200,
                name: "test-network".to_string(),
                ip: Some(IpAddr::from([10, 0, 0, 1])),
                mask: 24,
                ipv6: None,
                ipv6_mask: 0,
                pn_server,
            },
            members,
        }
    }

    #[tokio::test]
    async fn resolver_maps_member_targets_to_selected_pn_server() {
        let resolver = P2pVpnPnProxyRouteResolver::new();
        let target = node_id(1);
        let relay = p2p_id(2);
        let pn_server = PnServerInfo::new(relay.to_string(), IpAddr::from([127, 0, 0, 1]), 3624);

        resolver
            .update_routes(&[vpn_info(
                Some(pn_server),
                vec![NetworkMember {
                    id: target.clone(),
                    ip: "10.0.0.2".to_string(),
                    ipv6: None,
                }],
            )])
            .unwrap();

        let resolved = resolver
            .resolve_pn_server(&P2pId::from(target.as_slice()))
            .await
            .unwrap();
        assert_eq!(resolved, relay);
    }

    #[tokio::test]
    async fn resolver_replaces_stale_routes_on_refresh() {
        let resolver = P2pVpnPnProxyRouteResolver::new();
        let target = node_id(3);
        let relay = p2p_id(4);
        let pn_server = PnServerInfo::new(relay.to_string(), IpAddr::from([127, 0, 0, 1]), 3624);

        resolver
            .update_routes(&[vpn_info(
                Some(pn_server),
                vec![NetworkMember {
                    id: target.clone(),
                    ip: "10.0.0.3".to_string(),
                    ipv6: None,
                }],
            )])
            .unwrap();
        resolver.update_routes(&[vpn_info(None, vec![])]).unwrap();

        let result = resolver
            .resolve_pn_server(&P2pId::from(target.as_slice()))
            .await;
        assert!(result.is_err());
    }

    #[test]
    fn resolver_skips_networks_without_pn_server() {
        let resolver = P2pVpnPnProxyRouteResolver::new();

        resolver
            .update_routes(&[vpn_info(
                None,
                vec![NetworkMember {
                    id: node_id(5),
                    ip: "10.0.0.4".to_string(),
                    ipv6: None,
                }],
            )])
            .unwrap();

        assert!(resolver.routes.lock().unwrap().is_empty());
    }

    #[test]
    fn resolver_rejects_invalid_pn_server_id() {
        let resolver = P2pVpnPnProxyRouteResolver::new();
        let pn_server = PnServerInfo::new(
            "not-a-valid-p2p-id".to_string(),
            IpAddr::from([127, 0, 0, 1]),
            3624,
        );

        let result = resolver.update_routes(&[vpn_info(
            Some(pn_server),
            vec![NetworkMember {
                id: node_id(6),
                ip: "10.0.0.5".to_string(),
                ipv6: None,
            }],
        )]);

        assert!(result.is_err());
    }
}
