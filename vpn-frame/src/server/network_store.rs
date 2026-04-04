use crate::NodeNetwork;
use crate::errors::VpnResult;
use crate::server::node_store::NodeId;
use bucky_raw_codec::{RawDecode, RawEncode};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

pub struct JoinedNode {
    pub group_id: NetworkGroupId,
    pub node_id: NodeId,
    pub allow_join: bool,
    pub name: String,
    pub comment: String,
}

#[derive(RawDecode, RawEncode, Clone)]
pub struct NetworkMember {
    pub id: NodeId,
    pub ip: String,
    pub ipv6: Option<String>,
}

pub type NetworkGroupId = u64;
pub type NetworkId = u64;

pub struct Network {
    pub id: NetworkId,
    pub group_id: NetworkGroupId,
    pub name: String,
    pub ip_seg: Option<Ipv4Addr>,
    pub mask: u8,
    pub ipv6_seg: Option<Ipv6Addr>,
    pub ipv6_mask: u8,
}

impl Network {
    pub fn is_in_seg(&self, ip: &IpAddr) -> bool {
        match ip {
            IpAddr::V4(ip) => {
                if let Some(ip_seg) = self.ip_seg {
                    let mask = u32::MAX << (32 - self.mask);
                    let ip = u32::from(*ip);
                    let ip_seg = u32::from(ip_seg);
                    (ip & mask) == (ip_seg & mask)
                } else {
                    false
                }
            }
            IpAddr::V6(ip) => {
                if let Some(ip_seg) = self.ipv6_seg {
                    let mask = u128::MAX << (128 - self.ipv6_mask);
                    let ip = u128::from(*ip);
                    let ip_seg = u128::from(ip_seg);
                    (ip & mask) == (ip_seg & mask)
                } else {
                    false
                }
            }
        }
    }
}

#[async_trait::async_trait]
pub trait NetworkStore: 'static + Send + Sync {
    async fn add_network_group(&mut self, group_id: &NetworkGroupId) -> VpnResult<()>;
    async fn exist_network_group(&mut self, group_id: &NetworkGroupId) -> VpnResult<bool>;
    async fn has_joined(&mut self, group_id: &NetworkGroupId, node_id: &NodeId) -> VpnResult<bool>;
    async fn add_joined_node(&mut self, node: &JoinedNode) -> VpnResult<()>;
    async fn del_joined_node(
        &mut self,
        group_id: &NetworkGroupId,
        node_id: &NodeId,
    ) -> VpnResult<()>;
    async fn get_joined_node(
        &mut self,
        group_id: &NetworkGroupId,
        node_id: &NodeId,
    ) -> VpnResult<Option<JoinedNode>>;
    async fn get_joined_nodes(&mut self, group_id: &NetworkGroupId) -> VpnResult<Vec<JoinedNode>>;
    async fn update_joined_node(&mut self, node: &JoinedNode) -> VpnResult<()>;
    async fn get_joined_network_group(&mut self, node_id: &NodeId) -> VpnResult<Vec<JoinedNode>>;
    async fn get_networks(&mut self, group_id: &NetworkGroupId) -> VpnResult<Vec<Network>>;
    async fn add_network(&mut self, network: &Network) -> VpnResult<()>;
    async fn del_network(&mut self, network_id: &NetworkId) -> VpnResult<()>;
    async fn get_network(&mut self, network_id: &NetworkId) -> VpnResult<Option<Network>>;
    async fn update_network(&mut self, network: &Network) -> VpnResult<()>;
    async fn exist_network(&mut self, network_id: &NetworkId) -> VpnResult<bool>;
    async fn add_member(&mut self, network_id: &NetworkId, member: &NetworkMember)
    -> VpnResult<()>;
    async fn del_member(&mut self, network_id: &NetworkId, member: &NodeId) -> VpnResult<()>;
    async fn has_member(&mut self, network_id: &NetworkId, member: &NodeId) -> VpnResult<bool>;
    async fn update_member(
        &mut self,
        network_id: &NetworkId,
        member: &NetworkMember,
    ) -> VpnResult<()>;
    async fn get_members(&mut self, network_id: &NetworkId) -> VpnResult<Vec<NetworkMember>>;
    async fn get_allowed_members(
        &mut self,
        network_id: &NetworkId,
    ) -> VpnResult<Vec<NetworkMember>>;
    async fn get_member(
        &mut self,
        network_id: &NetworkId,
        ip_addr: &IpAddr,
    ) -> VpnResult<Option<NetworkMember>>;
    async fn get_networks_of_node(&mut self, node_id: &NodeId) -> VpnResult<Vec<NodeNetwork>>;
}
