use crate::client::tunnel_manager::{Factory, PkgSend, TunnelManager};
use crate::errors::{VpnErrorCode, VpnResult, into_vpn_err};
use crate::server::{NetworkGroupId, NetworkId, NodeId};
use crate::{DataHeader, VpnTunnelFactory, VpnTunnelListener, VpnTunnelRecv, VpnTunnelSend};
use bucky_raw_codec::RawConvertTo;
use pnet_packet::Packet;
use pnet_packet::ip::{IpNextHeaderProtocol, IpNextHeaderProtocols};
use pnet_packet::ipv4::Ipv4Packet;
use pnet_packet::ipv6::Ipv6Packet;
use pnet_packet::tcp::TcpPacket;
use pnet_packet::udp::UdpPacket;
use sfo_pool::WorkerGuard;
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

const DEFAULT_SHARDS_PER_TARGET: usize = 8;
const DEFAULT_QUEUE_CAPACITY_PER_SHARD: usize = 1024;
const DEFAULT_MAX_BATCH_PKTS: usize = 32;
const DEFAULT_MAX_BATCH_BYTES: usize = 128 * 1024;
const DEFAULT_RECONNECT_BACKOFF_MIN_MS: u64 = 20;
const DEFAULT_RECONNECT_BACKOFF_MAX_MS: u64 = 1000;

#[derive(Debug, Clone)]
pub struct PacketDispatcherConfig {
    pub shards_per_target: usize,
    pub queue_capacity_per_shard: usize,
    pub max_batch_pkts: usize,
    pub max_batch_bytes: usize,
    pub reconnect_backoff_min_ms: u64,
    pub reconnect_backoff_max_ms: u64,
}

impl Default for PacketDispatcherConfig {
    fn default() -> Self {
        Self {
            shards_per_target: DEFAULT_SHARDS_PER_TARGET,
            queue_capacity_per_shard: DEFAULT_QUEUE_CAPACITY_PER_SHARD,
            max_batch_pkts: DEFAULT_MAX_BATCH_PKTS,
            max_batch_bytes: DEFAULT_MAX_BATCH_BYTES,
            reconnect_backoff_min_ms: DEFAULT_RECONNECT_BACKOFF_MIN_MS,
            reconnect_backoff_max_ms: DEFAULT_RECONNECT_BACKOFF_MAX_MS,
        }
    }
}

impl PacketDispatcherConfig {
    fn normalized(&self) -> Self {
        let reconnect_backoff_min_ms = self.reconnect_backoff_min_ms.max(1);
        let reconnect_backoff_max_ms = self.reconnect_backoff_max_ms.max(reconnect_backoff_min_ms);

        Self {
            shards_per_target: self.shards_per_target.max(1),
            queue_capacity_per_shard: self.queue_capacity_per_shard.max(1),
            max_batch_pkts: self.max_batch_pkts.max(1),
            max_batch_bytes: self.max_batch_bytes.max(1),
            reconnect_backoff_min_ms,
            reconnect_backoff_max_ms,
        }
    }
}

#[derive(Clone)]
struct OutboundPacket {
    header: Arc<[u8]>,
    payload: Arc<[u8]>,
    flow_hash: u64,
}

#[derive(Hash, Eq, PartialEq, Clone)]
struct DispatchTarget {
    network_group_id: NetworkGroupId,
    network_id: NetworkId,
    target: IpAddr,
}

struct TargetShard {
    tx: mpsc::Sender<OutboundPacket>,
    handle: JoinHandle<()>,
}

impl Drop for TargetShard {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

struct TargetDispatcher<
    R: VpnTunnelRecv,
    S: VpnTunnelSend,
    F: VpnTunnelFactory<R, S>,
    L: VpnTunnelListener<R, S>,
> {
    shards: Vec<TargetShard>,
    _p: std::marker::PhantomData<fn() -> (R, S, F, L)>,
}

impl<R: VpnTunnelRecv, S: VpnTunnelSend, F: VpnTunnelFactory<R, S>, L: VpnTunnelListener<R, S>>
    TargetDispatcher<R, S, F, L>
{
    fn new(
        tunnel_manager: Arc<TunnelManager<R, S, F, L>>,
        target: DispatchTarget,
        config: PacketDispatcherConfig,
    ) -> Self {
        let mut shards = Vec::with_capacity(config.shards_per_target);
        for _ in 0..config.shards_per_target {
            let (tx, rx) = mpsc::channel(config.queue_capacity_per_shard);
            let shard_target = target.clone();
            let shard_manager = tunnel_manager.clone();
            let shard_config = config.clone();
            let handle = tokio::spawn(async move {
                Self::shard_loop(shard_manager, shard_target, shard_config, rx).await;
            });
            shards.push(TargetShard { tx, handle });
        }

        Self {
            shards,
            _p: std::marker::PhantomData,
        }
    }

    async fn dispatch(&self, packet: OutboundPacket) -> VpnResult<()> {
        let shard_index = (packet.flow_hash as usize) % self.shards.len();
        self.shards[shard_index]
            .tx
            .send(packet)
            .await
            .map_err(into_vpn_err!(
                VpnErrorCode::Failed,
                "target shard queue closed"
            ))?;
        Ok(())
    }

    async fn shard_loop(
        tunnel_manager: Arc<TunnelManager<R, S, F, L>>,
        target: DispatchTarget,
        config: PacketDispatcherConfig,
        mut rx: mpsc::Receiver<OutboundPacket>,
    ) {
        let mut sender: Option<WorkerGuard<PkgSend<S>, Factory<R, S, F, L>>> = None;
        let mut backoff_ms = config.reconnect_backoff_min_ms;

        while let Some(first_packet) = rx.recv().await {
            let mut batch = Vec::with_capacity(config.max_batch_pkts);
            let mut batch_bytes = 0usize;

            batch_bytes += first_packet.header.len() + first_packet.payload.len();
            batch.push(first_packet);

            while batch.len() < config.max_batch_pkts && batch_bytes < config.max_batch_bytes {
                match rx.try_recv() {
                    Ok(packet) => {
                        let packet_bytes = packet.header.len() + packet.payload.len();
                        if batch_bytes + packet_bytes > config.max_batch_bytes {
                            break;
                        }
                        batch_bytes += packet_bytes;
                        batch.push(packet);
                    }
                    Err(_) => break,
                }
            }

            if sender.is_none() {
                match tunnel_manager
                    .get_send(target.network_group_id, target.network_id, target.target)
                    .await
                {
                    Ok(worker) => {
                        sender = Some(worker);
                        backoff_ms = config.reconnect_backoff_min_ms;
                    }
                    Err(e) => {
                        log::warn!(
                            "get sender failed for target {} network {}: {:?}",
                            target.target,
                            target.network_id,
                            e
                        );
                        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                        backoff_ms = (backoff_ms * 2).min(config.reconnect_backoff_max_ms);
                        continue;
                    }
                }
            }

            let mut failed = false;
            if let Some(writer) = sender.as_mut() {
                for packet in &batch {
                    if writer.write_all(packet.header.as_ref()).await.is_err() {
                        failed = true;
                        break;
                    }
                    if writer.write_all(packet.payload.as_ref()).await.is_err() {
                        failed = true;
                        break;
                    }
                }

                if !failed && writer.flush().await.is_err() {
                    failed = true;
                }
            }

            if failed {
                sender = None;
            }
        }
    }
}

pub(crate) struct PacketDispatcher<
    R: VpnTunnelRecv,
    S: VpnTunnelSend,
    F: VpnTunnelFactory<R, S>,
    L: VpnTunnelListener<R, S>,
> {
    config: PacketDispatcherConfig,
    tunnel_manager: Arc<TunnelManager<R, S, F, L>>,
    targets: Mutex<HashMap<DispatchTarget, Arc<TargetDispatcher<R, S, F, L>>>>,
}

impl<R: VpnTunnelRecv, S: VpnTunnelSend, F: VpnTunnelFactory<R, S>, L: VpnTunnelListener<R, S>>
    PacketDispatcher<R, S, F, L>
{
    pub fn new(
        tunnel_manager: Arc<TunnelManager<R, S, F, L>>,
        config: PacketDispatcherConfig,
    ) -> Arc<Self> {
        Arc::new(Self {
            config: config.normalized(),
            tunnel_manager,
            targets: Mutex::new(HashMap::new()),
        })
    }

    pub async fn dispatch(
        &self,
        network_group_id: NetworkGroupId,
        network_id: NetworkId,
        target: IpAddr,
        packet: &[u8],
        is_fanout: bool,
    ) -> VpnResult<()> {
        let outbound = OutboundPacket {
            header: encode_header(network_id, packet.len())?,
            payload: Arc::<[u8]>::from(packet.to_vec()),
            flow_hash: calc_flow_hash(packet),
        };

        if is_fanout {
            let targets = self.fanout_targets(network_group_id, network_id, target);
            for fanout_target in targets {
                let dispatcher = self.get_or_create_target_dispatcher(DispatchTarget {
                    network_group_id,
                    network_id,
                    target: fanout_target,
                });
                dispatcher.dispatch(outbound.clone()).await?;
            }
            return Ok(());
        }

        let dispatcher = self.get_or_create_target_dispatcher(DispatchTarget {
            network_group_id,
            network_id,
            target,
        });
        dispatcher.dispatch(outbound).await
    }

    fn get_or_create_target_dispatcher(
        &self,
        target: DispatchTarget,
    ) -> Arc<TargetDispatcher<R, S, F, L>> {
        let mut targets = self.targets.lock().unwrap();
        if let Some(dispatcher) = targets.get(&target) {
            return dispatcher.clone();
        }

        let dispatcher = Arc::new(TargetDispatcher::new(
            self.tunnel_manager.clone(),
            target.clone(),
            self.config.clone(),
        ));
        targets.insert(target, dispatcher.clone());
        dispatcher
    }

    fn fanout_targets(
        &self,
        network_group_id: NetworkGroupId,
        network_id: NetworkId,
        packet_target: IpAddr,
    ) -> Vec<IpAddr> {
        let all_nodes = self
            .tunnel_manager
            .get_router()
            .get_all_nodes(network_group_id, network_id);
        let mut targets = HashMap::<NodeId, IpAddr>::new();
        let expect_ipv6 = matches!(packet_target, IpAddr::V6(_));

        for (ip, node_id) in all_nodes {
            if expect_ipv6 != matches!(ip, IpAddr::V6(_)) {
                continue;
            }
            targets.entry(node_id).or_insert(ip);
        }

        targets.into_values().collect()
    }
}

fn encode_header(network_id: NetworkId, packet_len: usize) -> VpnResult<Arc<[u8]>> {
    let data_header = DataHeader {
        network_id,
        pkg_len: packet_len as u16,
    };
    let data_header = data_header
        .to_vec()
        .map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?;
    Ok(Arc::<[u8]>::from(data_header))
}

fn calc_flow_hash(packet: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    match packet.first().map(|v| v >> 4) {
        Some(4) => hash_ipv4_packet(packet, &mut hasher),
        Some(6) => hash_ipv6_packet(packet, &mut hasher),
        _ => packet.iter().take(64).for_each(|b| b.hash(&mut hasher)),
    }
    hasher.finish()
}

fn hash_ipv4_packet(packet: &[u8], hasher: &mut DefaultHasher) {
    if let Some(ipv4) = Ipv4Packet::new(packet) {
        ipv4.get_source().hash(hasher);
        ipv4.get_destination().hash(hasher);
        ipv4.get_next_level_protocol().0.hash(hasher);
        hash_transport(ipv4.get_next_level_protocol(), ipv4.payload(), hasher);
        return;
    }

    packet.iter().take(64).for_each(|b| b.hash(hasher));
}

fn hash_ipv6_packet(packet: &[u8], hasher: &mut DefaultHasher) {
    if let Some(ipv6) = Ipv6Packet::new(packet) {
        ipv6.get_source().hash(hasher);
        ipv6.get_destination().hash(hasher);
        ipv6.get_next_header().0.hash(hasher);
        hash_transport(ipv6.get_next_header(), ipv6.payload(), hasher);
        return;
    }

    packet.iter().take(64).for_each(|b| b.hash(hasher));
}

fn hash_transport(protocol: IpNextHeaderProtocol, payload: &[u8], hasher: &mut DefaultHasher) {
    match protocol {
        IpNextHeaderProtocols::Tcp => {
            if let Some(tcp) = TcpPacket::new(payload) {
                tcp.get_source().hash(hasher);
                tcp.get_destination().hash(hasher);
            }
        }
        IpNextHeaderProtocols::Udp => {
            if let Some(udp) = UdpPacket::new(payload) {
                udp.get_source().hash(hasher);
                udp.get_destination().hash(hasher);
            }
        }
        _ => {}
    }
}
