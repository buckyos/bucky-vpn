use crate::client::vpn_client::VpnClient;
use crate::errors::VpnResult;
use crate::{VpnTunnelFactory, VpnTunnelListener, VpnTunnelRecv, VpnTunnelSend};
use async_trait::async_trait;
use sfo_cmd_server::CmdTunnelMeta;
use sfo_cmd_server::client::{CmdClient, CmdSend, SendGuard};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[async_trait]
pub trait VpnClientFactory<
    M: CmdTunnelMeta,
    CS: CmdSend<M>,
    G: SendGuard<M, CS>,
    T: CmdClient<u16, u8, M, CS, G>,
    R: VpnTunnelRecv,
    S: VpnTunnelSend,
    F: VpnTunnelFactory<R, S>,
    L: VpnTunnelListener<R, S>,
>: Send + Sync + 'static
{
    async fn create_client(&self, key: &str) -> VpnResult<Arc<VpnClient<M, CS, G, T, R, S, F, L>>>;
}

pub struct VpnClientManager<
    M: CmdTunnelMeta,
    CS: CmdSend<M>,
    G: SendGuard<M, CS>,
    T: CmdClient<u16, u8, M, CS, G>,
    R: VpnTunnelRecv,
    S: VpnTunnelSend,
    F: VpnTunnelFactory<R, S>,
    L: VpnTunnelListener<R, S>,
    CF: VpnClientFactory<M, CS, G, T, R, S, F, L>,
> {
    clients: Mutex<HashMap<String, Arc<VpnClient<M, CS, G, T, R, S, F, L>>>>,
    factory: Arc<CF>,
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
    CF: VpnClientFactory<M, CS, G, T, R, S, F, L>,
> VpnClientManager<M, CS, G, T, R, S, F, L, CF>
{
    pub fn new(factory: Arc<CF>) -> Self {
        Self {
            clients: Mutex::new(HashMap::new()),
            factory,
        }
    }

    pub async fn get_client(
        &self,
        key: &str,
    ) -> VpnResult<Arc<VpnClient<M, CS, G, T, R, S, F, L>>> {
        {
            let clients = self.clients.lock().unwrap();
            if let Some(client) = clients.get(key) {
                return Ok(client.clone());
            }
        }

        let client = self.factory.create_client(key).await?;
        let mut clients = self.clients.lock().unwrap();
        clients.insert(key.to_string(), client.clone());
        Ok(client)
    }

    pub fn remove_client(&mut self, key: &str) {
        let mut clients = self.clients.lock().unwrap();
        clients.remove(key);
    }
}
