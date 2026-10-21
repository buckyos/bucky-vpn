use super::tunnel_activity::{ChannelRegistry, SendPermit, TrackedRecv, TunnelChannel};
use crate::errors::{VpnErrorCode, VpnResult, into_vpn_err, vpn_err};
use crate::server::{NetworkGroupId, NetworkId, NetworkMember, NodeId};
use crate::{
    DataHeader, PnServerInfo, VpnTunnelFactory, VpnTunnelListener, VpnTunnelRecv, VpnTunnelSend,
};
use bucky_raw_codec::{RawFixedBytes, RawFrom};
use sfo_pool::{
    PoolErrorCode, PoolResult, Worker, WorkerFactory, WorkerGuard, WorkerPool,
    WorkerPoolRef, into_pool_err,
};
use std::collections::HashMap;
use std::io::Error;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWrite};
use tokio::spawn;
use tokio::task::JoinHandle;

const TUNNEL_POOL_MAX_COUNT: u16 = 20;
const TUNNEL_POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const TUNNEL_CLEANUP_INTERVAL: Duration = Duration::from_secs(30);

pub(crate) struct VpnRouter {
    routers: Mutex<HashMap<NetworkGroupId, HashMap<NetworkId, NetworkRoute>>>,
}

struct NetworkRoute {
    pn_server: Option<PnServerInfo>,
    members: HashMap<IpAddr, NodeId>,
}

impl VpnRouter {
    pub fn new() -> Self {
        Self {
            routers: Mutex::new(HashMap::new()),
        }
    }

    pub fn get_node(
        &self,
        network_group_id: NetworkGroupId,
        network_id: NetworkId,
        target: IpAddr,
    ) -> Option<(NodeId, Option<PnServerInfo>)> {
        let routers = self.routers.lock().unwrap();
        if let Some(networks) = routers.get(&network_group_id) {
            if let Some(route) = networks.get(&network_id) {
                if let Some(node) = route.members.get(&target) {
                    return Some((node.clone(), route.pn_server.clone()));
                }
            }
        }
        None
    }

    pub fn get_all_nodes(
        &self,
        network_group_id: NetworkGroupId,
        network_id: NetworkId,
    ) -> Vec<(IpAddr, NodeId, Option<PnServerInfo>)> {
        let routers = self.routers.lock().unwrap();
        let mut list = Vec::new();
        if let Some(group) = routers.get(&network_group_id) {
            if let Some(network) = group.get(&network_id) {
                for (ip, node_id) in network.members.iter() {
                    list.push((ip.clone(), node_id.clone(), network.pn_server.clone()));
                }
            }
        }
        list
    }

    pub fn add_network(
        &self,
        network_group_id: NetworkGroupId,
        network_id: NetworkId,
        pn_server: Option<PnServerInfo>,
        members: Vec<NetworkMember>,
    ) {
        let mut routers = self.routers.lock().unwrap();
        let network = routers.entry(network_group_id).or_insert(HashMap::new());
        let route = network.entry(network_id).or_insert(NetworkRoute {
            pn_server: None,
            members: HashMap::new(),
        });

        route.pn_server = pn_server;
        route.members.clear();
        for member in members {
            let ip: IpAddr = match member.ip.parse() {
                Ok(ip) => ip,
                Err(_) => {
                    continue;
                }
            };
            route.members.insert(ip, member.id.clone());

            if let Some(ipv6) = member.ipv6.clone() {
                let ip: IpAddr = match ipv6.parse() {
                    Ok(ip) => ip,
                    Err(_) => {
                        continue;
                    }
                };
                route.members.insert(ip, member.id.clone());
            }
        }
    }
}

pub(crate) struct PkgSend<S: VpnTunnelSend> {
    network_group_id: Option<NetworkGroupId>,
    network_id: Option<NetworkId>,
    target: Option<NodeId>,
    recv_handle: JoinHandle<()>,
    channel: Arc<TunnelChannel<S>>,
}

impl<S: VpnTunnelSend> PkgSend<S> {
    pub fn new(
        network_group_id: NetworkGroupId,
        network_id: NetworkId,
        target: NodeId,
        recv_handle: JoinHandle<()>,
        channel: Arc<TunnelChannel<S>>,
    ) -> Self {
        Self {
            network_group_id: Some(network_group_id),
            network_id: Some(network_id),
            target: Some(target),
            recv_handle,
            channel,
        }
    }

    pub fn new_pending(recv_handle: JoinHandle<()>, channel: Arc<TunnelChannel<S>>) -> Self {
        Self {
            network_group_id: None,
            network_id: None,
            target: None,
            recv_handle,
            channel,
        }
    }

    pub fn begin_send(&self) -> Option<SendPermit<S>> {
        if self.recv_handle.is_finished() {
            return None;
        }
        self.channel.begin_send()
    }

    pub async fn closed(&self) {
        self.channel.closed().await;
    }
}

impl<S: VpnTunnelSend> Drop for PkgSend<S> {
    fn drop(&mut self) {
        log::info!("PkgSend Drop");
        self.channel.close();
        self.recv_handle.abort();
    }
}

impl<S: VpnTunnelSend> Worker for PkgSend<S> {
    fn is_work(&self) -> bool {
        !self.recv_handle.is_finished() && self.channel.is_work()
    }
}

impl<S: VpnTunnelSend> AsyncWrite for PkgSend<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, Error>> {
        self.channel.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        self.channel.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        self.channel.poll_shutdown(cx)
    }
}

pub struct PendingSendCache<S: VpnTunnelSend> {
    cache: Mutex<Vec<PkgSend<S>>>,
}

impl<S: VpnTunnelSend> PendingSendCache<S> {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(Vec::new()),
        }
    }

    pub fn push(&self, send: PkgSend<S>) {
        self.cache.lock().unwrap().push(send);
    }

    pub fn cleanup(&self) {
        self.cache.lock().unwrap().retain(|send| send.is_work());
    }

    pub fn get_pending_send(
        &self,
        network_group_id: NetworkGroupId,
        network_id: NetworkId,
        target: &NodeId,
    ) -> Option<PkgSend<S>> {
        let mut cache = self.cache.lock().unwrap();
        let mut index = None;
        let mut delete_list = vec![];
        for (i, send) in cache.iter().enumerate() {
            if send.is_work() {
                let network_matches = match (send.network_group_id, send.network_id) {
                    (Some(send_group_id), Some(send_network_id)) => {
                        send_group_id == network_group_id && send_network_id == network_id
                    }
                    _ => true,
                };
                if network_matches
                    && send
                        .target
                        .as_ref()
                        .map_or(true, |send_target| send_target == target)
                    && send.channel.is_target_tunnel(target)
                {
                    index = Some(i);
                    break;
                }
            } else {
                delete_list.push(i);
            }
        }

        if let Some(index) = index {
            let send = Some(cache.remove(index));
            for i in delete_list.iter().rev() {
                cache.remove(*i);
            }
            send
        } else {
            for i in delete_list.iter().rev() {
                cache.remove(*i);
            }
            None
        }
    }
}

pub(crate) struct Factory<
    R: VpnTunnelRecv,
    S: VpnTunnelSend,
    F: VpnTunnelFactory<R, S>,
    A: VpnTunnelListener<R, S>,
> {
    network_group_id: NetworkGroupId,
    network_id: NetworkId,
    target: NodeId,
    tunnel_factory: Arc<F>,
    pending_sends: Arc<PendingSendCache<S>>,
    channels: Arc<ChannelRegistry<S>>,
    pkg_listener: Arc<dyn TunnelPkgRecv>,
    _p: Mutex<std::marker::PhantomData<(R, A)>>,
}

impl<R: VpnTunnelRecv, S: VpnTunnelSend, F: VpnTunnelFactory<R, S>, A: VpnTunnelListener<R, S>>
    Factory<R, S, F, A>
{
    pub fn new(
        network_group_id: NetworkGroupId,
        network_id: NetworkId,
        target: NodeId,
        tunnel_factory: Arc<F>,
        pending_sends: Arc<PendingSendCache<S>>,
        channels: Arc<ChannelRegistry<S>>,
        pkg_listener: Arc<dyn TunnelPkgRecv>,
    ) -> Self {
        let this = Self {
            network_group_id,
            network_id,
            target,
            tunnel_factory,
            pending_sends,
            channels,
            pkg_listener,
            _p: Default::default(),
        };

        this
    }

    fn create_handle(
        recv: R,
        recv_listener: Arc<dyn TunnelPkgRecv>,
        channel: &Arc<TunnelChannel<S>>,
    ) -> JoinHandle<()> {
        let mut recv = TrackedRecv::new(recv, channel);
        let weak_channel = Arc::downgrade(channel);
        let handle = spawn(async move {
            let ret: VpnResult<()> = async move {
                loop {
                    let mut header = vec![0u8; DataHeader::raw_bytes().unwrap()];
                    let n = recv
                        .read_exact(header.as_mut())
                        .await
                        .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
                    if n == 0 {
                        break;
                    }
                    let header = DataHeader::clone_from_slice(header.as_slice())
                        .map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?;
                    let mut buf = vec![0u8; header.pkg_len as usize];
                    let n = recv
                        .read_exact(&mut buf)
                        .await
                        .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
                    if n == 0 {
                        break;
                    }

                    if let Err(e) = recv_listener.on_recv(header.network_id, buf).await {
                        log::error!("recv_listener on_recv failed {}", e);
                    }
                }
                Ok(())
            }
            .await;
            if let Err(e) = ret {
                log::error!("vpn recv handle exit {}", e);
            }
            if let Some(channel) = weak_channel.upgrade() {
                channel.close();
            }
        });
        channel.set_recv_abort(handle.abort_handle());
        handle
    }
}

#[async_trait::async_trait]
impl<R: VpnTunnelRecv, S: VpnTunnelSend, F: VpnTunnelFactory<R, S>, A: VpnTunnelListener<R, S>>
    WorkerFactory<PkgSend<S>> for Factory<R, S, F, A>
{
    async fn create(&self) -> PoolResult<PkgSend<S>> {
        {
            if let Some(send) = self.pending_sends.get_pending_send(
                self.network_group_id,
                self.network_id,
                &self.target,
            ) {
                log::info!("get pending send");
                return Ok(send);
            }
        }
        let (recv, send) = self
            .tunnel_factory
            .create_tunnel(self.network_group_id, self.network_id, &self.target)
            .await
            .map_err(into_pool_err!(
                PoolErrorCode::Failed,
                "create tunnel failed"
            ))?;
        let recv_listener = self.pkg_listener.clone();
        let channel = TunnelChannel::new(send);
        self.channels.register(&channel);
        let handle = Self::create_handle(recv, recv_listener, &channel);

        Ok(PkgSend::new(
            self.network_group_id,
            self.network_id,
            self.target.clone(),
            handle,
            channel,
        ))
    }
}

#[callback_trait::callback_trait]
pub trait TunnelPkgRecv: Send + Sync + 'static {
    async fn on_recv(&self, network_id: NetworkId, data: Vec<u8>) -> VpnResult<()>;
}

#[derive(Hash, Eq, PartialEq)]
struct Target {
    ip: IpAddr,
    network_group_id: NetworkGroupId,
    network_id: NetworkId,
    pn_server: Option<PnServerInfo>,
}

pub struct TunnelManager<
    R: VpnTunnelRecv,
    S: VpnTunnelSend,
    F: VpnTunnelFactory<R, S>,
    A: VpnTunnelListener<R, S>,
> {
    tunnel_factory: Arc<F>,
    tunnels: Mutex<HashMap<Target, WorkerPoolRef<PkgSend<S>, Factory<R, S, F, A>>>>,
    pending_send_cache: Arc<PendingSendCache<S>>,
    channels: Arc<ChannelRegistry<S>>,
    recv_handle: JoinHandle<()>,
    cleanup_handle: JoinHandle<()>,
    pkg_listener: Arc<dyn TunnelPkgRecv>,
    router: Arc<VpnRouter>,
}

impl<R: VpnTunnelRecv, S: VpnTunnelSend, F: VpnTunnelFactory<R, S>, A: VpnTunnelListener<R, S>>
    TunnelManager<R, S, F, A>
{
    pub fn new(
        tunnel_factory: Arc<F>,
        tunnel_listener: Arc<A>,
        pkg_listener: Arc<dyn TunnelPkgRecv>,
    ) -> Self {
        Self::new_with_idle_policy(
            tunnel_factory,
            tunnel_listener,
            pkg_listener,
            TUNNEL_POOL_IDLE_TIMEOUT,
            TUNNEL_CLEANUP_INTERVAL,
        )
    }

    pub(crate) fn new_with_idle_policy(
        tunnel_factory: Arc<F>,
        tunnel_listener: Arc<A>,
        pkg_listener: Arc<dyn TunnelPkgRecv>,
        idle_timeout: Duration,
        cleanup_interval: Duration,
    ) -> Self {
        let channels = Arc::new(ChannelRegistry::new());
        let pending_send_cache = Arc::new(PendingSendCache::new());
        let tmp_send_cache = pending_send_cache.clone();
        let tmp_channels = channels.clone();
        let tmp_recv_listener = pkg_listener.clone();
        let tmp_tunnel_listener = tunnel_listener.clone();
        let recv_handle = tokio::spawn(async move {
            loop {
                match tmp_tunnel_listener.accept().await {
                    Ok((recv, send)) => {
                        let channel = TunnelChannel::new(send);
                        tmp_channels.register(&channel);
                        let handle = Factory::<R, S, F, A>::create_handle(
                            recv,
                            tmp_recv_listener.clone(),
                            &channel,
                        );
                        tmp_send_cache.push(PkgSend::new_pending(handle, channel));
                    }
                    Err(e) => {
                        log::error!("tunnel_listener accept failed {}", e);
                        break;
                    }
                }
            }
        });

        let weak_channels = Arc::downgrade(&channels);
        let weak_pending = Arc::downgrade(&pending_send_cache);
        let cleanup_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                let Some(channels) = weak_channels.upgrade() else {
                    break;
                };
                channels.cleanup(tokio::time::Instant::now(), idle_timeout);
                if let Some(pending) = weak_pending.upgrade() {
                    pending.cleanup();
                }
            }
        });

        TunnelManager {
            tunnel_factory,
            tunnels: Mutex::new(HashMap::new()),
            pending_send_cache,
            channels,
            recv_handle,
            cleanup_handle,
            pkg_listener,
            router: Arc::new(VpnRouter::new()),
        }
    }

    pub(crate) fn get_router(&self) -> &Arc<VpnRouter> {
        &self.router
    }

    pub async fn get_send(
        &self,
        network_group_id: NetworkGroupId,
        network_id: NetworkId,
        target: IpAddr,
    ) -> VpnResult<WorkerGuard<PkgSend<S>, Factory<R, S, F, A>>> {
        let (node, pn_server) = self
            .router
            .get_node(network_group_id, network_id, target)
            .ok_or_else(|| {
                vpn_err!(
                    VpnErrorCode::NotFoundNode,
                    "group {} network {} ip {}",
                    network_group_id,
                    network_id,
                    target
                )
            })?;
        let key = Target {
            ip: target.clone(),
            network_group_id,
            network_id,
            pn_server: pn_server.clone(),
        };
        let pool = {
            let mut tunnels = self.tunnels.lock().unwrap();
            if let Some(pool) = tunnels.get(&key) {
                pool.clone()
            } else {
                let pool = WorkerPool::new(
                    TUNNEL_POOL_MAX_COUNT,
                    Factory::new(
                        network_group_id,
                        network_id,
                        node,
                        self.tunnel_factory.clone(),
                        self.pending_send_cache.clone(),
                        self.channels.clone(),
                        self.pkg_listener.clone(),
                    ),
                );
                tunnels.insert(key, pool.clone());
                pool
            }
        };
        pool.get_worker().await.map_err(into_vpn_err!(
            VpnErrorCode::Failed,
            "get worker failed.group {} network {} ip {}",
            network_group_id,
            network_id,
            target
        ))
    }

    pub async fn get_all_send(
        &self,
        network_group_id: NetworkGroupId,
        network_id: NetworkId,
    ) -> VpnResult<Vec<WorkerGuard<PkgSend<S>, Factory<R, S, F, A>>>> {
        let nodes = self.router.get_all_nodes(network_group_id, network_id);
        let mut list = vec![];
        for (ip, node_id, pn_server) in nodes {
            let key = Target {
                ip,
                network_group_id,
                network_id,
                pn_server: pn_server.clone(),
            };
            let pool = {
                let mut tunnels = self.tunnels.lock().unwrap();
                if let Some(pool) = tunnels.get(&key) {
                    pool.clone()
                } else {
                    let pool = WorkerPool::new(
                        TUNNEL_POOL_MAX_COUNT,
                        Factory::new(
                            network_group_id,
                            network_id,
                            node_id,
                            self.tunnel_factory.clone(),
                            self.pending_send_cache.clone(),
                            self.channels.clone(),
                            self.pkg_listener.clone(),
                        ),
                    );
                    tunnels.insert(key, pool.clone());
                    pool
                }
            };
            let worker = pool.get_worker().await.map_err(into_vpn_err!(
                VpnErrorCode::Failed,
                "get worker failed.group {} network {}",
                network_group_id,
                network_id
            ))?;
            list.push(worker);
        }

        Ok(list)
    }
}

impl<R: VpnTunnelRecv, S: VpnTunnelSend, F: VpnTunnelFactory<R, S>, A: VpnTunnelListener<R, S>> Drop
    for TunnelManager<R, S, F, A>
{
    fn drop(&mut self) {
        log::info!("vpn TunnelManager drop");
        self.channels.close_all();
        self.cleanup_handle.abort();
        self.recv_handle.abort();
    }
}
