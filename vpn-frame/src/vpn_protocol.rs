use crate::errors::{VpnError, VpnErrorCode, VpnResult};
use crate::sequence::Sequence;
use crate::server::{NetworkGroupId, NetworkId, NetworkMember, NodeId};
use bucky_raw_codec::{RawDecode, RawEncode, RawFixedBytes};
use serde::{Deserialize, Serialize};
use sfo_cmd_server::CmdHeader;
use std::net::IpAddr;
use tokio::io::{AsyncRead, AsyncWrite};

pub trait VpnTunnelSend: AsyncWrite + Send + 'static + Unpin {
    fn is_target_tunnel(&self, target: &NodeId) -> bool;
    fn is_closed(&self) -> bool {
        false
    }
}

pub trait VpnTunnelRecv: AsyncRead + Send + 'static + Unpin {}

#[async_trait::async_trait]
pub trait VpnTunnelFactory<R: VpnTunnelRecv, S: VpnTunnelSend>: Send + Sync + 'static {
    async fn create_tunnel(
        &self,
        network_group_id: NetworkGroupId,
        network_id: NetworkId,
        node_id: &NodeId,
    ) -> VpnResult<(R, S)>;

    async fn on_vpn_info_received(&self, _vpn_infos: &[NodeVpnInfo]) -> VpnResult<()> {
        Ok(())
    }
}

#[async_trait::async_trait]
pub trait VpnTunnelListener<R: VpnTunnelRecv, S: VpnTunnelSend>: Send + Sync + 'static {
    async fn accept(&self) -> VpnResult<(R, S)>;
}

#[repr(u16)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd)]
pub enum VpnCmdCode {
    JoinNetworkGroup = 0x91,
    JoinNetworkGroupResp = 0x92,
    GetVpnInfo = 0x93,
    GetVpnInfoResp = 0x94,
    QueryNode = 0x95,
    QueryNodeResp = 0x96,
    Data = 0x97,
    ReportPnTrafficStats = 0x98,
    ReportPnTrafficStatsResp = 0x99,
    ValidatePnConnection = 0x9a,
    ValidatePnConnectionResp = 0x9b,
}

impl TryFrom<u8> for VpnCmdCode {
    type Error = VpnError;
    fn try_from(v: u8) -> std::result::Result<Self, Self::Error> {
        match v {
            0x91 => Ok(VpnCmdCode::JoinNetworkGroup),
            0x92 => Ok(VpnCmdCode::JoinNetworkGroupResp),
            0x93 => Ok(VpnCmdCode::GetVpnInfo),
            0x94 => Ok(VpnCmdCode::GetVpnInfoResp),
            0x95 => Ok(VpnCmdCode::QueryNode),
            0x96 => Ok(VpnCmdCode::QueryNodeResp),
            0x97 => Ok(VpnCmdCode::Data),
            0x98 => Ok(VpnCmdCode::ReportPnTrafficStats),
            0x99 => Ok(VpnCmdCode::ReportPnTrafficStatsResp),
            0x9a => Ok(VpnCmdCode::ValidatePnConnection),
            0x9b => Ok(VpnCmdCode::ValidatePnConnectionResp),
            _ => Err(VpnError::new(
                VpnErrorCode::InvalidParam,
                format!("invalid package command type value {}", v),
            )),
        }
    }
}

pub type VpnCmdHeader = CmdHeader<u16, u8>;
pub type VpnTunnelId = sfo_cmd_server::TunnelId;

#[derive(RawDecode, RawEncode)]
pub struct JoinNetworkGroupReq {
    pub seq: Sequence,
    pub name: Option<String>,
    pub group_id: NetworkGroupId,
}

#[derive(RawDecode, RawEncode)]
pub struct JoinNetworkGroupResp {
    pub seq: Sequence,
    pub result: u8,
}

#[derive(RawDecode, RawEncode)]
pub struct GetVpnInfoReq {
    pub seq: Sequence,
    pub info_version: Option<u16>,
    pub client_version: Option<String>,
}

fn default_endpoint_protocol() -> String {
    Endpoint::PROTOCOL_QUIC.to_string()
}

#[derive(
    Debug, RawEncode, RawDecode, Serialize, Deserialize, Clone, Eq, PartialEq, Hash, PartialOrd, Ord,
)]
pub struct Endpoint {
    #[serde(default = "default_endpoint_protocol")]
    pub protocol: String,
    pub ip: IpAddr,
    pub port: u16,
}

pub type PnServerAddress = Endpoint;

impl Endpoint {
    pub const PROTOCOL_QUIC: &'static str = "quic";
    pub const PROTOCOL_TCP: &'static str = "tcp";

    pub fn new(ip: IpAddr, port: u16) -> Self {
        Self::new_with_protocol(Self::PROTOCOL_QUIC, ip, port)
    }

    pub fn new_tcp(ip: IpAddr, port: u16) -> Self {
        Self::new_with_protocol(Self::PROTOCOL_TCP, ip, port)
    }

    pub fn new_with_protocol(protocol: impl Into<String>, ip: IpAddr, port: u16) -> Self {
        Self {
            protocol: protocol.into(),
            ip,
            port,
        }
    }
}

#[derive(
    Debug, RawEncode, RawDecode, Serialize, Deserialize, Clone, Eq, PartialEq, Hash, Default,
)]
pub struct PnServerPortMapping {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quic: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tcp: Option<u16>,
}

impl PnServerPortMapping {
    pub fn is_empty(&self) -> bool {
        self.quic.is_none() && self.tcp.is_none()
    }
}

#[derive(Debug, RawEncode, RawDecode, Serialize, Deserialize, Clone, Eq, PartialEq, Hash)]
pub struct PnServerInfo {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default)]
    pub endpoints: Vec<Endpoint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port_mapping: Option<PnServerPortMapping>,
}

impl PnServerInfo {
    pub fn new(id: String, ip: IpAddr, port: u16) -> Self {
        Self::new_with_endpoint(id, Endpoint::new(ip, port))
    }

    pub fn new_with_endpoint(id: String, endpoint: Endpoint) -> Self {
        Self::new_with_endpoints(id, vec![endpoint])
    }

    pub fn new_with_primary_address(
        id: String,
        primary: Endpoint,
        addresses: Vec<Endpoint>,
    ) -> Self {
        let mut endpoints = Vec::with_capacity(addresses.len() + 1);
        endpoints.push(primary);
        endpoints.extend(addresses);
        Self::new_with_endpoints(id, endpoints)
    }

    pub fn new_with_endpoints(id: String, endpoints: Vec<Endpoint>) -> Self {
        let mut info = Self {
            id,
            name: None,
            endpoints: Vec::new(),
            port_mapping: None,
        };
        for endpoint in endpoints {
            info.add_endpoint(endpoint);
        }
        info
    }

    pub fn new_with_addresses(id: String, ip: IpAddr, port: u16, addresses: Vec<Endpoint>) -> Self {
        let mut info = Self::new_with_endpoint(id, Endpoint::new(ip, port));
        for address in addresses {
            info.add_endpoint(address);
        }
        info
    }

    pub fn add_endpoint(&mut self, endpoint: Endpoint) {
        if !self.endpoints.contains(&endpoint) {
            self.endpoints.push(endpoint);
        }
    }

    pub fn add_address(&mut self, address: Endpoint) {
        self.add_endpoint(address);
    }

    pub fn primary_endpoint(&self) -> Option<&Endpoint> {
        self.endpoints.first()
    }

    pub fn with_name(mut self, name: Option<String>) -> Self {
        self.name = name.and_then(|name| {
            let name = name.trim().to_owned();
            if name.is_empty() { None } else { Some(name) }
        });
        self
    }

    pub fn with_port_mapping(mut self, port_mapping: Option<PnServerPortMapping>) -> Self {
        self.port_mapping = port_mapping.filter(|mapping| !mapping.is_empty());
        self
    }

    pub fn remote_name(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.id)
    }
}

#[derive(RawEncode, RawDecode, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct NodeNetwork {
    pub id: NetworkId,
    pub group_id: NetworkGroupId,
    pub name: String,
    pub ip: Option<IpAddr>,
    pub mask: u8,
    pub ipv6: Option<IpAddr>,
    pub ipv6_mask: u8,
    pub pn_server: Option<PnServerInfo>,
}

#[derive(RawDecode, RawEncode)]
pub struct NodeVpnInfo {
    pub node_info: NodeNetwork,
    pub members: Vec<NetworkMember>,
}

#[derive(RawDecode, RawEncode)]
pub struct GetVpnInfoResp {
    pub seq: Sequence,
    pub result: u8,
    pub info_version: u16,
    pub vpn_list: Vec<NodeVpnInfo>,
}

#[derive(RawDecode, RawEncode)]
pub struct DataHeader {
    pub network_id: NetworkId,
    pub pkg_len: u16,
}

impl RawFixedBytes for DataHeader {
    fn raw_bytes() -> Option<usize> {
        Some(NetworkId::raw_bytes().unwrap() + u16::raw_bytes().unwrap())
    }
}

#[derive(RawDecode, RawEncode)]
pub struct QueryNodeReq {
    pub seq: Sequence,
    pub group_id: NetworkGroupId,
    pub network_id: NetworkId,
    pub ip: IpAddr,
}

#[derive(RawDecode, RawEncode)]
pub struct QueryNodeResp {
    pub seq: Sequence,
    pub node_id: Option<NodeId>,
}

#[derive(RawDecode, RawEncode)]
pub struct ReportPnTrafficStatsReq {
    pub seq: Sequence,
    pub node_id: NodeId,
    pub pn_server: Option<PnServerInfo>,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
}

#[derive(RawDecode, RawEncode)]
pub struct ReportPnTrafficStatsResp {
    pub seq: Sequence,
    pub result: u8,
}

#[derive(RawDecode, RawEncode)]
pub struct ValidatePnConnectionReq {
    pub seq: Sequence,
    pub from: NodeId,
    pub to: NodeId,
}

#[derive(RawDecode, RawEncode)]
pub struct ValidatePnConnectionResp {
    pub seq: Sequence,
    pub result: u8,
    pub allowed: bool,
}

#[cfg(test)]
mod tests {
    use super::{PnServerAddress, PnServerInfo, PnServerPortMapping};
    use std::net::IpAddr;

    #[test]
    fn pn_server_info_carries_node_id_and_ipv4_address() {
        let endpoint = PnServerAddress::new(IpAddr::from([127, 0, 0, 1]), 3624);
        let pn_server = PnServerInfo::new(
            "server-node-id".to_string(),
            IpAddr::from([127, 0, 0, 1]),
            3624,
        );

        assert_eq!(pn_server.id, "server-node-id");
        assert_eq!(pn_server.primary_endpoint(), Some(&endpoint));
        assert_eq!(pn_server.endpoints, vec![endpoint]);
    }

    #[test]
    fn pn_server_info_carries_node_id_and_ipv6_address() {
        let pn_server = PnServerInfo::new(
            "server-node-id".to_string(),
            "::1".parse::<IpAddr>().unwrap(),
            3624,
        );

        assert_eq!(pn_server.id, "server-node-id");
        assert_eq!(
            pn_server.primary_endpoint(),
            Some(&PnServerAddress::new(
                "::1".parse::<IpAddr>().unwrap(),
                3624
            ))
        );
    }

    #[test]
    fn pn_server_info_normalizes_optional_remote_name() {
        let named = PnServerInfo::new(
            "server-node-id".to_string(),
            IpAddr::from([127, 0, 0, 1]),
            3624,
        )
        .with_name(Some(" proxy-a ".to_string()));

        assert_eq!(named.name.as_deref(), Some("proxy-a"));
        assert_eq!(named.remote_name(), "proxy-a");

        let unnamed = PnServerInfo::new(
            "server-node-id".to_string(),
            IpAddr::from([127, 0, 0, 1]),
            3624,
        )
        .with_name(Some("  ".to_string()));

        assert_eq!(unnamed.name, None);
        assert_eq!(unnamed.remote_name(), "server-node-id");
    }

    #[test]
    fn pn_server_info_carries_port_mapping_without_rewriting_endpoint_port() {
        let pn_server = PnServerInfo::new(
            "server-node-id".to_string(),
            IpAddr::from([127, 0, 0, 1]),
            3624,
        )
        .with_port_mapping(Some(PnServerPortMapping {
            quic: Some(443),
            tcp: None,
        }));

        assert_eq!(
            pn_server.primary_endpoint(),
            Some(&PnServerAddress::new(IpAddr::from([127, 0, 0, 1]), 3624))
        );
        assert_eq!(
            pn_server.port_mapping,
            Some(PnServerPortMapping {
                quic: Some(443),
                tcp: None,
            })
        );
    }
}
