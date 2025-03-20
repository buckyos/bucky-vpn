use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex, Weak};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use bucky_raw_codec::{RawConvertTo, RawDecode};
use sfo_cmd_server::client::CmdClient;
use tokio::io::AsyncWriteExt;
use tokio::task::JoinHandle;
use crate::client::{PacketRecv, VpnDevice, VpnServerClient};
use crate::client::tunnel_manager::{TunnelManager, TunnelPkgRecv};
use crate::errors::{into_vpn_err, vpn_err, VpnErrorCode, VpnResult};
use crate::server::{NetworkGroupId, NetworkId};
use crate::{DataHeader, VpnCmdCode, VpnCmdHeader, VpnTunnelFactory, VpnTunnelListener, VpnTunnelRecv, VpnTunnelSend};

struct DevicePkgRecv<
    R: VpnTunnelRecv,
    S: VpnTunnelSend,
    F: VpnTunnelFactory<R, S>,
    L: VpnTunnelListener<R, S>> {
    tunnel_manager: Arc<TunnelManager<R, S, F, L>>,
    network_group_id: NetworkGroupId,
    network_id: NetworkId,
    ipv4_mask: u8,
    ipv6_mask: u8,
}

impl<R: VpnTunnelRecv,
    S: VpnTunnelSend,
    F: VpnTunnelFactory<R, S>,
    L: VpnTunnelListener<R, S>> DevicePkgRecv<R, S, F, L> {
    pub fn new(tunnel_manager: Arc<TunnelManager<R, S, F, L>>,
               network_group_id: NetworkGroupId,
               network_id: NetworkId,
               ipv4_mask: u8,
               ipv6_mask: u8) -> Self {
        Self {
            tunnel_manager,
            network_group_id,
            network_id,
            ipv4_mask,
            ipv6_mask,
        }
    }
}

#[async_trait::async_trait]
impl<R: VpnTunnelRecv,
    S: VpnTunnelSend,
    F: VpnTunnelFactory<R, S>,
    L: VpnTunnelListener<R, S>> PacketRecv for DevicePkgRecv<R, S, F, L> {
    async fn on_recv(&self, target: IpAddr, packet: &[u8]) -> VpnResult<()> {
        // 首先判断目标ip地址是否是广播地址
        if let IpAddr::V4(ipv4) = target {
            let is_broadcast = if self.ipv4_mask < 32 {
                // Calculate the broadcast address using the netmask
                let host_bits = 32 - self.ipv4_mask;
                let host_mask = (1u32 << host_bits) - 1;
                let addr_bits = u32::from_be_bytes(ipv4.octets());
                (addr_bits & host_mask) == host_mask
            } else {
                false
            };

            if is_broadcast || target.is_multicast() {
                let all_send = self.tunnel_manager.get_all_send(self.network_group_id, self.network_id).await?;
                for mut send in all_send {
                    let data_header = DataHeader {
                        dest_ip: target,
                        network_id: self.network_id,
                        group_id: self.network_group_id,
                    };
                    let data_header = data_header.to_vec().map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?;
                    let data_cmd = VpnCmdHeader::new(0, VpnCmdCode::Data as u8, (data_header.len() + packet.len()) as u16);
                    send.write_all(data_cmd.to_vec().map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?.as_slice()).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
                    send.write_all(data_header.as_slice()).await.map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?;
                    send.write_all(packet).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
                    send.flush().await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
                }
                return Ok(());
            }
        }
        let mut send = self.tunnel_manager.get_send(self.network_group_id, self.network_id, target).await?;
        let data_header = DataHeader {
            dest_ip: target,
            network_id: self.network_id,
            group_id: self.network_group_id,
        };
        let data_header = data_header.to_vec().map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?;
        let data_cmd = VpnCmdHeader::new(0, VpnCmdCode::Data as u8, (data_header.len() + packet.len()) as u16);

        send.write_all(data_cmd.to_vec().map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?.as_slice()).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        send.write_all(data_header.as_slice()).await.map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?;
        send.write_all(packet).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        send.flush().await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }
}

struct ClientTunnelPkgRecv<
    T: CmdClient<u16, u8>,
    R: VpnTunnelRecv,
    S: VpnTunnelSend,
    F: VpnTunnelFactory<R, S>,
    L: VpnTunnelListener<R, S>> {
    vpn_client: Weak<VpnClient<T, R, S, F, L>>
}

impl<T: CmdClient<u16, u8>,
    R: VpnTunnelRecv,
    S: VpnTunnelSend,
    F: VpnTunnelFactory<R, S>,
    L: VpnTunnelListener<R, S>> ClientTunnelPkgRecv<T, R, S, F, L> {
    pub fn new(vpn_client: Weak<VpnClient<T, R, S, F, L>>) -> Self {
        Self {
            vpn_client,
        }
    }
}

#[async_trait::async_trait]
impl<T: CmdClient<u16, u8>,
    R: VpnTunnelRecv,
    S: VpnTunnelSend,
    F: VpnTunnelFactory<R, S>,
    L: VpnTunnelListener<R, S>> TunnelPkgRecv for ClientTunnelPkgRecv<T, R, S, F, L> {
    async fn on_recv(&self, data: Vec<u8>) -> VpnResult<()> {
        let (header, pkg) = DataHeader::raw_decode(data.as_slice()).map_err(into_vpn_err!(VpnErrorCode::RawCodecError))?;
        if let Some(vpn_client) = self.vpn_client.upgrade() {
            let device = {
                let vpn_devices = vpn_client.vpn_devices.lock().unwrap();
                if let Some(device) = vpn_devices.as_ref().unwrap().get(&header.network_id) {
                    device.get_send()
                } else {
                    None
                }
            };
            if let Some(device) = device {
                device.send(pkg).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
            } else {
                log::error!("device network {} ip {} unavailable", header.network_id, header.dest_ip.to_string());
            }
            Ok(())
        } else {
            Err(vpn_err!(VpnErrorCode::Failed))
        }
    }
}
pub struct VpnClient<T: CmdClient<u16, u8>, R: VpnTunnelRecv, S: VpnTunnelSend, F: VpnTunnelFactory<R, S>, L: VpnTunnelListener<R, S>> {
    server_client: Arc<VpnServerClient<T>>,
    vpn_devices: Mutex<Option<HashMap<NetworkId, VpnDevice<DevicePkgRecv<R, S, F, L>>>>>,
    tunnel_manager: Mutex<Option<Arc<TunnelManager<R, S, F, L>>>>,
    run_handle: Mutex<Option<JoinHandle<()>>>,
    cur_version: AtomicU64,
    client_version: String,
}

impl<T: CmdClient<u16, u8>, R: VpnTunnelRecv, S: VpnTunnelSend, F: VpnTunnelFactory<R, S>, L: VpnTunnelListener<R, S>> VpnClient<T, R, S, F, L> {
    pub fn new(server_client: Arc<VpnServerClient<T>>, tunnel_factory: Arc<F>, tunnel_listener: Arc<L>, client_version: String) -> Arc<Self> {
        let this = Arc::new(Self {
            server_client: server_client.clone(),
            vpn_devices: Mutex::new(Some(HashMap::new())),
            tunnel_manager: Mutex::new(None),
            run_handle: Mutex::new(None),
            cur_version: AtomicU64::new(0),
            client_version,
        });
        let tunnel_manager = TunnelManager::new(tunnel_factory,
                                                tunnel_listener,
                                                Arc::new(ClientTunnelPkgRecv::new(Arc::downgrade(&this))));
        *this.tunnel_manager.lock().unwrap() = Some(Arc::new(tunnel_manager));
        this
    }

    pub fn run(self: &Arc<Self>) {
        let this = self.clone();
        let mut run_handle = self.run_handle.lock().unwrap();
        if run_handle.is_some() {
            return;
        }
        let handle = tokio::spawn(async move {
            loop {
                if let Err(e) = this.run_proc().await {
                    log::error!("run_proc failed: {:?}", e);
                }
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        });
        *run_handle = Some(handle);
    }

    async fn run_proc(self: &Arc<Self>) -> VpnResult<()> {
        let (server_version, vpn_infos) = self.server_client.get_vpn_info(self.cur_version.load(Ordering::SeqCst), self.client_version.clone()).await?;
        if server_version == self.cur_version.load(Ordering::SeqCst) {
            return Ok(());
        }
        let mut vpn_devices = {
            let mut vpn_devices = self.vpn_devices.lock().unwrap();
            let devices = vpn_devices.take().unwrap();
            *vpn_devices = Some(HashMap::new());
            devices
        };
        let tunnel_manager = {
            let tunnel_manager = self.tunnel_manager.lock().unwrap();
            if tunnel_manager.is_none() {
                return Ok(());
            }
            tunnel_manager.as_ref().unwrap().clone()
        };
        for vpn_info in vpn_infos {
            let group_id = vpn_info.node_info.group_id;
            let network_id = vpn_info.node_info.id;
            let members = vpn_info.members
                .iter()
                .filter(|x| vpn_info.node_info.ip.is_none() || x.ip != vpn_info.node_info.ip.as_ref().unwrap().to_string()).map(|x| x.clone()).collect::<Vec<_>>();
            let vpn_device = vpn_devices.remove(&vpn_info.node_info.id);
            if vpn_device.is_none() {
                let mut vpn_device = VpnDevice::new(vpn_info.node_info.clone());
                let tunnel_manager = {
                    let tunnel_manager = self.tunnel_manager.lock().unwrap();
                    tunnel_manager.as_ref().unwrap().clone()
                };
                if let Ok(_) = vpn_device.start(Arc::new(DevicePkgRecv::new(tunnel_manager,
                                                                            vpn_info.node_info.group_id,
                                                                            vpn_info.node_info.id,
                                                                            vpn_info.node_info.mask,
                                                                            vpn_info.node_info.ipv6_mask))) {
                    let mut vpn_devices = self.vpn_devices.lock().unwrap();
                    vpn_devices.as_mut().unwrap().insert(network_id, vpn_device);
                }
            } else {
                let mut vpn_device = vpn_device.unwrap();
                if let Ok(_) = vpn_device.update_device(vpn_info.node_info) {
                    let mut vpn_devices = self.vpn_devices.lock().unwrap();
                    vpn_devices.as_mut().unwrap().insert(network_id, vpn_device);
                }
            }

            tunnel_manager.get_router().add_network(group_id, network_id, members);
        }
        self.cur_version.store(server_version, Ordering::SeqCst);
        Ok(())
    }

    pub async fn join(&self, network_group_id: NetworkGroupId, name: Option<String>) -> VpnResult<()> {
        self.server_client.join_network_group(network_group_id, name).await
    }
}

impl<T: CmdClient<u16, u8>,
    R: VpnTunnelRecv,
    S: VpnTunnelSend,
    F: VpnTunnelFactory<R, S>,
    L: VpnTunnelListener<R, S>> Drop for VpnClient<T, R, S, F, L> {
    fn drop(&mut self) {
        log::info!("Vpn client dropped");
        let mut run_handle = self.run_handle.lock().unwrap();
        if let Some(handle) = run_handle.take() {
            handle.abort();
        }
    }
}
