use crate::errors::{VpnError, VpnErrorCode, VpnResult};
use crate::proxy_node::{
    ClientProxyNodeInfo, NodeTrafficReport, NodeTrafficReportResp, ProxyNodeHeartbeat,
    ProxyTrafficReport, ProxyTrafficReportResp,
};
use crate::sequence::Sequence;
use crate::server::{NetworkGroupId, NetworkId, NetworkMember, NodeId};
use bucky_raw_codec::{RawDecode, RawEncode, RawFixedBytes};
use serde::{Deserialize, Serialize};
use sfo_cmd_server::{CmdHeader, U16};
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
    ReportProxyTraffic = 0x9c,
    ReportProxyTrafficResp = 0x9d,
    ReportProxyHeartbeat = 0x9e,
    ReportProxyHeartbeatResp = 0x9f,
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
            0x9c => Ok(VpnCmdCode::ReportProxyTraffic),
            0x9d => Ok(VpnCmdCode::ReportProxyTrafficResp),
            0x9e => Ok(VpnCmdCode::ReportProxyHeartbeat),
            0x9f => Ok(VpnCmdCode::ReportProxyHeartbeatResp),
            _ => Err(VpnError::new(
                VpnErrorCode::InvalidParam,
                format!("invalid package command type value {}", v),
            )),
        }
    }
}

pub type VpnCmdPkgLen = U16;
pub type VpnCmdHeader = CmdHeader<VpnCmdPkgLen, u8>;
pub type VpnTunnelId = sfo_cmd_server::TunnelId;
pub const VPN_CMD_VERSION: u8 = 1;

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
    pub pn_info_version: Option<u32>,
    pub client_version: Option<String>,
}

#[derive(Debug, RawEncode, RawDecode, Serialize, Deserialize, Clone, Eq, PartialEq, Hash)]
pub struct PnServerInfo {
    pub id: String,
    pub info: Vec<u8>,
}

impl PnServerInfo {
    pub fn new(id: String, info: Vec<u8>) -> Self {
        Self { id, info }
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
    pub pn_server: Option<ClientProxyNodeInfo>,
}

#[derive(RawDecode, RawEncode)]
pub struct NodeVpnInfo {
    pub node_info: NodeNetwork,
    pub members: Vec<NetworkMember>,
    pub pn_server_changed: bool,
}

#[derive(RawDecode, RawEncode)]
pub struct GetVpnInfoResp {
    pub seq: Sequence,
    pub result: u8,
    pub info_version: u16,
    pub pn_info_version: u32,
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
    pub reports: Vec<NodeTrafficReport>,
}

#[derive(RawDecode, RawEncode)]
pub struct ReportPnTrafficStatsResp {
    pub seq: Sequence,
    pub result: u8,
    pub reports: Vec<NodeTrafficReportResp>,
}

#[derive(RawDecode, RawEncode)]
pub struct ReportProxyHeartbeatReq {
    pub seq: Sequence,
    pub heartbeat: ProxyNodeHeartbeat,
}

#[derive(RawDecode, RawEncode)]
pub struct ReportProxyHeartbeatResp {
    pub seq: Sequence,
    pub result: u8,
}

#[derive(RawDecode, RawEncode)]
pub struct ReportProxyTrafficReq {
    pub seq: Sequence,
    pub reports: Vec<ProxyTrafficReport>,
}

#[derive(RawDecode, RawEncode)]
pub struct ReportProxyTrafficResp {
    pub seq: Sequence,
    pub result: u8,
    pub reports: Vec<ProxyTrafficReportResp>,
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
    pub network_id: Option<NetworkId>,
}

impl ValidatePnConnectionResp {
    pub fn validated_connection(&self) -> VpnResult<Option<crate::ValidatedPnConnection>> {
        if !self.allowed {
            return Ok(None);
        }
        self.network_id
            .map(|network_id| Some(crate::ValidatedPnConnection { network_id }))
            .ok_or_else(|| {
                crate::errors::vpn_err!(
                    VpnErrorCode::InvalidParam,
                    "allowed pn connection response is missing network_id"
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::PnServerInfo;
    use crate::ClientProxyNodeInfo;
    use crate::server::NodeId;

    #[test]
    fn pn_server_info_carries_opaque_info() {
        let pn_server = PnServerInfo::new("server-node-id".to_string(), vec![1, 2, 3]);
        assert_eq!(pn_server.id, "server-node-id");
        assert_eq!(pn_server.info, vec![1, 2, 3]);
    }

    #[test]
    fn client_proxy_node_info_serializes_as_typed_client_contract() {
        let proxy = ClientProxyNodeInfo {
            proxy_id: NodeId::from(vec![7u8; 32].as_slice()),
            name: Some("proxy-7".to_string()),
            endpoints: Vec::new(),
        };

        let json = serde_json::to_value(&proxy).unwrap();
        assert_eq!(json["proxy_id"], proxy.proxy_id.to_base36());
        assert_eq!(json["name"], "proxy-7");
        assert!(json.get("info").is_none());
        assert_eq!(
            serde_json::from_value::<ClientProxyNodeInfo>(json).unwrap(),
            proxy
        );
    }
}
