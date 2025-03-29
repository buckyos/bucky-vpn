use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use async_named_locker::Locker;
use crate::errors::{vpn_err, VpnErrorCode, VpnResult};
use crate::server::{JoinedNode, Network, NetworkGroupId, NetworkId, NetworkMember, Node, NodeId, VpnStore, VpnStoreFactory};
use crate::{NodeNetwork};

pub struct NetworkManager<S: VpnStore, F: VpnStoreFactory<S>> {
    store_factory: Arc<F>,
    _p: std::marker::PhantomData<S>,
}

impl <S: VpnStore, F: VpnStoreFactory<S>> NetworkManager<S, F> {
    pub fn new(store_factory: Arc<F>) -> Arc<Self> {
        Arc::new(Self { store_factory, _p: std::marker::PhantomData })
    }

    pub async fn new_network_group(&self) -> VpnResult<NetworkGroupId> {
        let mut store = self.store_factory.get_vpn_store().await?;
        loop {
            let id = rand::random::<u64>();
            if !store.exist_network_group(&id).await? {
                store.add_network_group(&id).await?;
                return Ok(id);
            }
        }
    }

    pub async fn new_network(&self, group_id: &NetworkGroupId) -> VpnResult<Network> {
        let _lock = Locker::get_locker("new_network").await;
        let mut store = self.store_factory.get_vpn_store().await?;
        loop {
            let id = rand::random::<u64>();
            if !store.exist_network(&id).await? {
                let network = Network {
                    id,
                    group_id: *group_id,
                    name: "".to_string(),
                    ip_seg: None,
                    mask: 0,
                    ipv6_seg: None,
                    ipv6_mask: 0,
                };
                store.add_network(&network).await?;
                return Ok(network);
            }
        }
    }

    pub async fn get_network(&self, network_id: &NetworkId) -> VpnResult<Option<Network>> {
        let mut store = self.store_factory.get_vpn_store().await?;
        store.get_network(network_id).await
    }

    pub async fn update_network(&self, network: &Network) -> VpnResult<()> {
        let mut store = self.store_factory.get_vpn_store().await?;
        store.begin_transaction().await?;
        let cur_network = store.get_network(&network.id).await?;
        if let Some(old_network) = cur_network {
            let members = store.get_members(&network.id).await?;
            for member in members {
                let mut new_member = member.clone();
                let mut has_change = false;
                if old_network.ip_seg != network.ip_seg || old_network.mask != network.mask {
                    has_change = true;
                    if let Some(seg_ip) = network.ip_seg {
                        match member.ip.parse::<Ipv4Addr>() {
                            Ok(ip) => {
                                let mask = u32::MAX << (32 - network.mask);
                                let mut ip = u32::from(ip);
                                let ip_seg = u32::from(seg_ip);
                                ip = (ip & !mask) | (ip_seg & mask);
                                new_member.ip = Ipv4Addr::from(ip).to_string();
                            },
                            Err(_) => {
                                new_member.ip = "".to_string();
                            }
                        }
                    } else {
                        new_member.ip = "".to_string();
                    }
                }

                if old_network.ipv6_seg != network.ipv6_seg || old_network.ipv6_mask != network.ipv6_mask {
                    has_change = true;
                    if let Some(seg_ip) = network.ipv6_seg {
                        if let Some(ipv6) = member.ipv6 {
                            match ipv6.parse::<Ipv6Addr>() {
                                Ok(ip) => {
                                    let mask = u128::MAX << (128 - network.ipv6_mask);
                                    let mut ip = u128::from(ip);
                                    let ip_seg = u128::from(seg_ip);
                                    ip = (ip & !mask) | (ip_seg & mask);
                                    new_member.ipv6 = Some(Ipv6Addr::from(ip).to_string());
                                },
                                Err(_) => {
                                    new_member.ipv6 = None;
                                }
                            }
                        }
                    } else {
                        new_member.ipv6 = None;
                    }
                }
                if has_change {
                    store.update_member(&network.id, &new_member).await?;
                    store.inc_info_version(&member.id).await?;
                }
            }
        }

        store.update_network(&network).await?;
        store.commit_transaction().await?;
        Ok(())
    }

    pub async fn del_network(&self, network_id: &NetworkId) -> VpnResult<()> {
        let mut store = self.store_factory.get_vpn_store().await?;
        store.begin_transaction().await?;
        let members = store.get_members(network_id).await?;
        for member in members {
            store.del_member(network_id, &member.id).await?;
            store.inc_info_version(&member.id).await?;
        }
        store.del_network(network_id).await?;
        store.commit_transaction().await?;
        Ok(())
    }

    pub async fn exist_network_group(&self, network_group_id: &NetworkGroupId) -> VpnResult<bool> {
        let mut store = self.store_factory.get_vpn_store().await?;
        store.exist_network_group(network_group_id).await
    }

    pub async fn get_networks_of_group(&self, network_group_id: &NetworkGroupId) -> VpnResult<Vec<Network>> {
        let mut store = self.store_factory.get_vpn_store().await?;
        store.get_networks(network_group_id).await
    }

    pub async fn get_networks_of_node(&self, node_id: &NodeId) -> VpnResult<Vec<NodeNetwork>> {
        let mut store = self.store_factory.get_vpn_store().await?;
        store.get_networks_of_node(node_id).await
    }

    pub async fn has_joined(&self, network_group_id: &NetworkGroupId, node_id: &NodeId) -> VpnResult<bool> {
        let mut store = self.store_factory.get_vpn_store().await?;
        store.has_joined(network_group_id, node_id).await
    }

    pub async fn add_joined_node(&self, network_group_id: &NetworkGroupId, node_id: &NodeId, name: Option<String>) -> VpnResult<()> {
        let mut store = self.store_factory.get_vpn_store().await?;
        store.begin_transaction().await?;
        if !store.exist_node(&node_id).await? {
            store.add_node(&Node {
                id: node_id.clone(),
                info_version: 0,
            }).await?;
        }
        store.add_joined_node(&JoinedNode {
            group_id: *network_group_id,
            node_id: node_id.clone(),
            allow_join: false,
            name: name.unwrap_or_else(|| "".to_string()),
            comment: "".to_string()
        }).await?;
        store.commit_transaction().await?;
        Ok(())
    }

    pub async fn update_allow_join(&self, network_group_id: &NetworkGroupId, node_id: &NodeId, allow_join: bool) -> VpnResult<()> {
        let mut store = self.store_factory.get_vpn_store().await?;
        let joined_node = store.get_joined_node(network_group_id, node_id).await?;
        if joined_node.is_none() {
            return Err(vpn_err!(VpnErrorCode::NotFoundNode, "can't find joined node {}", node_id.to_base58()));
        }

        store.begin_transaction().await?;
        let mut joined_node = joined_node.unwrap();
        joined_node.allow_join = allow_join;
        store.update_joined_node(&joined_node).await?;
        store.inc_info_version(node_id).await?;
        store.commit_transaction().await?;
        Ok(())
    }

    pub async fn del_joined_node(&self, network_group_id: &NetworkGroupId, node_id: &NodeId) -> VpnResult<()> {
        let mut store = self.store_factory.get_vpn_store().await?;
        store.begin_transaction().await?;
        let networks = store.get_networks(network_group_id).await?;
        for network in networks {
            let members = store.get_members(&network.id).await?;
            if members.iter().find(|member| member.id == *node_id).is_some() {
                for member in members.iter() {
                    store.inc_info_version(&member.id).await?;
                }
                store.del_member(&network.id, node_id).await?;
            }
        }
        store.del_joined_node(network_group_id, node_id).await?;
        store.commit_transaction().await?;
        Ok(())
    }

    pub async fn update_joined_node_comment(&self, network_group_id: &NetworkGroupId, node_id: &NodeId, comment: String) -> VpnResult<()> {
        let mut store = self.store_factory.get_vpn_store().await?;
        let joined_node = store.get_joined_node(network_group_id, node_id).await?;
        if joined_node.is_none() {
            return Err(vpn_err!(VpnErrorCode::NotFoundNode, "can't find node {}", node_id.to_base58()));
        }

        let mut joined_node = joined_node.unwrap();
        joined_node.comment = comment;
        store.update_joined_node(&joined_node).await
    }

    pub async fn get_joint_nodes(&self, network_group_id: &NetworkGroupId) -> VpnResult<Vec<JoinedNode>> {
        let mut store = self.store_factory.get_vpn_store().await?;
        store.get_joined_nodes(network_group_id).await
    }

    pub async fn add_network_member(&self, network_id: &NetworkId, member: &NodeId, ip: String, ipv6: Option<String>) -> VpnResult<()> {
        let mut store = self.store_factory.get_vpn_store().await?;
        if !store.has_member(network_id, member).await? {
            store.begin_transaction().await?;
            let members = store.get_members(network_id).await?;
            for member in members {
                store.inc_info_version(&member.id).await?;
            }
            store.add_member(network_id, &NetworkMember {
                id: member.clone(),
                ip,
                ipv6
            }).await?;
            store.inc_info_version(member).await?;
            store.commit_transaction().await?;
        }

        Ok(())
    }

    pub async fn del_network_member(&self, network_id: &NetworkId, member: &NodeId) -> VpnResult<()> {
        let mut store = self.store_factory.get_vpn_store().await?;
        store.begin_transaction().await?;
        store.del_member(network_id, member).await?;
        store.inc_info_version(member).await?;
        let members = store.get_members(network_id).await?;
        for member in members {
            store.inc_info_version(&member.id).await?;
        }
        store.commit_transaction().await?;
        Ok(())
    }

    pub async fn update_network_member(&self, network_id: &NetworkId, node_id: NodeId, ip: String, ip_v6: Option<String>) -> VpnResult<()> {
        let mut store = self.store_factory.get_vpn_store().await?;
        store.begin_transaction().await?;
        store.update_member(network_id, &NetworkMember {
            id: node_id,
            ip,
            ipv6: ip_v6,
        }).await?;

        let members = store.get_members(network_id).await?;
        for member in members {
            store.inc_info_version(&member.id).await?;
        }
        store.commit_transaction().await?;
        Ok(())
    }

    pub async fn get_network_member(&self, network_id: &NetworkId) -> VpnResult<Vec<NetworkMember>> {
        let mut store = self.store_factory.get_vpn_store().await?;
        store.get_members(network_id).await
    }

    pub async fn get_member_of_ip(&self, _group_id: &NetworkGroupId, network_id: &NetworkId, ip_addr: &IpAddr) -> VpnResult<Option<NodeId>> {
        let mut store = self.store_factory.get_vpn_store().await?;
        store.get_member(network_id, ip_addr).await.map(|v| v.map(|m| m.id))
    }
}
