use crate::server_config::PnTrafficUploadConfig;
use crate::sqlite_store_factory::{
    NODE_TRAFFIC_REPORT_CLEANUP_BATCH_SIZE, SqliteStoreFactory,
};
use p2p_frame::pn::{PnServer, PnUserTrafficSnapshot};
use std::collections::{HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use vpn_frame::errors::{VpnErrorCode, VpnResult, vpn_err};
use vpn_frame::server::{NetworkGroupId, NetworkStore, NodeId, VpnStoreFactory};
use vpn_frame::{
    NodeTrafficDelta, NodeTrafficReport, NodeTrafficReportId, NodeTrafficReportResp,
    ProxyTrafficReportApplyResult,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UserTrafficSnapshot {
    pub tx_bytes: u64,
    pub tx_speed: u64,
    pub rx_bytes: u64,
    pub rx_speed: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NodeReportState {
    next_report_seq: u64,
    wall_clock_anchor_ms: u64,
    monotonic_anchor: Instant,
    last_report_ended_at_ms: Option<u64>,
}

impl Default for NodeReportState {
    fn default() -> Self {
        Self {
            next_report_seq: 0,
            wall_clock_anchor_ms: now_ms(),
            monotonic_anchor: Instant::now(),
            last_report_ended_at_ms: None,
        }
    }
}

impl NodeReportState {
    fn next_collection_window(&self) -> VpnResult<(u64, u64)> {
        let started_at_ms = self
            .last_report_ended_at_ms
            .unwrap_or(self.wall_clock_anchor_ms);
        let minimum_ended_at_ms = started_at_ms.checked_add(1).ok_or_else(|| {
            vpn_err!(
                VpnErrorCode::Failed,
                "node traffic report timestamp is exhausted"
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeTrafficSourceSnapshot {
    pub node_id: NodeId,
    pub tx_bytes: u64,
    pub tx_speed: u64,
    pub rx_bytes: u64,
    pub rx_speed: u64,
    pub tx_delta_bytes: u64,
    pub rx_delta_bytes: u64,
}

impl NodeTrafficSourceSnapshot {
    fn from_upstream(node_id: NodeId, snapshot: PnUserTrafficSnapshot) -> Self {
        Self {
            node_id,
            tx_bytes: snapshot.tx_bytes,
            tx_speed: snapshot.tx_speed,
            rx_bytes: snapshot.rx_bytes,
            rx_speed: snapshot.rx_speed,
            tx_delta_bytes: snapshot.tx_delta_bytes,
            rx_delta_bytes: snapshot.rx_delta_bytes,
        }
    }

    fn has_reportable_traffic(&self) -> bool {
        self.tx_delta_bytes > 0
            || self.rx_delta_bytes > 0
            || self.tx_speed > 0
            || self.rx_speed > 0
    }
}

pub trait NodeTrafficSource: Send + Sync + 'static {
    /// Consumes the source baseline for every returned node.
    fn take_node_traffic_snapshots(&self) -> Vec<NodeTrafficSourceSnapshot>;
}

impl NodeTrafficSource for PnServer {
    fn take_node_traffic_snapshots(&self) -> Vec<NodeTrafficSourceSnapshot> {
        self.iter_user_traffic_snapshots()
            .map(|(node_id, snapshot)| {
                NodeTrafficSourceSnapshot::from_upstream(
                    NodeId::from(node_id.as_slice()),
                    snapshot,
                )
            })
            .collect()
    }
}

pub type NodeTrafficSourceRef = Arc<dyn NodeTrafficSource>;

#[derive(Clone, Debug)]
struct PendingNodeTrafficRecord {
    report: NodeTrafficReport,
}

#[derive(Clone, Debug)]
struct NodeTrafficReportBatch {
    batch_id: u64,
    records: Vec<PendingNodeTrafficRecord>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeTrafficQueueStatus {
    pub queued_batches: usize,
    pub queued_records: usize,
    pub oldest_batch_id: Option<u64>,
    pub terminal_rejected_records: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeTrafficShutdownStatus {
    pub collector_exited: bool,
    pub final_collection_succeeded: bool,
    pub final_collection_error: Option<String>,
    pub uploader_exited: bool,
    pub cleanup_exited: bool,
    pub queue: NodeTrafficQueueStatus,
}

impl NodeTrafficShutdownStatus {
    pub fn is_success(&self) -> bool {
        self.collector_exited
            && self.final_collection_succeeded
            && self.uploader_exited
            && self.cleanup_exited
            && self.queue.queued_batches == 0
            && self.queue.queued_records == 0
    }
}

#[async_trait::async_trait]
pub trait PnTrafficReporter: Send + Sync + 'static {
    async fn report_heartbeat(&self) -> VpnResult<()>;

    async fn report_node_traffic(
        &self,
        reports: Vec<NodeTrafficReport>,
    ) -> VpnResult<Vec<NodeTrafficReportResp>>;
}

pub type PnTrafficReporterRef = Arc<dyn PnTrafficReporter>;

pub struct PnTrafficService {
    store_factory: Option<Arc<SqliteStoreFactory>>,
    remote_reporter: Mutex<Option<PnTrafficReporterRef>>,
    node_traffic_source: Mutex<Option<NodeTrafficSourceRef>>,
    upload_config: Mutex<PnTrafficUploadConfig>,
    report_state: Mutex<NodeReportState>,
    pending_batches: Mutex<VecDeque<NodeTrafficReportBatch>>,
    collect_lock: Mutex<()>,
    upload_lock: tokio::sync::Mutex<()>,
    upload_notify: tokio::sync::Notify,
    collector_notify: tokio::sync::Notify,
    cleanup_notify: tokio::sync::Notify,
    collector_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    uploader_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    cleanup_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
    uploader_started: AtomicBool,
    cleanup_started: AtomicBool,
    collector_shutdown: AtomicBool,
    uploader_shutdown: AtomicBool,
    cleanup_shutdown: AtomicBool,
    terminal_rejected: AtomicU64,
}

pub type PnTrafficServiceRef = Arc<PnTrafficService>;

impl PnTrafficService {
    pub fn new(store_factory: Arc<SqliteStoreFactory>) -> PnTrafficServiceRef {
        Self::create(Some(store_factory))
    }

    pub fn new_without_store() -> PnTrafficServiceRef {
        Self::create(None)
    }

    fn create(store_factory: Option<Arc<SqliteStoreFactory>>) -> PnTrafficServiceRef {
        Arc::new(Self {
            store_factory,
            remote_reporter: Mutex::new(None),
            node_traffic_source: Mutex::new(None),
            upload_config: Mutex::new(PnTrafficUploadConfig::default()),
            report_state: Mutex::new(NodeReportState::default()),
            pending_batches: Mutex::new(VecDeque::new()),
            collect_lock: Mutex::new(()),
            upload_lock: tokio::sync::Mutex::new(()),
            upload_notify: tokio::sync::Notify::new(),
            collector_notify: tokio::sync::Notify::new(),
            cleanup_notify: tokio::sync::Notify::new(),
            collector_task: Mutex::new(None),
            uploader_task: Mutex::new(None),
            cleanup_task: Mutex::new(None),
            uploader_started: AtomicBool::new(false),
            cleanup_started: AtomicBool::new(false),
            collector_shutdown: AtomicBool::new(false),
            uploader_shutdown: AtomicBool::new(false),
            cleanup_shutdown: AtomicBool::new(false),
            terminal_rejected: AtomicU64::new(0),
        })
    }

    pub fn set_remote_reporter(&self, reporter: PnTrafficReporterRef) {
        *self.remote_reporter.lock().unwrap() = Some(reporter);
        self.upload_notify.notify_one();
    }

    pub fn set_node_traffic_source(&self, source: NodeTrafficSourceRef) {
        *self.node_traffic_source.lock().unwrap() = Some(source);
    }

    pub fn set_proxy_upload_config(&self, config: PnTrafficUploadConfig) {
        *self.upload_config.lock().unwrap() = config;
    }

    pub fn start_background_flush(self: &Arc<Self>, interval: Duration) {
        self.start_uploader();
        let this = self.clone();
        let task = tokio::spawn(async move {
            let first_tick = tokio::time::Instant::now() + interval;
            let mut ticker = tokio::time::interval_at(first_tick, interval);
            loop {
                tokio::select! {
                    _ = this.collector_notify.notified() => {
                        if this.collector_shutdown.load(Ordering::SeqCst) {
                            break;
                        }
                    }
                    _ = ticker.tick() => {
                        if this.collector_shutdown.load(Ordering::SeqCst) {
                            break;
                        }
                        if let Err(err) = this.collect_node_traffic() {
                            log::warn!(
                                "collect pn node traffic failed: code={:?} msg={}",
                                err.code(),
                                err.msg()
                            );
                        }
                    }
                }
            }
        });
        *self.collector_task.lock().unwrap() = Some(task);
    }

    pub fn start_uploader(self: &Arc<Self>) {
        if self
            .uploader_started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }
        let this = self.clone();
        let task = tokio::spawn(async move {
            loop {
                if this.uploader_shutdown.load(Ordering::SeqCst)
                    && this.pending_batch_count() == 0
                {
                    break;
                }
                if this.pending_batch_count() == 0 || this.remote_reporter().is_none() {
                    this.upload_notify.notified().await;
                    continue;
                }
                if let Err(err) = this.drain_upload_once().await {
                    log::warn!(
                        "upload node traffic batch failed: code={:?} msg={}",
                        err.code(),
                        err.msg()
                    );
                    let delay = this.upload_config.lock().unwrap().retry_delay_ms;
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
            }
        });
        *self.uploader_task.lock().unwrap() = Some(task);
    }

    pub fn start_node_traffic_cleanup(self: &Arc<Self>) {
        let Some(store_factory) = self.store_factory.clone() else {
            return;
        };
        if self
            .cleanup_started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        let retention = store_factory.node_traffic_idempotency_retention();
        let cleanup_interval = retention.min(Duration::from_secs(60));
        let this = self.clone();
        let task = tokio::spawn(async move {
            let first_tick = tokio::time::Instant::now() + cleanup_interval;
            let mut ticker = tokio::time::interval_at(first_tick, cleanup_interval);
            loop {
                if this.cleanup_shutdown.load(Ordering::SeqCst) {
                    break;
                }
                tokio::select! {
                    _ = this.cleanup_notify.notified() => {
                        if this.cleanup_shutdown.load(Ordering::SeqCst) {
                            break;
                        }
                    }
                    _ = ticker.tick() => {
                        if this.cleanup_shutdown.load(Ordering::SeqCst) {
                            break;
                        }
                        let cutoff_ms = match store_factory.expiration_cutoff_ms() {
                            Ok(Some(cutoff_ms)) => cutoff_ms,
                            Ok(None) => continue,
                            Err(err) => {
                                log::warn!(
                                    "calculate expired pn node traffic report cutoff failed: code={:?} msg={}",
                                    err.code(),
                                    err.msg()
                                );
                                continue;
                            }
                        };
                        loop {
                            if this.cleanup_shutdown.load(Ordering::SeqCst) {
                                break;
                            }
                            match store_factory
                                .cleanup_expired_node_traffic_reports(
                                    cutoff_ms,
                                    NODE_TRAFFIC_REPORT_CLEANUP_BATCH_SIZE,
                                )
                                .await
                            {
                                Ok(deleted) => {
                                    if deleted
                                        < u64::try_from(NODE_TRAFFIC_REPORT_CLEANUP_BATCH_SIZE)
                                            .unwrap_or(u64::MAX)
                                    {
                                        break;
                                    }
                                    if this.cleanup_shutdown.load(Ordering::SeqCst) {
                                        break;
                                    }
                                    tokio::task::yield_now().await;
                                }
                                Err(err) => {
                                    log::warn!(
                                        "cleanup expired pn node traffic reports failed: code={:?} msg={}",
                                        err.code(),
                                        err.msg()
                                    );
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        });
        *self.cleanup_task.lock().unwrap() = Some(task);
    }

    pub async fn shutdown_node_traffic(&self, timeout: Duration) -> NodeTrafficShutdownStatus {
        let deadline = Instant::now() + timeout;
        self.collector_shutdown.store(true, Ordering::SeqCst);
        self.collector_notify.notify_waiters();
        self.cleanup_shutdown.store(true, Ordering::SeqCst);
        self.cleanup_notify.notify_one();
        let collector_task = self.collector_task.lock().unwrap().take();
        let collector_exited = wait_for_background_task(collector_task, deadline).await;
        let cleanup_task = self.cleanup_task.lock().unwrap().take();
        let cleanup_exited = wait_for_background_task(cleanup_task, deadline).await;

        let (final_collection_succeeded, final_collection_error) = if collector_exited {
            match self.collect_node_traffic() {
                Ok(_) => (true, None),
                Err(err) => (false, Some(err.msg().to_owned())),
            }
        } else {
            (
                false,
                Some("node traffic collector did not exit before the shutdown deadline".to_owned()),
            )
        };

        self.uploader_shutdown.store(true, Ordering::SeqCst);
        self.upload_notify.notify_waiters();
        let uploader_task = self.uploader_task.lock().unwrap().take();
        let uploader_exited = wait_for_background_task(uploader_task, deadline).await;
        NodeTrafficShutdownStatus {
            collector_exited,
            final_collection_succeeded,
            final_collection_error,
            uploader_exited,
            cleanup_exited,
            queue: self.queue_status(),
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
        let (persisted, node_ids) = {
            let mut store = self.store_factory()?.get_vpn_store().await?;
            let persisted = store.get_persisted_group_traffic(group_id).await?;
            let node_ids = store
                .get_joined_nodes(group_id)
                .await?
                .into_iter()
                .map(|node| node.node_id)
                .collect::<Vec<_>>();
            (persisted, node_ids)
        };
        let speed = self.store_factory()?.get_group_traffic_speed(&node_ids);
        Ok(UserTrafficSnapshot {
            tx_bytes: persisted.tx_bytes,
            tx_speed: speed.tx_bytes,
            rx_bytes: persisted.rx_bytes,
            rx_speed: speed.rx_bytes,
        })
    }

    pub async fn flush_all(&self) -> VpnResult<()> {
        self.collect_node_traffic()?;
        self.drain_upload_once().await?;
        Ok(())
    }

    pub async fn flush_node_traffic(&self) -> VpnResult<Vec<NodeTrafficReportResp>> {
        self.collect_node_traffic()?;
        self.drain_upload_once().await
    }

    pub fn pending_batch_count(&self) -> usize {
        self.pending_batches.lock().unwrap().len()
    }

    pub fn queue_status(&self) -> NodeTrafficQueueStatus {
        let queue = self.pending_batches.lock().unwrap();
        NodeTrafficQueueStatus {
            queued_batches: queue.len(),
            queued_records: queue.iter().map(|batch| batch.records.len()).sum(),
            oldest_batch_id: queue.front().map(|batch| batch.batch_id),
            terminal_rejected_records: self.terminal_rejected.load(Ordering::SeqCst),
        }
    }

    pub fn collect_node_traffic(&self) -> VpnResult<bool> {
        let _collect_guard = self.collect_lock.lock().unwrap();
        if self.uploader_shutdown.load(Ordering::SeqCst) {
            return Err(vpn_err!(
                VpnErrorCode::Failed,
                "node traffic collector is shutting down"
            ));
        }
        let Some(source) = self.node_traffic_source() else {
            return Ok(false);
        };

        // Hold the queue reservation while consuming the upstream iterator.
        // Every consumed delta is therefore installed in a pending batch before
        // another collection can observe capacity or the function can return.
        let capacity = self.upload_config.lock().unwrap().backlog_batches;
        let mut queue = self.pending_batches.lock().unwrap();
        if queue.len() >= capacity {
            return Err(vpn_err!(
                VpnErrorCode::Failed,
                "node traffic batch backlog is full ({})",
                capacity
            ));
        }
        let (started_at_ms, ended_at_ms) = self
            .report_state
            .lock()
            .unwrap()
            .next_collection_window()?;
        let snapshots = source.take_node_traffic_snapshots();
        let deltas = snapshots
            .into_iter()
            .filter(NodeTrafficSourceSnapshot::has_reportable_traffic)
            .map(|snapshot| NodeTrafficDelta {
                node_id: snapshot.node_id,
                tx_bytes: snapshot.tx_delta_bytes,
                rx_bytes: snapshot.rx_delta_bytes,
                tx_speed: snapshot.tx_speed,
                rx_speed: snapshot.rx_speed,
            })
            .collect::<Vec<_>>();
        if deltas.is_empty() {
            return Ok(false);
        }
        let batch = self.make_batch(started_at_ms, ended_at_ms, deltas);
        queue.push_back(batch);
        drop(queue);
        self.upload_notify.notify_one();
        Ok(true)
    }

    pub fn submit_node_batch(
        &self,
        started_at_ms: u64,
        ended_at_ms: u64,
        deltas: Vec<NodeTrafficDelta>,
    ) -> VpnResult<bool> {
        if started_at_ms > ended_at_ms {
            return Err(vpn_err!(
                VpnErrorCode::InvalidParam,
                "node traffic batch interval is reversed"
            ));
        }
        if self.uploader_shutdown.load(Ordering::SeqCst) {
            return Err(vpn_err!(
                VpnErrorCode::Failed,
                "node traffic uploader is shutting down"
            ));
        }
        let deltas = deltas
            .into_iter()
            .filter(|delta| {
                delta.tx_bytes > 0
                    || delta.rx_bytes > 0
                    || delta.tx_speed > 0
                    || delta.rx_speed > 0
            })
            .collect::<Vec<_>>();
        if deltas.is_empty() {
            return Ok(false);
        }
        let _collect_guard = self.collect_lock.lock().unwrap();
        let capacity = self.upload_config.lock().unwrap().backlog_batches;
        let mut queue = self.pending_batches.lock().unwrap();
        if queue.len() >= capacity {
            return Err(vpn_err!(
                VpnErrorCode::Failed,
                "node traffic batch backlog is full ({})",
                capacity
            ));
        }
        queue.push_back(self.make_batch(started_at_ms, ended_at_ms, deltas));
        drop(queue);
        self.upload_notify.notify_one();
        Ok(true)
    }

    fn make_batch(
        &self,
        started_at_ms: u64,
        ended_at_ms: u64,
        deltas: Vec<NodeTrafficDelta>,
    ) -> NodeTrafficReportBatch {
        let mut state = self.report_state.lock().unwrap();
        let batch_id = state.next_report_seq;
        state.next_report_seq = state.next_report_seq.wrapping_add(1);
        state.last_report_ended_at_ms = Some(ended_at_ms);
        let records = deltas
            .into_iter()
            .enumerate()
            .map(|(index, delta)| PendingNodeTrafficRecord {
                report: NodeTrafficReport {
                    report_id: NodeTrafficReportId(format!(
                        "node-traffic-{}-{}-{}-{}",
                        started_at_ms, ended_at_ms, batch_id, index
                    )),
                    started_at_ms,
                    ended_at_ms,
                    delta,
                },
            })
            .collect();
        NodeTrafficReportBatch { batch_id, records }
    }

    async fn drain_upload_once(&self) -> VpnResult<Vec<NodeTrafficReportResp>> {
        let _upload_guard = self.upload_lock.lock().await;
        let Some(reporter) = self.remote_reporter() else {
            return Ok(Vec::new());
        };
        let config = *self.upload_config.lock().unwrap();
        let mut per_batch_chunks = self
            .pending_batches
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
                    let result = reporter.report_node_traffic(requested.clone()).await;
                    (requested, result)
                });
            }
            let Some(joined) = joins.join_next().await else {
                break;
            };
            match joined {
                Ok((requested, Ok(chunk_responses))) => {
                    match validate_node_report_responses(&requested, &chunk_responses) {
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

        let retryable_response = self.reconcile_responses(&responses);
        if self.pending_batch_count() > 0 {
            self.upload_notify.notify_one();
        }
        if let Some(message) = first_error {
            Err(vpn_err!(
                VpnErrorCode::Failed,
                "node traffic upload failed: {}",
                message
            ))
        } else if retryable_response {
            Err(vpn_err!(
                VpnErrorCode::Failed,
                "node traffic upload returned retryable record results"
            ))
        } else {
            Ok(responses)
        }
    }

    fn reconcile_responses(&self, responses: &[NodeTrafficReportResp]) -> bool {
        let terminal = responses
            .iter()
            .filter(|response| response.result != ProxyTrafficReportApplyResult::Retryable)
            .map(|response| response.report_id.clone())
            .collect::<HashSet<_>>();
        let retryable_response = responses
            .iter()
            .any(|response| response.result == ProxyTrafficReportApplyResult::Retryable);
        for response in responses
            .iter()
            .filter(|response| response.result == ProxyTrafficReportApplyResult::Rejected)
        {
            self.terminal_rejected.fetch_add(1, Ordering::SeqCst);
            log::error!(
                "node traffic record terminally rejected report_id={} error_code={:?}",
                response.report_id.0,
                response.error_code
            );
        }
        let mut queue = self.pending_batches.lock().unwrap();
        for batch in queue.iter_mut() {
            batch
                .records
                .retain(|record| !terminal.contains(&record.report.report_id));
        }
        queue.retain(|batch| !batch.records.is_empty());
        retryable_response
    }

    fn remote_reporter(&self) -> Option<PnTrafficReporterRef> {
        self.remote_reporter.lock().unwrap().clone()
    }

    fn node_traffic_source(&self) -> Option<NodeTrafficSourceRef> {
        self.node_traffic_source.lock().unwrap().clone()
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

fn validate_node_report_responses(
    requested: &[NodeTrafficReport],
    responses: &[NodeTrafficReportResp],
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
            "node traffic response ids do not match requested record ids"
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/unit/pn_traffic_service_tests.rs"]
mod node_traffic_tests;
