#![allow(unused)]

use p2p_frame::cmd_server::server::CmdServer;
use p2p_frame::sn::service::{SnServerRef, SnServiceRef};
use sfo_sql::Row;
use sfo_sql::errors::SqlErrorCode;
use sfo_sql::mysql::sql_query;
use sfo_sql::sqlite::{SqlConnection, SqlPool, SqliteJournalMode};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};
use vpn_frame::cmd_server::errors::CmdResult;
use vpn_frame::cmd_server::{CmdBody, CmdHandler, PeerId, TunnelId};
use vpn_frame::errors::{VpnErrorCode, VpnResult, into_vpn_err, vpn_err};
use vpn_frame::server::{
    JoinedNode, Network, NetworkGroupId, NetworkId, NetworkManager, NetworkMember, NetworkStore,
    Node, NodeId, NodeManager, NodeStore, PnStore, VpnCmdServer, VpnServer, VpnStore,
    VpnStoreFactory, VpnStoreGuard,
};
use vpn_frame::{
    ClientProxyNodeInfo, NodeNetwork, NodeTrafficReport, PnServerInfo, ProxyTrafficReport,
    ProxyTrafficReportApplyResult, ProxyTrafficReportResp, UserRemainingTraffic,
};

pub struct SqliteVpnStore {
    conn: SqlConnection,
    transaction_state: SqliteTransactionState,
    traffic_speed_cache: Arc<Mutex<TrafficSpeedCache>>,
}

const PROXY_TRAFFIC_SPEED_TTL: Duration = Duration::from_secs(15);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct TrafficSpeedKey {
    network_id: NetworkId,
    node_a: NodeId,
    node_b: NodeId,
}

#[derive(Clone, Copy)]
struct TrafficSpeedEntry {
    node_a_to_b: u64,
    node_b_to_a: u64,
    ended_at_ms: u64,
    expires_at: Instant,
}

#[derive(Default)]
struct TrafficSpeedCache {
    pairs: HashMap<TrafficSpeedKey, TrafficSpeedEntry>,
}

impl TrafficSpeedCache {
    fn retain_live(&mut self) {
        let now = Instant::now();
        self.pairs.retain(|_, entry| entry.expires_at > now);
    }

    fn update(&mut self, report: &ProxyTrafficReport) {
        self.retain_live();
        let sample = &report.traffic_sample;
        let forward = sample.source_id.as_slice() <= sample.dest_id.as_slice();
        let (node_a, node_b, node_a_to_b, node_b_to_a) = if forward {
            (
                sample.source_id.clone(),
                sample.dest_id.clone(),
                sample.source_to_dest.speed_bytes_per_sec,
                sample.dest_to_source.speed_bytes_per_sec,
            )
        } else {
            (
                sample.dest_id.clone(),
                sample.source_id.clone(),
                sample.dest_to_source.speed_bytes_per_sec,
                sample.source_to_dest.speed_bytes_per_sec,
            )
        };
        let key = TrafficSpeedKey {
            network_id: sample.network_id,
            node_a,
            node_b,
        };
        if self
            .pairs
            .get(&key)
            .is_some_and(|entry| entry.ended_at_ms > report.ended_at_ms)
        {
            return;
        }
        self.pairs.insert(
            key,
            TrafficSpeedEntry {
                node_a_to_b,
                node_b_to_a,
                ended_at_ms: report.ended_at_ms,
                expires_at: Instant::now() + PROXY_TRAFFIC_SPEED_TTL,
            },
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SqliteTransactionState {
    Idle,
    Active,
    Poisoned,
}

fn node_id_db_key(node_id: &NodeId) -> String {
    node_id.to_base58()
}

fn sqlite_i64(value: u64, field: &'static str) -> VpnResult<i64> {
    i64::try_from(value).map_err(|_| {
        vpn_err!(
            VpnErrorCode::InvalidParam,
            "{} exceeds sqlite signed integer range",
            field
        )
    })
}

fn network_pn_server_from_db(id: String) -> Option<PnServerInfo> {
    if id.is_empty() {
        return None;
    }
    Some(PnServerInfo::new(id, Vec::new()))
}

fn network_client_proxy_from_db(id: String) -> VpnResult<Option<ClientProxyNodeInfo>> {
    if id.is_empty() {
        return Ok(None);
    }
    Ok(Some(ClientProxyNodeInfo {
        proxy_id: NodeId::from_p2p_base36(&id)
            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?,
        name: None,
        endpoints: Vec::new(),
    }))
}

fn network_pn_server_db_id(pn_server: Option<&PnServerInfo>) -> String {
    match pn_server {
        Some(pn_server) => pn_server.id.clone(),
        None => "".to_string(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProxyNodeApprovalStatus {
    Pending,
    Approved,
    Rejected,
}

impl ProxyNodeApprovalStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
        }
    }

    fn from_str(status: &str) -> VpnResult<Self> {
        match status {
            "pending" => Ok(Self::Pending),
            "approved" => Ok(Self::Approved),
            "rejected" => Ok(Self::Rejected),
            _ => Err(vpn_err!(
                VpnErrorCode::InvalidParam,
                "invalid proxy node approval status {}",
                status
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyNodeApproval {
    pub pn_server_id: String,
    pub status: ProxyNodeApprovalStatus,
    pub updated_at: u64,
    pub comment: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PersistedTrafficStats {
    pub tx_bytes: u64,
    pub rx_bytes: u64,
}

impl SqliteVpnStore {
    pub fn new(conn: SqlConnection) -> Self {
        Self {
            conn,
            transaction_state: SqliteTransactionState::Idle,
            traffic_speed_cache: Arc::new(Mutex::new(TrafficSpeedCache::default())),
        }
    }

    fn new_with_traffic_speed_cache(
        conn: SqlConnection,
        traffic_speed_cache: Arc<Mutex<TrafficSpeedCache>>,
    ) -> Self {
        Self {
            conn,
            transaction_state: SqliteTransactionState::Idle,
            traffic_speed_cache,
        }
    }

    async fn finish_transaction<R>(&mut self, operation: VpnResult<R>) -> VpnResult<R> {
        match operation {
            Ok(value) => match self.commit_transaction().await {
                Ok(()) => Ok(value),
                Err(commit_err) => match self.rollback_transaction().await {
                    Ok(()) => Err(commit_err),
                    Err(rollback_err) => Err(vpn_err!(
                        VpnErrorCode::IoError,
                        "sqlite commit failed: {}; rollback failed: {}",
                        commit_err,
                        rollback_err
                    )),
                },
            },
            Err(operation_err) => match self.rollback_transaction().await {
                Ok(()) => Err(operation_err),
                Err(rollback_err) => Err(vpn_err!(
                    VpnErrorCode::IoError,
                    "sqlite transaction operation failed: {}; rollback failed: {}",
                    operation_err,
                    rollback_err
                )),
            },
        }
    }

    pub async fn init_db(&mut self) -> VpnResult<()> {
        let sql = r#"CREATE TABLE IF NOT EXISTS node (
            id varchar(45) PRIMARY KEY,
            info_version integer NOT NULL DEFAULT 0
        )"#;
        self.conn
            .execute_sql(sql_query(sql))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        let sql = r#"CREATE TABLE IF NOT EXISTS pn_node_traffic_report (
            pn_node_id varchar(45) NOT NULL,
            report_id TEXT NOT NULL,
            started_at_ms integer NOT NULL,
            ended_at_ms integer NOT NULL,
            applied_at_ms integer NOT NULL,
            PRIMARY KEY (pn_node_id, report_id)
        )"#;
        self.conn
            .execute_sql(sql_query(sql))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        let sql = r#"CREATE TABLE IF NOT EXISTS network_group (
            id integer PRIMARY KEY
        )"#;
        self.conn
            .execute_sql(sql_query(sql))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        let sql = r#"CREATE TABLE IF NOT EXISTS joined_node (
            group_id integer NOT NULL,
            node_id varchar(45) NOT NULL,
            allow_join BOOLEAN NOT NULL DEFAULT FALSE,
            name TEXT NOT NULL,
            comment TEXT NOT NULL,
            PRIMARY KEY (group_id, node_id)
        )"#;
        self.conn
            .execute_sql(sql_query(sql))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let sql = "CREATE INDEX IF NOT EXISTS joined_node_node_id ON joined_node(node_id)";
        self.conn
            .execute_sql(sql_query(sql))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        let sql = r#"CREATE TABLE IF NOT EXISTS network (
            id integer PRIMARY KEY,
            group_id integer NOT NULL,
            name TEXT NOT NULL,
            ip TEXT NOT NULL,
            mask INTEGER NOT NULL,
            ipv6 TEXT,
            ipv6_mask INTEGER,
            pn_server_id TEXT NOT NULL DEFAULT '',
            FOREIGN KEY (group_id) REFERENCES network_group(id)
        )"#;
        self.conn
            .execute_sql(sql_query(sql))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        self.ensure_network_pn_server_columns().await?;

        let sql = r#"CREATE TABLE IF NOT EXISTS network_member (
            network_id integer NOT NULL,
            node_id varchar(45) NOT NULL,
            ip varchar(15) NOT NULL,
            ipv6 varchar(32) NOT NULL,
            PRIMARY KEY (network_id, node_id),
            FOREIGN KEY (network_id) REFERENCES network(id)
        )"#;
        self.conn
            .execute_sql(sql_query(sql))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let sql = "CREATE INDEX IF NOT EXISTS network_member_node_id ON network_member(node_id)";
        self.conn
            .execute_sql(sql_query(sql))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let sql = "CREATE INDEX IF NOT EXISTS network_member_ip ON network_member(network_id, ip)";
        self.conn
            .execute_sql(sql_query(sql))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let sql =
            "CREATE INDEX IF NOT EXISTS network_member_ipv6 ON network_member(network_id, ipv6)";
        self.conn
            .execute_sql(sql_query(sql))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        let sql = r#"CREATE TABLE IF NOT EXISTS pn_node_traffic_stat (
            node_id varchar(45) PRIMARY KEY,
            tx_bytes integer NOT NULL DEFAULT 0,
            rx_bytes integer NOT NULL DEFAULT 0
        )"#;
        self.conn
            .execute_sql(sql_query(sql))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        let sql = r#"CREATE TABLE IF NOT EXISTS pn_group_traffic_stat (
            group_id integer PRIMARY KEY,
            tx_bytes integer NOT NULL DEFAULT 0,
            rx_bytes integer NOT NULL DEFAULT 0
        )"#;
        self.conn
            .execute_sql(sql_query(sql))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        let sql = r#"CREATE TABLE IF NOT EXISTS user (
            id varchar(64) PRIMARY KEY,
            network_id integer,
            server_id TEXT
        )"#;
        self.conn
            .execute_sql(sql_query(sql))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        let sql = r#"CREATE TABLE IF NOT EXISTS pn_proxy_traffic_stat (
            network_id integer NOT NULL,
            source_id varchar(45) NOT NULL,
            dest_id varchar(45) NOT NULL,
            source_to_dest_bytes integer NOT NULL DEFAULT 0,
            dest_to_source_bytes integer NOT NULL DEFAULT 0,
            PRIMARY KEY (network_id, source_id, dest_id)
        )"#;
        self.conn
            .execute_sql(sql_query(sql))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        let sql = r#"CREATE TABLE IF NOT EXISTS pn_proxy_traffic_report (
            pn_node_id varchar(45) NOT NULL,
            report_id TEXT NOT NULL,
            started_at_ms integer NOT NULL,
            ended_at_ms integer NOT NULL,
            applied_at_ms integer NOT NULL,
            PRIMARY KEY (pn_node_id, report_id)
        )"#;
        self.conn
            .execute_sql(sql_query(sql))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        let sql = r#"CREATE TABLE IF NOT EXISTS pn_proxy_node (
            pn_server_id TEXT PRIMARY KEY,
            status TEXT NOT NULL,
            updated_at integer NOT NULL DEFAULT 0,
            comment TEXT NOT NULL DEFAULT ''
        )"#;
        self.conn
            .execute_sql(sql_query(sql))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        self.ensure_proxy_node_pn_server_columns().await?;
        let sql = "CREATE INDEX IF NOT EXISTS pn_proxy_node_status ON pn_proxy_node(status)";
        self.conn
            .execute_sql(sql_query(sql))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        Ok(())
    }

    async fn ensure_network_pn_server_columns(&mut self) -> VpnResult<()> {
        let rows = self
            .conn
            .query_all(sql_query("PRAGMA table_info(network)"))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let columns: Vec<String> = rows.iter().map(|row| row.get("name")).collect();

        if !columns.iter().any(|existing| existing == "pn_server_id") {
            self.conn
                .execute_sql(sql_query(
                    "ALTER TABLE network ADD COLUMN pn_server_id TEXT NOT NULL DEFAULT ''",
                ))
                .await
                .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        }

        for column in [
            "pn_server_name",
            "pn_server_ip",
            "pn_server_port",
            "pn_server_addresses",
        ] {
            if columns.iter().any(|existing| existing == column) {
                let sql = format!("ALTER TABLE network DROP COLUMN {}", column);
                self.conn
                    .execute_sql(sql_query(&sql))
                    .await
                    .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
            }
        }

        Ok(())
    }

    async fn ensure_proxy_node_pn_server_columns(&mut self) -> VpnResult<()> {
        let rows = self
            .conn
            .query_all(sql_query("PRAGMA table_info(pn_proxy_node)"))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let columns: Vec<String> = rows.iter().map(|row| row.get("name")).collect();

        for column in [
            "pn_server_name",
            "pn_server_ip",
            "pn_server_port",
            "pn_server_addresses",
        ] {
            if columns.iter().any(|existing| existing == column) {
                let sql = format!("ALTER TABLE pn_proxy_node DROP COLUMN {}", column);
                self.conn
                    .execute_sql(sql_query(&sql))
                    .await
                    .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
            }
        }

        Ok(())
    }

    fn now_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or_default()
    }

    pub async fn list_all_joined_node_ids(&mut self) -> VpnResult<Vec<NodeId>> {
        let sql = r#"SELECT DISTINCT node_id FROM joined_node"#;
        let rows = self
            .conn
            .query_all(sql_query(sql))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let mut node_ids = Vec::new();
        for row in rows {
            let node_id: String = row.get("node_id");
            node_ids.push(
                NodeId::from_base36_or_base58(&node_id)
                    .map_err(into_vpn_err!(VpnErrorCode::IoError))?,
            );
        }
        Ok(node_ids)
    }

    pub async fn get_persisted_node_traffic(
        &mut self,
        node_id: &NodeId,
    ) -> VpnResult<PersistedTrafficStats> {
        let sql = r#"SELECT tx_bytes, rx_bytes FROM pn_node_traffic_stat WHERE node_id = ?"#;
        match self
            .conn
            .query_one(sql_query(sql).bind(node_id_db_key(node_id)))
            .await
        {
            Ok(row) => Ok(PersistedTrafficStats {
                tx_bytes: row.get::<i64, _>("tx_bytes") as u64,
                rx_bytes: row.get::<i64, _>("rx_bytes") as u64,
            }),
            Err(err) => {
                if err.code() == SqlErrorCode::NotFound {
                    Ok(PersistedTrafficStats::default())
                } else {
                    Err(vpn_err!(
                        VpnErrorCode::IoError,
                        "query node traffic {} failed",
                        node_id_db_key(node_id)
                    ))
                }
            }
        }
    }

    pub async fn add_persisted_node_traffic(
        &mut self,
        node_id: &NodeId,
        stats: PersistedTrafficStats,
    ) -> VpnResult<()> {
        let tx_bytes = sqlite_i64(stats.tx_bytes, "node tx bytes")?;
        let rx_bytes = sqlite_i64(stats.rx_bytes, "node rx bytes")?;
        let sql = r#"INSERT INTO pn_node_traffic_stat (node_id, tx_bytes, rx_bytes)
            VALUES (?, ?, ?)
            ON CONFLICT(node_id) DO UPDATE SET
                tx_bytes = tx_bytes + excluded.tx_bytes,
                rx_bytes = rx_bytes + excluded.rx_bytes
            WHERE tx_bytes <= ? - excluded.tx_bytes
              AND rx_bytes <= ? - excluded.rx_bytes"#;
        let result = self.conn
            .execute_sql(
                sql_query(sql)
                    .bind(node_id_db_key(node_id))
                    .bind(tx_bytes)
                    .bind(rx_bytes)
                    .bind(i64::MAX)
                    .bind(i64::MAX),
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        if result.rows_affected() != 1 {
            return Err(vpn_err!(
                VpnErrorCode::InvalidParam,
                "node traffic cumulative total exceeds sqlite signed integer range"
            ));
        }
        Ok(())
    }

    pub async fn get_persisted_group_traffic(
        &mut self,
        group_id: &NetworkGroupId,
    ) -> VpnResult<PersistedTrafficStats> {
        let sql = r#"SELECT tx_bytes, rx_bytes FROM pn_group_traffic_stat WHERE group_id = ?"#;
        match self
            .conn
            .query_one(sql_query(sql).bind(*group_id as i64))
            .await
        {
            Ok(row) => Ok(PersistedTrafficStats {
                tx_bytes: row.get::<i64, _>("tx_bytes") as u64,
                rx_bytes: row.get::<i64, _>("rx_bytes") as u64,
            }),
            Err(err) => {
                if err.code() == SqlErrorCode::NotFound {
                    Ok(PersistedTrafficStats::default())
                } else {
                    Err(vpn_err!(
                        VpnErrorCode::IoError,
                        "query group traffic {} failed",
                        group_id
                    ))
                }
            }
        }
    }

    pub async fn add_persisted_group_traffic(
        &mut self,
        group_id: &NetworkGroupId,
        stats: PersistedTrafficStats,
    ) -> VpnResult<()> {
        let tx_bytes = sqlite_i64(stats.tx_bytes, "group tx bytes")?;
        let rx_bytes = sqlite_i64(stats.rx_bytes, "group rx bytes")?;
        let sql = r#"INSERT INTO pn_group_traffic_stat (group_id, tx_bytes, rx_bytes)
            VALUES (?, ?, ?)
            ON CONFLICT(group_id) DO UPDATE SET
                tx_bytes = tx_bytes + excluded.tx_bytes,
                rx_bytes = rx_bytes + excluded.rx_bytes
            WHERE tx_bytes <= ? - excluded.tx_bytes
              AND rx_bytes <= ? - excluded.rx_bytes"#;
        let result = self.conn
            .execute_sql(
                sql_query(sql)
                    .bind(*group_id as i64)
                    .bind(tx_bytes)
                    .bind(rx_bytes)
                    .bind(i64::MAX)
                    .bind(i64::MAX),
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        if result.rows_affected() != 1 {
            return Err(vpn_err!(
                VpnErrorCode::InvalidParam,
                "group traffic cumulative total exceeds sqlite signed integer range"
            ));
        }
        Ok(())
    }

    pub async fn ensure_proxy_node_pending(&mut self, pn_server: &PnServerInfo) -> VpnResult<()> {
        let sql = r#"INSERT INTO pn_proxy_node (pn_server_id, status, updated_at, comment)
            VALUES (?, ?, ?, '')
            ON CONFLICT(pn_server_id) DO UPDATE SET
                updated_at = excluded.updated_at"#;
        self.conn
            .execute_sql(
                sql_query(sql)
                    .bind(&pn_server.id)
                    .bind(ProxyNodeApprovalStatus::Pending.as_str())
                    .bind(Self::now_secs() as i64),
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    pub async fn set_proxy_node_approval(
        &mut self,
        pn_server: &PnServerInfo,
        status: ProxyNodeApprovalStatus,
        comment: Option<&str>,
    ) -> VpnResult<()> {
        let sql = r#"INSERT INTO pn_proxy_node (pn_server_id, status, updated_at, comment)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(pn_server_id) DO UPDATE SET
                status = excluded.status,
                updated_at = excluded.updated_at,
                comment = excluded.comment"#;
        self.conn
            .execute_sql(
                sql_query(sql)
                    .bind(&pn_server.id)
                    .bind(status.as_str())
                    .bind(Self::now_secs() as i64)
                    .bind(comment.unwrap_or("")),
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    pub async fn is_proxy_node_approved(&mut self, pn_server: &PnServerInfo) -> VpnResult<bool> {
        let sql = r#"SELECT status FROM pn_proxy_node WHERE pn_server_id = ?"#;
        match self
            .conn
            .query_one(sql_query(sql).bind(&pn_server.id))
            .await
        {
            Ok(row) => {
                let status: String = row.get("status");
                Ok(
                    ProxyNodeApprovalStatus::from_str(&status)?
                        == ProxyNodeApprovalStatus::Approved,
                )
            }
            Err(err) => {
                if err.code() == SqlErrorCode::NotFound {
                    Ok(false)
                } else {
                    Err(vpn_err!(
                        VpnErrorCode::IoError,
                        "query proxy node approval {} failed",
                        pn_server.id
                    ))
                }
            }
        }
    }

    pub async fn list_proxy_node_approvals(&mut self) -> VpnResult<Vec<ProxyNodeApproval>> {
        let sql = r#"SELECT pn_server_id, status, updated_at, comment FROM pn_proxy_node ORDER BY pn_server_id"#;
        let rows = self
            .conn
            .query_all(sql_query(sql))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let mut approvals = Vec::new();
        for row in rows {
            let status: String = row.get("status");
            let pn_server_id: String = row.get("pn_server_id");
            if pn_server_id.is_empty() {
                return Err(vpn_err!(VpnErrorCode::InvalidParam, "empty proxy node id"));
            }
            approvals.push(ProxyNodeApproval {
                pn_server_id,
                status: ProxyNodeApprovalStatus::from_str(&status)?,
                updated_at: row.get::<i64, _>("updated_at") as u64,
                comment: row.get("comment"),
            });
        }
        Ok(approvals)
    }

    async fn get_network_group_id(&mut self, network_id: &NetworkId) -> VpnResult<NetworkGroupId> {
        let sql = r#"SELECT group_id FROM network WHERE id = ?"#;
        match self
            .conn
            .query_one(sql_query(sql).bind(*network_id as i64))
            .await
        {
            Ok(row) => Ok(row.get::<i64, _>("group_id") as NetworkGroupId),
            Err(err) => {
                if err.code() == SqlErrorCode::NotFound {
                    Err(vpn_err!(
                        VpnErrorCode::InvalidParam,
                        "network {} does not exist",
                        network_id
                    ))
                } else {
                    Err(vpn_err!(
                        VpnErrorCode::IoError,
                        "query network {} group failed",
                        network_id
                    ))
                }
            }
        }
    }

    async fn get_user_id_for_group(&mut self, group_id: &NetworkGroupId) -> VpnResult<String> {
        let sql = r#"SELECT id FROM user WHERE network_id = ? ORDER BY id LIMIT 1"#;
        match self
            .conn
            .query_one(sql_query(sql).bind(*group_id as i64))
            .await
        {
            Ok(row) => Ok(row.get("id")),
            Err(err) => {
                if err.code() == SqlErrorCode::NotFound {
                    Ok(group_id.to_string())
                } else {
                    Err(vpn_err!(
                        VpnErrorCode::IoError,
                        "query user for network group {} failed",
                        group_id
                    ))
                }
            }
        }
    }

    async fn remaining_traffic_for_report(
        &mut self,
        report: &ProxyTrafficReport,
    ) -> VpnResult<Vec<UserRemainingTraffic>> {
        let mut users = BTreeMap::new();
        for sample in std::slice::from_ref(&report.traffic_sample) {
            let group_id = self.get_network_group_id(&sample.network_id).await?;
            let user_id = self.get_user_id_for_group(&group_id).await?;
            users.entry(user_id).or_insert(UserRemainingTraffic {
                user_id: group_id.to_string(),
                remaining_bytes: None,
            });
        }

        Ok(users
            .into_keys()
            .map(|user_id| UserRemainingTraffic {
                user_id,
                remaining_bytes: None,
            })
            .collect())
    }

    async fn has_proxy_traffic_report(
        &mut self,
        pn_node_id: &NodeId,
        report_id: &str,
    ) -> VpnResult<bool> {
        let sql = r#"SELECT report_id FROM pn_proxy_traffic_report WHERE pn_node_id = ? AND report_id = ?"#;
        match self
            .conn
            .query_one(
                sql_query(sql)
                    .bind(node_id_db_key(pn_node_id))
                    .bind(report_id),
            )
            .await
        {
            Ok(_) => Ok(true),
            Err(err) => {
                if err.code() == SqlErrorCode::NotFound {
                    Ok(false)
                } else {
                    Err(vpn_err!(
                        VpnErrorCode::IoError,
                        "query proxy traffic report {} failed",
                        report_id
                    ))
                }
            }
        }
    }

    async fn insert_proxy_traffic_report(
        &mut self,
        pn_node_id: &NodeId,
        report: &ProxyTrafficReport,
    ) -> VpnResult<()> {
        let started_at_ms = sqlite_i64(report.started_at_ms, "proxy report start timestamp")?;
        let ended_at_ms = sqlite_i64(report.ended_at_ms, "proxy report end timestamp")?;
        let applied_at_ms = sqlite_i64(
            Self::now_secs().checked_mul(1000).ok_or_else(|| {
                vpn_err!(VpnErrorCode::InvalidParam, "proxy report applied timestamp overflow")
            })?,
            "proxy report applied timestamp",
        )?;
        let sql = r#"INSERT INTO pn_proxy_traffic_report
            (pn_node_id, report_id, started_at_ms, ended_at_ms, applied_at_ms)
            VALUES (?, ?, ?, ?, ?)"#;
        self.conn
            .execute_sql(
                sql_query(sql)
                    .bind(node_id_db_key(pn_node_id))
                    .bind(report.report_id.0.as_str())
                    .bind(started_at_ms)
                    .bind(ended_at_ms)
                    .bind(applied_at_ms),
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    async fn add_proxy_traffic_sample(
        &mut self,
        sample: &vpn_frame::PnTrafficSample,
    ) -> VpnResult<()> {
        let network_id = sample.network_id as i64;
        let source_to_dest = sqlite_i64(sample.source_to_dest.bytes, "proxy source-to-dest bytes")?;
        let dest_to_source = sqlite_i64(sample.dest_to_source.bytes, "proxy dest-to-source bytes")?;
        let sql = r#"INSERT INTO pn_proxy_traffic_stat
            (network_id, source_id, dest_id, source_to_dest_bytes, dest_to_source_bytes)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(network_id, source_id, dest_id) DO UPDATE SET
                source_to_dest_bytes = source_to_dest_bytes + excluded.source_to_dest_bytes,
                dest_to_source_bytes = dest_to_source_bytes + excluded.dest_to_source_bytes
            WHERE source_to_dest_bytes <= ? - excluded.source_to_dest_bytes
              AND dest_to_source_bytes <= ? - excluded.dest_to_source_bytes"#;
        let result = self.conn
            .execute_sql(
                sql_query(sql)
                    .bind(network_id)
                    .bind(node_id_db_key(&sample.source_id))
                    .bind(node_id_db_key(&sample.dest_id))
                    .bind(source_to_dest)
                    .bind(dest_to_source)
                    .bind(i64::MAX)
                    .bind(i64::MAX),
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        if result.rows_affected() != 1 {
            return Err(vpn_err!(
                VpnErrorCode::InvalidParam,
                "proxy traffic cumulative total exceeds sqlite signed integer range"
            ));
        }
        Ok(())
    }

    async fn add_proxy_traffic_to_legacy_stats(
        &mut self,
        sample: &vpn_frame::PnTrafficSample,
    ) -> VpnResult<()> {
        let group_id = self.get_network_group_id(&sample.network_id).await?;
        self.add_persisted_node_traffic(
            &sample.source_id,
            PersistedTrafficStats {
                tx_bytes: sample.source_to_dest.bytes,
                rx_bytes: sample.dest_to_source.bytes,
            },
        )
        .await?;
        self.add_persisted_node_traffic(
            &sample.dest_id,
            PersistedTrafficStats {
                tx_bytes: sample.dest_to_source.bytes,
                rx_bytes: sample.source_to_dest.bytes,
            },
        )
        .await?;
        let total = sample
            .source_to_dest
            .bytes
            .checked_add(sample.dest_to_source.bytes)
            .ok_or_else(|| {
                vpn_err!(
                    VpnErrorCode::InvalidParam,
                    "derived group traffic bytes overflow"
                )
            })?;
        self.add_persisted_group_traffic(
            &group_id,
            PersistedTrafficStats {
                tx_bytes: total,
                rx_bytes: total,
            },
        )
        .await
    }
}

#[async_trait::async_trait]
impl VpnStore for SqliteVpnStore {
    async fn begin_transaction(&mut self) -> VpnResult<()> {
        if self.transaction_state != SqliteTransactionState::Idle {
            return Err(vpn_err!(
                VpnErrorCode::IoError,
                "cannot begin sqlite transaction while state is {:?}",
                self.transaction_state
            ));
        }
        self.conn
            .begin_transaction()
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        self.transaction_state = SqliteTransactionState::Active;
        Ok(())
    }

    async fn commit_transaction(&mut self) -> VpnResult<()> {
        if self.transaction_state != SqliteTransactionState::Active {
            return Err(vpn_err!(
                VpnErrorCode::IoError,
                "cannot commit sqlite transaction while state is {:?}",
                self.transaction_state
            ));
        }
        match self
            .conn
            .commit_transaction()
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))
        {
            Ok(()) => {
                self.transaction_state = SqliteTransactionState::Idle;
                Ok(())
            }
            Err(err) => {
                // sfo-sql consumes the transaction handle before SQLite reports
                // the commit result. A failed commit therefore cannot be safely
                // rolled back through this connection.
                self.transaction_state = SqliteTransactionState::Poisoned;
                self.conn.close_on_drop();
                Err(err)
            }
        }
    }

    async fn rollback_transaction(&mut self) -> VpnResult<()> {
        if self.transaction_state != SqliteTransactionState::Active {
            return Err(vpn_err!(
                VpnErrorCode::IoError,
                "cannot rollback sqlite transaction while state is {:?}",
                self.transaction_state
            ));
        }
        match self
            .conn
            .rollback_transaction()
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))
        {
            Ok(()) => {
                self.transaction_state = SqliteTransactionState::Idle;
                Ok(())
            }
            Err(err) => {
                self.transaction_state = SqliteTransactionState::Poisoned;
                self.conn.close_on_drop();
                Err(err)
            }
        }
    }
}

#[async_trait::async_trait]
impl PnStore for SqliteVpnStore {
    async fn apply_node_traffic_report(
        &mut self,
        pn_node_id: &NodeId,
        report: &NodeTrafficReport,
    ) -> VpnResult<ProxyTrafficReportApplyResult> {
        if report.started_at_ms > report.ended_at_ms {
            return Err(vpn_err!(
                VpnErrorCode::InvalidParam,
                "node traffic record {} has an invalid range",
                report.report_id.0
            ));
        }
        let started_at_ms = sqlite_i64(report.started_at_ms, "node report start timestamp")?;
        let ended_at_ms = sqlite_i64(report.ended_at_ms, "node report end timestamp")?;
        let applied_at_ms = sqlite_i64(
            Self::now_secs().checked_mul(1000).ok_or_else(|| {
                vpn_err!(VpnErrorCode::InvalidParam, "node report applied timestamp overflow")
            })?,
            "node report applied timestamp",
        )?;

        self.begin_transaction().await?;
        let result: VpnResult<ProxyTrafficReportApplyResult> = async {
            let duplicate_sql = r#"SELECT report_id FROM pn_node_traffic_report
                WHERE pn_node_id = ? AND report_id = ?"#;
            match self
                .conn
                .query_one(
                    sql_query(duplicate_sql)
                        .bind(node_id_db_key(pn_node_id))
                        .bind(report.report_id.0.as_str()),
                )
                .await
            {
                Ok(_) => return Ok(ProxyTrafficReportApplyResult::Duplicate),
                Err(err) if err.code() == SqlErrorCode::NotFound => {}
                Err(_) => {
                    return Err(vpn_err!(
                        VpnErrorCode::IoError,
                        "query node traffic report {} failed",
                        report.report_id.0
                    ));
                }
            }

            let insert_sql = r#"INSERT INTO pn_node_traffic_report
                (pn_node_id, report_id, started_at_ms, ended_at_ms, applied_at_ms)
                VALUES (?, ?, ?, ?, ?)"#;
            self.conn
                .execute_sql(
                    sql_query(insert_sql)
                        .bind(node_id_db_key(pn_node_id))
                        .bind(report.report_id.0.as_str())
                        .bind(started_at_ms)
                        .bind(ended_at_ms)
                        .bind(applied_at_ms),
                )
                .await
                .map_err(into_vpn_err!(VpnErrorCode::IoError))?;

            let delta = &report.delta;
            let persisted = PersistedTrafficStats {
                tx_bytes: delta.tx_bytes,
                rx_bytes: delta.rx_bytes,
            };
            self.add_persisted_node_traffic(&delta.node_id, persisted)
                .await?;
            for joined in self.get_joined_network_group(&delta.node_id).await? {
                self.add_persisted_group_traffic(&joined.group_id, persisted)
                    .await?;
            }
            Ok(ProxyTrafficReportApplyResult::Applied)
        }
        .await;

        self.finish_transaction(result).await
    }

    async fn add_pn_traffic_delta(
        &mut self,
        node_id: &NodeId,
        tx_bytes: u64,
        rx_bytes: u64,
    ) -> VpnResult<()> {
        self.begin_transaction().await?;
        let result: VpnResult<()> = async {
            self.add_persisted_node_traffic(node_id, PersistedTrafficStats { tx_bytes, rx_bytes })
                .await?;
            let groups = self.get_joined_network_group(node_id).await?;
            for joined in groups.iter() {
                self.add_persisted_group_traffic(
                    &joined.group_id,
                    PersistedTrafficStats { tx_bytes, rx_bytes },
                )
                .await?;
            }
            Ok(())
        }
        .await;
        self.finish_transaction(result).await
    }

    async fn ensure_proxy_node_pending(&mut self, pn_server: &PnServerInfo) -> VpnResult<()> {
        SqliteVpnStore::ensure_proxy_node_pending(self, pn_server).await
    }

    async fn set_proxy_node_approval(
        &mut self,
        pn_server: &PnServerInfo,
        status: vpn_frame::ProxyNodeApprovalStatus,
        comment: Option<&str>,
    ) -> VpnResult<()> {
        let status = match status {
            vpn_frame::ProxyNodeApprovalStatus::Pending => ProxyNodeApprovalStatus::Pending,
            vpn_frame::ProxyNodeApprovalStatus::Approved => ProxyNodeApprovalStatus::Approved,
            vpn_frame::ProxyNodeApprovalStatus::Rejected => ProxyNodeApprovalStatus::Rejected,
        };
        SqliteVpnStore::set_proxy_node_approval(self, pn_server, status, comment).await
    }

    async fn is_proxy_node_approved(&mut self, pn_server: &PnServerInfo) -> VpnResult<bool> {
        SqliteVpnStore::is_proxy_node_approved(self, pn_server).await
    }

    async fn list_proxy_node_approvals(&mut self) -> VpnResult<Vec<vpn_frame::ProxyNodeApproval>> {
        let approvals = SqliteVpnStore::list_proxy_node_approvals(self).await?;
        let mut result = Vec::with_capacity(approvals.len());
        for approval in approvals {
            result.push(vpn_frame::ProxyNodeApproval {
                pn_node_id: NodeId::from_p2p_base36(&approval.pn_server_id)
                    .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?,
                status: match approval.status {
                    ProxyNodeApprovalStatus::Pending => {
                        vpn_frame::ProxyNodeApprovalStatus::Pending
                    }
                    ProxyNodeApprovalStatus::Approved => {
                        vpn_frame::ProxyNodeApprovalStatus::Approved
                    }
                    ProxyNodeApprovalStatus::Rejected => {
                        vpn_frame::ProxyNodeApprovalStatus::Rejected
                    }
                },
                updated_at: approval.updated_at,
                comment: approval.comment,
            });
        }
        Ok(result)
    }

    async fn apply_proxy_traffic_report(
        &mut self,
        pn_node_id: &NodeId,
        report: &ProxyTrafficReport,
    ) -> VpnResult<ProxyTrafficReportResp> {
        if report.started_at_ms > report.ended_at_ms {
            return Err(vpn_err!(
                VpnErrorCode::InvalidParam,
                "proxy traffic report {} has invalid time range",
                report.report_id.0
            ));
        }

        let remaining = self.remaining_traffic_for_report(report).await?;
        self.begin_transaction().await?;
        let result: VpnResult<ProxyTrafficReportApplyResult> = async {
            if self
                .has_proxy_traffic_report(pn_node_id, report.report_id.0.as_str())
                .await?
            {
                return Ok(ProxyTrafficReportApplyResult::Duplicate);
            }

            self.insert_proxy_traffic_report(pn_node_id, report).await?;
            let sample = &report.traffic_sample;
            self.add_proxy_traffic_sample(sample).await?;
            self.add_proxy_traffic_to_legacy_stats(sample).await?;
            Ok(ProxyTrafficReportApplyResult::Applied)
        }
        .await;
        let result = self.finish_transaction(result).await?;
        if result == ProxyTrafficReportApplyResult::Applied {
            self.traffic_speed_cache.lock().unwrap().update(report);
        }
        Ok(ProxyTrafficReportResp {
                    report_id: report.report_id.clone(),
                    result,
                    error_code: None,
                    remaining,
                })
    }

    async fn get_proxy_traffic_total(
        &mut self,
        network_id: &NetworkId,
        source_id: &NodeId,
        dest_id: &NodeId,
    ) -> VpnResult<vpn_frame::PersistedTrafficStats> {
        let sql = r#"SELECT source_to_dest_bytes, dest_to_source_bytes
            FROM pn_proxy_traffic_stat
            WHERE network_id = ? AND source_id = ? AND dest_id = ?"#;
        match self
            .conn
            .query_one(
                sql_query(sql)
                    .bind(*network_id as i64)
                    .bind(node_id_db_key(source_id))
                    .bind(node_id_db_key(dest_id)),
            )
            .await
        {
            Ok(row) => Ok(vpn_frame::PersistedTrafficStats {
                network_id: *network_id,
                source_id: source_id.clone(),
                dest_id: dest_id.clone(),
                tx_bytes: row.get::<i64, _>("source_to_dest_bytes") as u64,
                rx_bytes: row.get::<i64, _>("dest_to_source_bytes") as u64,
            }),
            Err(err) => {
                if err.code() == SqlErrorCode::NotFound {
                    Ok(vpn_frame::PersistedTrafficStats {
                        network_id: *network_id,
                        source_id: source_id.clone(),
                        dest_id: dest_id.clone(),
                        tx_bytes: 0,
                        rx_bytes: 0,
                    })
                } else {
                    Err(vpn_err!(
                        VpnErrorCode::IoError,
                        "query proxy traffic total failed network={} source={} dest={}",
                        network_id,
                        node_id_db_key(source_id),
                        node_id_db_key(dest_id)
                    ))
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl NodeStore for SqliteVpnStore {
    async fn add_node(&mut self, node: &Node) -> VpnResult<()> {
        let sql = r#"INSERT INTO node (id) VALUES (?)"#;
        self.conn
            .execute_sql(sql_query(sql).bind(node_id_db_key(&node.id)))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    async fn remove_node(&mut self, id: &NodeId) -> VpnResult<()> {
        let sql = r#"DELETE FROM node WHERE id = ?"#;
        self.conn
            .execute_sql(sql_query(sql).bind(node_id_db_key(id)))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    async fn get_node(&mut self, id: &NodeId) -> VpnResult<Option<Node>> {
        let sql = r#"SELECT id, info_version FROM node WHERE id = ?"#;
        match self
            .conn
            .query_one(sql_query(sql).bind(node_id_db_key(id)))
            .await
        {
            Ok(row) => {
                let id: String = row.get("id");
                let info_version: i64 = row.get("info_version");
                Ok(Some(Node {
                    id: NodeId::from_base36_or_base58(&id)
                        .map_err(into_vpn_err!(VpnErrorCode::IoError))?,
                    info_version: info_version as u16,
                }))
            }
            Err(e) => {
                if e.code() == SqlErrorCode::NotFound {
                    Ok(None)
                } else {
                    Err(vpn_err!(
                        VpnErrorCode::IoError,
                        "query node {} failed",
                        node_id_db_key(id)
                    ))
                }
            }
        }
    }

    async fn exist_node(&mut self, id: &NodeId) -> VpnResult<bool> {
        let sql = r#"SELECT id FROM node WHERE id = ?"#;
        match self
            .conn
            .query_one(sql_query(sql).bind(node_id_db_key(id)))
            .await
        {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.code() == SqlErrorCode::NotFound {
                    Ok(false)
                } else {
                    Err(vpn_err!(
                        VpnErrorCode::IoError,
                        "query node {} failed",
                        node_id_db_key(id)
                    ))
                }
            }
        }
    }

    async fn inc_info_version(&mut self, id: &NodeId) -> VpnResult<()> {
        let sql = r#"UPDATE node SET info_version = info_version + 1 WHERE id = ?"#;
        self.conn
            .execute_sql(sql_query(sql).bind(node_id_db_key(id)))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl NetworkStore for SqliteVpnStore {
    async fn add_network_group(&mut self, group_id: &NetworkGroupId) -> VpnResult<()> {
        let sql = r#"INSERT INTO network_group (id) VALUES (?)"#;
        self.conn
            .execute_sql(sql_query(sql).bind(*group_id as i64))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    async fn exist_network_group(&mut self, group_id: &NetworkGroupId) -> VpnResult<bool> {
        let sql = r#"SELECT id FROM network_group WHERE id = ?"#;
        match self
            .conn
            .query_one(sql_query(sql).bind(*group_id as i64))
            .await
        {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.code() == SqlErrorCode::NotFound {
                    Ok(false)
                } else {
                    Err(vpn_err!(
                        VpnErrorCode::IoError,
                        "query network group {} failed",
                        group_id
                    ))
                }
            }
        }
    }

    async fn has_joined(&mut self, group_id: &NetworkGroupId, node_id: &NodeId) -> VpnResult<bool> {
        let sql = r#"SELECT group_id FROM joined_node WHERE group_id = ? AND node_id = ?"#;
        match self
            .conn
            .query_one(
                sql_query(sql)
                    .bind(*group_id as i64)
                    .bind(node_id_db_key(node_id)),
            )
            .await
        {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.code() == SqlErrorCode::NotFound {
                    Ok(false)
                } else {
                    Err(vpn_err!(
                        VpnErrorCode::IoError,
                        "query joined node {} failed",
                        node_id_db_key(node_id)
                    ))
                }
            }
        }
    }

    async fn add_joined_node(&mut self, node: &JoinedNode) -> VpnResult<()> {
        let sql = r#"INSERT INTO joined_node (group_id, node_id, allow_join, name, comment) VALUES (?, ?, ?, ?, ?)"#;
        self.conn
            .execute_sql(
                sql_query(sql)
                    .bind(node.group_id as i64)
                    .bind(node_id_db_key(&node.node_id))
                    .bind(node.allow_join)
                    .bind(node.name.as_str())
                    .bind(node.comment.as_str()),
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    async fn del_joined_node(
        &mut self,
        group_id: &NetworkGroupId,
        node_id: &NodeId,
    ) -> VpnResult<()> {
        let sql = r#"DELETE FROM joined_node WHERE group_id = ? AND node_id = ?"#;
        self.conn
            .execute_sql(
                sql_query(sql)
                    .bind(*group_id as i64)
                    .bind(node_id_db_key(node_id)),
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    async fn get_joined_node(
        &mut self,
        group_id: &NetworkGroupId,
        node_id: &NodeId,
    ) -> VpnResult<Option<JoinedNode>> {
        let sql = r#"SELECT group_id, node_id, allow_join, name, comment FROM joined_node WHERE group_id = ? AND node_id = ?"#;
        match self
            .conn
            .query_one(
                sql_query(sql)
                    .bind(*group_id as i64)
                    .bind(node_id_db_key(node_id)),
            )
            .await
        {
            Ok(row) => {
                let group_id: i64 = row.get("group_id");
                let node_id: String = row.get("node_id");
                let allow_join: bool = row.get("allow_join");
                let name: String = row.get("name");
                let comment: String = row.get("comment");
                Ok(Some(JoinedNode {
                    group_id: group_id as NetworkGroupId,
                    node_id: NodeId::from_base36_or_base58(&node_id)
                        .map_err(into_vpn_err!(VpnErrorCode::IoError))?,
                    allow_join,
                    name,
                    comment,
                }))
            }
            Err(e) => {
                if e.code() == SqlErrorCode::NotFound {
                    Ok(None)
                } else {
                    Err(vpn_err!(
                        VpnErrorCode::IoError,
                        "query joined node {} failed",
                        node_id_db_key(node_id)
                    ))
                }
            }
        }
    }

    async fn get_joined_nodes(&mut self, group_id: &NetworkGroupId) -> VpnResult<Vec<JoinedNode>> {
        let sql = r#"SELECT group_id, node_id, allow_join, name, comment FROM joined_node WHERE group_id = ?"#;
        let rows = self
            .conn
            .query_all(sql_query(sql).bind(*group_id as i64))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let mut nodes = Vec::new();
        for row in rows {
            let group_id: i64 = row.get("group_id");
            let node_id: String = row.get("node_id");
            let allow_join: bool = row.get("allow_join");
            let name: String = row.get("name");
            let comment: String = row.get("comment");
            nodes.push(JoinedNode {
                group_id: group_id as NetworkGroupId,
                node_id: NodeId::from_base36_or_base58(&node_id)
                    .map_err(into_vpn_err!(VpnErrorCode::IoError))?,
                allow_join,
                name,
                comment,
            });
        }
        Ok(nodes)
    }

    async fn update_joined_node(&mut self, node: &JoinedNode) -> VpnResult<()> {
        let sql = r#"UPDATE joined_node SET allow_join = ?, name = ?, comment = ? WHERE group_id = ? AND node_id = ?"#;
        self.conn
            .execute_sql(
                sql_query(sql)
                    .bind(node.allow_join)
                    .bind(node.name.as_str())
                    .bind(node.comment.as_str())
                    .bind(node.group_id as i64)
                    .bind(node_id_db_key(&node.node_id)),
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    async fn get_joined_network_group(&mut self, node_id: &NodeId) -> VpnResult<Vec<JoinedNode>> {
        let sql = r#"SELECT group_id, node_id, allow_join, name, comment FROM joined_node WHERE node_id = ?"#;
        let rows = self
            .conn
            .query_all(sql_query(sql).bind(node_id_db_key(node_id)))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let mut nodes = Vec::new();
        for row in rows {
            let group_id: i64 = row.get("group_id");
            let node_id: String = row.get("node_id");
            let allow_join: bool = row.get("allow_join");
            let name: String = row.get("name");
            let comment: String = row.get("comment");
            nodes.push(JoinedNode {
                group_id: group_id as NetworkGroupId,
                node_id: NodeId::from_base36_or_base58(&node_id)
                    .map_err(into_vpn_err!(VpnErrorCode::IoError))?,
                allow_join,
                name,
                comment,
            });
        }
        Ok(nodes)
    }

    async fn get_networks(&mut self, group_id: &NetworkGroupId) -> VpnResult<Vec<Network>> {
        let sql = r#"SELECT id, name, ip, mask, ipv6, ipv6_mask, pn_server_id FROM network WHERE group_id = ?"#;
        let rows = self
            .conn
            .query_all(sql_query(sql).bind(*group_id as i64))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let mut networks = Vec::new();
        for row in rows {
            let id: NetworkId = (row.get::<i64, _>("id")) as u64;
            let name: String = row.get("name");
            let ip: String = row.get("ip");
            let mask: i64 = row.get("mask");
            let ipv6: String = row.get("ipv6");
            let ipv6_mask: i64 = row.get("ipv6_mask");
            let pn_server_id: String = row.get("pn_server_id");
            let pn_server = network_pn_server_from_db(pn_server_id);
            networks.push(Network {
                id,
                group_id: *group_id,
                name,
                ip_seg: if ip.is_empty() {
                    None
                } else {
                    Some(
                        Ipv4Addr::from_str(ip.as_str())
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?,
                    )
                },
                mask: mask as u8,
                ipv6_seg: if ipv6.is_empty() {
                    None
                } else {
                    Some(
                        Ipv6Addr::from_str(ipv6.as_str())
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?,
                    )
                },
                ipv6_mask: ipv6_mask as u8,
                pn_server,
            });
        }
        Ok(networks)
    }

    async fn add_network(&mut self, network: &Network) -> VpnResult<()> {
        let pn_server_id = network_pn_server_db_id(network.pn_server.as_ref());
        let sql = r#"INSERT INTO network (id, group_id, name, ip, mask, ipv6, ipv6_mask, pn_server_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"#;
        self.conn
            .execute_sql(
                sql_query(sql)
                    .bind(network.id as i64)
                    .bind(network.group_id as i64)
                    .bind(&network.name)
                    .bind(
                        &network
                            .ip_seg
                            .map(|v| v.to_string())
                            .unwrap_or("".to_string()),
                    )
                    .bind(network.mask as i64)
                    .bind(
                        network
                            .ipv6_seg
                            .map(|v| v.to_string())
                            .unwrap_or("".to_string()),
                    )
                    .bind(network.ipv6_mask as i64)
                    .bind(pn_server_id),
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    async fn del_network(&mut self, network_id: &NetworkId) -> VpnResult<()> {
        let sql = r#"DELETE FROM network WHERE id = ?"#;
        self.conn
            .execute_sql(sql_query(sql).bind(*network_id as i64))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    async fn get_network(&mut self, network_id: &NetworkId) -> VpnResult<Option<Network>> {
        let sql = r#"SELECT id, group_id, name, ip, mask, ipv6, ipv6_mask, pn_server_id FROM network WHERE id = ?"#;
        match self
            .conn
            .query_one(sql_query(sql).bind(*network_id as i64))
            .await
        {
            Ok(row) => {
                let id: NetworkId = row.get::<i64, _>("id") as u64;
                let group_id: NetworkGroupId = row.get::<i64, _>("group_id") as u64;
                let name: String = row.get("name");
                let ip: String = row.get("ip");
                let mask: i64 = row.get("mask");
                let ipv6: String = row.get("ipv6");
                let ipv6_mask: i64 = row.get("ipv6_mask");
                let pn_server_id: String = row.get("pn_server_id");
                let pn_server = network_pn_server_from_db(pn_server_id);
                Ok(Some(Network {
                    id,
                    group_id,
                    name,
                    ip_seg: if ip.is_empty() {
                        None
                    } else {
                        Some(
                            Ipv4Addr::from_str(ip.as_str())
                                .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?,
                        )
                    },
                    mask: mask as u8,
                    ipv6_seg: if ipv6.is_empty() {
                        None
                    } else {
                        Some(
                            Ipv6Addr::from_str(ipv6.as_str())
                                .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?,
                        )
                    },
                    ipv6_mask: ipv6_mask as u8,
                    pn_server,
                }))
            }
            Err(e) => {
                if e.code() == SqlErrorCode::NotFound {
                    Ok(None)
                } else {
                    Err(vpn_err!(
                        VpnErrorCode::IoError,
                        "query network {} failed",
                        network_id
                    ))
                }
            }
        }
    }

    async fn update_network(&mut self, network: &Network) -> VpnResult<()> {
        let pn_server_id = network_pn_server_db_id(network.pn_server.as_ref());
        let sql = r#"UPDATE network SET name = ?, ip = ?, mask = ?, ipv6 = ?, ipv6_mask = ?, pn_server_id = ? WHERE id = ?"#;
        self.conn
            .execute_sql(
                sql_query(sql)
                    .bind(&network.name)
                    .bind(
                        &network
                            .ip_seg
                            .map(|v| v.to_string())
                            .unwrap_or("".to_string()),
                    )
                    .bind(network.mask as i64)
                    .bind(
                        network
                            .ipv6_seg
                            .map(|v| v.to_string())
                            .unwrap_or("".to_string()),
                    )
                    .bind(network.ipv6_mask as i64)
                    .bind(pn_server_id)
                    .bind(network.id as i64),
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    async fn exist_network(&mut self, network_id: &NetworkId) -> VpnResult<bool> {
        let sql = r#"SELECT id FROM network WHERE id = ?"#;
        match self
            .conn
            .query_one(sql_query(sql).bind(*network_id as i64))
            .await
        {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.code() == SqlErrorCode::NotFound {
                    Ok(false)
                } else {
                    Err(vpn_err!(
                        VpnErrorCode::IoError,
                        "query network {} failed",
                        network_id
                    ))
                }
            }
        }
    }

    async fn add_member(
        &mut self,
        network_id: &NetworkId,
        member: &NetworkMember,
    ) -> VpnResult<()> {
        let sql =
            r#"INSERT INTO network_member (network_id, node_id, ip, ipv6) VALUES (?, ?, ?, ?)"#;
        self.conn
            .execute_sql(
                sql_query(sql)
                    .bind(*network_id as i64)
                    .bind(node_id_db_key(&member.id))
                    .bind(&member.ip.to_string())
                    .bind(
                        &member
                            .ipv6
                            .as_ref()
                            .map(|v| v.to_string())
                            .unwrap_or("".to_string()),
                    ),
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    async fn del_member(&mut self, network_id: &NetworkId, member: &NodeId) -> VpnResult<()> {
        let sql = r#"DELETE FROM network_member WHERE network_id = ? AND node_id = ?"#;
        self.conn
            .execute_sql(
                sql_query(sql)
                    .bind(*network_id as i64)
                    .bind(node_id_db_key(member)),
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    async fn has_member(&mut self, network_id: &NetworkId, member: &NodeId) -> VpnResult<bool> {
        let sql = r#"SELECT network_id FROM network_member WHERE network_id = ? AND node_id = ?"#;
        match self
            .conn
            .query_one(
                sql_query(sql)
                    .bind(*network_id as i64)
                    .bind(node_id_db_key(member)),
            )
            .await
        {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.code() == SqlErrorCode::NotFound {
                    Ok(false)
                } else {
                    Err(vpn_err!(
                        VpnErrorCode::IoError,
                        "query network member {} failed",
                        node_id_db_key(member)
                    ))
                }
            }
        }
    }

    async fn update_member(
        &mut self,
        network_id: &NetworkId,
        member: &NetworkMember,
    ) -> VpnResult<()> {
        let sql =
            r#"UPDATE network_member SET ip = ?, ipv6 = ? WHERE network_id = ? AND node_id = ?"#;
        self.conn
            .execute_sql(
                sql_query(sql)
                    .bind(&member.ip.to_string())
                    .bind(
                        &member
                            .ipv6
                            .as_ref()
                            .map(|v| v.to_string())
                            .unwrap_or("".to_string()),
                    )
                    .bind(*network_id as i64)
                    .bind(node_id_db_key(&member.id)),
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    async fn get_members(&mut self, network_id: &NetworkId) -> VpnResult<Vec<NetworkMember>> {
        let sql =
            r#"SELECT network_id, node_id, ip, ipv6 FROM network_member WHERE network_id = ?"#;
        let rows = self
            .conn
            .query_all(sql_query(sql).bind(*network_id as i64))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let mut members = Vec::new();
        for row in rows {
            let node_id: String = row.get("node_id");
            let ip: String = row.get("ip");
            let ipv6: String = row.get("ipv6");
            members.push(NetworkMember {
                id: NodeId::from_base36_or_base58(&node_id)
                    .map_err(into_vpn_err!(VpnErrorCode::IoError))?,
                ip,
                ipv6: if ipv6.is_empty() { None } else { Some(ipv6) },
            });
        }
        Ok(members)
    }

    async fn get_allowed_members(
        &mut self,
        network_id: &NetworkId,
    ) -> VpnResult<Vec<NetworkMember>> {
        let sql = r#"SELECT network_member.network_id, network_member.node_id, network_member.ip, network_member.ipv6 FROM network_member
         JOIN joined_node ON joined_node.node_id = network_member.node_id  WHERE network_member.network_id =? and joined_node.allow_join = TRUE"#;
        let rows = self
            .conn
            .query_all(sql_query(sql).bind(*network_id as i64))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let mut members = Vec::new();
        for row in rows {
            let node_id: String = row.get("node_id");
            let ip: String = row.get("ip");
            let ipv6: String = row.get("ipv6");
            members.push(NetworkMember {
                id: NodeId::from_base36_or_base58(&node_id)
                    .map_err(into_vpn_err!(VpnErrorCode::IoError))?,
                ip,
                ipv6: if ipv6.is_empty() { None } else { Some(ipv6) },
            });
        }
        Ok(members)
    }

    async fn get_member(
        &mut self,
        network_id: &NetworkId,
        ip_addr: &IpAddr,
    ) -> VpnResult<Option<NetworkMember>> {
        let result = match ip_addr {
            IpAddr::V4(ipv4) => {
                let sql = "SELECT network_id, node_id, ip, ipv6 FROM network_member WHERE network_id = ? AND ip = ?";
                self.conn
                    .query_one(
                        sql_query(sql)
                            .bind(*network_id as i64)
                            .bind(ipv4.to_string()),
                    )
                    .await
            }
            IpAddr::V6(ipv6) => {
                let sql = "SELECT network_id, node_id, ip, ipv6 FROM network_member WHERE network_id = ? AND ipv6 = ?";
                self.conn
                    .query_one(
                        sql_query(sql)
                            .bind(*network_id as i64)
                            .bind(ipv6.to_string()),
                    )
                    .await
            }
        };
        match result {
            Ok(row) => {
                let node_id: String = row.get("node_id");
                let ip: String = row.get("ip");
                let ipv6: String = row.get("ipv6");
                Ok(Some(NetworkMember {
                    id: NodeId::from_base36_or_base58(&node_id)
                        .map_err(into_vpn_err!(VpnErrorCode::IoError))?,
                    ip,
                    ipv6: if ipv6.is_empty() { None } else { Some(ipv6) },
                }))
            }
            Err(e) => {
                if e.code() == SqlErrorCode::NotFound {
                    Ok(None)
                } else {
                    Err(vpn_err!(
                        VpnErrorCode::IoError,
                        "query network member failed"
                    ))
                }
            }
        }
    }

    async fn get_networks_of_node(&mut self, node_id: &NodeId) -> VpnResult<Vec<NodeNetwork>> {
        //需要联合查询network_member,joined_node和network表，只有被允许的node网络才可以
        let sql = r#"SELECT
    network.id,
    network.group_id,
    network.name,
    network.mask,
    network.ipv6_mask,
    network.pn_server_id,
    network_member.ip,
    network_member.ipv6
FROM network_member
JOIN network ON network_member.network_id = network.id
JOIN joined_node ON network.group_id = joined_node.group_id AND joined_node.node_id = network_member.node_id
WHERE network_member.node_id = ? AND joined_node.allow_join = TRUE
ORDER BY network.id"#;

        let rows = self
            .conn
            .query_all(sql_query(sql).bind(node_id_db_key(node_id)))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let mut networks = Vec::new();
        for row in rows {
            let id: NetworkId = row.get::<i64, _>("id") as u64;
            let group_id: NetworkGroupId = row.get::<i64, _>("group_id") as u64;
            let name: String = row.get("name");
            let mask: i64 = row.get("mask");
            let ipv6_mask: i64 = row.get("ipv6_mask");
            let pn_server_id: String = row.get("pn_server_id");
            let pn_server = network_client_proxy_from_db(pn_server_id)?;
            let member_ip: String = row.get("ip");
            let member_ipv6: String = row.get("ipv6");
            networks.push(NodeNetwork {
                id,
                group_id,
                name,
                ip: if member_ip.is_empty() {
                    None
                } else {
                    Some(IpAddr::V4(
                        Ipv4Addr::from_str(&member_ip)
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?,
                    ))
                },
                mask: mask as u8,
                ipv6: if member_ipv6.is_empty() {
                    None
                } else {
                    Some(IpAddr::V6(
                        Ipv6Addr::from_str(&member_ipv6)
                            .map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?,
                    ))
                },
                ipv6_mask: ipv6_mask as u8,
                pn_server,
            });
        }
        Ok(networks)
    }
}

pub struct SqliteStoreFactory {
    pool: SqlPool,
    traffic_speed_cache: Arc<Mutex<TrafficSpeedCache>>,
}

impl SqliteStoreFactory {
    pub async fn create(db_path: &str) -> VpnResult<Self> {
        let pool = SqlPool::open(db_path, 300, Some(SqliteJournalMode::Wal))
            .await
            .map_err(into_vpn_err!(
                VpnErrorCode::IoError,
                "open sqlite db {} failed",
                db_path
            ))?;
        Ok(Self {
            pool,
            traffic_speed_cache: Arc::new(Mutex::new(TrafficSpeedCache::default())),
        })
    }

    pub fn from_pool(pool: SqlPool) -> Self {
        Self {
            pool,
            traffic_speed_cache: Arc::new(Mutex::new(TrafficSpeedCache::default())),
        }
    }

    pub fn get_node_traffic_speed(&self, node_id: &NodeId) -> PersistedTrafficStats {
        let mut cache = self.traffic_speed_cache.lock().unwrap();
        cache.retain_live();
        cache.pairs.iter().fold(
            PersistedTrafficStats::default(),
            |mut total, (key, entry)| {
                if &key.node_a == node_id {
                    total.tx_bytes = total.tx_bytes.saturating_add(entry.node_a_to_b);
                    total.rx_bytes = total.rx_bytes.saturating_add(entry.node_b_to_a);
                } else if &key.node_b == node_id {
                    total.tx_bytes = total.tx_bytes.saturating_add(entry.node_b_to_a);
                    total.rx_bytes = total.rx_bytes.saturating_add(entry.node_a_to_b);
                }
                total
            },
        )
    }

    pub fn get_network_traffic_speed(&self, network_ids: &HashSet<NetworkId>) -> u64 {
        let mut cache = self.traffic_speed_cache.lock().unwrap();
        cache.retain_live();
        cache
            .pairs
            .iter()
            .filter(|(key, _)| network_ids.contains(&key.network_id))
            .fold(0u64, |total, (_, entry)| {
                total
                    .saturating_add(entry.node_a_to_b)
                    .saturating_add(entry.node_b_to_a)
            })
    }
}

#[async_trait::async_trait]
impl VpnStoreFactory<SqliteVpnStore> for SqliteStoreFactory {
    async fn get_vpn_store(&self) -> VpnResult<VpnStoreGuard<SqliteVpnStore>> {
        Ok(VpnStoreGuard::new(SqliteVpnStore::new_with_traffic_speed_cache(
            self.pool
                .get_conn()
                .await
                .map_err(into_vpn_err!(VpnErrorCode::IoError))?,
            self.traffic_speed_cache.clone(),
        )))
    }
}

pub struct P2pSnCmdServer {
    sn_service: SnServerRef,
}

impl P2pSnCmdServer {
    pub fn new(sn_service: SnServerRef) -> Self {
        Self { sn_service }
    }
}

#[async_trait::async_trait]
impl CmdServer<u16, u8> for P2pSnCmdServer {
    fn register_cmd_handler(&self, cmd: u8, handler: impl CmdHandler<u16, u8>) {
        self.sn_service
            .get_cmd_server()
            .register_cmd_handler(cmd, handler);
    }

    async fn send(&self, peer_id: &PeerId, cmd: u8, version: u8, body: &[u8]) -> CmdResult<()> {
        self.sn_service
            .get_cmd_server()
            .send(peer_id, cmd, version, body)
            .await
    }

    async fn send_with_resp(
        &self,
        peer_id: &PeerId,
        cmd: u8,
        version: u8,
        body: &[u8],
        timeout: Duration,
    ) -> CmdResult<CmdBody> {
        self.sn_service
            .get_cmd_server()
            .send_with_resp(peer_id, cmd, version, body, timeout)
            .await
    }

    async fn send_parts(
        &self,
        peer_id: &PeerId,
        cmd: u8,
        version: u8,
        body: &[&[u8]],
    ) -> CmdResult<()> {
        self.sn_service
            .get_cmd_server()
            .send_parts(peer_id, cmd, version, body)
            .await
    }

    async fn send_parts_with_resp(
        &self,
        peer_id: &PeerId,
        cmd: u8,
        version: u8,
        body: &[&[u8]],
        timeout: Duration,
    ) -> CmdResult<CmdBody> {
        self.sn_service
            .get_cmd_server()
            .send_parts_with_resp(peer_id, cmd, version, body, timeout)
            .await
    }

    async fn send2(&self, peer_id: &PeerId, cmd: u8, version: u8, body: &[&[u8]]) -> CmdResult<()> {
        self.sn_service
            .get_cmd_server()
            .send_parts(peer_id, cmd, version, body)
            .await
    }

    async fn send2_with_resp(
        &self,
        peer_id: &PeerId,
        cmd: u8,
        version: u8,
        body: &[&[u8]],
        timeout: Duration,
    ) -> CmdResult<CmdBody> {
        self.sn_service
            .get_cmd_server()
            .send_parts_with_resp(peer_id, cmd, version, body, timeout)
            .await
    }

    async fn send_cmd(
        &self,
        peer_id: &PeerId,
        cmd: u8,
        version: u8,
        body: CmdBody,
    ) -> CmdResult<()> {
        self.sn_service
            .get_cmd_server()
            .send_cmd(peer_id, cmd, version, body)
            .await
    }

    async fn send_cmd_with_resp(
        &self,
        peer_id: &PeerId,
        cmd: u8,
        version: u8,
        body: CmdBody,
        timeout: Duration,
    ) -> CmdResult<CmdBody> {
        self.sn_service
            .get_cmd_server()
            .send_cmd_with_resp(peer_id, cmd, version, body, timeout)
            .await
    }

    async fn send_by_specify_tunnel(
        &self,
        peer_id: &PeerId,
        tunnel_id: TunnelId,
        cmd: u8,
        version: u8,
        body: &[u8],
    ) -> CmdResult<()> {
        self.sn_service
            .get_cmd_server()
            .send_by_specify_tunnel(peer_id, tunnel_id, cmd, version, body)
            .await
    }

    async fn send_by_specify_tunnel_with_resp(
        &self,
        peer_id: &PeerId,
        tunnel_id: TunnelId,
        cmd: u8,
        version: u8,
        body: &[u8],
        timeout: Duration,
    ) -> CmdResult<CmdBody> {
        self.sn_service
            .get_cmd_server()
            .send_by_specify_tunnel_with_resp(peer_id, tunnel_id, cmd, version, body, timeout)
            .await
    }

    async fn send_parts_by_specify_tunnel(
        &self,
        peer_id: &PeerId,
        tunnel_id: TunnelId,
        cmd: u8,
        version: u8,
        body: &[&[u8]],
    ) -> CmdResult<()> {
        self.sn_service
            .get_cmd_server()
            .send_parts_by_specify_tunnel(peer_id, tunnel_id, cmd, version, body)
            .await
    }

    async fn send_parts_by_specify_tunnel_with_resp(
        &self,
        peer_id: &PeerId,
        tunnel_id: TunnelId,
        cmd: u8,
        version: u8,
        body: &[&[u8]],
        timeout: Duration,
    ) -> CmdResult<CmdBody> {
        self.sn_service
            .get_cmd_server()
            .send_parts_by_specify_tunnel_with_resp(peer_id, tunnel_id, cmd, version, body, timeout)
            .await
    }

    async fn send2_by_specify_tunnel(
        &self,
        peer_id: &PeerId,
        tunnel_id: TunnelId,
        cmd: u8,
        version: u8,
        body: &[&[u8]],
    ) -> CmdResult<()> {
        self.sn_service
            .get_cmd_server()
            .send_parts_by_specify_tunnel(peer_id, tunnel_id, cmd, version, body)
            .await
    }

    async fn send2_by_specify_tunnel_with_resp(
        &self,
        peer_id: &PeerId,
        tunnel_id: TunnelId,
        cmd: u8,
        version: u8,
        body: &[&[u8]],
        timeout: Duration,
    ) -> CmdResult<CmdBody> {
        self.sn_service
            .get_cmd_server()
            .send_parts_by_specify_tunnel_with_resp(peer_id, tunnel_id, cmd, version, body, timeout)
            .await
    }

    async fn send_cmd_by_specify_tunnel(
        &self,
        peer_id: &PeerId,
        tunnel_id: TunnelId,
        cmd: u8,
        version: u8,
        body: CmdBody,
    ) -> CmdResult<()> {
        self.sn_service
            .get_cmd_server()
            .send_cmd_by_specify_tunnel(peer_id, tunnel_id, cmd, version, body)
            .await
    }

    async fn send_cmd_by_specify_tunnel_with_resp(
        &self,
        peer_id: &PeerId,
        tunnel_id: TunnelId,
        cmd: u8,
        version: u8,
        body: CmdBody,
        timeout: Duration,
    ) -> CmdResult<CmdBody> {
        self.sn_service
            .get_cmd_server()
            .send_cmd_by_specify_tunnel_with_resp(peer_id, tunnel_id, cmd, version, body, timeout)
            .await
    }

    async fn send_by_all_tunnels(
        &self,
        peer_id: &PeerId,
        cmd: u8,
        version: u8,
        body: &[u8],
    ) -> CmdResult<()> {
        self.sn_service
            .get_cmd_server()
            .send_by_all_tunnels(peer_id, cmd, version, body)
            .await
    }

    async fn send_parts_by_all_tunnels(
        &self,
        peer_id: &PeerId,
        cmd: u8,
        version: u8,
        body: &[&[u8]],
    ) -> CmdResult<()> {
        self.sn_service
            .get_cmd_server()
            .send_parts_by_all_tunnels(peer_id, cmd, version, body)
            .await
    }

    async fn send2_by_all_tunnels(
        &self,
        peer_id: &PeerId,
        cmd: u8,
        version: u8,
        body: &[&[u8]],
    ) -> CmdResult<()> {
        self.sn_service
            .get_cmd_server()
            .send_parts_by_all_tunnels(peer_id, cmd, version, body)
            .await
    }
}

#[async_trait::async_trait]
impl VpnCmdServer for P2pSnCmdServer {
    async fn get_peer_wan_ip(&self, peer_id: &PeerId) -> VpnResult<Vec<IpAddr>> {
        let list = self
            .sn_service
            .service()
            .get_peer_wan_ep(&peer_id)
            .await
            .iter()
            .map(|ep| ep.addr().ip().clone())
            .collect();
        Ok(list)
    }
}

pub type NodeManagerRef = Arc<NodeManager<SqliteVpnStore, SqliteStoreFactory>>;
pub type NetworkManagerRef = Arc<NetworkManager<SqliteVpnStore, SqliteStoreFactory>>;
pub type VpnServerRef = Arc<
    VpnServer<
        P2pSnCmdServer,
        SqliteVpnStore,
        SqliteStoreFactory,
        crate::pn_control_server::ProxyControlCmdService,
    >,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use vpn_frame::{
        NodeTrafficDelta, NodeTrafficReport, NodeTrafficReportId, PnTrafficDirectionSample,
        PnTrafficSample, ProxyTrafficReport, ProxyTrafficReportApplyResult, ProxyTrafficReportId,
    };

    async fn new_test_store() -> VpnStoreGuard<SqliteVpnStore> {
        let db_path = std::env::temp_dir().join(format!(
            "bucky-vpn-proxy-traffic-{}.sqlite",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store_factory = SqliteStoreFactory::create(db_path.to_str().unwrap())
            .await
            .unwrap();
        let mut store = store_factory.get_vpn_store().await.unwrap();
        store.init_db().await.unwrap();
        store
    }

    #[tokio::test]
    async fn proxy_traffic_report_is_batch_applied_and_idempotent() {
        let mut store = new_test_store().await;
        let group_id = 42;
        let network_id = 100;
        let pn_node_id = NodeId::from(vec![9u8; 32].as_slice());
        let source_id = NodeId::from(vec![1u8; 32].as_slice());
        let dest_id = NodeId::from(vec![2u8; 32].as_slice());

        store.add_network_group(&group_id).await.unwrap();
        store
            .conn
            .execute_sql(
                sql_query("INSERT INTO user (id, network_id, server_id) VALUES (?, ?, ?)")
                    .bind("alice")
                    .bind(group_id as i64)
                    .bind("server-1"),
            )
            .await
            .unwrap();
        store
            .add_network(&Network {
                id: network_id,
                group_id,
                name: "test-network".to_string(),
                ip_seg: Some("10.0.0.0".parse().unwrap()),
                mask: 24,
                ipv6_seg: None,
                ipv6_mask: 0,
                pn_server: None,
            })
            .await
            .unwrap();

        let report = ProxyTrafficReport {
            report_id: ProxyTrafficReportId("report-1".to_string()),
            started_at_ms: 1000,
            ended_at_ms: 6000,
            traffic_sample: PnTrafficSample {
                network_id,
                source_id: source_id.clone(),
                dest_id: dest_id.clone(),
                source_to_dest: PnTrafficDirectionSample {
                    bytes: 120,
                    speed_bytes_per_sec: 24,
                },
                dest_to_source: PnTrafficDirectionSample {
                    bytes: 80,
                    speed_bytes_per_sec: 16,
                },
            },
        };

        let applied = store
            .apply_proxy_traffic_report(&pn_node_id, &report)
            .await
            .unwrap();
        assert_eq!(applied.result, ProxyTrafficReportApplyResult::Applied);
        assert_eq!(applied.remaining.len(), 1);
        assert_eq!(applied.remaining[0].user_id, "alice");
        assert_eq!(applied.remaining[0].remaining_bytes, None);

        let total = store
            .get_proxy_traffic_total(&network_id, &source_id, &dest_id)
            .await
            .unwrap();
        assert_eq!(total.tx_bytes, 120);
        assert_eq!(total.rx_bytes, 80);

        let source_stats = store.get_persisted_node_traffic(&source_id).await.unwrap();
        assert_eq!(source_stats.tx_bytes, 120);
        assert_eq!(source_stats.rx_bytes, 80);
        let group_stats = store.get_persisted_group_traffic(&group_id).await.unwrap();
        assert_eq!(group_stats.tx_bytes, 200);
        assert_eq!(group_stats.rx_bytes, 200);

        {
            let cache = store.traffic_speed_cache.lock().unwrap();
            let speed = cache.pairs.values().next().unwrap();
            assert_eq!(speed.node_a_to_b, 24);
            assert_eq!(speed.node_b_to_a, 16);
        }

        let mut duplicate_report = report.clone();
        duplicate_report.ended_at_ms = 7000;
        duplicate_report
            .traffic_sample
            .source_to_dest
            .speed_bytes_per_sec = 240;
        duplicate_report
            .traffic_sample
            .dest_to_source
            .speed_bytes_per_sec = 160;
        let duplicate = store
            .apply_proxy_traffic_report(&pn_node_id, &duplicate_report)
            .await
            .unwrap();
        assert_eq!(duplicate.result, ProxyTrafficReportApplyResult::Duplicate);

        {
            let cache = store.traffic_speed_cache.lock().unwrap();
            let speed = cache.pairs.values().next().unwrap();
            assert_eq!(speed.node_a_to_b, 24);
            assert_eq!(speed.node_b_to_a, 16);
        }

        let total = store
            .get_proxy_traffic_total(&network_id, &source_id, &dest_id)
            .await
            .unwrap();
        assert_eq!(total.tx_bytes, 120);
        assert_eq!(total.rx_bytes, 80);

        {
            let mut cache = store.traffic_speed_cache.lock().unwrap();
            cache.pairs.values_mut().next().unwrap().expires_at = Instant::now();
            cache.retain_live();
            assert!(cache.pairs.is_empty());
        }
    }

    #[tokio::test]
    async fn rollback_transaction_discards_pending_traffic_write() {
        let mut store = new_test_store().await;
        let node_id = NodeId::from(vec![21u8; 32].as_slice());

        store.begin_transaction().await.unwrap();
        store
            .add_persisted_node_traffic(
                &node_id,
                PersistedTrafficStats {
                    tx_bytes: 100,
                    rx_bytes: 200,
                },
            )
            .await
            .unwrap();
        store.rollback_transaction().await.unwrap();

        assert_eq!(
            store.get_persisted_node_traffic(&node_id).await.unwrap(),
            PersistedTrafficStats::default()
        );
    }

    #[tokio::test]
    async fn node_traffic_record_rolls_back_and_retries_idempotently() {
        let mut store = new_test_store().await;
        let pn_node_id = NodeId::from(vec![29u8; 32].as_slice());
        let first = NodeId::from(vec![22u8; 32].as_slice());
        let report = NodeTrafficReport {
            report_id: NodeTrafficReportId("node-record-rollback".to_string()),
            started_at_ms: 100,
            ended_at_ms: 200,
            delta: NodeTrafficDelta {
                node_id: first.clone(),
                tx_bytes: 10,
                rx_bytes: 20,
            },
        };
        let trigger = format!(
            "CREATE TRIGGER reject_node_record BEFORE INSERT ON pn_node_traffic_stat WHEN NEW.node_id = '{}' BEGIN SELECT RAISE(FAIL, 'injected failure'); END",
            node_id_db_key(&first)
        );
        store.conn.execute_sql(sql_query(&trigger)).await.unwrap();

        assert!(store
            .apply_node_traffic_report(&pn_node_id, &report)
            .await
            .is_err());
        assert_eq!(
            store.get_persisted_node_traffic(&first).await.unwrap(),
            PersistedTrafficStats::default()
        );
        store
            .conn
            .execute_sql(sql_query("DROP TRIGGER reject_node_record"))
            .await
            .unwrap();

        assert_eq!(
            store
                .apply_node_traffic_report(&pn_node_id, &report)
                .await
                .unwrap(),
            ProxyTrafficReportApplyResult::Applied
        );
        assert_eq!(
            store
                .apply_node_traffic_report(&pn_node_id, &report)
                .await
                .unwrap(),
            ProxyTrafficReportApplyResult::Duplicate
        );
        assert_eq!(
            store.get_persisted_node_traffic(&first).await.unwrap(),
            PersistedTrafficStats {
                tx_bytes: 10,
                rx_bytes: 20,
            }
        );
    }
}
