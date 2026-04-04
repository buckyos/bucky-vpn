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
}

pub trait VpnTunnelRecv: AsyncRead + Send + 'static + Unpin {}

#[async_trait::async_trait]
pub trait VpnTunnelFactory<R: VpnTunnelRecv, S: VpnTunnelSend>: Send + Sync + 'static {
    async fn create_tunnel(&self, node_id: &NodeId) -> VpnResult<(R, S)>;
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

#[derive(RawEncode, RawDecode, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct NodeNetwork {
    pub id: NetworkId,
    pub group_id: NetworkGroupId,
    pub name: String,
    pub ip: Option<IpAddr>,
    pub mask: u8,
    pub ipv6: Option<IpAddr>,
    pub ipv6_mask: u8,
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
