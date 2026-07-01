use crate::sqlite_store_factory::{PersistedTrafficStats, SqliteStoreFactory};
use p2p_frame::p2p_identity::P2pId;
use p2p_frame::pn::{PnServer, PnUserTrafficSnapshot};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use vpn_frame::errors::{VpnErrorCode, VpnResult, vpn_err};
use vpn_frame::server::{NetworkGroupId, NetworkStore, NodeId, VpnStore, VpnStoreFactory};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FlushState {
    tx_bytes: u64,
    rx_bytes: u64,
}

pub trait PnTrafficSnapshotProvider: Send + Sync + 'static {
    fn get_node_traffic_snapshot(&self, node_id: &NodeId) -> PnUserTrafficSnapshot;

    fn list_tracked_node_ids(&self) -> Vec<NodeId> {
        Vec::new()
    }
}

pub trait PnTrafficNodeTracker: Send + Sync + 'static {
    fn track_node(&self, node_id: NodeId);
}

pub type PnTrafficNodeTrackerRef = Arc<dyn PnTrafficNodeTracker>;

pub struct PnTrafficNodeSet {
    node_ids: Mutex<HashSet<NodeId>>,
}

pub type PnTrafficNodeSetRef = Arc<PnTrafficNodeSet>;

impl PnTrafficNodeSet {
    pub fn new() -> PnTrafficNodeSetRef {
        Arc::new(Self {
            node_ids: Mutex::new(HashSet::new()),
        })
    }

    fn list_node_ids(&self) -> Vec<NodeId> {
        self.node_ids.lock().unwrap().iter().cloned().collect()
    }
}

impl PnTrafficNodeTracker for PnTrafficNodeSet {
    fn track_node(&self, node_id: NodeId) {
        self.node_ids.lock().unwrap().insert(node_id);
    }
}

pub struct TrackedPnTrafficSnapshotProvider {
    provider: PnTrafficSnapshotProviderRef,
    node_set: PnTrafficNodeSetRef,
}

impl TrackedPnTrafficSnapshotProvider {
    pub fn new(provider: PnTrafficSnapshotProviderRef, node_set: PnTrafficNodeSetRef) -> Arc<Self> {
        Arc::new(Self { provider, node_set })
    }
}

impl PnTrafficSnapshotProvider for TrackedPnTrafficSnapshotProvider {
    fn get_node_traffic_snapshot(&self, node_id: &NodeId) -> PnUserTrafficSnapshot {
        self.provider.get_node_traffic_snapshot(node_id)
    }

    fn list_tracked_node_ids(&self) -> Vec<NodeId> {
        self.node_set.list_node_ids()
    }
}

pub struct NoopPnTrafficSnapshotProvider;

impl PnTrafficSnapshotProvider for NoopPnTrafficSnapshotProvider {
    fn get_node_traffic_snapshot(&self, _node_id: &NodeId) -> PnUserTrafficSnapshot {
        PnUserTrafficSnapshot::default()
    }
}

impl PnTrafficSnapshotProvider for PnServer {
    fn get_node_traffic_snapshot(&self, node_id: &NodeId) -> PnUserTrafficSnapshot {
        self.get_user_traffic_snapshot(&P2pId::from(node_id.as_slice()))
            .unwrap_or_default()
    }
}

pub type PnTrafficSnapshotProviderRef = Arc<dyn PnTrafficSnapshotProvider>;

#[async_trait::async_trait]
pub trait PnTrafficReporter: Send + Sync + 'static {
    async fn report_delta(&self, node_id: &NodeId, delta: PersistedTrafficStats) -> VpnResult<()>;
}

pub type PnTrafficReporterRef = Arc<dyn PnTrafficReporter>;

pub struct PnTrafficService {
    snapshot_provider: PnTrafficSnapshotProviderRef,
    store_factory: Option<Arc<SqliteStoreFactory>>,
    flush_state: Mutex<HashMap<NodeId, FlushState>>,
    remote_reporter: Mutex<Option<PnTrafficReporterRef>>,
}

pub type PnTrafficServiceRef = Arc<PnTrafficService>;

impl PnTrafficService {
    pub fn new(
        snapshot_provider: PnTrafficSnapshotProviderRef,
        store_factory: Arc<SqliteStoreFactory>,
    ) -> PnTrafficServiceRef {
        Arc::new(Self {
            snapshot_provider,
            store_factory: Some(store_factory),
            flush_state: Mutex::new(HashMap::new()),
            remote_reporter: Mutex::new(None),
        })
    }

    pub fn new_without_store(
        snapshot_provider: PnTrafficSnapshotProviderRef,
    ) -> PnTrafficServiceRef {
        Arc::new(Self {
            snapshot_provider,
            store_factory: None,
            flush_state: Mutex::new(HashMap::new()),
            remote_reporter: Mutex::new(None),
        })
    }

    pub fn set_remote_reporter(&self, reporter: PnTrafficReporterRef) {
        *self.remote_reporter.lock().unwrap() = Some(reporter);
    }

    pub fn start_background_flush(self: &Arc<Self>, interval: Duration) {
        let this = self.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                if let Err(err) = this.flush_all().await {
                    log::warn!(
                        "flush pn traffic stats failed: code={:?} msg={}",
                        err.code(),
                        err.msg()
                    );
                }
            }
        });
    }

    pub fn start_remote_heartbeat(self: &Arc<Self>, node_id: NodeId, interval: Duration) {
        let this = self.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                let Some(reporter) = this.remote_reporter() else {
                    continue;
                };
                if let Err(err) = reporter
                    .report_delta(&node_id, PersistedTrafficStats::default())
                    .await
                {
                    log::warn!(
                        "report proxy node heartbeat failed: node={} code={:?} msg={}",
                        node_id.to_base58(),
                        err.code(),
                        err.msg()
                    );
                }
            }
        });
    }

    pub async fn get_node_snapshot(&self, node_id: &NodeId) -> VpnResult<PnUserTrafficSnapshot> {
        let runtime = self.snapshot_provider.get_node_traffic_snapshot(node_id);
        let flush_state = self.get_flush_state(node_id);
        let persisted = {
            let mut store = self.store_factory()?.get_vpn_store().await?;
            store.get_persisted_node_traffic(node_id).await?
        };
        Ok(merge_persisted_and_runtime(persisted, runtime, flush_state))
    }

    pub async fn get_group_snapshot(
        &self,
        group_id: &NetworkGroupId,
    ) -> VpnResult<PnUserTrafficSnapshot> {
        let (persisted, joined_nodes) = {
            let mut store = self.store_factory()?.get_vpn_store().await?;
            (
                store.get_persisted_group_traffic(group_id).await?,
                store.get_joined_nodes(group_id).await?,
            )
        };

        let mut tx_bytes = persisted.tx_bytes;
        let mut tx_speed: u64 = 0;
        let mut seen = HashSet::new();
        for joined in joined_nodes {
            if !seen.insert(joined.node_id.clone()) {
                continue;
            }
            let runtime = self
                .snapshot_provider
                .get_node_traffic_snapshot(&joined.node_id);
            let flush_state = self.get_flush_state(&joined.node_id);
            tx_bytes =
                tx_bytes.saturating_add(pending_bytes(runtime.tx_bytes, flush_state.tx_bytes));
            tx_speed = tx_speed.saturating_add(runtime.tx_speed);
        }

        // Group-level user stats use only the ingress side to avoid counting the
        // same relay traffic twice now that both endpoints have node snapshots.
        Ok(PnUserTrafficSnapshot {
            tx_bytes,
            tx_speed,
            rx_bytes: tx_bytes,
            rx_speed: tx_speed,
        })
    }

    pub async fn flush_all(&self) -> VpnResult<()> {
        let mut node_ids = if let Some(store_factory) = &self.store_factory {
            let mut store = store_factory.get_vpn_store().await?;
            store.list_all_joined_node_ids().await?
        } else {
            Vec::new()
        };
        node_ids.extend(self.snapshot_provider.list_tracked_node_ids());
        let mut seen = HashSet::new();
        for node_id in node_ids {
            if seen.insert(node_id.clone()) {
                self.flush_node(&node_id).await?;
            }
        }
        Ok(())
    }

    pub async fn flush_node(&self, node_id: &NodeId) -> VpnResult<()> {
        let runtime = self.snapshot_provider.get_node_traffic_snapshot(node_id);
        let flush_state = self.get_flush_state(node_id);
        let delta = PersistedTrafficStats {
            tx_bytes: pending_bytes(runtime.tx_bytes, flush_state.tx_bytes),
            rx_bytes: pending_bytes(runtime.rx_bytes, flush_state.rx_bytes),
        };

        if delta == PersistedTrafficStats::default() {
            self.set_flush_state(
                node_id.clone(),
                FlushState {
                    tx_bytes: runtime.tx_bytes,
                    rx_bytes: runtime.rx_bytes,
                },
            );
            return Ok(());
        }

        if let Some(reporter) = self.remote_reporter() {
            reporter.report_delta(node_id, delta).await?;
            self.set_flush_state(
                node_id.clone(),
                FlushState {
                    tx_bytes: runtime.tx_bytes,
                    rx_bytes: runtime.rx_bytes,
                },
            );
            return Ok(());
        }

        self.apply_node_delta(node_id, delta).await?;

        self.set_flush_state(
            node_id.clone(),
            FlushState {
                tx_bytes: runtime.tx_bytes,
                rx_bytes: runtime.rx_bytes,
            },
        );

        Ok(())
    }

    pub async fn apply_node_delta(
        &self,
        node_id: &NodeId,
        delta: PersistedTrafficStats,
    ) -> VpnResult<()> {
        if delta == PersistedTrafficStats::default() {
            return Ok(());
        }

        let groups = {
            let mut store = self.store_factory()?.get_vpn_store().await?;
            store.begin_transaction().await?;
            let result: VpnResult<Vec<_>> = async {
                store.add_persisted_node_traffic(node_id, delta).await?;
                let groups = store.get_joined_network_group(node_id).await?;
                for joined in groups.iter() {
                    store
                        .add_persisted_group_traffic(
                            &joined.group_id,
                            PersistedTrafficStats {
                                tx_bytes: delta.tx_bytes,
                                rx_bytes: delta.rx_bytes,
                            },
                        )
                        .await?;
                }
                Ok(groups)
            }
            .await;
            match result {
                Ok(groups) => {
                    store.commit_transaction().await?;
                    groups
                }
                Err(err) => {
                    let _ = store.rollback_transaction().await;
                    return Err(err);
                }
            }
        };

        if groups.is_empty() {
            log::debug!(
                "recorded pn traffic delta for node {} without joined group",
                node_id.to_base58()
            );
        }

        Ok(())
    }

    fn get_flush_state(&self, node_id: &NodeId) -> FlushState {
        self.flush_state
            .lock()
            .unwrap()
            .get(node_id)
            .copied()
            .unwrap_or_default()
    }

    fn set_flush_state(&self, node_id: NodeId, state: FlushState) {
        self.flush_state.lock().unwrap().insert(node_id, state);
    }

    fn remote_reporter(&self) -> Option<PnTrafficReporterRef> {
        self.remote_reporter.lock().unwrap().clone()
    }

    fn store_factory(&self) -> VpnResult<&Arc<SqliteStoreFactory>> {
        self.store_factory.as_ref().ok_or_else(|| {
            vpn_err!(
                VpnErrorCode::Failed,
                "pn traffic service has no local sqlite store"
            )
        })
    }
}

fn pending_bytes(runtime_total: u64, flushed_total: u64) -> u64 {
    if runtime_total >= flushed_total {
        runtime_total - flushed_total
    } else {
        runtime_total
    }
}

fn merge_persisted_and_runtime(
    persisted: PersistedTrafficStats,
    runtime: PnUserTrafficSnapshot,
    flushed: FlushState,
) -> PnUserTrafficSnapshot {
    PnUserTrafficSnapshot {
        tx_bytes: persisted
            .tx_bytes
            .saturating_add(pending_bytes(runtime.tx_bytes, flushed.tx_bytes)),
        tx_speed: runtime.tx_speed,
        rx_bytes: persisted
            .rx_bytes
            .saturating_add(pending_bytes(runtime.rx_bytes, flushed.rx_bytes)),
        rx_speed: runtime.rx_speed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use vpn_frame::server::{JoinedNode, NetworkStore};

    struct FakeSnapshotProvider {
        snapshots: Mutex<HashMap<NodeId, PnUserTrafficSnapshot>>,
    }

    impl FakeSnapshotProvider {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                snapshots: Mutex::new(HashMap::new()),
            })
        }

        fn set_snapshot(&self, node_id: NodeId, snapshot: PnUserTrafficSnapshot) {
            self.snapshots.lock().unwrap().insert(node_id, snapshot);
        }
    }

    impl PnTrafficSnapshotProvider for FakeSnapshotProvider {
        fn get_node_traffic_snapshot(&self, node_id: &NodeId) -> PnUserTrafficSnapshot {
            self.snapshots
                .lock()
                .unwrap()
                .get(node_id)
                .cloned()
                .unwrap_or_default()
        }
    }

    async fn new_test_store_factory() -> (Arc<SqliteStoreFactory>, PathBuf) {
        let db_dir = std::env::temp_dir().join(format!(
            "bucky-vpn-server-pn-traffic-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&db_dir).unwrap();
        let db_path = db_dir.join("vpn.db");
        let store_factory = Arc::new(
            SqliteStoreFactory::create(db_path.to_str().unwrap())
                .await
                .unwrap(),
        );
        {
            let mut store = store_factory.get_vpn_store().await.unwrap();
            store.init_db().await.unwrap();
        }
        (store_factory, db_dir)
    }

    async fn add_joined_node(
        store_factory: &Arc<SqliteStoreFactory>,
        group_id: NetworkGroupId,
        node_id: &NodeId,
    ) {
        let mut store = store_factory.get_vpn_store().await.unwrap();
        store
            .add_joined_node(&JoinedNode {
                group_id,
                node_id: node_id.clone(),
                allow_join: true,
                name: "node".to_string(),
                comment: String::new(),
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn persists_node_and_group_bytes_across_service_restart() {
        let (store_factory, db_dir) = new_test_store_factory().await;
        let provider = FakeSnapshotProvider::new();
        let node_id = NodeId::from(vec![1u8; 32].as_slice());
        add_joined_node(&store_factory, 7, &node_id).await;

        let service = PnTrafficService::new(provider.clone(), store_factory.clone());
        provider.set_snapshot(
            node_id.clone(),
            PnUserTrafficSnapshot {
                tx_bytes: 100,
                tx_speed: 10,
                rx_bytes: 60,
                rx_speed: 6,
            },
        );

        let node_before_flush = service.get_node_snapshot(&node_id).await.unwrap();
        assert_eq!(node_before_flush.tx_bytes, 100);
        assert_eq!(node_before_flush.rx_bytes, 60);

        service.flush_all().await.unwrap();

        provider.set_snapshot(
            node_id.clone(),
            PnUserTrafficSnapshot {
                tx_bytes: 0,
                tx_speed: 0,
                rx_bytes: 0,
                rx_speed: 0,
            },
        );
        let restarted = PnTrafficService::new(provider.clone(), store_factory.clone());
        let node_after_restart = restarted.get_node_snapshot(&node_id).await.unwrap();
        let group_after_restart = restarted.get_group_snapshot(&7).await.unwrap();
        assert_eq!(node_after_restart.tx_bytes, 100);
        assert_eq!(node_after_restart.rx_bytes, 60);
        assert_eq!(group_after_restart.tx_bytes, 100);
        assert_eq!(group_after_restart.rx_bytes, 100);

        provider.set_snapshot(
            node_id.clone(),
            PnUserTrafficSnapshot {
                tx_bytes: 40,
                tx_speed: 4,
                rx_bytes: 20,
                rx_speed: 2,
            },
        );
        let merged = restarted.get_node_snapshot(&node_id).await.unwrap();
        assert_eq!(merged.tx_bytes, 140);
        assert_eq!(merged.rx_bytes, 80);

        restarted.flush_all().await.unwrap();

        let final_node = restarted.get_node_snapshot(&node_id).await.unwrap();
        let final_group = restarted.get_group_snapshot(&7).await.unwrap();
        assert_eq!(final_node.tx_bytes, 140);
        assert_eq!(final_node.rx_bytes, 80);
        assert_eq!(final_group.tx_bytes, 140);
        assert_eq!(final_group.rx_bytes, 140);

        drop(restarted);
        drop(service);
        drop(store_factory);
        let _ = std::fs::remove_dir_all(db_dir);
    }

    #[tokio::test]
    async fn group_total_remains_after_node_removed() {
        let (store_factory, db_dir) = new_test_store_factory().await;
        let provider = FakeSnapshotProvider::new();
        let node_id = NodeId::from(vec![2u8; 32].as_slice());
        add_joined_node(&store_factory, 9, &node_id).await;

        let service = PnTrafficService::new(provider.clone(), store_factory.clone());
        provider.set_snapshot(
            node_id.clone(),
            PnUserTrafficSnapshot {
                tx_bytes: 55,
                tx_speed: 5,
                rx_bytes: 44,
                rx_speed: 4,
            },
        );
        service.flush_all().await.unwrap();

        {
            let mut store = store_factory.get_vpn_store().await.unwrap();
            store.del_joined_node(&9, &node_id).await.unwrap();
        }

        provider.set_snapshot(node_id.clone(), PnUserTrafficSnapshot::default());
        let restarted = PnTrafficService::new(provider.clone(), store_factory.clone());
        let group_snapshot = restarted.get_group_snapshot(&9).await.unwrap();
        assert_eq!(group_snapshot.tx_bytes, 55);
        assert_eq!(group_snapshot.rx_bytes, 55);

        drop(restarted);
        drop(service);
        drop(store_factory);
        let _ = std::fs::remove_dir_all(db_dir);
    }

    #[tokio::test]
    async fn group_snapshot_uses_single_sided_bytes_and_speed() {
        let (store_factory, db_dir) = new_test_store_factory().await;
        let provider = FakeSnapshotProvider::new();
        let first = NodeId::from(vec![3u8; 32].as_slice());
        let second = NodeId::from(vec![4u8; 32].as_slice());
        add_joined_node(&store_factory, 11, &first).await;
        add_joined_node(&store_factory, 11, &second).await;

        let service = PnTrafficService::new(provider.clone(), store_factory.clone());
        provider.set_snapshot(
            first.clone(),
            PnUserTrafficSnapshot {
                tx_bytes: 70,
                tx_speed: 7,
                rx_bytes: 170,
                rx_speed: 17,
            },
        );
        provider.set_snapshot(
            second.clone(),
            PnUserTrafficSnapshot {
                tx_bytes: 30,
                tx_speed: 3,
                rx_bytes: 130,
                rx_speed: 13,
            },
        );

        let group_snapshot = service.get_group_snapshot(&11).await.unwrap();
        assert_eq!(group_snapshot.tx_bytes, 100);
        assert_eq!(group_snapshot.tx_speed, 10);
        assert_eq!(group_snapshot.rx_bytes, 100);
        assert_eq!(group_snapshot.rx_speed, 10);

        drop(service);
        drop(store_factory);
        let _ = std::fs::remove_dir_all(db_dir);
    }

    #[tokio::test]
    async fn apply_node_delta_persists_node_and_group_delta() {
        let (store_factory, db_dir) = new_test_store_factory().await;
        let provider = FakeSnapshotProvider::new();
        let node_id = NodeId::from(vec![5u8; 32].as_slice());
        add_joined_node(&store_factory, 13, &node_id).await;

        let service = PnTrafficService::new(provider, store_factory.clone());
        service
            .apply_node_delta(
                &node_id,
                PersistedTrafficStats {
                    tx_bytes: 88,
                    rx_bytes: 77,
                },
            )
            .await
            .unwrap();

        let node_snapshot = service.get_node_snapshot(&node_id).await.unwrap();
        let group_snapshot = service.get_group_snapshot(&13).await.unwrap();
        assert_eq!(node_snapshot.tx_bytes, 88);
        assert_eq!(node_snapshot.rx_bytes, 77);
        assert_eq!(group_snapshot.tx_bytes, 88);
        assert_eq!(group_snapshot.rx_bytes, 88);

        drop(service);
        drop(store_factory);
        let _ = std::fs::remove_dir_all(db_dir);
    }
}
