use std::sync::Arc;
use std::time::Duration;
use mini_moka::sync::Cache;
use crate::errors::{VpnResult};
use crate::server::{Node, NodeId, VpnStore, VpnStoreFactory};

pub struct NodeManager<S: VpnStore, F: VpnStoreFactory<S>> {
    store_factory: Arc<F>,
    cache: Cache<NodeId, Node>,
    _p: std::marker::PhantomData<S>,
}

impl<S: VpnStore, F: VpnStoreFactory<S>> NodeManager<S, F> {
    pub fn new(store_factory: Arc<F>) -> Arc<Self> {
        Arc::new(Self {
            store_factory,
            cache: Cache::builder()
                .time_to_live(Duration::from_secs(600))
                .max_capacity(10000)
                .build(),
            _p: std::marker::PhantomData,
        })
    }

    pub async fn get_node(&self, node_id: &NodeId) -> VpnResult<Option<Node>> {
        if let Some(node) = self.cache.get(node_id) {
            return Ok(Some(node));
        }

        let mut store = self.store_factory.get_vpn_store().await?;
        if let Some(node) = store.get_node(&node_id).await? {
            self.cache.insert(node_id.clone(), node.clone());
            Ok(Some(node))
        } else {
            Ok(None)
        }
    }

    pub async fn remove_node(&self, node_id: &NodeId) {
        self.cache.invalidate(node_id);
    }
}
