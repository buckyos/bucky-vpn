use crate::server::NodeId;
use crate::{NodeNetworkPnInfo, NodePnInfoState};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

impl NodePnInfoState {
    fn new(node_id: NodeId) -> Self {
        Self {
            node_id,
            version: 0,
            networks: Vec::new(),
        }
    }

    fn update_networks(&mut self, mut networks: Vec<NodeNetworkPnInfo>) -> bool {
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
        let networks = canonical;
        if self.networks == networks {
            return false;
        }
        self.networks = networks;
        self.version = self.version.wrapping_add(1);
        true
    }
}

pub struct NodePnManager {
    states: Mutex<HashMap<NodeId, NodePnInfoState>>,
}

impl NodePnManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            states: Mutex::new(HashMap::new()),
        })
    }

    pub fn update_node_pn_info(
        &self,
        node_id: &NodeId,
        networks: Vec<NodeNetworkPnInfo>,
    ) -> (u16, bool) {
        let mut states = self.states.lock().unwrap();
        let state = states
            .entry(node_id.clone())
            .or_insert_with(|| NodePnInfoState::new(node_id.clone()));
        let changed = state.update_networks(networks);
        (state.version, changed)
    }

    pub fn get_node_pn_info(&self, node_id: &NodeId) -> Option<NodePnInfoState> {
        self.states.lock().unwrap().get(node_id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::{NodePnInfoState, NodePnManager};
    use crate::server::NodeId;
    use crate::{ClientProxyNodeInfo, NodeNetworkPnInfo};

    #[test]
    fn node_pn_info_state_versions_only_change_when_info_changes() {
        let node = NodeId::from([9; 32].as_slice());
        let proxy = NodeId::from([8; 32].as_slice());
        let mut state = NodePnInfoState::new(node);
        let networks = vec![NodeNetworkPnInfo {
            network_id: 1,
            proxy: Some(ClientProxyNodeInfo {
                proxy_id: proxy.clone(),
                name: Some("proxy".to_string()),
                endpoints: Vec::new(),
            }),
        }];

        assert!(state.update_networks(networks.clone()));
        assert_eq!(state.version, 1);

        assert!(!state.update_networks(networks));
        assert_eq!(state.version, 1);

        assert!(state.update_networks(vec![NodeNetworkPnInfo {
            network_id: 1,
            proxy: Some(ClientProxyNodeInfo {
                proxy_id: proxy,
                name: Some("renamed".to_string()),
                endpoints: Vec::new(),
            }),
        }]));
        assert_eq!(state.version, 2);
    }

    #[test]
    fn node_pn_manager_tracks_versions_per_node() {
        let manager = NodePnManager::new();
        let node1 = NodeId::from([1; 32].as_slice());
        let node2 = NodeId::from([2; 32].as_slice());

        assert_eq!(
            manager.update_node_pn_info(
                &node1,
                vec![NodeNetworkPnInfo {
                    network_id: 1,
                    proxy: None,
                }]
            ),
            (1, true)
        );
        assert_eq!(
            manager.update_node_pn_info(
                &node1,
                vec![NodeNetworkPnInfo {
                    network_id: 1,
                    proxy: None,
                }]
            ),
            (1, false)
        );
        assert_eq!(
            manager.update_node_pn_info(
                &node2,
                vec![NodeNetworkPnInfo {
                    network_id: 1,
                    proxy: None,
                }]
            ),
            (1, true)
        );
    }

    #[test]
    fn node_pn_info_is_canonicalized_by_network_id() {
        let node = NodeId::from([7; 32].as_slice());
        let manager = NodePnManager::new();
        let unordered = vec![
            NodeNetworkPnInfo {
                network_id: 9,
                proxy: None,
            },
            NodeNetworkPnInfo {
                network_id: 2,
                proxy: None,
            },
        ];

        assert_eq!(manager.update_node_pn_info(&node, unordered), (1, true));
        let state = manager.get_node_pn_info(&node).unwrap();
        assert_eq!(
            state
                .networks
                .iter()
                .map(|network| network.network_id)
                .collect::<Vec<_>>(),
            vec![2, 9]
        );

        assert_eq!(
            manager.update_node_pn_info(
                &node,
                vec![
                    NodeNetworkPnInfo {
                        network_id: 2,
                        proxy: None,
                    },
                    NodeNetworkPnInfo {
                        network_id: 9,
                        proxy: None,
                    },
                ]
            ),
            (1, false)
        );
    }
}
