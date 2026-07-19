use super::*;
use crate::server_config::{
    build_server_config, get_node_traffic_idempotency_retention_secs,
};
use crate::sqlx_store::{SqliteConnection, open_sqlite_pool};
use crate::sqlite_store_factory::{NodeTrafficControlClock, PersistedTrafficStats};
use sqlx::sqlite::SqliteJournalMode;
use sqlx::{Row, SqlitePool, query};
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use vpn_frame::server::{JoinedNode, NetworkStore, PnStore};

static RETENTION_TTL_TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn retention_ttl_temp_path(kind: &str, extension: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "bucky-vpn-node-traffic-retention-ttl-{kind}-{}-{}.{extension}",
        std::process::id(),
        RETENTION_TTL_TEST_SEQ.fetch_add(1, Ordering::Relaxed),
    ))
}

struct MutableNodeTrafficControlClock {
    now_ms: AtomicU64,
    calls: AtomicUsize,
}

impl MutableNodeTrafficControlClock {
    fn new(now_ms: u64) -> Arc<Self> {
        Arc::new(Self {
            now_ms: AtomicU64::new(now_ms),
            calls: AtomicUsize::new(0),
        })
    }

    fn set(&self, now_ms: u64) {
        self.now_ms.store(now_ms, Ordering::SeqCst);
    }
}

impl NodeTrafficControlClock for MutableNodeTrafficControlClock {
    fn now_unix_ms(&self) -> VpnResult<u64> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.now_ms.load(Ordering::SeqCst))
    }
}

async fn retention_ttl_factory_with_clock(
    retention: Duration,
    speed_ttl: Duration,
    clock: Arc<dyn NodeTrafficControlClock>,
) -> (Arc<SqliteStoreFactory>, SqlitePool) {
    let db_path = retention_ttl_temp_path("store", "sqlite");
    let pool = open_sqlite_pool(
        db_path.to_str().unwrap(),
        5,
        Some(SqliteJournalMode::Wal),
    )
    .await
    .unwrap();
    let factory = Arc::new(
        SqliteStoreFactory::from_pool_with_node_traffic_settings_and_clock(
        pool.clone(),
        retention,
        speed_ttl,
        clock,
    ));
    let mut store = factory.get_vpn_store().await.unwrap();
    store.init_db().await.unwrap();
    drop(store);
    (factory, pool)
}

async fn retention_ttl_factory(
    retention: Duration,
    speed_ttl: Duration,
) -> (Arc<SqliteStoreFactory>, SqlitePool) {
    retention_ttl_factory_with_clock(
        retention,
        speed_ttl,
        MutableNodeTrafficControlClock::new(1_000_000),
    )
    .await
}

fn retention_ttl_report(
    report_id: &str,
    node_byte: u8,
    started_at_ms: u64,
    ended_at_ms: u64,
) -> NodeTrafficReport {
    NodeTrafficReport {
        report_id: NodeTrafficReportId(report_id.to_owned()),
        started_at_ms,
        ended_at_ms,
        delta: delta(node_byte, 10, 20, 3, 4),
    }
}

async fn retention_ttl_report_row_count(pool: &SqlitePool) -> i64 {
    let mut conn = SqliteConnection::acquire(pool).await.unwrap();
    conn.fetch_one(query("SELECT COUNT(*) AS count FROM pn_node_traffic_report"))
        .await
        .unwrap()
        .get("count")
}

#[tokio::test]
async fn sqlx_close_on_drop_discards_physical_connection() {
    let db_path = retention_ttl_temp_path("close-on-drop", "sqlite");
    let pool = open_sqlite_pool(db_path.to_str().unwrap(), 1, None)
        .await
        .unwrap();

    {
        let mut conn = SqliteConnection::acquire(&pool).await.unwrap();
        conn.execute(query(
            "CREATE TEMP TABLE physical_connection_identity (value INTEGER NOT NULL)",
        ))
        .await
        .unwrap();
        conn.execute(query(
            "INSERT INTO physical_connection_identity (value) VALUES (73)",
        ))
        .await
        .unwrap();
    }

    {
        let mut conn = SqliteConnection::acquire(&pool).await.unwrap();
        let value: i64 = conn
            .fetch_one(query("SELECT value FROM physical_connection_identity"))
            .await
            .unwrap()
            .get("value");
        assert_eq!(value, 73, "normal drop must reuse the physical connection");
        conn.close_on_drop();
    }

    let mut replacement = SqliteConnection::acquire(&pool).await.unwrap();
    assert!(
        replacement
            .fetch_one(query("SELECT value FROM physical_connection_identity"))
            .await
            .is_err(),
        "close_on_drop must discard the physical connection and its TEMP schema"
    );
    drop(replacement);
    pool.close().await;
    fs::remove_file(db_path).unwrap();
}

fn node(byte: u8) -> NodeId {
    NodeId::from(vec![byte; 32].as_slice())
}

fn source_snapshot(
    byte: u8,
    tx_delta_bytes: u64,
    rx_delta_bytes: u64,
    tx_speed: u64,
    rx_speed: u64,
) -> NodeTrafficSourceSnapshot {
    NodeTrafficSourceSnapshot {
        node_id: node(byte),
        tx_bytes: tx_delta_bytes,
        tx_speed,
        rx_bytes: rx_delta_bytes,
        rx_speed,
        tx_delta_bytes,
        rx_delta_bytes,
    }
}

fn delta(byte: u8, tx: u64, rx: u64, tx_speed: u64, rx_speed: u64) -> NodeTrafficDelta {
    NodeTrafficDelta {
        node_id: node(byte),
        tx_bytes: tx,
        rx_bytes: rx,
        tx_speed,
        rx_speed,
    }
}

struct FakeNodeTrafficSource {
    snapshots: Mutex<VecDeque<Vec<NodeTrafficSourceSnapshot>>>,
    calls: AtomicUsize,
}

impl FakeNodeTrafficSource {
    fn new(snapshots: Vec<Vec<NodeTrafficSourceSnapshot>>) -> Arc<Self> {
        Arc::new(Self {
            snapshots: Mutex::new(snapshots.into()),
            calls: AtomicUsize::new(0),
        })
    }

    fn push(&self, snapshots: Vec<NodeTrafficSourceSnapshot>) {
        self.snapshots.lock().unwrap().push_back(snapshots);
    }
}

impl NodeTrafficSource for FakeNodeTrafficSource {
    fn take_node_traffic_snapshots(&self) -> Vec<NodeTrafficSourceSnapshot> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.snapshots
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_default()
    }
}

#[derive(Clone, Copy)]
enum ReporterReply {
    Applied,
    RetryFirst,
    RejectFirst,
    Malformed,
}

struct FakeReporter {
    replies: Mutex<VecDeque<ReporterReply>>,
    calls: Mutex<Vec<Vec<NodeTrafficReport>>>,
    delay: Duration,
}

impl FakeReporter {
    fn new(replies: Vec<ReporterReply>) -> Arc<Self> {
        Arc::new(Self {
            replies: Mutex::new(replies.into()),
            calls: Mutex::new(Vec::new()),
            delay: Duration::ZERO,
        })
    }

    fn delayed(delay: Duration) -> Arc<Self> {
        Arc::new(Self {
            replies: Mutex::new(VecDeque::new()),
            calls: Mutex::new(Vec::new()),
            delay,
        })
    }
}

#[async_trait::async_trait]
impl PnTrafficReporter for FakeReporter {
    async fn report_heartbeat(&self) -> VpnResult<()> {
        Ok(())
    }

    async fn report_node_traffic(
        &self,
        reports: Vec<NodeTrafficReport>,
    ) -> VpnResult<Vec<NodeTrafficReportResp>> {
        self.calls.lock().unwrap().push(reports.clone());
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
        let reply = self
            .replies
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(ReporterReply::Applied);
        if matches!(reply, ReporterReply::Malformed) {
            return Ok(Vec::new());
        }
        Ok(reports
            .into_iter()
            .enumerate()
            .map(|(index, report)| NodeTrafficReportResp {
                report_id: report.report_id,
                result: match (reply, index) {
                    (ReporterReply::RetryFirst, 0) => ProxyTrafficReportApplyResult::Retryable,
                    (ReporterReply::RejectFirst, 0) => ProxyTrafficReportApplyResult::Rejected,
                    _ => ProxyTrafficReportApplyResult::Applied,
                },
                error_code: None,
            })
            .collect())
    }
}

fn upload_config(backlog_batches: usize) -> PnTrafficUploadConfig {
    PnTrafficUploadConfig {
        backlog_batches,
        ..PnTrafficUploadConfig::default()
    }
}

#[test]
fn collection_consumes_each_node_once_and_preserves_delta_and_speed() {
    let service = PnTrafficService::new_without_store();
    let source = FakeNodeTrafficSource::new(vec![vec![
        source_snapshot(1, 11, 12, 3, 4),
        source_snapshot(2, 21, 22, 5, 6),
        source_snapshot(3, 0, 0, 0, 0),
    ]]);
    service.set_node_traffic_source(source.clone());

    assert!(service.collect_node_traffic().unwrap());
    assert_eq!(source.calls.load(Ordering::SeqCst), 1);
    let queue = service.pending_batches.lock().unwrap();
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].records.len(), 2);
    assert_eq!(queue[0].records[0].report.delta, delta(1, 11, 12, 3, 4));
    assert_eq!(queue[0].records[1].report.delta, delta(2, 21, 22, 5, 6));
    assert_ne!(
        queue[0].records[0].report.report_id,
        queue[0].records[1].report.report_id
    );
}

#[test]
fn full_backlog_rejects_before_consuming_upstream_iterator() {
    let service = PnTrafficService::new_without_store();
    service.set_proxy_upload_config(upload_config(1));
    let source = FakeNodeTrafficSource::new(vec![vec![source_snapshot(1, 1, 2, 3, 4)]]);
    service.set_node_traffic_source(source.clone());
    assert!(service.collect_node_traffic().unwrap());
    source.push(vec![source_snapshot(2, 5, 6, 7, 8)]);

    assert!(service.collect_node_traffic().is_err());
    assert_eq!(source.calls.load(Ordering::SeqCst), 1);
    assert_eq!(service.queue_status().queued_records, 1);
}

#[tokio::test]
async fn retryable_record_keeps_stable_id_while_terminal_sibling_progresses() {
    let service = PnTrafficService::new_without_store();
    let reporter = FakeReporter::new(vec![ReporterReply::RetryFirst, ReporterReply::Applied]);
    service.set_remote_reporter(reporter.clone());
    service
        .submit_node_batch(10, 20, vec![delta(1, 10, 20, 3, 4), delta(2, 30, 40, 5, 6)])
        .unwrap();

    assert!(service.drain_upload_once().await.is_err());
    assert_eq!(service.queue_status().queued_records, 1);
    let first_id = reporter.calls.lock().unwrap()[0][0].report_id.clone();

    service.drain_upload_once().await.unwrap();
    assert_eq!(service.queue_status().queued_records, 0);
    let calls = reporter.calls.lock().unwrap();
    assert_eq!(calls[1].len(), 1);
    assert_eq!(calls[1][0].report_id, first_id);
}

#[tokio::test]
async fn malformed_response_retains_owned_records_for_retry() {
    let service = PnTrafficService::new_without_store();
    let reporter = FakeReporter::new(vec![ReporterReply::Malformed, ReporterReply::Applied]);
    service.set_remote_reporter(reporter);
    service
        .submit_node_batch(10, 20, vec![delta(1, 10, 20, 3, 4)])
        .unwrap();

    assert!(service.drain_upload_once().await.is_err());
    assert_eq!(service.queue_status().queued_records, 1);
    service.drain_upload_once().await.unwrap();
    assert_eq!(service.queue_status().queued_records, 0);
}

#[tokio::test]
async fn rejected_record_is_terminal_and_observable() {
    let service = PnTrafficService::new_without_store();
    service.set_remote_reporter(FakeReporter::new(vec![ReporterReply::RejectFirst]));
    service
        .submit_node_batch(10, 20, vec![delta(1, 10, 20, 3, 4)])
        .unwrap();

    service.drain_upload_once().await.unwrap();
    let status = service.queue_status();
    assert_eq!(status.queued_records, 0);
    assert_eq!(status.terminal_rejected_records, 1);
}

#[tokio::test]
async fn bounded_chunking_uploads_every_node_report_once() {
    let service = PnTrafficService::new_without_store();
    let reporter = FakeReporter::new(Vec::new());
    service.set_remote_reporter(reporter.clone());
    let deltas = (0..257)
        .map(|index| delta((index % 250) as u8, index + 1, 1, 2, 3))
        .collect();
    service.submit_node_batch(10, 20, deltas).unwrap();

    let responses = service.drain_upload_once().await.unwrap();
    assert_eq!(responses.len(), 257);
    let calls = reporter.calls.lock().unwrap();
    assert_eq!(calls.len(), 3);
    assert!(calls.iter().all(|call| call.len() <= 128));
    let ids = calls
        .iter()
        .flatten()
        .map(|report| report.report_id.clone())
        .collect::<HashSet<_>>();
    assert_eq!(ids.len(), 257);
}

#[tokio::test]
async fn graceful_shutdown_collects_final_nodes_and_drains_uploader() {
    let service = PnTrafficService::new_without_store();
    let source = FakeNodeTrafficSource::new(vec![vec![source_snapshot(1, 10, 20, 3, 4)]]);
    let reporter = FakeReporter::new(Vec::new());
    service.set_node_traffic_source(source);
    service.set_remote_reporter(reporter.clone());
    service.start_uploader();

    let status = service.shutdown_node_traffic(Duration::from_secs(1)).await;
    assert!(status.is_success(), "{status:?}");
    assert_eq!(reporter.calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn shutdown_timeout_reports_retained_queue() {
    let service = PnTrafficService::new_without_store();
    service.set_remote_reporter(FakeReporter::delayed(Duration::from_millis(200)));
    service
        .submit_node_batch(10, 20, vec![delta(1, 10, 20, 3, 4)])
        .unwrap();
    service.start_uploader();

    let status = service
        .shutdown_node_traffic(Duration::from_millis(20))
        .await;
    assert!(!status.uploader_exited);
    assert_eq!(status.queue.queued_records, 1);
}

#[tokio::test]
async fn group_snapshot_sums_distinct_node_upload_and_observational_download() {
    let db_path = std::env::temp_dir().join(format!(
        "bucky-vpn-node-traffic-group-{}-{}.sqlite",
        std::process::id(),
        now_ms()
    ));
    let factory = Arc::new(
        SqliteStoreFactory::create(db_path.to_str().unwrap())
            .await
            .unwrap(),
    );
    let group_id = 42;
    let first = node(1);
    let second = node(2);
    {
        let mut store = factory.get_vpn_store().await.unwrap();
        store.init_db().await.unwrap();
        store.add_network_group(&group_id).await.unwrap();
        for (node_id, name) in [(first.clone(), "first"), (second.clone(), "second")] {
            store
                .add_joined_node(&JoinedNode {
                    group_id,
                    node_id,
                    allow_join: true,
                    name: name.to_owned(),
                    comment: String::new(),
                })
                .await
                .unwrap();
        }
        store
            .add_persisted_group_traffic(
                &group_id,
                PersistedTrafficStats {
                    tx_bytes: 9_000,
                    rx_bytes: 9_000,
                },
            )
            .await
            .unwrap();
        let pn = node(9);
        for (index, delta) in [delta(1, 10, 20, 3, 4), delta(2, 30, 40, 5, 6)]
            .into_iter()
            .enumerate()
        {
            store
                .apply_node_traffic_report(
                    &pn,
                    &NodeTrafficReport {
                        report_id: NodeTrafficReportId(format!("group-node-{index}")),
                        started_at_ms: 10,
                        ended_at_ms: 20,
                        delta,
                    },
                )
                .await
                .unwrap();
        }
    }

    let service = PnTrafficService::new(factory);
    let snapshot = service.get_group_snapshot(&group_id).await.unwrap();
    assert_eq!(snapshot.tx_bytes, 40);
    assert_eq!(snapshot.rx_bytes, 60);
    assert_eq!(snapshot.tx_speed, 8);
    assert_eq!(snapshot.rx_speed, 10);
}

#[test]
fn node_traffic_retention_ttl_config_default_override_and_invalid_values() {
    let default_dir = retention_ttl_temp_path("config-default", "dir");
    fs::create_dir_all(&default_dir).unwrap();
    let config = build_server_config(None, &default_dir).unwrap();
    assert_eq!(
        get_node_traffic_idempotency_retention_secs(&config).unwrap(),
        600
    );
    fs::remove_dir_all(default_dir).unwrap();

    let override_dir = retention_ttl_temp_path("config-override", "dir");
    fs::create_dir_all(&override_dir).unwrap();
    fs::write(
        override_dir.join("config.yaml"),
        "pn:\n  node_traffic_idempotency_retention_secs: 37\n",
    )
    .unwrap();
    let config = build_server_config(None, &override_dir).unwrap();
    assert_eq!(
        get_node_traffic_idempotency_retention_secs(&config).unwrap(),
        37
    );
    fs::remove_dir_all(override_dir).unwrap();

    for (case, value) in [
        ("zero", "0"),
        ("negative", "-1"),
        ("floating-point", "1.5"),
        ("non-numeric", "\"not-a-number\""),
        ("over-u64", "18446744073709551616"),
    ] {
        let invalid_dir = retention_ttl_temp_path(case, "dir");
        fs::create_dir_all(&invalid_dir).unwrap();
        fs::write(
            invalid_dir.join("config.yaml"),
            format!("pn:\n  node_traffic_idempotency_retention_secs: {value}\n"),
        )
        .unwrap();
        let error = build_server_config(None, &invalid_dir)
            .and_then(|config| get_node_traffic_idempotency_retention_secs(&config))
            .expect_err("invalid explicit retention must be rejected");
        assert!(!error.to_string().is_empty(), "case={case}");
        fs::remove_dir_all(invalid_dir).unwrap();
    }
}

#[tokio::test]
async fn node_traffic_retention_ttl_duplicate_boundary_and_expired_replay_accumulate() {
    let retention = Duration::from_secs(10);
    let base_ms = 1_000_000;
    let clock = MutableNodeTrafficControlClock::new(base_ms);
    let (factory, _) = retention_ttl_factory_with_clock(
        retention,
        Duration::from_secs(15),
        clock.clone(),
    )
    .await;
    let pn_node_id = node(90);
    let traffic_node_id = node(91);
    let report = retention_ttl_report("retention-boundary", 91, 10, 20);
    let mut store = factory.get_vpn_store().await.unwrap();

    let calls = clock.calls.load(Ordering::SeqCst);
    assert_eq!(
        store
            .apply_node_traffic_report(&pn_node_id, &report)
            .await
            .unwrap(),
        ProxyTrafficReportApplyResult::Applied
    );
    assert_eq!(clock.calls.load(Ordering::SeqCst), calls + 1);

    clock.set(base_ms + 9_999);
    let calls = clock.calls.load(Ordering::SeqCst);
    assert_eq!(
        store
            .apply_node_traffic_report(&pn_node_id, &report)
            .await
            .unwrap(),
        ProxyTrafficReportApplyResult::Duplicate
    );
    assert_eq!(clock.calls.load(Ordering::SeqCst), calls + 1);
    assert_eq!(
        store
            .get_persisted_node_traffic(&traffic_node_id)
            .await
            .unwrap(),
        PersistedTrafficStats {
            tx_bytes: 10,
            rx_bytes: 20,
        }
    );

    clock.set(base_ms + 10_000);
    let calls = clock.calls.load(Ordering::SeqCst);
    assert_eq!(
        store
            .apply_node_traffic_report(&pn_node_id, &report)
            .await
            .unwrap(),
        ProxyTrafficReportApplyResult::Applied,
        "a row at the exact retention cutoff is expired"
    );
    assert_eq!(clock.calls.load(Ordering::SeqCst), calls + 1);

    clock.set(base_ms + 20_001);
    let calls = clock.calls.load(Ordering::SeqCst);
    assert_eq!(
        store
            .apply_node_traffic_report(&pn_node_id, &report)
            .await
            .unwrap(),
        ProxyTrafficReportApplyResult::Applied,
        "a replay beyond the retention horizon is a new application"
    );
    assert_eq!(clock.calls.load(Ordering::SeqCst), calls + 1);
    assert_eq!(
        store
            .get_persisted_node_traffic(&traffic_node_id)
            .await
            .unwrap(),
        PersistedTrafficStats {
            tx_bytes: 30,
            rx_bytes: 60,
        }
    );
}

#[tokio::test]
async fn node_traffic_retention_ttl_cleanup_index_limit_and_continuation() {
    let (factory, pool) = retention_ttl_factory(
        Duration::from_secs(600),
        Duration::from_secs(15),
    )
    .await;
    let mut conn = SqliteConnection::acquire(&pool).await.unwrap();
    let indexes = conn
        .fetch_all(query("PRAGMA index_list('pn_node_traffic_report')"))
        .await
        .unwrap();
    assert!(indexes.iter().any(|row| {
        row.get::<String, _>("name") == "pn_node_traffic_report_applied_at_ms"
    }));
    conn.execute(query(
        r#"WITH digits(n) AS (
                VALUES(0),(1),(2),(3),(4),(5),(6),(7),(8),(9)
            ), seq(n) AS (
                SELECT a.n + 10*b.n + 100*c.n + 1000*d.n
                FROM digits a, digits b, digits c, digits d
                ORDER BY 1 LIMIT 1025
            )
            INSERT INTO pn_node_traffic_report
                (pn_node_id, report_id, started_at_ms, ended_at_ms, applied_at_ms)
            SELECT 'pn-cleanup', 'expired-' || n, 0, 0, 1000 FROM seq"#,
    ))
    .await
    .unwrap();
    conn.execute(query(
        "INSERT INTO pn_node_traffic_report (pn_node_id, report_id, started_at_ms, ended_at_ms, applied_at_ms) VALUES ('pn-cleanup', 'fresh', 0, 0, 2000)",
    ))
    .await
    .unwrap();
    drop(conn);

    assert!(
        factory
            .cleanup_expired_node_traffic_reports(1000, 0)
            .await
            .is_err()
    );
    assert!(
        factory
            .cleanup_expired_node_traffic_reports(
                1000,
                NODE_TRAFFIC_REPORT_CLEANUP_BATCH_SIZE + 1,
            )
            .await
            .is_err()
    );
    assert_eq!(
        factory
            .cleanup_expired_node_traffic_reports(
                1000,
                NODE_TRAFFIC_REPORT_CLEANUP_BATCH_SIZE,
            )
            .await
            .unwrap(),
        1024
    );
    assert_eq!(retention_ttl_report_row_count(&pool).await, 2);
    assert_eq!(
        factory
            .cleanup_expired_node_traffic_reports(
                1000,
                NODE_TRAFFIC_REPORT_CLEANUP_BATCH_SIZE,
            )
            .await
            .unwrap(),
        1
    );
    assert_eq!(retention_ttl_report_row_count(&pool).await, 1);
}

#[tokio::test]
async fn node_traffic_retention_ttl_startup_drain_uses_fixed_cutoff_and_batches() {
    let retention = Duration::from_secs(10);
    let clock = MutableNodeTrafficControlClock::new(20_000);
    let (factory, pool) = retention_ttl_factory_with_clock(
        retention,
        Duration::from_secs(15),
        clock.clone(),
    )
    .await;
    let mut conn = SqliteConnection::acquire(&pool).await.unwrap();
    conn.execute(query(
        r#"WITH digits(n) AS (
                VALUES(0),(1),(2),(3),(4),(5),(6),(7),(8),(9)
            ), seq(n) AS (
                SELECT a.n + 10*b.n + 100*c.n + 1000*d.n
                FROM digits a, digits b, digits c, digits d
                ORDER BY 1 LIMIT 1025
            )
            INSERT INTO pn_node_traffic_report
                (pn_node_id, report_id, started_at_ms, ended_at_ms, applied_at_ms)
            SELECT 'pn-startup', 'startup-' || n, 0, 0, 10000 FROM seq"#,
    ))
    .await
    .unwrap();
    conn.execute(query(
        "INSERT INTO pn_node_traffic_report (pn_node_id, report_id, started_at_ms, ended_at_ms, applied_at_ms) VALUES ('pn-startup', 'fresh', 0, 0, 10001)",
    ))
    .await
    .unwrap();
    drop(conn);

    assert_eq!(
        crate::drain_startup_expired_node_traffic_reports(factory.as_ref())
            .await
            .unwrap(),
        1025
    );
    assert_eq!(clock.calls.load(Ordering::SeqCst), 1);
    assert_eq!(retention_ttl_report_row_count(&pool).await, 1);
    let mut conn = SqliteConnection::acquire(&pool).await.unwrap();
    let remaining: String = conn
        .fetch_one(query(
            "SELECT report_id FROM pn_node_traffic_report LIMIT 1",
        ))
        .await
        .unwrap()
        .get("report_id");
    assert_eq!(remaining, "fresh");
}

#[tokio::test]
async fn node_traffic_retention_ttl_periodic_db_error_recovers_and_shutdown_joins() {
    let clock = MutableNodeTrafficControlClock::new(100_000);
    let (factory, pool) = retention_ttl_factory_with_clock(
        Duration::from_millis(40),
        Duration::from_secs(15),
        clock.clone(),
    )
    .await;
    let mut conn = SqliteConnection::acquire(&pool).await.unwrap();
    conn.execute(query(
        "ALTER TABLE pn_node_traffic_report RENAME TO pn_node_traffic_report_unavailable",
    ))
    .await
    .unwrap();
    drop(conn);

    let service = PnTrafficService::new(factory);
    service.start_node_traffic_cleanup();
    service.start_node_traffic_cleanup();
    tokio::time::timeout(Duration::from_secs(1), async {
        while clock.calls.load(Ordering::SeqCst) < 2 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("cleanup must survive multiple real sqlite failures");

    let mut conn = SqliteConnection::acquire(&pool).await.unwrap();
    conn.execute(query(
        "ALTER TABLE pn_node_traffic_report_unavailable RENAME TO pn_node_traffic_report",
    ))
    .await
    .unwrap();
    conn.execute(query(
        "INSERT INTO pn_node_traffic_report (pn_node_id, report_id, started_at_ms, ended_at_ms, applied_at_ms) VALUES ('pn-periodic', 'expired-after-recovery', 0, 0, 99900)",
    ))
    .await
    .unwrap();
    drop(conn);
    clock.set(100_100);
    tokio::time::timeout(Duration::from_secs(1), async {
        while retention_ttl_report_row_count(&pool).await != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("a later cleanup tick must recover and remove the expired row");
    let status = service.shutdown_node_traffic(Duration::from_secs(1)).await;
    assert!(status.cleanup_exited, "{status:?}");
    assert!(status.is_success(), "{status:?}");
}

#[tokio::test]
async fn node_traffic_retention_ttl_speed_uses_configured_ttl_not_report_window() {
    let configured_ttl = Duration::from_millis(150);
    let (factory, _) =
        retention_ttl_factory(Duration::from_secs(600), configured_ttl).await;
    let pn_node_id = node(92);
    let reports = [
        ("short-window-speed", 93, 10, 11),
        ("normal-window-speed", 94, 10, 5_010),
        ("long-idle-window-speed", 95, 1, 30 * 24 * 60 * 60 * 1000),
        ("rollback-shaped-window-speed", 96, 0, 1),
    ];
    let mut store = factory.get_vpn_store().await.unwrap();
    for (report_id, node_byte, started_at_ms, ended_at_ms) in reports {
        let report = retention_ttl_report(
            report_id,
            node_byte,
            started_at_ms,
            ended_at_ms,
        );
        assert_eq!(
            store
                .apply_node_traffic_report(&pn_node_id, &report)
                .await
                .unwrap(),
            ProxyTrafficReportApplyResult::Applied
        );
        assert_eq!(
            factory.get_node_traffic_speed(&node(node_byte)),
            PersistedTrafficStats {
                tx_bytes: 3,
                rx_bytes: 4,
            },
            "report_id={report_id}"
        );
    }
    tokio::time::sleep(configured_ttl + Duration::from_millis(50)).await;
    for (report_id, node_byte, _, _) in reports {
        assert_eq!(
            factory.get_node_traffic_speed(&node(node_byte)),
            PersistedTrafficStats::default(),
            "report window must not extend configured cache expiry: {report_id}"
        );
    }
}
