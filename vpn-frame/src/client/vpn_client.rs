#![allow(unused)]

use crate::client::PacketDispatcherConfig;
use crate::client::packet_dispatcher::PacketDispatcher;
use crate::client::tunnel_manager::{TunnelManager, TunnelPkgRecv};
use crate::client::{PacketRecv, VpnDevice, VpnServerClient};
use crate::errors::{VpnErrorCode, VpnResult, into_vpn_err, vpn_err};
use crate::server::{NetworkGroupId, NetworkId};
use crate::{
    VpnCmdCode, VpnCmdHeader, VpnTunnelFactory, VpnTunnelListener, VpnTunnelRecv, VpnTunnelSend,
};
use bucky_raw_codec::RawDecode;
use sfo_cmd_server::CmdTunnelMeta;
use sfo_cmd_server::client::{CmdClient, CmdSend, SendGuard};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;
use tokio::task::JoinHandle;

struct DevicePkgRecv<
    R: VpnTunnelRecv,
    S: VpnTunnelSend,
    F: VpnTunnelFactory<R, S>,
    L: VpnTunnelListener<R, S>,
> {
    packet_dispatcher: Arc<PacketDispatcher<R, S, F, L>>,
    network_group_id: NetworkGroupId,
    network_id: NetworkId,
    ipv4_mask: u8,
    ipv6_mask: u8,
}

impl<R: VpnTunnelRecv, S: VpnTunnelSend, F: VpnTunnelFactory<R, S>, L: VpnTunnelListener<R, S>>
    DevicePkgRecv<R, S, F, L>
{
    pub fn new(
        packet_dispatcher: Arc<PacketDispatcher<R, S, F, L>>,
        network_group_id: NetworkGroupId,
        network_id: NetworkId,
        ipv4_mask: u8,
        ipv6_mask: u8,
    ) -> Self {
        Self {
            packet_dispatcher,
            network_group_id,
            network_id,
            ipv4_mask,
            ipv6_mask,
        }
    }
}

#[async_trait::async_trait]
impl<R: VpnTunnelRecv, S: VpnTunnelSend, F: VpnTunnelFactory<R, S>, L: VpnTunnelListener<R, S>>
    PacketRecv for DevicePkgRecv<R, S, F, L>
{
    async fn on_recv(&self, target: IpAddr, packet: &[u8]) -> VpnResult<()> {
        self.packet_dispatcher
            .dispatch(
                self.network_group_id,
                self.network_id,
                target,
                packet,
                is_broadcast_or_multicast(target, self.ipv4_mask),
            )
            .await
    }
}

struct ClientTunnelPkgRecv<
    M: CmdTunnelMeta,
    CS: CmdSend<M>,
    G: SendGuard<M, CS>,
    T: CmdClient<u16, u8, M, CS, G>,
    R: VpnTunnelRecv,
    S: VpnTunnelSend,
    F: VpnTunnelFactory<R, S>,
    L: VpnTunnelListener<R, S>,
> {
    vpn_client: Weak<VpnClient<M, CS, G, T, R, S, F, L>>,
}

impl<
    M: CmdTunnelMeta,
    CS: CmdSend<M>,
    G: SendGuard<M, CS>,
    T: CmdClient<u16, u8, M, CS, G>,
    R: VpnTunnelRecv,
    S: VpnTunnelSend,
    F: VpnTunnelFactory<R, S>,
    L: VpnTunnelListener<R, S>,
> ClientTunnelPkgRecv<M, CS, G, T, R, S, F, L>
{
    pub fn new(vpn_client: Weak<VpnClient<M, CS, G, T, R, S, F, L>>) -> Self {
        Self { vpn_client }
    }
}

#[async_trait::async_trait]
impl<
    M: CmdTunnelMeta,
    CS: CmdSend<M>,
    G: SendGuard<M, CS>,
    T: CmdClient<u16, u8, M, CS, G>,
    R: VpnTunnelRecv,
    S: VpnTunnelSend,
    F: VpnTunnelFactory<R, S>,
    L: VpnTunnelListener<R, S>,
> TunnelPkgRecv for ClientTunnelPkgRecv<M, CS, G, T, R, S, F, L>
{
    async fn on_recv(&self, network_id: NetworkId, data: Vec<u8>) -> VpnResult<()> {
        if let Some(vpn_client) = self.vpn_client.upgrade() {
            let device = {
                let vpn_devices = vpn_client.vpn_devices.lock().unwrap();
                if let Some(device) = vpn_devices.as_ref().unwrap().get(&network_id) {
                    device.get_send()
                } else {
                    None
                }
            };
            if let Some(device) = device {
                device
                    .send(data.as_slice())
                    .await
                    .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
            } else {
                log::error!("device network {} unavailable", network_id);
            }
            Ok(())
        } else {
            Err(vpn_err!(VpnErrorCode::Failed))
        }
    }
}
pub struct VpnClient<
    M: CmdTunnelMeta,
    CS: CmdSend<M>,
    G: SendGuard<M, CS>,
    T: CmdClient<u16, u8, M, CS, G>,
    R: VpnTunnelRecv,
    S: VpnTunnelSend,
    F: VpnTunnelFactory<R, S>,
    L: VpnTunnelListener<R, S>,
> {
    server_client: Arc<VpnServerClient<M, CS, G, T>>,
    vpn_devices: Mutex<Option<HashMap<NetworkId, VpnDevice<DevicePkgRecv<R, S, F, L>>>>>,
    tunnel_manager: Mutex<Option<Arc<TunnelManager<R, S, F, L>>>>,
    packet_dispatcher: Mutex<Option<Arc<PacketDispatcher<R, S, F, L>>>>,
    run_handle: Mutex<Option<JoinHandle<()>>>,
    cur_version: AtomicU16,
    is_first: AtomicBool,
    client_version: String,
}

impl<
    M: CmdTunnelMeta,
    CS: CmdSend<M>,
    G: SendGuard<M, CS>,
    T: CmdClient<u16, u8, M, CS, G>,
    R: VpnTunnelRecv,
    S: VpnTunnelSend,
    F: VpnTunnelFactory<R, S>,
    L: VpnTunnelListener<R, S>,
> VpnClient<M, CS, G, T, R, S, F, L>
{
    pub fn new(
        server_client: Arc<VpnServerClient<M, CS, G, T>>,
        tunnel_factory: Arc<F>,
        tunnel_listener: Arc<L>,
        client_version: String,
    ) -> Arc<Self> {
        Self::new_with_packet_dispatcher_config(
            server_client,
            tunnel_factory,
            tunnel_listener,
            client_version,
            PacketDispatcherConfig::default(),
        )
    }

    pub fn new_with_packet_dispatcher_config(
        server_client: Arc<VpnServerClient<M, CS, G, T>>,
        tunnel_factory: Arc<F>,
        tunnel_listener: Arc<L>,
        client_version: String,
        packet_dispatcher_config: PacketDispatcherConfig,
    ) -> Arc<Self> {
        let this = Arc::new(Self {
            server_client: server_client.clone(),
            vpn_devices: Mutex::new(Some(HashMap::new())),
            tunnel_manager: Mutex::new(None),
            packet_dispatcher: Mutex::new(None),
            run_handle: Mutex::new(None),
            cur_version: AtomicU16::new(0),
            is_first: AtomicBool::new(true),
            client_version,
        });
        let tunnel_manager = TunnelManager::new(
            tunnel_factory,
            tunnel_listener,
            Arc::new(ClientTunnelPkgRecv::new(Arc::downgrade(&this))),
        );
        let tunnel_manager = Arc::new(tunnel_manager);
        *this.packet_dispatcher.lock().unwrap() = Some(PacketDispatcher::new(
            tunnel_manager.clone(),
            packet_dispatcher_config,
        ));
        *this.tunnel_manager.lock().unwrap() = Some(tunnel_manager);
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
        let (server_version, vpn_infos) = if self.is_first.load(Ordering::Relaxed) {
            let (server_version, vpn_infos) = self
                .server_client
                .get_vpn_info(None, Some(self.client_version.clone()))
                .await?;
            (server_version, vpn_infos)
        } else {
            self.server_client
                .get_vpn_info(Some(self.cur_version.load(Ordering::SeqCst)), None)
                .await?
        };

        if !self.is_first.load(Ordering::Relaxed)
            && server_version == self.cur_version.load(Ordering::SeqCst)
        {
            return Ok(());
        }
        self.is_first.store(false, Ordering::Relaxed);
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
            let members = vpn_info
                .members
                .iter()
                .filter(|x| {
                    vpn_info.node_info.ip.is_none()
                        || x.ip != vpn_info.node_info.ip.as_ref().unwrap().to_string()
                })
                .map(|x| x.clone())
                .collect::<Vec<_>>();
            let vpn_device = vpn_devices.remove(&vpn_info.node_info.id);
            if vpn_device.is_none() {
                let mut vpn_device = VpnDevice::new(vpn_info.node_info.clone());
                let packet_dispatcher = {
                    let packet_dispatcher = self.packet_dispatcher.lock().unwrap();
                    packet_dispatcher.as_ref().unwrap().clone()
                };
                if let Ok(_) = vpn_device.start(Arc::new(DevicePkgRecv::new(
                    packet_dispatcher,
                    vpn_info.node_info.group_id,
                    vpn_info.node_info.id,
                    vpn_info.node_info.mask,
                    vpn_info.node_info.ipv6_mask,
                ))) {
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

            tunnel_manager
                .get_router()
                .add_network(group_id, network_id, members);
        }
        self.cur_version.store(server_version, Ordering::SeqCst);
        Ok(())
    }

    pub async fn join(
        &self,
        network_group_id: NetworkGroupId,
        name: Option<String>,
    ) -> VpnResult<()> {
        self.server_client
            .join_network_group(network_group_id, name)
            .await
    }
}

fn is_broadcast_or_multicast(target: IpAddr, ipv4_mask: u8) -> bool {
    if target.is_multicast() {
        return true;
    }

    if let IpAddr::V4(ipv4) = target {
        if ipv4_mask < 32 {
            let host_bits = 32 - ipv4_mask;
            let host_mask = (1u32 << host_bits) - 1;
            let addr_bits = u32::from_be_bytes(ipv4.octets());
            return (addr_bits & host_mask) == host_mask;
        }
    }

    false
}

impl<
    M: CmdTunnelMeta,
    CS: CmdSend<M>,
    G: SendGuard<M, CS>,
    T: CmdClient<u16, u8, M, CS, G>,
    R: VpnTunnelRecv,
    S: VpnTunnelSend,
    F: VpnTunnelFactory<R, S>,
    L: VpnTunnelListener<R, S>,
> Drop for VpnClient<M, CS, G, T, R, S, F, L>
{
    fn drop(&mut self) {
        log::info!("Vpn client dropped");
        let mut run_handle = self.run_handle.lock().unwrap();
        if let Some(handle) = run_handle.take() {
            handle.abort();
        }
    }
}
