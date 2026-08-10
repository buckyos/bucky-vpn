use crate::server::NodeId;
use crate::{NodeNetworkPnInfo, NodePnInfoState};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

type NowSeconds = dyn Fn() -> u32 + Send + Sync;

fn system_time_seconds() -> u32 {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    u32::try_from(seconds).unwrap_or(u32::MAX)
}

fn canonicalize_networks(mut networks: Vec<NodeNetworkPnInfo>) -> Vec<NodeNetworkPnInfo> {
    networks.sort_by_key(|network| network.network_id);
    let mut canonical: Vec<NodeNetworkPnInfo> = Vec::with_capacity(networks.len());
    for network in networks {
        if canonical
            .last()
            .is_some_and(|current| current.network_id == network.network_id)
        {
            *canonical.last_mut().unwrap() = network;
        } else {
            canonical.push(network);
        }
    }
    canonical
}

impl NodePnInfoState {
    fn new(node_id: NodeId, networks: Vec<NodeNetworkPnInfo>, version: u32) -> Self {
        Self {
            node_id,
            version,
            networks,
        }
    }

    fn update_networks(&mut self, networks: Vec<NodeNetworkPnInfo>, now_seconds: u32) -> bool {
        if self.networks == networks {
            return false;
        }
        self.networks = networks;
        self.version = now_seconds;
        true
    }
}

pub struct NodePnManager {
    states: Mutex<HashMap<NodeId, NodePnInfoState>>,
    startup_version: u32,
    now_seconds: Box<NowSeconds>,
}

impl NodePnManager {
    pub fn new() -> Arc<Self> {
        Self::with_now_seconds(system_time_seconds)
    }

    fn with_now_seconds(now_seconds: impl Fn() -> u32 + Send + Sync + 'static) -> Arc<Self> {
        let startup_version = now_seconds();
        Arc::new(Self {
            states: Mutex::new(HashMap::new()),
            startup_version,
            now_seconds: Box::new(now_seconds),
        })
    }

    pub fn update_node_pn_info(
        &self,
        node_id: &NodeId,
        networks: Vec<NodeNetworkPnInfo>,
    ) -> (u32, bool) {
        let networks = canonicalize_networks(networks);
        let network_count = networks.len();
        let mut states = self.states.lock().unwrap();
        match states.entry(node_id.clone()) {
            Entry::Vacant(entry) => {
                log::info!(
                    "pn assignment version initialized: node_id={}, pn_info_version={}, network_count={}",
                    node_id.to_base36(),
                    self.startup_version,
                    network_count
                );
                entry.insert(NodePnInfoState::new(
                    node_id.clone(),
                    networks,
                    self.startup_version,
                ));
                (self.startup_version, true)
            }
            Entry::Occupied(mut entry) => {
                let state = entry.get_mut();
                let previous_version = state.version;
                let changed = if state.networks == networks {
                    false
                } else {
                    state.update_networks(networks, (self.now_seconds)())
                };
                if changed {
                    log::info!(
                        "pn assignment version changed: node_id={}, previous_pn_info_version={}, pn_info_version={}, network_count={}",
                        node_id.to_base36(),
                        previous_version,
                        state.version,
                        network_count
                    );
                }
                (state.version, changed)
            }
        }
    }

    pub fn get_node_pn_info(&self, node_id: &NodeId) -> Option<NodePnInfoState> {
        self.states.lock().unwrap().get(node_id).cloned()
    }
}

#[cfg(test)]
mod tests {
    include!("../../tests/unit/node_pn_manager_tests.rs");
}
