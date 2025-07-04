use std::collections::HashMap;
use std::io::Error;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use bucky_raw_codec::{RawFixedBytes, RawFrom};
use sfo_pool::{into_pool_err, PoolErrorCode, PoolResult, Worker, WorkerFactory, WorkerGuard, WorkerPool, WorkerPoolRef};
use tokio::io::{AsyncReadExt, AsyncWrite};
use tokio::spawn;
use tokio::task::JoinHandle;
use crate::{DataHeader, VpnTunnelFactory, VpnTunnelListener, VpnTunnelRecv, VpnTunnelSend};
use crate::errors::{into_vpn_err, vpn_err, VpnErrorCode, VpnResult};
use crate::server::{NetworkGroupId, NetworkId, NetworkMember, NodeId};

pub(crate) struct VpnRouter {
    routers: Mutex<HashMap<NetworkGroupId, HashMap<NetworkId, HashMap<IpAddr, NodeId>>>>,
}

impl VpnRouter {
    pub fn new() -> Self {
        Self {
            routers: Mutex::new(HashMap::new())
        }
    }

    pub fn get_node(&self, network_group_id: NetworkGroupId, network_id: NetworkId, target: IpAddr) -> Option<NodeId> {
        let routers = self.routers.lock().unwrap();
        if let Some(networks) = routers.get(&network_group_id) {
            if let Some(nodes) = networks.get(&network_id) {
                if let Some(node) = nodes.get(&target) {
                    return Some(node.clone());
                }
            }
        }
        None
    }

    pub fn get_all_nodes(&self, network_group_id: NetworkGroupId, network_id: NetworkId) -> Vec<(IpAddr, NodeId)> {
        let routers = self.routers.lock().unwrap();
        let mut list = Vec::new();
        if let Some(group) = routers.get(&network_group_id) {
            if let Some(network) = group.get(&network_id) {
                for (ip, node_id) in network.iter() {
                    list.push((ip.clone(), node_id.clone()));
                }
            }
        }
        list
    }

    pub fn add_network(&self, network_group_id: NetworkGroupId, network_id: NetworkId, members: Vec<NetworkMember>) {
        let mut routers = self.routers.lock().unwrap();
        let network = routers.entry(network_group_id).or_insert(HashMap::new());
        let member_map = network.entry(network_id).or_insert(HashMap::new());

        member_map.clear();
        for member in members {
            let ip: IpAddr = match member.ip.parse() {
                Ok(ip) => ip,
                Err(_) => {
                    continue;
                }
            };
            member_map.insert(ip, member.id.clone());

            if let Some(ipv6) = member.ipv6.clone() {
                let ip: IpAddr = match ipv6.parse() {
                    Ok(ip) => ip,
                    Err(_) => {
                        continue;
                    }
                };
                member_map.insert(ip, member.id.clone());
            }
        }
    }
}

pub(crate) struct PkgSend<S: VpnTunnelSend> {
    recv_handle: JoinHandle<()>,
    send: S,
    is_work: bool
}

impl<S: VpnTunnelSend> PkgSend<S> {
    pub fn new(recv_handle: JoinHandle<()>, send: S) -> Self {
        Self {
            recv_handle,
            send,
            is_work: true,
        }
    }
}

impl<S: VpnTunnelSend> Drop for PkgSend<S> {
    fn drop(&mut self) {
        log::info!("PkgSend Drop");
        self.recv_handle.abort();
    }
}

impl<S: VpnTunnelSend> Worker for PkgSend<S> {
    fn is_work(&self) -> bool {
        if self.is_work {
            !self.recv_handle.is_finished()
        } else {
            false
        }
    }
}

impl<S: VpnTunnelSend> AsyncWrite for PkgSend<S> {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, Error>> {
        match Pin::new(&mut self.send).poll_write(cx, buf) {
            Poll::Ready(ret) => {
                if let Err(e) = ret {
                    self.is_work = false;
                    return Poll::Ready(Err(e));
                }
                Poll::Ready(ret)
            }
            Poll::Pending => {
                Poll::Pending
            }
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        match Pin::new(&mut self.send).poll_flush(cx) {
            Poll::Ready(ret) => {
                if let Err(e) = ret {
                    self.is_work = false;
                    return Poll::Ready(Err(e));
                }
                Poll::Ready(ret)
            }
            Poll::Pending => {
                Poll::Pending
            }
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Pin::new(&mut self.send).poll_shutdown(cx)
    }
}

pub struct PendingSendCache<S: VpnTunnelSend> {
    cache: Mutex<Vec<PkgSend<S>>>
}

impl<S: VpnTunnelSend> PendingSendCache<S> {
    pub fn new() -> Self {
        Self {
            cache: Mutex::new(Vec::new())
        }
    }

    pub fn push(&self, send: PkgSend<S>) {
        self.cache.lock().unwrap().push(send);
    }

    pub fn get_pending_send(&self, target: &NodeId) -> Option<PkgSend<S>> {
        let mut cache = self.cache.lock().unwrap();
        let mut index = None;
        let mut delete_list = vec![];
        for (i, send) in cache.iter().enumerate() {
            if send.is_work() {
                if send.send.is_target_tunnel(target) {
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

pub(crate) struct Factory<R: VpnTunnelRecv, S: VpnTunnelSend, F: VpnTunnelFactory<R, S>, A: VpnTunnelListener<R, S>> {
    target: NodeId,
    tunnel_factory: Arc<F>,
    pending_sends: Arc<PendingSendCache<S>>,
    pkg_listener: Arc<dyn TunnelPkgRecv>,
    _p: Mutex<std::marker::PhantomData<(R, A)>>
}

impl<R: VpnTunnelRecv, S: VpnTunnelSend, F: VpnTunnelFactory<R, S>, A: VpnTunnelListener<R, S>> Factory<R, S, F, A> {
    pub fn new(target: NodeId,
               tunnel_factory: Arc<F>,
               pending_sends: Arc<PendingSendCache<S>>,
               pkg_listener: Arc<dyn TunnelPkgRecv>,) -> Self {
        let this = Self {
            target,
            tunnel_factory,
            pending_sends,
            pkg_listener,
            _p: Default::default(),
        };

        this
    }

    fn create_handle(mut recv: R, recv_listener: Arc<dyn TunnelPkgRecv>) -> JoinHandle<()> {
        let handle = spawn(async move {
            let ret: VpnResult<()> = async move {
                loop {
                    let mut header = vec![0u8; DataHeader::raw_bytes().unwrap()];
                    let n = recv.read_exact(header.as_mut()).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
                    if n == 0 {
                        break;
                    }
                    let header = DataHeader::clone_from_slice(header.as_slice()).map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?;
                    let mut buf = vec![0u8; header.pkg_len as usize];
                    let n = recv.read_exact(&mut buf).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
                    if n == 0 {
                        break;
                    }

                    if let Err(e) = recv_listener.on_recv(header.network_id, buf).await {
                        log::error!("recv_listener on_recv failed {}", e);
                    }
                }
                Ok(())
            }.await;
            if let Err(e) = ret {
                log::error!("vpn recv handle exit {}", e);
            }
        });
        handle
    }
}

#[async_trait::async_trait]
impl<R: VpnTunnelRecv, S: VpnTunnelSend, F: VpnTunnelFactory<R, S>, A: VpnTunnelListener<R, S>> WorkerFactory<PkgSend<S>> for Factory<R, S, F, A> {
    async fn create(&self) -> PoolResult<PkgSend<S>> {
        {
            if let Some(send) = self.pending_sends.get_pending_send(&self.target) {
                log::info!("get pending send");
                return Ok(send);
            }
        }
        let (recv, send) = self.tunnel_factory.create_tunnel(&self.target).await
            .map_err(into_pool_err!(PoolErrorCode::Failed, "create tunnel failed"))?;
        let recv_listener = self.pkg_listener.clone();
        let handle = Self::create_handle(recv, recv_listener);

        Ok(PkgSend::new(handle, send))
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
    network_id: NetworkId
}

pub struct TunnelManager<
    R: VpnTunnelRecv,
    S: VpnTunnelSend,
    F: VpnTunnelFactory<R, S>, A: VpnTunnelListener<R, S>> {
    tunnel_factory: Arc<F>,
    tunnels: Mutex<HashMap<Target, WorkerPoolRef<PkgSend<S>, Factory<R, S, F, A>>>>,
    pending_send_cache: Arc<PendingSendCache<S>>,
    recv_handle: JoinHandle<()>,
    pkg_listener: Arc<dyn TunnelPkgRecv>,
    router: Arc<VpnRouter>,
}

impl<
    R: VpnTunnelRecv,
    S: VpnTunnelSend,
    F: VpnTunnelFactory<R, S>,
    A: VpnTunnelListener<R, S>> TunnelManager<R, S, F, A> {
    pub fn new(tunnel_factory: Arc<F>,
               tunnel_listener: Arc<A>,
               pkg_listener: Arc<dyn TunnelPkgRecv>,) -> Self {
        let pending_send_cache = Arc::new(PendingSendCache::new());
        let tmp_send_cache = pending_send_cache.clone();
        let tmp_recv_listener = pkg_listener.clone();
        let tmp_tunnel_listener = tunnel_listener.clone();
        let recv_handle = tokio::spawn(async move {
            loop {
                match tmp_tunnel_listener.accept().await {
                    Ok((recv, send)) => {
                        let handle = Factory::<R, S, F, A>::create_handle(recv, tmp_recv_listener.clone());
                        tmp_send_cache.push(PkgSend::new(handle, send));
                    }
                    Err(e) => {
                        log::error!("tunnel_listener accept failed {}", e);
                        break;
                    }
                }
            }
        });

        TunnelManager {
            tunnel_factory,
            tunnels: Mutex::new(HashMap::new()),
            pending_send_cache,
            recv_handle,
            pkg_listener,
            router: Arc::new(VpnRouter::new()),
        }
    }

    pub(crate) fn get_router(&self) -> &Arc<VpnRouter> {
        &self.router
    }

    pub async fn get_send(&self, network_group_id: NetworkGroupId, network_id: NetworkId, target: IpAddr) -> VpnResult<WorkerGuard<PkgSend<S>, Factory<R, S, F, A>>> {
        let key = Target {
            ip: target.clone(),
            network_group_id,
            network_id
        };
        let pool = {
            let mut tunnels = self.tunnels.lock().unwrap();
            if let Some(pool) = tunnels.get(&key) {
                pool.clone()
            } else {
                let node = self.router.get_node(network_group_id, network_id, target);
                if node.is_none() {
                    return Err(vpn_err!(VpnErrorCode::NotFoundNode, "group {} network {} ip {}", network_group_id, network_id, target));
                }
                let pool = WorkerPool::new(5, Factory::new(node.unwrap(),
                                                           self.tunnel_factory.clone(),
                                                           self.pending_send_cache.clone(),
                                                           self.pkg_listener.clone(), ));
                tunnels.insert(key, pool.clone());
                pool
            }
        };
        pool.get_worker().await.map_err(into_vpn_err!(VpnErrorCode::Failed, "get worker failed.group {} network {} ip {}", network_group_id, network_id, target))
    }

    pub async fn get_all_send(&self, network_group_id: NetworkGroupId, network_id: NetworkId) -> VpnResult<Vec<WorkerGuard<PkgSend<S>, Factory<R, S, F, A>>>> {
        let nodes = self.router.get_all_nodes(network_group_id, network_id);
        let mut list = vec![];
        for (ip, node_id) in nodes {
            let key = Target {
                ip,
                network_group_id,
                network_id
            };
            let pool = {
                let mut tunnels = self.tunnels.lock().unwrap();
                if let Some(pool) = tunnels.get(&key) {
                    pool.clone()
                } else {
                    let pool = WorkerPool::new(5, Factory::new(node_id,
                                                               self.tunnel_factory.clone(),
                                                               self.pending_send_cache.clone(),
                                                               self.pkg_listener.clone(), ));
                    tunnels.insert(key, pool.clone());
                    pool
                }
            };
            let worker = pool.get_worker().await
                .map_err(into_vpn_err!(VpnErrorCode::Failed, "get worker failed.group {} network {}", network_group_id, network_id))?;
            list.push(worker);
        }

        Ok(list)
    }
}

impl<R: VpnTunnelRecv,
    S: VpnTunnelSend,
    F: VpnTunnelFactory<R, S>,
    A: VpnTunnelListener<R, S>> Drop for TunnelManager<R, S, F, A> {
    fn drop(&mut self) {
        log::info!("vpn TunnelManager drop");
        self.recv_handle.abort();
    }
}
