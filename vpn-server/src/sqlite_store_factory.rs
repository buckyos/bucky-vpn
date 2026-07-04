#![allow(unused)]

use p2p_frame::cmd_server::server::CmdServer;
use p2p_frame::sn::service::{SnServerRef, SnServiceRef};
use sfo_sql::Row;
use sfo_sql::errors::SqlErrorCode;
use sfo_sql::mysql::sql_query;
use sfo_sql::sqlite::{SqlConnection, SqlPool, SqliteJournalMode};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use vpn_frame::cmd_server::errors::CmdResult;
use vpn_frame::cmd_server::{CmdBody, CmdHandler, PeerId, TunnelId};
use vpn_frame::errors::{VpnErrorCode, VpnResult, into_vpn_err, vpn_err};
use vpn_frame::server::{
    JoinedNode, Network, NetworkGroupId, NetworkId, NetworkManager, NetworkMember, NetworkStore,
    Node, NodeId, NodeManager, NodeStore, VpnCmdServer, VpnServer, VpnStore, VpnStoreFactory,
    VpnStoreGuard,
};
use vpn_frame::{NodeNetwork, PnServerInfo};

pub struct SqliteVpnStore {
    conn: SqlConnection,
}

fn node_id_db_key(node_id: &NodeId) -> String {
    node_id.to_base58()
}

fn pn_server_addresses_db_key(pn_server: &PnServerInfo) -> VpnResult<String> {
    serde_json::to_string(&pn_server.addresses).map_err(into_vpn_err!(VpnErrorCode::InvalidParam))
}

fn pn_server_from_db(
    id: String,
    ip: String,
    port: i64,
    addresses: String,
) -> VpnResult<Option<PnServerInfo>> {
    if id.is_empty() {
        return Ok(None);
    }
    let addresses = if addresses.is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(&addresses).map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?
    };
    Ok(Some(PnServerInfo::new_with_addresses(
        id,
        IpAddr::from_str(&ip).map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?,
        port as u16,
        addresses,
    )))
}

fn pn_server_db_parts(
    pn_server: Option<&PnServerInfo>,
) -> VpnResult<(String, String, i64, String)> {
    match pn_server {
        Some(pn_server) => Ok((
            pn_server.id.clone(),
            pn_server.ip.to_string(),
            pn_server.port as i64,
            pn_server_addresses_db_key(pn_server)?,
        )),
        None => Ok(("".to_string(), "".to_string(), 0, "".to_string())),
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
    pub pn_server: PnServerInfo,
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
        Self { conn }
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
            pn_server_ip TEXT NOT NULL DEFAULT '',
            pn_server_port INTEGER NOT NULL DEFAULT 0,
            pn_server_addresses TEXT NOT NULL DEFAULT '',
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

        let sql = r#"CREATE TABLE IF NOT EXISTS pn_proxy_node (
            pn_server_id TEXT PRIMARY KEY,
            pn_server_ip TEXT NOT NULL,
            pn_server_port INTEGER NOT NULL,
            pn_server_addresses TEXT NOT NULL DEFAULT '',
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

        for (column, definition) in [
            ("pn_server_id", "TEXT NOT NULL DEFAULT ''"),
            ("pn_server_ip", "TEXT NOT NULL DEFAULT ''"),
            ("pn_server_port", "INTEGER NOT NULL DEFAULT 0"),
            ("pn_server_addresses", "TEXT NOT NULL DEFAULT ''"),
        ] {
            if !columns.iter().any(|existing| existing == column) {
                let sql = format!("ALTER TABLE network ADD COLUMN {} {}", column, definition);
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

        for (column, definition) in [("pn_server_addresses", "TEXT NOT NULL DEFAULT ''")] {
            if !columns.iter().any(|existing| existing == column) {
                let sql = format!(
                    "ALTER TABLE pn_proxy_node ADD COLUMN {} {}",
                    column, definition
                );
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
        let sql = r#"INSERT INTO pn_node_traffic_stat (node_id, tx_bytes, rx_bytes)
            VALUES (?, ?, ?)
            ON CONFLICT(node_id) DO UPDATE SET
                tx_bytes = tx_bytes + excluded.tx_bytes,
                rx_bytes = rx_bytes + excluded.rx_bytes"#;
        self.conn
            .execute_sql(
                sql_query(sql)
                    .bind(node_id_db_key(node_id))
                    .bind(stats.tx_bytes as i64)
                    .bind(stats.rx_bytes as i64),
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
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
        let sql = r#"INSERT INTO pn_group_traffic_stat (group_id, tx_bytes, rx_bytes)
            VALUES (?, ?, ?)
            ON CONFLICT(group_id) DO UPDATE SET
                tx_bytes = tx_bytes + excluded.tx_bytes,
                rx_bytes = rx_bytes + excluded.rx_bytes"#;
        self.conn
            .execute_sql(
                sql_query(sql)
                    .bind(*group_id as i64)
                    .bind(stats.tx_bytes as i64)
                    .bind(stats.rx_bytes as i64),
            )
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    pub async fn ensure_proxy_node_pending(&mut self, pn_server: &PnServerInfo) -> VpnResult<()> {
        let sql = r#"INSERT INTO pn_proxy_node (pn_server_id, pn_server_ip, pn_server_port, pn_server_addresses, status, updated_at, comment)
            VALUES (?, ?, ?, ?, ?, ?, '')
            ON CONFLICT(pn_server_id) DO UPDATE SET
                pn_server_ip = excluded.pn_server_ip,
                pn_server_port = excluded.pn_server_port,
                pn_server_addresses = excluded.pn_server_addresses,
                updated_at = excluded.updated_at"#;
        self.conn
            .execute_sql(
                sql_query(sql)
                    .bind(&pn_server.id)
                    .bind(pn_server.ip.to_string())
                    .bind(pn_server.port as i64)
                    .bind(pn_server_addresses_db_key(pn_server)?)
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
        let sql = r#"INSERT INTO pn_proxy_node (pn_server_id, pn_server_ip, pn_server_port, pn_server_addresses, status, updated_at, comment)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(pn_server_id) DO UPDATE SET
                pn_server_ip = excluded.pn_server_ip,
                pn_server_port = excluded.pn_server_port,
                pn_server_addresses = excluded.pn_server_addresses,
                status = excluded.status,
                updated_at = excluded.updated_at,
                comment = excluded.comment"#;
        self.conn
            .execute_sql(
                sql_query(sql)
                    .bind(&pn_server.id)
                    .bind(pn_server.ip.to_string())
                    .bind(pn_server.port as i64)
                    .bind(pn_server_addresses_db_key(pn_server)?)
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
        let sql = r#"SELECT pn_server_id, pn_server_ip, pn_server_port, pn_server_addresses, status, updated_at, comment FROM pn_proxy_node ORDER BY pn_server_id"#;
        let rows = self
            .conn
            .query_all(sql_query(sql))
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let mut approvals = Vec::new();
        for row in rows {
            let status: String = row.get("status");
            let pn_server_id: String = row.get("pn_server_id");
            let pn_server_ip: String = row.get("pn_server_ip");
            let pn_server_port: i64 = row.get("pn_server_port");
            let pn_server_addresses: String = row.get("pn_server_addresses");
            approvals.push(ProxyNodeApproval {
                pn_server: pn_server_from_db(
                    pn_server_id,
                    pn_server_ip,
                    pn_server_port,
                    pn_server_addresses,
                )?
                .ok_or_else(|| vpn_err!(VpnErrorCode::InvalidParam, "empty proxy node id"))?,
                status: ProxyNodeApprovalStatus::from_str(&status)?,
                updated_at: row.get::<i64, _>("updated_at") as u64,
                comment: row.get("comment"),
            });
        }
        Ok(approvals)
    }
}

#[async_trait::async_trait]
impl VpnStore for SqliteVpnStore {
    async fn begin_transaction(&mut self) -> VpnResult<()> {
        self.conn
            .begin_transaction()
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))
    }

    async fn commit_transaction(&mut self) -> VpnResult<()> {
        self.conn
            .commit_transaction()
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))
    }

    async fn rollback_transaction(&mut self) -> VpnResult<()> {
        Ok(())
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
        match result {
            Ok(()) => self.commit_transaction().await,
            Err(err) => {
                let _ = self.rollback_transaction().await;
                Err(err)
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
        let sql = r#"SELECT id, name, ip, mask, ipv6, ipv6_mask, pn_server_id, pn_server_ip, pn_server_port, pn_server_addresses FROM network WHERE group_id = ?"#;
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
            let pn_server_ip: String = row.get("pn_server_ip");
            let pn_server_port: i64 = row.get("pn_server_port");
            let pn_server_addresses: String = row.get("pn_server_addresses");
            let pn_server = pn_server_from_db(
                pn_server_id,
                pn_server_ip,
                pn_server_port,
                pn_server_addresses,
            )?;
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
        let (pn_server_id, pn_server_ip, pn_server_port, pn_server_addresses) =
            pn_server_db_parts(network.pn_server.as_ref())?;
        let sql = r#"INSERT INTO network (id, group_id, name, ip, mask, ipv6, ipv6_mask, pn_server_id, pn_server_ip, pn_server_port, pn_server_addresses) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#;
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
                    .bind(pn_server_id)
                    .bind(pn_server_ip)
                    .bind(pn_server_port)
                    .bind(pn_server_addresses),
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
        let sql = r#"SELECT id, group_id, name, ip, mask, ipv6, ipv6_mask, pn_server_id, pn_server_ip, pn_server_port, pn_server_addresses FROM network WHERE id = ?"#;
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
                let pn_server_ip: String = row.get("pn_server_ip");
                let pn_server_port: i64 = row.get("pn_server_port");
                let pn_server_addresses: String = row.get("pn_server_addresses");
                let pn_server = pn_server_from_db(
                    pn_server_id,
                    pn_server_ip,
                    pn_server_port,
                    pn_server_addresses,
                )?;
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
        let (pn_server_id, pn_server_ip, pn_server_port, pn_server_addresses) =
            pn_server_db_parts(network.pn_server.as_ref())?;
        let sql = r#"UPDATE network SET name = ?, ip = ?, mask = ?, ipv6 = ?, ipv6_mask = ?, pn_server_id = ?, pn_server_ip = ?, pn_server_port = ?, pn_server_addresses = ? WHERE id = ?"#;
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
                    .bind(pn_server_ip)
                    .bind(pn_server_port)
                    .bind(pn_server_addresses)
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
    network.pn_server_ip,
    network.pn_server_port,
    network.pn_server_addresses,
    network_member.ip,
    network_member.ipv6
FROM network_member
JOIN network ON network_member.network_id = network.id
JOIN joined_node ON network.group_id = joined_node.group_id AND joined_node.node_id = network_member.node_id
WHERE network_member.node_id = ? AND joined_node.allow_join = TRUE"#;

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
            let pn_server_ip: String = row.get("pn_server_ip");
            let pn_server_port: i64 = row.get("pn_server_port");
            let pn_server_addresses: String = row.get("pn_server_addresses");
            let pn_server = pn_server_from_db(
                pn_server_id,
                pn_server_ip,
                pn_server_port,
                pn_server_addresses,
            )?;
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
        Ok(Self { pool })
    }

    pub fn from_pool(pool: SqlPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl VpnStoreFactory<SqliteVpnStore> for SqliteStoreFactory {
    async fn get_vpn_store(&self) -> VpnResult<VpnStoreGuard<SqliteVpnStore>> {
        Ok(VpnStoreGuard::new(SqliteVpnStore::new(
            self.pool
                .get_conn()
                .await
                .map_err(into_vpn_err!(VpnErrorCode::IoError))?,
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
pub type VpnServerRef = Arc<VpnServer<P2pSnCmdServer, SqliteVpnStore, SqliteStoreFactory>>;
