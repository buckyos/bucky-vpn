use base58::{FromBase58, ToBase58};
use bucky_raw_codec::{RawDecode, RawEncode};
use crate::errors::{vpn_err, VpnErrorCode, VpnResult};

#[derive(Debug, Clone, Eq, PartialEq, Hash, RawEncode, RawDecode)]
pub struct NodeId(Vec<u8>);
impl NodeId {
    pub fn to_base58(&self) -> String {
        self.0.as_slice().to_base58()
    }

    pub fn from_base58(base58: &str) -> VpnResult<Self> {
        Ok(Self(base58.from_base58().map_err(|_e|vpn_err!(VpnErrorCode::InvalidParam, "invalid node id {}", base58))?))
    }

    pub fn to_base36(&self) -> String {
        base36::encode(self.0.as_slice())
    }

    pub fn from_base36(base36: &str) -> VpnResult<Self> {
        Ok(Self(base36::decode(base36).map_err(|_e|vpn_err!(VpnErrorCode::InvalidParam, "invalid node id {}", base36))?))
    }

    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }
}

impl From<&[u8]> for NodeId {
    fn from(key: &[u8]) -> Self {
        Self(key.to_vec())
    }
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: NodeId,
    pub info_version: u64,
}

#[async_trait::async_trait]
pub trait NodeStore: 'static + Send + Sync {
    async fn add_node(&mut self, node: &Node) -> VpnResult<()>;
    async fn remove_node(&mut self, id: &NodeId) -> VpnResult<()>;
    async fn get_node(&mut self, id: &NodeId) -> VpnResult<Option<Node>>;
    async fn exist_node(&mut self, id: &NodeId) -> VpnResult<bool>;
    async fn inc_info_version(&mut self, id: &NodeId) -> VpnResult<()>;
}
