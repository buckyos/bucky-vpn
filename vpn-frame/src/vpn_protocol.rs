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

#[derive(Debug, RawEncode, RawDecode, Serialize, Deserialize, Clone, Eq, PartialEq, Hash)]
pub struct PnServerInfo {
    pub id: String,
    pub ip: IpAddr,
    pub port: u16,
}

impl PnServerInfo {
    pub fn new(id: String, ip: IpAddr, port: u16) -> Self {
        Self { id, ip, port }
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
    use super::PnServerInfo;
    use std::net::IpAddr;

    #[test]
    fn pn_server_info_carries_node_id_and_ipv4_address() {
        let pn_server = PnServerInfo::new(
            "server-node-id".to_string(),
            IpAddr::from([127, 0, 0, 1]),
            3624,
        );

        assert_eq!(pn_server.id, "server-node-id");
        assert_eq!(pn_server.ip, IpAddr::from([127, 0, 0, 1]));
        assert_eq!(pn_server.port, 3624);
    }

    #[test]
    fn pn_server_info_carries_node_id_and_ipv6_address() {
        let pn_server = PnServerInfo::new(
            "server-node-id".to_string(),
            "::1".parse::<IpAddr>().unwrap(),
            3624,
        );

        assert_eq!(pn_server.id, "server-node-id");
        assert_eq!(pn_server.ip, "::1".parse::<IpAddr>().unwrap());
        assert_eq!(pn_server.port, 3624);
    }
}
