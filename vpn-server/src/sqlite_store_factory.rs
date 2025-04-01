#![allow(unused)]

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;
use std::sync::Arc;
use p2p_frame::sn::service::SnServiceRef;
use sfo_sql::errors::{SqlErrorCode};
use sfo_sql::mysql::sql_query;
use sfo_sql::sqlite::{SqlConnection, SqlPool, SqliteJournalMode};
use sfo_sql::{Row};
use vpn_frame::cmd_server::{CmdHandler, PeerId, TunnelId};
use vpn_frame::cmd_server::errors::CmdResult;
use vpn_frame::cmd_server::server::CmdServer;
use vpn_frame::errors::{into_vpn_err, vpn_err, VpnErrorCode, VpnResult};
use vpn_frame::NodeNetwork;
use vpn_frame::server::{JoinedNode, Network, NetworkGroupId, NetworkId, NetworkManager, NetworkMember, NetworkStore, Node, NodeId, NodeManager, NodeStore, VpnCmdServer, VpnServer, VpnStore, VpnStoreFactory, VpnStoreGuard};

pub struct SqliteVpnStore {
    conn: SqlConnection
}

impl SqliteVpnStore {
    pub fn new(conn: SqlConnection) -> Self {
        Self {
            conn
        }
    }

    pub async fn init_db(&mut self) -> VpnResult<()> {
        let sql = r#"CREATE TABLE IF NOT EXISTS node (
            id varchar(45) PRIMARY KEY,
            info_version integer NOT NULL DEFAULT 0
        )"#;
        self.conn.execute_sql(sql_query(sql)).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        let sql = r#"CREATE TABLE IF NOT EXISTS network_group (
            id integer PRIMARY KEY
        )"#;
        self.conn.execute_sql(sql_query(sql)).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        let sql = r#"CREATE TABLE IF NOT EXISTS joined_node (
            group_id integer NOT NULL,
            node_id varchar(45) NOT NULL,
            allow_join BOOLEAN NOT NULL DEFAULT FALSE,
            name TEXT NOT NULL,
            comment TEXT NOT NULL,
            PRIMARY KEY (group_id, node_id)
        )"#;
        self.conn.execute_sql(sql_query(sql)).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let sql = "CREATE INDEX IF NOT EXISTS joined_node_node_id ON joined_node(node_id)";
        self.conn.execute_sql(sql_query(sql)).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        let sql = r#"CREATE TABLE IF NOT EXISTS network (
            id integer PRIMARY KEY,
            group_id integer NOT NULL,
            name TEXT NOT NULL,
            ip TEXT NOT NULL,
            mask INTEGER NOT NULL,
            ipv6 TEXT,
            ipv6_mask INTEGER,
            FOREIGN KEY (group_id) REFERENCES network_group(id)
        )"#;
        self.conn.execute_sql(sql_query(sql)).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        let sql = r#"CREATE TABLE IF NOT EXISTS network_member (
            network_id integer NOT NULL,
            node_id varchar(45) NOT NULL,
            ip varchar(15) NOT NULL,
            ipv6 varchar(32) NOT NULL,
            PRIMARY KEY (network_id, node_id),
            FOREIGN KEY (network_id) REFERENCES network(id)
        )"#;
        self.conn.execute_sql(sql_query(sql)).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let sql = "CREATE INDEX IF NOT EXISTS network_member_node_id ON network_member(node_id)";
        self.conn.execute_sql(sql_query(sql)).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let sql = "CREATE INDEX IF NOT EXISTS network_member_ip ON network_member(network_id, ip)";
        self.conn.execute_sql(sql_query(sql)).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let sql = "CREATE INDEX IF NOT EXISTS network_member_ipv6 ON network_member(network_id, ipv6)";
        self.conn.execute_sql(sql_query(sql)).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl VpnStore for SqliteVpnStore {
    async fn begin_transaction(&mut self) -> VpnResult<()> {
        self.conn.begin_transaction().await.map_err(into_vpn_err!(VpnErrorCode::IoError))
    }

    async fn commit_transaction(&mut self) -> VpnResult<()> {
        self.conn.commit_transaction().await.map_err(into_vpn_err!(VpnErrorCode::IoError))
    }

    async fn rollback_transaction(&mut self) -> VpnResult<()> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl NodeStore for SqliteVpnStore {
    async fn add_node(&mut self, node: &Node) -> VpnResult<()> {
        let sql = r#"INSERT INTO node (id) VALUES (?)"#;
        self.conn.execute_sql(sql_query(sql).bind(&node.id.to_base58())).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    async fn remove_node(&mut self, id: &NodeId) -> VpnResult<()> {
        let sql = r#"DELETE FROM node WHERE id = ?"#;
        self.conn.execute_sql(sql_query(sql).bind(&id.to_base58())).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    async fn get_node(&mut self, id: &NodeId) -> VpnResult<Option<Node>> {
        let sql = r#"SELECT id, info_version FROM node WHERE id = ?"#;
        match self.conn.query_one(sql_query(sql).bind(&id.to_base58())).await {
            Ok(row) => {
                let id: String = row.get("id");
                let info_version: i64 = row.get("info_version");
                Ok(Some(Node {
                    id: NodeId::from_base58(&id).map_err(into_vpn_err!(VpnErrorCode::IoError))?,
                    info_version: info_version as u16,
                }))}
            Err(e) => {
                if e.code() == SqlErrorCode::NotFound {
                    Ok(None)
                } else {
                    Err(vpn_err!(VpnErrorCode::IoError, "query node {} failed", id.to_base58()))
                }
            }
        }
    }

    async fn exist_node(&mut self, id: &NodeId) -> VpnResult<bool> {
        let sql = r#"SELECT id FROM node WHERE id = ?"#;
        match self.conn.query_one(sql_query(sql).bind(&id.to_base58())).await {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.code() == SqlErrorCode::NotFound {
                    Ok(false)
                } else {
                    Err(vpn_err!(VpnErrorCode::IoError, "query node {} failed", id.to_base58()))
                }
            }
        }
    }

    async fn inc_info_version(&mut self, id: &NodeId) -> VpnResult<()> {
        let sql = r#"UPDATE node SET info_version = info_version + 1 WHERE id = ?"#;
        self.conn.execute_sql(sql_query(sql).bind(&id.to_base58())).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl NetworkStore for SqliteVpnStore {
    async fn add_network_group(&mut self, group_id: &NetworkGroupId) -> VpnResult<()> {
        let sql = r#"INSERT INTO network_group (id) VALUES (?)"#;
        self.conn.execute_sql(sql_query(sql).bind(*group_id as i64)).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    async fn exist_network_group(&mut self, group_id: &NetworkGroupId) -> VpnResult<bool> {
        let sql = r#"SELECT id FROM network_group WHERE id = ?"#;
        match self.conn.query_one(sql_query(sql).bind(*group_id as i64)).await {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.code() == SqlErrorCode::NotFound {
                    Ok(false)
                } else {
                    Err(vpn_err!(VpnErrorCode::IoError, "query network group {} failed", group_id))
                }
            }
        }
    }

    async fn has_joined(&mut self, group_id: &NetworkGroupId, node_id: &NodeId) -> VpnResult<bool> {
        let sql = r#"SELECT group_id FROM joined_node WHERE group_id = ? AND node_id = ?"#;
        match self.conn.query_one(sql_query(sql).bind(*group_id as i64).bind(&node_id.to_base58())).await {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.code() == SqlErrorCode::NotFound {
                    Ok(false)
                } else {
                    Err(vpn_err!(VpnErrorCode::IoError, "query joined node {} failed", node_id.to_base58()))
                }
            }
        }
    }

    async fn add_joined_node(&mut self, node: &JoinedNode) -> VpnResult<()> {
        let sql = r#"INSERT INTO joined_node (group_id, node_id, allow_join, name, comment) VALUES (?, ?, ?, ?, ?)"#;
        self.conn.execute_sql(sql_query(sql)
            .bind(node.group_id as i64)
            .bind(&node.node_id.to_base58())
            .bind(node.allow_join)
            .bind(node.name.as_str())
            .bind(node.comment.as_str())).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    async fn del_joined_node(&mut self, group_id: &NetworkGroupId, node_id: &NodeId) -> VpnResult<()> {
        let sql = r#"DELETE FROM joined_node WHERE group_id = ? AND node_id = ?"#;
        self.conn.execute_sql(sql_query(sql).bind(*group_id as i64).bind(&node_id.to_base58())).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    async fn get_joined_node(&mut self, group_id: &NetworkGroupId, node_id: &NodeId) -> VpnResult<Option<JoinedNode>> {
        let sql = r#"SELECT group_id, node_id, allow_join, name, comment FROM joined_node WHERE group_id = ? AND node_id = ?"#;
        match self.conn.query_one(sql_query(sql).bind(*group_id as i64).bind(&node_id.to_base58())).await {
            Ok(row) => {
                let group_id: i64 = row.get("group_id");
                let node_id: String = row.get("node_id");
                let allow_join: bool = row.get("allow_join");
                let name: String = row.get("name");
                let comment: String = row.get("comment");
                Ok(Some(JoinedNode {
                    group_id: group_id as NetworkGroupId,
                    node_id: NodeId::from_base58(&node_id).map_err(into_vpn_err!(VpnErrorCode::IoError))?,
                    allow_join,
                    name,
                    comment,
                }))
            }
            Err(e) => {
                if e.code() == SqlErrorCode::NotFound {
                    Ok(None)
                } else {
                    Err(vpn_err!(VpnErrorCode::IoError, "query joined node {} failed", node_id.to_base58()))
                }
            }
        }
    }

    async fn get_joined_nodes(&mut self, group_id: &NetworkGroupId) -> VpnResult<Vec<JoinedNode>> {
        let sql = r#"SELECT group_id, node_id, allow_join, name, comment FROM joined_node WHERE group_id = ?"#;
        let rows = self.conn.query_all(sql_query(sql).bind(*group_id as i64)).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let mut nodes = Vec::new();
        for row in rows {
            let group_id: i64 = row.get("group_id");
            let node_id: String = row.get("node_id");
            let allow_join: bool = row.get("allow_join");
            let name: String = row.get("name");
            let comment: String = row.get("comment");
            nodes.push(JoinedNode {
                group_id: group_id as NetworkGroupId,
                node_id: NodeId::from_base58(&node_id).map_err(into_vpn_err!(VpnErrorCode::IoError))?,
                allow_join,
                name,
                comment,
            });
        }
        Ok(nodes)
    }

    async fn update_joined_node(&mut self, node: &JoinedNode) -> VpnResult<()> {
        let sql = r#"UPDATE joined_node SET allow_join = ?, name = ?, comment = ? WHERE group_id = ? AND node_id = ?"#;
        self.conn.execute_sql(sql_query(sql)
            .bind(node.allow_join)
            .bind(node.name.as_str())
            .bind(node.comment.as_str())
            .bind(node.group_id as i64)
            .bind(&node.node_id.to_base58())).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    async fn get_joined_network_group(&mut self, node_id: &NodeId) -> VpnResult<Vec<JoinedNode>> {
        let sql = r#"SELECT group_id, node_id, allow_join, name, comment FROM joined_node WHERE node_id = ?"#;
        let rows = self.conn.query_all(sql_query(sql).bind(&node_id.to_base58())).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let mut nodes = Vec::new();
        for row in rows {
            let group_id: i64 = row.get("group_id");
            let node_id: String = row.get("node_id");
            let allow_join: bool = row.get("allow_join");
            let name: String = row.get("name");
            let comment: String = row.get("comment");
            nodes.push(JoinedNode {
                group_id: group_id as NetworkGroupId,
                node_id: NodeId::from_base58(&node_id).map_err(into_vpn_err!(VpnErrorCode::IoError))?,
                allow_join,
                name,
                comment,
            });
        }
        Ok(nodes)
    }

    async fn get_networks(&mut self, group_id: &NetworkGroupId) -> VpnResult<Vec<Network>> {
        let sql = r#"SELECT id, name, ip, mask, ipv6, ipv6_mask FROM network WHERE group_id = ?"#;
        let rows = self.conn.query_all(sql_query(sql).bind(*group_id as i64)).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let mut networks = Vec::new();
        for row in rows {
            let id: NetworkId = (row.get::<i64, _>("id")) as u64;
            let name: String = row.get("name");
            let ip: String = row.get("ip");
            let mask: i64 = row.get("mask");
            let ipv6: String = row.get("ipv6");
            let ipv6_mask: i64 = row.get("ipv6_mask");
            networks.push(Network {
                id,
                group_id: *group_id,
                name,
                ip_seg: if ip.is_empty() { None } else { Some(Ipv4Addr::from_str(ip.as_str()).map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?) },
                mask: mask as u8,
                ipv6_seg: if ipv6.is_empty() { None } else { Some(Ipv6Addr::from_str(ipv6.as_str()).map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?) },
                ipv6_mask: ipv6_mask as u8,
            });
        }
        Ok(networks)
    }

    async fn add_network(&mut self, network: &Network) -> VpnResult<()> {
        let sql = r#"INSERT INTO network (id, group_id, name, ip, mask, ipv6, ipv6_mask) VALUES (?, ?, ?, ?, ?, ?, ?)"#;
        self.conn.execute_sql(sql_query(sql)
            .bind(network.id as i64)
            .bind(network.group_id as i64)
            .bind(&network.name)
            .bind(&network.ip_seg.map(|v| v.to_string()).unwrap_or("".to_string()))
            .bind(network.mask as i64)
            .bind(network.ipv6_seg.map(|v| v.to_string()).unwrap_or("".to_string()))
            .bind(network.ipv6_mask as i64)).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    async fn del_network(&mut self, network_id: &NetworkId) -> VpnResult<()> {
        let sql = r#"DELETE FROM network WHERE id = ?"#;
        self.conn.execute_sql(sql_query(sql).bind(*network_id as i64)).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    async fn get_network(&mut self, network_id: &NetworkId) -> VpnResult<Option<Network>> {
        let sql = r#"SELECT id, group_id, name, ip, mask, ipv6, ipv6_mask FROM network WHERE id = ?"#;
        match self.conn.query_one(sql_query(sql).bind(*network_id as i64)).await {
            Ok(row) => {
                let id: NetworkId = row.get::<i64, _>("id") as u64;
                let group_id: NetworkGroupId = row.get::<i64, _>("group_id") as u64;
                let name: String = row.get("name");
                let ip: String = row.get("ip");
                let mask: i64 = row.get("mask");
                let ipv6: String = row.get("ipv6");
                let ipv6_mask: i64 = row.get("ipv6_mask");
                Ok(Some(Network {
                    id,
                    group_id,
                    name,
                    ip_seg: if ip.is_empty() { None } else { Some(Ipv4Addr::from_str(ip.as_str()).map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?) },
                    mask: mask as u8,
                    ipv6_seg: if ipv6.is_empty() { None } else { Some(Ipv6Addr::from_str(ipv6.as_str()).map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?) },
                    ipv6_mask: ipv6_mask as u8,
                }))
            }
            Err(e) => {
                if e.code() == SqlErrorCode::NotFound {
                    Ok(None)
                } else {
                    Err(vpn_err!(VpnErrorCode::IoError, "query network {} failed", network_id))
                }
            }
        }
    }

    async fn update_network(&mut self, network: &Network) -> VpnResult<()> {
        let sql = r#"UPDATE network SET name = ?, ip = ?, mask = ?, ipv6 = ?, ipv6_mask = ? WHERE id = ?"#;
        self.conn.execute_sql(sql_query(sql)
            .bind(&network.name)
            .bind(&network.ip_seg.map(|v| v.to_string()).unwrap_or("".to_string()))
            .bind(network.mask as i64)
            .bind(network.ipv6_seg.map(|v| v.to_string()).unwrap_or("".to_string()))
            .bind(network.ipv6_mask as i64)
            .bind(network.id as i64)).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    async fn exist_network(&mut self, network_id: &NetworkId) -> VpnResult<bool> {
        let sql = r#"SELECT id FROM network WHERE id = ?"#;
        match self.conn.query_one(sql_query(sql).bind(*network_id as i64)).await {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.code() == SqlErrorCode::NotFound {
                    Ok(false)
                } else {
                    Err(vpn_err!(VpnErrorCode::IoError, "query network {} failed", network_id))
                }
            }
        }
    }

    async fn add_member(&mut self, network_id: &NetworkId, member: &NetworkMember) -> VpnResult<()> {
        let sql = r#"INSERT INTO network_member (network_id, node_id, ip, ipv6) VALUES (?, ?, ?, ?)"#;
        self.conn.execute_sql(sql_query(sql)
            .bind(*network_id as i64)
            .bind(&member.id.to_base58())
            .bind(&member.ip.to_string())
            .bind(&member.ipv6.as_ref().map(|v| v.to_string()).unwrap_or("".to_string()))).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    async fn del_member(&mut self, network_id: &NetworkId, member: &NodeId) -> VpnResult<()> {
        let sql = r#"DELETE FROM network_member WHERE network_id = ? AND node_id = ?"#;
        self.conn.execute_sql(sql_query(sql).bind(*network_id as i64).bind(&member.to_base58())).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    async fn has_member(&mut self, network_id: &NetworkId, member: &NodeId) -> VpnResult<bool> {
        let sql = r#"SELECT network_id FROM network_member WHERE network_id = ? AND node_id = ?"#;
        match self.conn.query_one(sql_query(sql).bind(*network_id as i64).bind(&member.to_base58())).await {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.code() == SqlErrorCode::NotFound {
                    Ok(false)
                } else {
                    Err(vpn_err!(VpnErrorCode::IoError, "query network member {} failed", member.to_base58()))
                }
            }
        }
    }

    async fn update_member(&mut self, network_id: &NetworkId, member: &NetworkMember) -> VpnResult<()> {
        let sql = r#"UPDATE network_member SET ip = ?, ipv6 = ? WHERE network_id = ? AND node_id = ?"#;
        self.conn.execute_sql(sql_query(sql)
            .bind(&member.ip.to_string())
            .bind(&member.ipv6.as_ref().map(|v| v.to_string()).unwrap_or("".to_string()))
            .bind(*network_id as i64)
            .bind(&member.id.to_base58())).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    async fn get_members(&mut self, network_id: &NetworkId) -> VpnResult<Vec<NetworkMember>> {
        let sql = r#"SELECT network_id, node_id, ip, ipv6 FROM network_member WHERE network_id = ?"#;
        let rows = self.conn.query_all(sql_query(sql).bind(*network_id as i64)).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let mut members = Vec::new();
        for row in rows {
            let node_id: String = row.get("node_id");
            let ip: String = row.get("ip");
            let ipv6: String = row.get("ipv6");
            members.push(NetworkMember {
                id: NodeId::from_base58(&node_id).map_err(into_vpn_err!(VpnErrorCode::IoError))?,
                ip,
                ipv6: if ipv6.is_empty() { None } else { Some(ipv6) },
            });
        }
        Ok(members)
    }

    async fn get_allowed_members(&mut self, network_id: &NetworkId) -> VpnResult<Vec<NetworkMember>> {
        let sql = r#"SELECT network_member.network_id, network_member.node_id, network_member.ip, network_member.ipv6 FROM network_member
         JOIN joined_node ON joined_node.node_id = network_member.node_id  WHERE network_member.network_id =? and joined_node.allow_join = TRUE"#;
        let rows = self.conn.query_all(sql_query(sql).bind(*network_id as i64)).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let mut members = Vec::new();
        for row in rows {
            let node_id: String = row.get("node_id");
            let ip: String = row.get("ip");
            let ipv6: String = row.get("ipv6");
            members.push(NetworkMember {
                id: NodeId::from_base58(&node_id).map_err(into_vpn_err!(VpnErrorCode::IoError))?,
                ip,
                ipv6: if ipv6.is_empty() { None } else { Some(ipv6) },
            });
        }
        Ok(members)
    }

    async fn get_member(&mut self, network_id: &NetworkId, ip_addr: &IpAddr) -> VpnResult<Option<NetworkMember>> {
        let result = match ip_addr {
            IpAddr::V4(ipv4) => {
                let sql = "SELECT network_id, node_id, ip, ipv6 FROM network_member WHERE network_id = ? AND ip = ?";
                self.conn.query_one(sql_query(sql).bind(*network_id as i64).bind(ipv4.to_string())).await
            }
            IpAddr::V6(ipv6) => {
                let sql = "SELECT network_id, node_id, ip, ipv6 FROM network_member WHERE network_id = ? AND ipv6 = ?";
                self.conn.query_one(sql_query(sql).bind(*network_id as i64).bind(ipv6.to_string())).await
            }
        };
        match result {
            Ok(row) => {
                let node_id: String = row.get("node_id");
                let ip: String = row.get("ip");
                let ipv6: String = row.get("ipv6");
                Ok(Some(NetworkMember {
                    id: NodeId::from_base58(&node_id).map_err(into_vpn_err!(VpnErrorCode::IoError))?,
                    ip,
                    ipv6: if ipv6.is_empty() { None } else { Some(ipv6) },
                }))
            }
            Err(e) => {
                if e.code() == SqlErrorCode::NotFound {
                    Ok(None)
                } else {
                    Err(vpn_err!(VpnErrorCode::IoError, "query network member failed"))
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
    network_member.ip,
    network_member.ipv6
FROM network_member
JOIN network ON network_member.network_id = network.id
JOIN joined_node ON network.group_id = joined_node.group_id AND joined_node.node_id = network_member.node_id
WHERE network_member.node_id = ? AND joined_node.allow_join = TRUE"#;

        let rows = self.conn.query_all(sql_query(sql).bind(&node_id.to_base58())).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        let mut networks = Vec::new();
        for row in rows {
            let id: NetworkId = row.get::<i64, _>("id") as u64;
            let group_id: NetworkGroupId = row.get::<i64, _>("group_id") as u64;
            let name: String = row.get("name");
            let mask: i64 = row.get("mask");
            let ipv6_mask: i64 = row.get("ipv6_mask");
            let member_ip: String = row.get("ip");
            let member_ipv6: String = row.get("ipv6");
            networks.push(NodeNetwork {
                id,
                group_id,
                name,
                ip: if member_ip.is_empty() { None } else { Some(IpAddr::V4(Ipv4Addr::from_str(&member_ip).map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?)) },
                mask: mask as u8,
                ipv6: if member_ipv6.is_empty() { None } else { Some(IpAddr::V6(Ipv6Addr::from_str(&member_ipv6).map_err(into_vpn_err!(VpnErrorCode::InvalidParam))?)) },
                ipv6_mask: ipv6_mask as u8,
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
        let pool = SqlPool::open(db_path, 300, Some(SqliteJournalMode::Wal)).await
            .map_err(into_vpn_err!(VpnErrorCode::IoError, "open sqlite db {} failed", db_path))?;
        Ok(Self {
            pool
        })
    }

    pub fn from_pool(pool: SqlPool) -> Self {
        Self {
            pool
        }
    }
}

#[async_trait::async_trait]
impl VpnStoreFactory<SqliteVpnStore> for SqliteStoreFactory {
    async fn get_vpn_store(&self) -> VpnResult<VpnStoreGuard<SqliteVpnStore>> {
        Ok(VpnStoreGuard::new(SqliteVpnStore::new(self.pool.get_conn().await.map_err(into_vpn_err!(VpnErrorCode::IoError))?)))
    }
}

pub struct P2pSnCmdServer {
    sn_service: SnServiceRef,
}

impl P2pSnCmdServer {
    pub fn new(sn_service: SnServiceRef) -> Self {
        Self {
            sn_service,
        }
    }
}

#[async_trait::async_trait]
impl CmdServer<u16, u8> for P2pSnCmdServer {
    fn register_cmd_handler(&self, cmd: u8, handler: impl CmdHandler<u16, u8>) {
        self.sn_service.get_cmd_server().register_cmd_handler(cmd, handler);
    }

    async fn send(&self, peer_id: &PeerId, cmd: u8, version: u8, body: &[u8]) -> CmdResult<()> {
        self.sn_service.get_cmd_server().send(peer_id, cmd, version, body).await
    }

    async fn send2(&self, peer_id: &PeerId, cmd: u8, version: u8, body: &[&[u8]]) -> CmdResult<()> {
        self.sn_service.get_cmd_server().send2(peer_id, cmd, version, body).await
    }

    async fn send_by_specify_tunnel(&self, peer_id: &PeerId, tunnel_id: TunnelId, cmd: u8, version: u8, body: &[u8]) -> CmdResult<()> {
        self.sn_service.get_cmd_server().send_by_specify_tunnel(peer_id, tunnel_id, cmd, version, body).await
    }

    async fn send2_by_specify_tunnel(&self, peer_id: &PeerId, tunnel_id: TunnelId, cmd: u8, version: u8, body: &[&[u8]]) -> CmdResult<()> {
        self.sn_service.get_cmd_server().send2_by_specify_tunnel(peer_id, tunnel_id, cmd, version, body).await
    }

    async fn send_by_all_tunnels(&self, peer_id: &PeerId, cmd: u8, version: u8, body: &[u8]) -> CmdResult<()> {
        self.sn_service.get_cmd_server().send_by_all_tunnels(peer_id, cmd, version, body).await
    }

    async fn send2_by_all_tunnels(&self, peer_id: &PeerId, cmd: u8, version: u8, body: &[&[u8]]) -> CmdResult<()> {
        self.sn_service.get_cmd_server().send2_by_all_tunnels(peer_id, cmd, version, body).await
    }
}

#[async_trait::async_trait]
impl VpnCmdServer for P2pSnCmdServer {
    async fn get_peer_wan_ip(&self, peer_id: &PeerId) -> VpnResult<Vec<IpAddr>> {
        let list = self.sn_service.get_peer_wan_ep(&peer_id).await.iter().map(|ep| ep.addr().ip().clone()).collect();
        Ok(list)
    }
}

pub type NodeManagerRef = Arc<NodeManager<SqliteVpnStore, SqliteStoreFactory>>;
pub type NetworkManagerRef = Arc<NetworkManager<SqliteVpnStore, SqliteStoreFactory>>;
pub type VpnServerRef = Arc<VpnServer<P2pSnCmdServer, SqliteVpnStore, SqliteStoreFactory>>;
