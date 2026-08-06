use crate::NodeNetwork;
use crate::errors::{VpnErrorCode, VpnResult, into_vpn_err};
use pnet_packet::ipv4::Ipv4Packet;
use pnet_packet::ipv6::Ipv6Packet;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tun_rs::{AsyncDevice, Layer, ToIpv4Address, ToIpv6Address};

#[async_trait::async_trait]
pub trait PacketRecv: Send + Sync + 'static {
    async fn on_recv(&self, target: IpAddr, packet: &[u8]) -> VpnResult<()>;
}

fn ip_version(packet: &[u8]) -> u8 {
    let version = packet[0] >> 4;
    version
}

fn spawn_recv_task<S: PacketRecv>(
    dev: Arc<AsyncDevice>,
    network: NodeNetwork,
    recv: Arc<S>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut buf = [0; 65535];
        loop {
            match dev.recv(&mut buf).await {
                Ok(size) => {
                    let packet = &buf[..size];
                    match ip_version(packet) {
                        4 => {
                            if network.ip.is_some() {
                                let mask = u32::MAX << (32 - network.mask);
                                if let Some(ip_pkg) = Ipv4Packet::new(packet) {
                                    let target = ip_pkg.get_destination();
                                    if network.ip.as_ref().unwrap().ipv4().unwrap().to_bits()
                                        & mask
                                        != target.to_bits() & mask
                                    {
                                        continue;
                                    }
                                    if let Err(e) = recv.on_recv(IpAddr::V4(target), packet).await {
                                        log::error!("failed to process packet: {:?}", e);
                                    }
                                }
                            }
                        }
                        6 => {
                            if network.ipv6.is_some() {
                                let mask = u128::MAX << (128 - network.ipv6_mask);
                                if let Some(ip_pkg) = Ipv6Packet::new(packet) {
                                    let target = ip_pkg.get_destination();
                                    if u128::from(
                                        network.ipv6.as_ref().unwrap().ipv6().unwrap(),
                                    ) & mask
                                        != u128::from(target) & mask
                                    {
                                        continue;
                                    }
                                    if let Err(e) = recv.on_recv(IpAddr::V6(target), packet).await {
                                        log::error!("failed to process packet: {:?}", e);
                                    }
                                }
                            }
                        }
                        v => {
                            log::error!("unsupported ethertype: {:?}", v);
                        }
                    }
                }
                Err(_e) => {
                    log::error!("failed to receive packet");
                    break;
                }
            }
        }
    })
}

pub struct DeviceSend {
    dev: Arc<AsyncDevice>,
}

impl DeviceSend {
    pub fn new(dev: Arc<AsyncDevice>) -> Self {
        Self { dev }
    }

    pub async fn send(&self, packet: &[u8]) -> VpnResult<()> {
        self.dev
            .send(packet)
            .await
            .map_err(into_vpn_err!(VpnErrorCode::Failed))?;
        Ok(())
    }
}

pub struct VpnDevice<S: PacketRecv> {
    network: NodeNetwork,
    dev: Option<Arc<AsyncDevice>>,
    handle: Option<JoinHandle<()>>,
    recv: Option<Arc<S>>,
}

impl<S: PacketRecv> VpnDevice<S> {
    pub fn new(network: NodeNetwork) -> Self {
        Self {
            network,
            dev: None,
            handle: None,
            recv: None,
        }
    }

    pub fn network_info(&self) -> &NodeNetwork {
        &self.network
    }

    pub fn create_device(&mut self) -> VpnResult<()> {
        let mut config = tun_rs::DeviceBuilder::new();

        if self.network.ip.is_some() {
            log::info!(
                "create tun device {} ip {}",
                self.network.name,
                self.network.ip.as_ref().unwrap().to_string()
            );
            config = config.ipv4(
                self.network.ip.as_ref().unwrap().clone(),
                self.network.mask,
                None,
            );
        }
        if self.network.ipv6.is_some() {
            config = config.ipv6(
                self.network.ipv6.as_ref().unwrap().clone(),
                self.network.ipv6_mask,
            );
        }

        #[cfg(windows)]
        {
            config = config.wintun_file("./wintun.dll".to_string());
        }

        let truncated_name = if cfg!(target_os = "macos") {
            let name = format!("utun{}", self.network.id);
            // MacOS: 15 chars max
            name[..std::cmp::min(name.len(), 8)].to_string()
        } else if cfg!(target_os = "linux") {
            let name = format!("tun_{}", self.network.id);
            // Linux: 15 chars max
            name[..std::cmp::min(name.len(), 15)].to_string()
        } else if cfg!(windows) {
            let name = format!("tun_{}", self.network.id);
            // Windows: 32 chars max for compatibility (even though wintun allows 128)
            name[..std::cmp::min(name.len(), 32)].to_string()
        } else {
            let name = format!("tun_{}", self.network.id);
            name[..std::cmp::min(name.len(), 15)].to_string()
        };
        let dev = config
            .name(truncated_name)
            .mtu(1400)
            .layer(Layer::L3)
            .build_async()
            .map_err(into_vpn_err!(VpnErrorCode::Failed))?;
        self.dev = Some(Arc::new(dev));

        Ok(())
    }

    pub fn start(&mut self, recv: Arc<S>) -> VpnResult<()> {
        self.recv = Some(recv);
        if self.dev.is_none() {
            self.create_device().map_err(|e| {
                log::error!(
                    "failed to create tun device for network {}: {:?}",
                    self.network.id,
                    e
                );
                e
            })?;
        }
        self.restart_recv_task();
        Ok(())
    }

    pub fn get_send(&self) -> Option<DeviceSend> {
        self.dev.clone().map(|dev| DeviceSend::new(dev))
    }

    pub fn update_device(&mut self, network: NodeNetwork) -> VpnResult<()> {
        if self.network == network {
            return Ok(());
        }

        if let Some(recv) = self.recv.clone() {
            return self.reconcile(network, recv);
        }

        let tun_changed = Self::tun_effective_changed(&self.network, &network);
        self.network = network;
        if tun_changed {
            self.stop_recv_task();
            self.dev.take();
        }
        Ok(())
    }

    pub(crate) fn reconcile(&mut self, network: NodeNetwork, recv: Arc<S>) -> VpnResult<()> {
        let tun_changed = Self::tun_effective_changed(&self.network, &network);
        let dispatch_changed = self.network.group_id != network.group_id;

        // Save the desired snapshot and receive context before destructive work so a
        // failed create remains managed and can be retried by the next refresh.
        self.network = network;
        self.recv = Some(recv);

        if tun_changed || self.dev.is_none() {
            self.stop_recv_task();
            self.dev.take();
            self.create_device().map_err(|e| {
                log::error!(
                    "failed to reconcile tun device for network {}: {:?}",
                    self.network.id,
                    e
                );
                e
            })?;
            self.restart_recv_task();
        } else if dispatch_changed
            || self
                .handle
                .as_ref()
                .map_or(true, JoinHandle::is_finished)
        {
            self.restart_recv_task();
        }

        Ok(())
    }

    fn tun_effective_changed(current: &NodeNetwork, desired: &NodeNetwork) -> bool {
        current.id != desired.id
            || current.ip != desired.ip
            || current.mask != desired.mask
            || current.ipv6 != desired.ipv6
            || current.ipv6_mask != desired.ipv6_mask
    }

    fn stop_recv_task(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }

    fn restart_recv_task(&mut self) {
        self.stop_recv_task();
        if let (Some(dev), Some(recv)) = (self.dev.clone(), self.recv.clone()) {
            self.handle = Some(spawn_recv_task(dev, self.network.clone(), recv));
        }
    }
}

impl<S: PacketRecv> Drop for VpnDevice<S> {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }

        log::info!(
            "drop tun device {} ip {}",
            self.network.name,
            self.network.ip.as_ref().unwrap().to_string()
        );
        let _ = self.dev.take();
    }
}
