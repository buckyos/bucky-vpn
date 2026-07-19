use crate::PnServerInfo;
use crate::pn_server_info::PnServerEndpoint;
use crate::server::{NetworkGroupId, NetworkId, NodeId};
use bucky_raw_codec::{RawDecode, RawEncode};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub type ReportedPnServerInfo = PnServerInfo;

/// Receiver-compatible upper bound for records in one traffic command.
pub const MAX_TRAFFIC_RECORDS_PER_COMMAND: usize = 256;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ValidatedPnConnection {
    pub network_id: NetworkId,
}

pub fn select_first_network_id<I>(network_ids: I) -> Option<NetworkId>
where
    I: IntoIterator<Item = NetworkId>,
{
    network_ids.into_iter().min()
}

pub fn select_first_eligible_pn_network(
    source_networks: &[crate::NodeNetwork],
    target_network_ids: &HashSet<NetworkId>,
    allowed_groups: &HashSet<NetworkGroupId>,
    pn_node_id: Option<&NodeId>,
) -> Option<NetworkId> {
    select_first_network_id(source_networks.iter().filter_map(|network| {
        (allowed_groups.contains(&network.group_id)
            && target_network_ids.contains(&network.id)
            && pn_node_id.is_none_or(|pn_node_id| {
                network
                    .pn_server
                    .as_ref()
                    .is_some_and(|pn_server| pn_server.proxy_id == *pn_node_id)
            }))
        .then_some(network.id)
    }))
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, RawEncode, RawDecode)]
pub struct ProxyNodeHeartbeatId(pub String);

#[derive(Debug, Clone, Eq, PartialEq, RawEncode, RawDecode)]
pub struct ProxyNodeHeartbeat {
    pub heartbeat_id: ProxyNodeHeartbeatId,
    pub pn_server: Option<ReportedPnServerInfo>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, RawEncode, RawDecode)]
pub struct ProxyTrafficReportId(pub String);

#[derive(Debug, Clone, Eq, PartialEq, Hash, RawEncode, RawDecode)]
pub struct NodeTrafficReportId(pub String);

#[derive(Debug, Clone, Eq, PartialEq, RawEncode, RawDecode)]
pub struct NodeTrafficDelta {
    pub node_id: NodeId,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
    pub tx_speed: u64,
    pub rx_speed: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, RawEncode, RawDecode)]
pub struct NodeTrafficReport {
    pub report_id: NodeTrafficReportId,
    pub started_at_ms: u64,
    pub ended_at_ms: u64,
    pub delta: NodeTrafficDelta,
}

#[derive(Debug, Clone, Eq, PartialEq, RawEncode, RawDecode)]
pub struct NodeTrafficReportResp {
    pub report_id: NodeTrafficReportId,
    pub result: ProxyTrafficReportApplyResult,
    pub error_code: Option<u8>,
}

#[derive(Debug, Clone, Eq, PartialEq, RawEncode, RawDecode)]
pub struct ProxyTrafficReport {
    pub report_id: ProxyTrafficReportId,
    pub started_at_ms: u64,
    pub ended_at_ms: u64,
    pub traffic_sample: PnTrafficSample,
}

#[derive(Debug, Clone, Eq, PartialEq, RawEncode, RawDecode)]
pub struct ProxyTrafficReportResp {
    pub report_id: ProxyTrafficReportId,
    pub result: ProxyTrafficReportApplyResult,
    pub error_code: Option<u8>,
    pub remaining: Vec<UserRemainingTraffic>,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, RawEncode, RawDecode)]
pub enum ProxyTrafficReportApplyResult {
    Applied = 0,
    Duplicate = 1,
    Rejected = 2,
    Retryable = 3,
}

#[derive(Debug, Clone, Eq, PartialEq, RawEncode, RawDecode)]
pub struct UserRemainingTraffic {
    pub user_id: String,
    pub remaining_bytes: Option<u64>,
}

#[derive(Debug, Clone, Eq, PartialEq, RawEncode, RawDecode)]
pub struct PnTrafficSample {
    pub network_id: NetworkId,
    pub source_id: NodeId,
    pub dest_id: NodeId,
    pub source_to_dest: PnTrafficDirectionSample,
    pub dest_to_source: PnTrafficDirectionSample,
}

#[derive(Debug, Clone, Eq, PartialEq, RawEncode, RawDecode)]
pub struct PnTrafficDirectionSample {
    pub bytes: u64,
    pub speed_bytes_per_sec: u64,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ProxyNodeApprovalStatus {
    Pending,
    Approved,
    Rejected,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProxyNodeApproval {
    pub pn_node_id: NodeId,
    pub status: ProxyNodeApprovalStatus,
    pub updated_at: u64,
    pub comment: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProxyNodeState {
    pub pn_server: PnServerInfo,
    pub status: ProxyNodeApprovalStatus,
    pub live: bool,
    pub updated_at: u64,
    pub comment: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, RawEncode, RawDecode, Serialize, Deserialize)]
pub struct ClientProxyNodeInfo {
    #[serde(
        serialize_with = "serialize_proxy_node_id",
        deserialize_with = "deserialize_proxy_node_id"
    )]
    pub proxy_id: NodeId,
    pub name: Option<String>,
    pub endpoints: Vec<PnServerEndpoint>,
}

fn serialize_proxy_node_id<S>(node_id: &NodeId, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&node_id.to_base36())
}

fn deserialize_proxy_node_id<'de, D>(deserializer: D) -> Result<NodeId, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    NodeId::from_base36_or_base58(&value).map_err(serde::de::Error::custom)
}

#[derive(Debug, Clone, Eq, PartialEq, RawEncode, RawDecode)]
pub struct NodeNetworkPnInfo {
    pub network_id: NetworkId,
    pub proxy: Option<ClientProxyNodeInfo>,
}

#[derive(Debug, Clone, Eq, PartialEq, RawEncode, RawDecode)]
pub struct NodePnInfoState {
    pub node_id: NodeId,
    pub version: u16,
    pub networks: Vec<NodeNetworkPnInfo>,
}

#[derive(Debug, Clone, Eq, PartialEq, RawEncode, RawDecode)]
pub struct ClientProxyNodeAssignments {
    pub node_id: NodeId,
    pub version: u16,
    pub networks: Vec<NodeNetworkPnInfo>,
}

pub type ClientPnServerInfo = ClientProxyNodeInfo;

#[derive(Debug, Clone, Eq, PartialEq, RawEncode, RawDecode)]
pub struct PnTrafficSnapshot {
    pub network_id: NetworkId,
    pub source_id: NodeId,
    pub dest_id: NodeId,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
    pub tx_speed: u64,
    pub rx_speed: u64,
}

#[derive(Debug, Clone, Eq, PartialEq, RawEncode, RawDecode)]
pub struct PersistedTrafficStats {
    pub network_id: NetworkId,
    pub source_id: NodeId,
    pub dest_id: NodeId,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AppliedProxyTrafficReport {
    pub pn_node_id: NodeId,
    pub report_id: ProxyTrafficReportId,
    pub started_at_ms: u64,
    pub ended_at_ms: u64,
    pub applied_at_ms: u64,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ProxyNodeObserved {
    pub pn_node_id: NodeId,
    pub observed_endpoint: Option<PnServerEndpoint>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RemoteProxyNodeState {
    pub reported: Option<ReportedPnServerInfo>,
    pub observed: Option<ProxyNodeObserved>,
    pub current: Option<ClientPnServerInfo>,
    pub last_seen_ms: u64,
}
