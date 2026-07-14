use crate::sqlite_store_factory::SqliteStoreFactory;
use crate::server_config::PnTrafficUploadConfig;
use p2p_frame::pn::{PnConnectionTrafficSnapshot, PnServer};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use vpn_frame::errors::{VpnErrorCode, VpnResult, vpn_err};
use vpn_frame::server::{NetworkGroupId, NetworkStore, NodeId, VpnStoreFactory};
use vpn_frame::{
    PnTrafficDirectionSample,
    PnTrafficSample, PnTrafficSnapshot, ProxyTrafficReport, ProxyTrafficReportApplyResult,
    ProxyTrafficReportId, ProxyTrafficReportResp,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UserTrafficSnapshot {
    pub tx_bytes: u64,
    pub tx_speed: u64,
    pub rx_bytes: u64,
    pub rx_speed: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProxyReportState {
    next_report_seq: u64,
    wall_clock_anchor_ms: u64,
    monotonic_anchor: Instant,
    last_report_ended_at_ms: Option<u64>,
}

impl Default for ProxyReportState {
    fn default() -> Self {
        Self {
            next_report_seq: 0,
            wall_clock_anchor_ms: now_ms(),
            monotonic_anchor: Instant::now(),
            last_report_ended_at_ms: None,
        }
    }
}

impl ProxyReportState {
    fn next_collection_window(&self) -> VpnResult<(u64, u64)> {
        let started_at_ms = self
            .last_report_ended_at_ms
            .unwrap_or(self.wall_clock_anchor_ms);
        let minimum_ended_at_ms = started_at_ms.checked_add(1).ok_or_else(|| {
            vpn_err!(
                VpnErrorCode::Failed,
                "proxy traffic report timestamp is exhausted"
            )
        })?;
        let elapsed_ms = u64::try_from(self.monotonic_anchor.elapsed().as_millis())
            .unwrap_or(u64::MAX);
        let monotonic_ended_at_ms = self.wall_clock_anchor_ms.saturating_add(elapsed_ms);
        Ok((
            started_at_ms,
            monotonic_ended_at_ms.max(minimum_ended_at_ms),
        ))
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ProxyConnectionKey {
    network_id: u64,
    tunnel_id: u32,
    from: NodeId,
    to: NodeId,
}

fn proxy_connection_key(snapshot: &PnConnectionTrafficSnapshot) -> ProxyConnectionKey {
    ProxyConnectionKey {
        network_id: snapshot.network_id,
        tunnel_id: snapshot.tunnel_id.value(),
        from: NodeId::from(snapshot.from.as_slice()),
        to: NodeId::from(snapshot.to.as_slice()),
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ProxyPairKey {
    network_id: u64,
    node_a: NodeId,
    node_b: NodeId,
}

#[derive(Clone, Debug)]
struct PendingProxyTrafficRecord {
    report: ProxyTrafficReport,
    source_snapshots: Vec<PnConnectionTrafficSnapshot>,
}

#[derive(Clone, Debug)]
struct TrafficReportBatch {
    batch_id: u64,
    records: Vec<PendingProxyTrafficRecord>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProxyTrafficQueueStatus {
    pub queued_batches: usize,
    pub queued_records: usize,
    pub oldest_batch_id: Option<u64>,
    pub terminal_rejected_records: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyTrafficShutdownStatus {
    pub collector_exited: bool,
    pub final_collection_succeeded: bool,
    pub final_collection_error: Option<String>,
    pub uploader_exited: bool,
    pub queue: ProxyTrafficQueueStatus,
}

impl ProxyTrafficShutdownStatus {
    pub fn is_success(&self) -> bool {
        self.collector_exited
            && self.final_collection_succeeded
            && self.uploader_exited
            && self.queue.queued_batches == 0
            && self.queue.queued_records == 0
    }
}

#[derive(Default)]
struct AggregatedProxyTraffic {
    node_a_to_b_bytes: u64,
    node_b_to_a_bytes: u64,
    node_a_to_b_speed: u64,
    node_b_to_a_speed: u64,
    source_snapshots: Vec<PnConnectionTrafficSnapshot>,
}

pub trait ProxyTrafficDeltaProvider: Send + Sync + 'static {
    fn read_proxy_traffic_deltas(&self) -> Vec<PnTrafficSnapshot>;
}

pub type ProxyTrafficDeltaProviderRef = Arc<dyn ProxyTrafficDeltaProvider>;

pub trait ProxyConnectionTrafficSource: Send + Sync + 'static {
    fn snapshot_connections(&self) -> Vec<PnConnectionTrafficSnapshot>;
    fn acknowledge_connections(&self, snapshots: &[PnConnectionTrafficSnapshot]) -> usize;
}

impl ProxyConnectionTrafficSource for PnServer {
    fn snapshot_connections(&self) -> Vec<PnConnectionTrafficSnapshot> {
        self.collect_connection_traffic_snapshots()
    }

    fn acknowledge_connections(&self, snapshots: &[PnConnectionTrafficSnapshot]) -> usize {
        self.acknowledge_connection_traffic_snapshots(snapshots)
    }
}

pub type ProxyConnectionTrafficSourceRef = Arc<dyn ProxyConnectionTrafficSource>;

#[async_trait::async_trait]
pub trait PnTrafficReporter: Send + Sync + 'static {
    async fn report_heartbeat(&self) -> VpnResult<()>;

    async fn report_proxy_traffic(
        &self,
        reports: Vec<ProxyTrafficReport>,
    ) -> VpnResult<Vec<ProxyTrafficReportResp>>;
}

pub type PnTrafficReporterRef = Arc<dyn PnTrafficReporter>;

pub struct PnTrafficService {
    store_factory: Option<Arc<SqliteStoreFactory>>,
    remote_reporter: Mutex<Option<PnTrafficReporterRef>>,
    proxy_delta_provider: Mutex<Option<ProxyTrafficDeltaProviderRef>>,
    proxy_connection_source: Mutex<Option<ProxyConnectionTrafficSourceRef>>,
    proxy_upload_config: Mutex<PnTrafficUploadConfig>,
    proxy_report_state: Mutex<ProxyReportState>,
    pending_proxy_batches: Mutex<VecDeque<TrafficReportBatch>>,
    proxy_collection_cursor: Mutex<HashMap<ProxyConnectionKey, (u64, u64)>>,
    proxy_accepted_baselines: Mutex<HashMap<ProxyConnectionKey, (u64, u64)>>,
    proxy_rejected_targets: Mutex<HashMap<ProxyConnectionKey, (u64, u64)>>,
    proxy_collect_lock: Mutex<()>,
    proxy_upload_lock: tokio::sync::Mutex<()>,
    proxy_upload_notify: tokio::sync::Notify,
    proxy_collector_notify: tokio::sync::Notify,
    proxy_collector_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    proxy_uploader_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    proxy_uploader_started: AtomicBool,
    proxy_collector_shutdown: AtomicBool,
    proxy_uploader_shutdown: AtomicBool,
    proxy_terminal_rejected: AtomicU64,
}

pub type PnTrafficServiceRef = Arc<PnTrafficService>;

impl PnTrafficService {
    pub fn new(store_factory: Arc<SqliteStoreFactory>) -> PnTrafficServiceRef {
        Arc::new(Self {
            store_factory: Some(store_factory),
            remote_reporter: Mutex::new(None),
            proxy_delta_provider: Mutex::new(None),
            proxy_connection_source: Mutex::new(None),
            proxy_upload_config: Mutex::new(PnTrafficUploadConfig::default()),
            proxy_report_state: Mutex::new(ProxyReportState::default()),
            pending_proxy_batches: Mutex::new(VecDeque::new()),
            proxy_collection_cursor: Mutex::new(HashMap::new()),
            proxy_accepted_baselines: Mutex::new(HashMap::new()),
            proxy_rejected_targets: Mutex::new(HashMap::new()),
            proxy_collect_lock: Mutex::new(()),
            proxy_upload_lock: tokio::sync::Mutex::new(()),
            proxy_upload_notify: tokio::sync::Notify::new(),
            proxy_collector_notify: tokio::sync::Notify::new(),
            proxy_collector_task: Mutex::new(None),
            proxy_uploader_task: Mutex::new(None),
            proxy_uploader_started: AtomicBool::new(false),
            proxy_collector_shutdown: AtomicBool::new(false),
            proxy_uploader_shutdown: AtomicBool::new(false),
            proxy_terminal_rejected: AtomicU64::new(0),
        })
    }

    pub fn new_without_store() -> PnTrafficServiceRef {
        Arc::new(Self {
            store_factory: None,
            remote_reporter: Mutex::new(None),
            proxy_delta_provider: Mutex::new(None),
            proxy_connection_source: Mutex::new(None),
            proxy_upload_config: Mutex::new(PnTrafficUploadConfig::default()),
            proxy_report_state: Mutex::new(ProxyReportState::default()),
            pending_proxy_batches: Mutex::new(VecDeque::new()),
            proxy_collection_cursor: Mutex::new(HashMap::new()),
            proxy_accepted_baselines: Mutex::new(HashMap::new()),
            proxy_rejected_targets: Mutex::new(HashMap::new()),
            proxy_collect_lock: Mutex::new(()),
            proxy_upload_lock: tokio::sync::Mutex::new(()),
            proxy_upload_notify: tokio::sync::Notify::new(),
            proxy_collector_notify: tokio::sync::Notify::new(),
            proxy_collector_task: Mutex::new(None),
            proxy_uploader_task: Mutex::new(None),
            proxy_uploader_started: AtomicBool::new(false),
            proxy_collector_shutdown: AtomicBool::new(false),
            proxy_uploader_shutdown: AtomicBool::new(false),
            proxy_terminal_rejected: AtomicU64::new(0),
        })
    }

    pub fn set_remote_reporter(&self, reporter: PnTrafficReporterRef) {
        *self.remote_reporter.lock().unwrap() = Some(reporter);
        self.proxy_upload_notify.notify_one();
    }

    pub fn set_proxy_connection_source(&self, source: ProxyConnectionTrafficSourceRef) {
        *self.proxy_connection_source.lock().unwrap() = Some(source);
    }

    pub fn set_proxy_upload_config(&self, config: PnTrafficUploadConfig) {
        *self.proxy_upload_config.lock().unwrap() = config;
    }

    #[allow(dead_code)]
    pub fn set_proxy_delta_provider(&self, provider: ProxyTrafficDeltaProviderRef) {
        *self.proxy_delta_provider.lock().unwrap() = Some(provider);
    }

    pub fn start_background_flush(self: &Arc<Self>, interval: Duration) {
        self.start_proxy_uploader();

        let this = self.clone();
        let task = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                tokio::select! {
                    _ = this.proxy_collector_notify.notified() => {
                        if this.proxy_collector_shutdown.load(Ordering::SeqCst) {
                            break;
                        }
                    }
                    _ = ticker.tick() => {
                        if this.proxy_collector_shutdown.load(Ordering::SeqCst) {
                            break;
                        }
                        if let Err(err) = this.collect_proxy_traffic() {
                            log::warn!(
                                "collect pn connection traffic failed: code={:?} msg={}",
                                err.code(),
                                err.msg()
                            );
                        }
                    }
                }
            }
        });
        *self.proxy_collector_task.lock().unwrap() = Some(task);
    }

    pub fn start_proxy_uploader(self: &Arc<Self>) {
        if self
            .proxy_uploader_started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        let this = self.clone();
        let task = tokio::spawn(async move {
            loop {
                if this.proxy_uploader_shutdown.load(Ordering::SeqCst)
                    && this.pending_proxy_batch_count() == 0
                {
                    break;
                }
                if this.pending_proxy_batch_count() == 0 || this.remote_reporter().is_none() {
                    this.proxy_upload_notify.notified().await;
                    continue;
                }
                if let Err(err) = this.drain_proxy_upload_once().await {
                    log::warn!(
                        "upload proxy traffic batch failed: code={:?} msg={}",
                        err.code(),
                        err.msg()
                    );
                    let delay = this.proxy_upload_config.lock().unwrap().retry_delay_ms;
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
            }
        });
        *self.proxy_uploader_task.lock().unwrap() = Some(task);
    }

    pub async fn shutdown_proxy_traffic(&self, timeout: Duration) -> ProxyTrafficShutdownStatus {
        let deadline = Instant::now() + timeout;
        self.proxy_collector_shutdown.store(true, Ordering::SeqCst);
        self.proxy_collector_notify.notify_waiters();
        let collector_task = self.proxy_collector_task.lock().unwrap().take();
        let collector_exited = wait_for_background_task(collector_task, deadline).await;

        let (final_collection_succeeded, final_collection_error) = if collector_exited {
            match self.collect_proxy_traffic() {
                Ok(_) => (true, None),
                Err(err) => (false, Some(err.msg().to_owned())),
            }
        } else {
            (
                false,
                Some("proxy traffic collector did not exit before the shutdown deadline".to_owned()),
            )
        };

        self.proxy_uploader_shutdown.store(true, Ordering::SeqCst);
        self.proxy_upload_notify.notify_waiters();
        let uploader_task = self.proxy_uploader_task.lock().unwrap().take();
        let uploader_exited = wait_for_background_task(uploader_task, deadline).await;
        ProxyTrafficShutdownStatus {
            collector_exited,
            final_collection_succeeded,
            final_collection_error,
            uploader_exited,
            queue: self.proxy_queue_status(),
        }
    }

    pub fn start_remote_heartbeat(self: &Arc<Self>, interval: Duration) {
        let this = self.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                let Some(reporter) = this.remote_reporter() else {
                    continue;
                };
                if let Err(err) = reporter.report_heartbeat().await {
                    log::warn!(
                        "report proxy node heartbeat failed: code={:?} msg={}",
                        err.code(),
                        err.msg()
                    );
                }
            }
        });
    }

    pub async fn get_node_snapshot(&self, node_id: &NodeId) -> VpnResult<UserTrafficSnapshot> {
        let persisted = {
            let mut store = self.store_factory()?.get_vpn_store().await?;
            store.get_persisted_node_traffic(node_id).await?
        };
        let speed = self.store_factory()?.get_node_traffic_speed(node_id);
        Ok(UserTrafficSnapshot {
            tx_bytes: persisted.tx_bytes,
            tx_speed: speed.tx_bytes,
            rx_bytes: persisted.rx_bytes,
            rx_speed: speed.rx_bytes,
        })
    }

    pub async fn get_group_snapshot(
        &self,
        group_id: &NetworkGroupId,
    ) -> VpnResult<UserTrafficSnapshot> {
        let (persisted, network_ids) = {
            let mut store = self.store_factory()?.get_vpn_store().await?;
            (
                store.get_persisted_group_traffic(group_id).await?,
                store.get_networks(group_id).await?.into_iter().map(|network| network.id).collect::<HashSet<_>>(),
            )
        };
        let speed = self.store_factory()?.get_network_traffic_speed(&network_ids);
        Ok(UserTrafficSnapshot {
            tx_bytes: persisted.tx_bytes,
            tx_speed: speed,
            rx_bytes: persisted.rx_bytes,
            rx_speed: speed,
        })
    }

    pub async fn flush_all(&self) -> VpnResult<()> {
        self.collect_proxy_traffic()?;
        self.drain_proxy_upload_once().await?;
        Ok(())
    }

    pub async fn flush_proxy_traffic(&self) -> VpnResult<Vec<ProxyTrafficReportResp>> {
        self.collect_proxy_traffic()?;
        self.drain_proxy_upload_once().await
    }

    fn remote_reporter(&self) -> Option<PnTrafficReporterRef> {
        self.remote_reporter.lock().unwrap().clone()
    }

    fn proxy_delta_provider(&self) -> Option<ProxyTrafficDeltaProviderRef> {
        self.proxy_delta_provider.lock().unwrap().clone()
    }

    fn proxy_connection_source(&self) -> Option<ProxyConnectionTrafficSourceRef> {
        self.proxy_connection_source.lock().unwrap().clone()
    }

    pub fn pending_proxy_batch_count(&self) -> usize {
        self.pending_proxy_batches.lock().unwrap().len()
    }

    pub fn proxy_queue_status(&self) -> ProxyTrafficQueueStatus {
        let queue = self.pending_proxy_batches.lock().unwrap();
        ProxyTrafficQueueStatus {
            queued_batches: queue.len(),
            queued_records: queue.iter().map(|batch| batch.records.len()).sum(),
            oldest_batch_id: queue.front().map(|batch| batch.batch_id),
            terminal_rejected_records: self.proxy_terminal_rejected.load(Ordering::SeqCst),
        }
    }

    pub fn collect_proxy_traffic(&self) -> VpnResult<bool> {
        let _collect_guard = self.proxy_collect_lock.lock().unwrap();
        if self.proxy_uploader_shutdown.load(Ordering::SeqCst) {
            return Err(vpn_err!(
                VpnErrorCode::Failed,
                "proxy traffic collector is shutting down"
            ));
        }
        let capacity = self.proxy_upload_config.lock().unwrap().backlog_batches;
        if self.pending_proxy_batch_count() >= capacity {
            return Err(vpn_err!(
                VpnErrorCode::Failed,
                "proxy traffic batch backlog is full ({})",
                capacity
            ));
        }

        if let Some(source) = self.proxy_connection_source() {
            return self.collect_connection_snapshots(source);
        }

        let Some(provider) = self.proxy_delta_provider() else {
            return Ok(false);
        };
        let samples = provider
            .read_proxy_traffic_deltas()
            .into_iter()
            .filter(|snapshot| snapshot.tx_bytes > 0 || snapshot.rx_bytes > 0)
            .map(proxy_snapshot_to_sample)
            .collect::<Vec<_>>();
        if samples.is_empty() {
            return Ok(false);
        }
        let (started_at_ms, ended_at_ms) = self
            .proxy_report_state
            .lock()
            .unwrap()
            .next_collection_window()?;
        self.enqueue_proxy_samples_locked(started_at_ms, ended_at_ms, samples)?;
        Ok(true)
    }

    pub fn submit_proxy_batch(
        &self,
        started_at_ms: u64,
        ended_at_ms: u64,
        samples: Vec<PnTrafficSample>,
    ) -> VpnResult<bool> {
        if started_at_ms > ended_at_ms {
            return Err(vpn_err!(
                VpnErrorCode::InvalidParam,
                "proxy traffic batch interval is reversed"
            ));
        }
        if self.proxy_uploader_shutdown.load(Ordering::SeqCst) {
            return Err(vpn_err!(
                VpnErrorCode::Failed,
                "proxy traffic uploader is shutting down"
            ));
        }
        let samples = samples
            .into_iter()
            .filter(|sample| {
                sample.source_to_dest.bytes > 0
                    || sample.dest_to_source.bytes > 0
                    || sample.source_to_dest.speed_bytes_per_sec > 0
                    || sample.dest_to_source.speed_bytes_per_sec > 0
            })
            .collect::<Vec<_>>();
        if samples.is_empty() {
            return Ok(false);
        }
        let _collect_guard = self.proxy_collect_lock.lock().unwrap();
        self.enqueue_proxy_samples_locked(started_at_ms, ended_at_ms, samples)?;
        Ok(true)
    }

    fn enqueue_proxy_samples_locked(
        &self,
        started_at_ms: u64,
        ended_at_ms: u64,
        samples: Vec<PnTrafficSample>,
    ) -> VpnResult<()> {
        let capacity = self.proxy_upload_config.lock().unwrap().backlog_batches;
        let mut queue = self.pending_proxy_batches.lock().unwrap();
        if queue.len() >= capacity {
            return Err(vpn_err!(
                VpnErrorCode::Failed,
                "proxy traffic batch backlog is full ({})",
                capacity
            ));
        }
        let (batch_id, records) = self.make_proxy_batch_records(
            started_at_ms,
            ended_at_ms,
            samples.into_iter().map(|sample| (sample, Vec::new())).collect(),
        );
        queue.push_back(TrafficReportBatch { batch_id, records });
        drop(queue);
        self.proxy_upload_notify.notify_one();
        Ok(())
    }

    fn collect_connection_snapshots(
        &self,
        source: ProxyConnectionTrafficSourceRef,
    ) -> VpnResult<bool> {
        let snapshots = source.snapshot_connections();
        if snapshots.is_empty() {
            return Ok(false);
        }

        let mut aggregated: HashMap<ProxyPairKey, AggregatedProxyTraffic> = HashMap::new();
        let mut cursor_updates = Vec::with_capacity(snapshots.len());
        let mut pruned_rejected_keys = Vec::new();
        let cursor = self.proxy_collection_cursor.lock().unwrap();
        for snapshot in snapshots.iter() {
            let from = NodeId::from(snapshot.from.as_slice());
            let to = NodeId::from(snapshot.to.as_slice());
            let connection_key = proxy_connection_key(snapshot);
            let rejected_target = self
                .proxy_rejected_targets
                .lock()
                .unwrap()
                .get(&connection_key)
                .copied();
            if rejected_target == Some((snapshot.tx_bytes, snapshot.rx_bytes)) {
                if !snapshot.active
                    && source.acknowledge_connections(std::slice::from_ref(snapshot)) == 1
                {
                    self.proxy_rejected_targets
                        .lock()
                        .unwrap()
                        .remove(&connection_key);
                    self.proxy_accepted_baselines
                        .lock()
                        .unwrap()
                        .remove(&connection_key);
                    pruned_rejected_keys.push(connection_key);
                }
                continue;
            }
            if rejected_target.is_some() {
                self.proxy_rejected_targets
                    .lock()
                    .unwrap()
                    .remove(&connection_key);
            }
            let previous = cursor.get(&connection_key).copied().unwrap_or_default();
            let tx_bytes = pending_bytes(snapshot.tx_bytes, previous.0);
            let rx_bytes = pending_bytes(snapshot.rx_bytes, previous.1);
            cursor_updates.push((connection_key, snapshot.tx_bytes, snapshot.rx_bytes, snapshot.active));
            if tx_bytes == 0
                && rx_bytes == 0
                && snapshot.tx_speed == 0
                && snapshot.rx_speed == 0
            {
                continue;
            }

            let forward = from.as_slice() <= to.as_slice();
            let (node_a, node_b) = if forward { (from, to) } else { (to, from) };
            let aggregate = aggregated
                .entry(ProxyPairKey {
                    network_id: snapshot.network_id,
                    node_a,
                    node_b,
                })
                .or_default();
            if forward {
                aggregate.node_a_to_b_bytes = aggregate.node_a_to_b_bytes.saturating_add(tx_bytes);
                aggregate.node_b_to_a_bytes = aggregate.node_b_to_a_bytes.saturating_add(rx_bytes);
                aggregate.node_a_to_b_speed = aggregate
                    .node_a_to_b_speed
                    .saturating_add(if snapshot.active { snapshot.tx_speed } else { 0 });
                aggregate.node_b_to_a_speed = aggregate
                    .node_b_to_a_speed
                    .saturating_add(if snapshot.active { snapshot.rx_speed } else { 0 });
            } else {
                aggregate.node_a_to_b_bytes = aggregate.node_a_to_b_bytes.saturating_add(rx_bytes);
                aggregate.node_b_to_a_bytes = aggregate.node_b_to_a_bytes.saturating_add(tx_bytes);
                aggregate.node_a_to_b_speed = aggregate
                    .node_a_to_b_speed
                    .saturating_add(if snapshot.active { snapshot.rx_speed } else { 0 });
                aggregate.node_b_to_a_speed = aggregate
                    .node_b_to_a_speed
                    .saturating_add(if snapshot.active { snapshot.tx_speed } else { 0 });
            }
            aggregate.source_snapshots.push(snapshot.clone());
        }
        drop(cursor);
        if !pruned_rejected_keys.is_empty() {
            let mut cursor = self.proxy_collection_cursor.lock().unwrap();
            for key in pruned_rejected_keys {
                cursor.remove(&key);
            }
        }

        if aggregated.is_empty() {
            let mut cursor = self.proxy_collection_cursor.lock().unwrap();
            for (key, tx, rx, _) in cursor_updates {
                cursor.insert(key, (tx, rx));
            }
            drop(cursor);
            let accepted = self.proxy_accepted_baselines.lock().unwrap();
            let safely_acknowledged = snapshots
                .iter()
                .filter(|snapshot| {
                    !snapshot.active
                        && accepted
                            .get(&proxy_connection_key(snapshot))
                            .is_some_and(|baseline| {
                                *baseline == (snapshot.tx_bytes, snapshot.rx_bytes)
                            })
                })
                .cloned()
                .collect::<Vec<_>>();
            drop(accepted);
            let rejected = self.proxy_rejected_targets.lock().unwrap();
            let safely_acknowledged = safely_acknowledged
                .into_iter()
                .filter(|snapshot| !rejected.contains_key(&proxy_connection_key(snapshot)))
                .collect::<Vec<_>>();
            drop(rejected);
            let pruned_keys = safely_acknowledged
                .iter()
                .filter(|snapshot| {
                    source.acknowledge_connections(std::slice::from_ref(snapshot)) == 1
                })
                .map(proxy_connection_key)
                .collect::<Vec<_>>();
            if !pruned_keys.is_empty() {
                let mut cursor = self.proxy_collection_cursor.lock().unwrap();
                let mut accepted = self.proxy_accepted_baselines.lock().unwrap();
                for key in pruned_keys {
                    cursor.remove(&key);
                    accepted.remove(&key);
                }
            }
            return Ok(false);
        }

        let (started_at_ms, ended_at_ms) = self
            .proxy_report_state
            .lock()
            .unwrap()
            .next_collection_window()?;
        let mut pairs = aggregated.into_iter().collect::<Vec<_>>();
        pairs.sort_by(|(left, _), (right, _)| {
            left.network_id
                .cmp(&right.network_id)
                .then_with(|| left.node_a.as_slice().cmp(right.node_a.as_slice()))
                .then_with(|| left.node_b.as_slice().cmp(right.node_b.as_slice()))
        });
        let records = pairs
            .into_iter()
            .map(|(pair, aggregate)| {
                (
                    PnTrafficSample {
                        network_id: pair.network_id,
                        source_id: pair.node_a,
                        dest_id: pair.node_b,
                        source_to_dest: PnTrafficDirectionSample {
                            bytes: aggregate.node_a_to_b_bytes,
                            speed_bytes_per_sec: aggregate.node_a_to_b_speed,
                        },
                        dest_to_source: PnTrafficDirectionSample {
                            bytes: aggregate.node_b_to_a_bytes,
                            speed_bytes_per_sec: aggregate.node_b_to_a_speed,
                        },
                    },
                    aggregate.source_snapshots,
                )
            })
            .collect();

        let capacity = self.proxy_upload_config.lock().unwrap().backlog_batches;
        let mut queue = self.pending_proxy_batches.lock().unwrap();
        if queue.len() >= capacity {
            return Err(vpn_err!(
                VpnErrorCode::Failed,
                "proxy traffic batch backlog is full ({})",
                capacity
            ));
        }
        let (batch_id, records) =
            self.make_proxy_batch_records(started_at_ms, ended_at_ms, records);
        queue.push_back(TrafficReportBatch { batch_id, records });
        drop(queue);
        let mut cursor = self.proxy_collection_cursor.lock().unwrap();
        for (key, tx, rx, _) in cursor_updates {
            cursor.insert(key, (tx, rx));
        }
        drop(cursor);
        self.proxy_upload_notify.notify_one();
        Ok(true)
    }

    fn make_proxy_batch_records(
        &self,
        started_at_ms: u64,
        ended_at_ms: u64,
        records: Vec<(PnTrafficSample, Vec<PnConnectionTrafficSnapshot>)>,
    ) -> (u64, Vec<PendingProxyTrafficRecord>) {
        let mut state = self.proxy_report_state.lock().unwrap();
        let batch_id = state.next_report_seq;
        state.next_report_seq = state.next_report_seq.wrapping_add(1);
        state.last_report_ended_at_ms = Some(ended_at_ms);
        let records = records
            .into_iter()
            .enumerate()
            .map(|(index, (traffic_sample, source_snapshots))| PendingProxyTrafficRecord {
                report: ProxyTrafficReport {
                    report_id: ProxyTrafficReportId(format!(
                        "proxy-traffic-{}-{}-{}-{}",
                        started_at_ms, ended_at_ms, batch_id, index
                    )),
                    started_at_ms,
                    ended_at_ms,
                    traffic_sample,
                },
                source_snapshots,
            })
            .collect();
        (batch_id, records)
    }

    async fn drain_proxy_upload_once(&self) -> VpnResult<Vec<ProxyTrafficReportResp>> {
        let _upload_guard = self.proxy_upload_lock.lock().await;
        let Some(reporter) = self.remote_reporter() else {
            return Ok(Vec::new());
        };
        let config = *self.proxy_upload_config.lock().unwrap();
        let mut per_batch_chunks = self
            .pending_proxy_batches
            .lock()
            .unwrap()
            .iter()
            .map(|batch| {
                batch
                    .records
                    .chunks(config.records_per_command)
                    .map(|records| {
                        records
                            .iter()
                            .map(|record| record.report.clone())
                            .collect::<Vec<_>>()
                    })
                    .collect::<VecDeque<_>>()
            })
            .collect::<Vec<_>>();
        if per_batch_chunks.iter().all(VecDeque::is_empty) {
            return Ok(Vec::new());
        }
        let mut chunks = Vec::new();
        loop {
            let mut made_progress = false;
            for batch in per_batch_chunks.iter_mut() {
                if let Some(chunk) = batch.pop_front() {
                    chunks.push(chunk);
                    made_progress = true;
                }
            }
            if !made_progress {
                break;
            }
        }
        let mut next_chunk = 0;
        let mut joins = tokio::task::JoinSet::new();
        let mut responses = Vec::new();
        let mut first_error = None;
        while next_chunk < chunks.len() || !joins.is_empty() {
            while next_chunk < chunks.len() && joins.len() < config.concurrent_commands {
                let reporter = reporter.clone();
                let requested = chunks[next_chunk].clone();
                next_chunk += 1;
                joins.spawn(async move {
                    let result = reporter.report_proxy_traffic(requested.clone()).await;
                    (requested, result)
                });
            }
            let Some(joined) = joins.join_next().await else {
                break;
            };
            match joined {
                Ok((requested, Ok(chunk_responses))) => {
                    match validate_proxy_report_responses(&requested, &chunk_responses) {
                        Ok(()) => responses.extend(chunk_responses),
                        Err(err) => {
                            first_error.get_or_insert_with(|| err.msg().to_owned());
                        }
                    }
                }
                Ok((_, Err(err))) => {
                    first_error.get_or_insert_with(|| err.msg().to_owned());
                }
                Err(err) => {
                    first_error.get_or_insert_with(|| err.to_string());
                }
            }
        }
        let retryable_response = self.reconcile_proxy_responses(&responses);
        if self.pending_proxy_batch_count() > 0 {
            self.proxy_upload_notify.notify_one();
        }
        if let Some(message) = first_error {
            Err(vpn_err!(VpnErrorCode::Failed, "proxy traffic upload failed: {}", message))
        } else if retryable_response {
            Err(vpn_err!(
                VpnErrorCode::Failed,
                "proxy traffic upload returned retryable record results"
            ))
        } else {
            Ok(responses)
        }
    }

    fn reconcile_proxy_responses(&self, responses: &[ProxyTrafficReportResp]) -> bool {
        let terminal = responses
            .iter()
            .filter(|response| {
                response.result != ProxyTrafficReportApplyResult::Retryable
            })
            .map(|response| response.report_id.clone())
            .collect::<HashSet<_>>();
        let retryable_response = responses.iter().any(|response| {
            response.result == ProxyTrafficReportApplyResult::Retryable
        });
        let accepted = responses
            .iter()
            .filter(|response| {
                matches!(
                    response.result,
                    ProxyTrafficReportApplyResult::Applied
                        | ProxyTrafficReportApplyResult::Duplicate
                )
            })
            .map(|response| response.report_id.clone())
            .collect::<HashSet<_>>();
        for response in responses.iter().filter(|response| {
            response.result == ProxyTrafficReportApplyResult::Rejected
        }) {
            self.proxy_terminal_rejected.fetch_add(1, Ordering::SeqCst);
            log::error!(
                "proxy traffic record terminally rejected report_id={} error_code={:?}",
                response.report_id.0,
                response.error_code
            );
        }
        let mut acknowledged_snapshots = Vec::new();
        let mut rejected_snapshots = Vec::new();
        let mut accepted_baselines = self.proxy_accepted_baselines.lock().unwrap();
        let mut queue = self.pending_proxy_batches.lock().unwrap();
        for batch in queue.iter_mut() {
            batch.records.retain(|record| {
                if !terminal.contains(&record.report.report_id) {
                    return true;
                }
                if accepted.contains(&record.report.report_id) {
                    for snapshot in record.source_snapshots.iter() {
                        let key = proxy_connection_key(snapshot);
                        accepted_baselines.insert(key, (snapshot.tx_bytes, snapshot.rx_bytes));
                    }
                    acknowledged_snapshots.extend(record.source_snapshots.iter().cloned());
                } else {
                    rejected_snapshots.extend(record.source_snapshots.iter().cloned());
                }
                false
            });
        }
        queue.retain(|batch| !batch.records.is_empty());
        drop(queue);
        drop(accepted_baselines);
        if !rejected_snapshots.is_empty() {
            let source = self.proxy_connection_source();
            for snapshot in rejected_snapshots {
                let key = proxy_connection_key(&snapshot);
                let target = (snapshot.tx_bytes, snapshot.rx_bytes);
                self.proxy_collection_cursor
                    .lock()
                    .unwrap()
                    .insert(key.clone(), target);
                self.proxy_rejected_targets
                    .lock()
                    .unwrap()
                    .insert(key.clone(), target);
                if !snapshot.active
                    && source.as_ref().is_some_and(|source| {
                        source.acknowledge_connections(std::slice::from_ref(&snapshot)) == 1
                    })
                {
                    self.proxy_collection_cursor.lock().unwrap().remove(&key);
                    self.proxy_accepted_baselines.lock().unwrap().remove(&key);
                    self.proxy_rejected_targets.lock().unwrap().remove(&key);
                }
            }
        }
        if !acknowledged_snapshots.is_empty() {
            if let Some(source) = self.proxy_connection_source() {
                for snapshot in acknowledged_snapshots.iter().filter(|snapshot| !snapshot.active) {
                    if source.acknowledge_connections(std::slice::from_ref(snapshot)) == 1 {
                        let key = proxy_connection_key(snapshot);
                        self.proxy_collection_cursor.lock().unwrap().remove(&key);
                        self.proxy_accepted_baselines.lock().unwrap().remove(&key);
                    }
                }
            }
        }
        retryable_response
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

async fn wait_for_background_task(
    task: Option<tokio::task::JoinHandle<()>>,
    deadline: Instant,
) -> bool {
    let Some(mut task) = task else {
        return true;
    };
    let remaining = deadline.saturating_duration_since(Instant::now());
    match tokio::time::timeout(remaining, &mut task).await {
        Ok(result) => result.is_ok(),
        Err(_) => {
            task.abort();
            false
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn proxy_snapshot_to_sample(snapshot: PnTrafficSnapshot) -> PnTrafficSample {
    PnTrafficSample {
        network_id: snapshot.network_id,
        source_id: snapshot.source_id,
        dest_id: snapshot.dest_id,
        source_to_dest: PnTrafficDirectionSample {
            bytes: snapshot.tx_bytes,
            speed_bytes_per_sec: snapshot.tx_speed,
        },
        dest_to_source: PnTrafficDirectionSample {
            bytes: snapshot.rx_bytes,
            speed_bytes_per_sec: snapshot.rx_speed,
        },
    }
}

fn pending_bytes(runtime_total: u64, flushed_total: u64) -> u64 {
    if runtime_total >= flushed_total {
        runtime_total - flushed_total
    } else {
        runtime_total
    }
}

fn validate_proxy_report_responses(
    requested: &[ProxyTrafficReport],
    responses: &[ProxyTrafficReportResp],
) -> VpnResult<()> {
    let requested_ids = requested
        .iter()
        .map(|report| report.report_id.clone())
        .collect::<HashSet<_>>();
    let response_ids = responses
        .iter()
        .map(|response| response.report_id.clone())
        .collect::<HashSet<_>>();
    if requested_ids.len() != requested.len()
        || response_ids.len() != responses.len()
        || requested_ids != response_ids
    {
        return Err(vpn_err!(
            VpnErrorCode::InvalidParam,
            "proxy traffic response ids do not match requested record ids"
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "pn_traffic_service_tests.rs"]
mod event_driven_tests;
