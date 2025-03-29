use std::net::IpAddr;
use std::sync::Arc;
use pnet_packet::ipv4::Ipv4Packet;
use pnet_packet::ipv6::Ipv6Packet;
use tokio::task::JoinHandle;
use tun_rs::{AsyncDevice, Layer, ToIpv4Address, ToIpv6Address};
use crate::errors::{into_vpn_err, VpnErrorCode, VpnResult};
use crate::NodeNetwork;

#[async_trait::async_trait]
pub trait PacketRecv: Send + Sync + 'static {
    async fn on_recv(&self, target: IpAddr, packet: &[u8]) -> VpnResult<()>;
}

fn ip_version(packet: &[u8]) -> u8 {
    let version = packet[0] >> 4;
    version
}

pub struct DeviceSend {
    dev: Arc<AsyncDevice>,
}

impl DeviceSend {
    pub fn new(dev: Arc<AsyncDevice>) -> Self {
        Self {
            dev
        }
    }

    pub async fn send(&self, packet: &[u8]) -> VpnResult<()> {
        self.dev.send(packet).await.map_err(into_vpn_err!(VpnErrorCode::Failed))?;
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
            log::info!("create tun device {} ip {}", self.network.name, self.network.ip.as_ref().unwrap().to_string());
            config = config.ipv4(self.network.ip.as_ref().unwrap().clone(), self.network.mask, None);
        }
        if self.network.ipv6.is_some() {
            config = config.ipv6(self.network.ipv6.as_ref().unwrap().clone(), self.network.mask);
        }

        #[cfg(windows)]
        {
            config = config.wintun_file("./wintun.dll".to_string());
        }

        let truncated_name = if cfg!(target_os = "macos") {
            let name =  format!("utun{}", self.network.id);
            // MacOS: 15 chars max
            name[..std::cmp::min(name.len(), 8)].to_string()
        } else if cfg!(target_os = "linux") {
            let name =  format!("tun_{}", self.network.id);
            // Linux: 15 chars max
            name[..std::cmp::min(name.len(), 15)].to_string()
        } else if cfg!(windows) {
            let name =  format!("tun_{}", self.network.id);
            // Windows: 32 chars max for compatibility (even though wintun allows 128)
            name[..std::cmp::min(name.len(), 32)].to_string()
        } else {
            let name =  format!("tun_{}", self.network.id);
            name[..std::cmp::min(name.len(), 15)].to_string()
        };
        let dev = config
            .name(truncated_name)
            .mtu(1400)
            .layer(Layer::L3)
            .build_async().map_err(into_vpn_err!(VpnErrorCode::Failed))?;
        self.dev = Some(Arc::new(dev));

        Ok(())
    }

    pub fn start(&mut self, recv: Arc<S>) -> VpnResult<()> {
        self.create_device()?;
        let dev = self.dev.clone().unwrap();
        let network = self.network.clone();
        let handle = tokio::spawn(async move {
            let mut buf = [0;65535];
            loop {
                match dev.recv(&mut buf).await {
                    Ok(size) => {
                        let packet = &buf[..size];
                        match ip_version(&buf[..size]) {
                            4 => {
                                if network.ip.is_some() {
                                    let mask = u32::MAX << (32 - network.mask);
                                    if let Some(ip_pkg) = Ipv4Packet::new(packet) {
                                        let target = ip_pkg.get_destination();
                                        if network.ip.as_ref().unwrap().ipv4().unwrap().to_bits() & mask != target.to_bits() & mask {
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
                                        if u128::from(network.ipv6.as_ref().unwrap().ipv6().unwrap()) & mask != u128::from(target) & mask {
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
        });
        self.handle = Some(handle);
        Ok(())
    }

    pub fn get_send(&self) -> Option<DeviceSend> {
        self.dev.clone().map(|dev| DeviceSend::new(dev))
    }

    pub fn update_device(&mut self, network: NodeNetwork) -> VpnResult<()> {
        if self.network == network {
            return Ok(());
        }

        self.network = network;
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }

        self.dev.take();

        let recv = self.recv.take().unwrap();
        self.start(recv)?;
        Ok(())
    }
}

impl<S: PacketRecv> Drop for VpnDevice<S> {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }

        log::info!("drop tun device {} ip {}", self.network.name, self.network.ip.as_ref().unwrap().to_string());
        let _ = self.dev.take();
    }
}
