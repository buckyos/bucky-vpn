use std::ops::{Deref, DerefMut};
use crate::errors::VpnResult;
use crate::server::{NetworkStore, NodeStore};

#[async_trait::async_trait]
pub trait VpnStore: NetworkStore + NodeStore {
    async fn begin_transaction(&mut self) -> VpnResult<()>;
    async fn commit_transaction(&mut self) -> VpnResult<()>;
    async fn rollback_transaction(&mut self) -> VpnResult<()>;
}

pub struct VpnStoreGuard<T: VpnStore> {
    store: T,
    is_transaction: bool,
}

impl<T: VpnStore> VpnStoreGuard<T> {
    pub fn new(store: T) -> Self {
        Self { store, is_transaction: false }
    }

    async fn begin_transaction(&mut self) -> VpnResult<()> {
        self.store.begin_transaction().await?;
        self.is_transaction = true;
        Ok(())
    }

    async fn commit_transaction(&mut self) -> VpnResult<()> {
        self.store.commit_transaction().await?;
        self.is_transaction = false;
        Ok(())
    }
}

impl<T: VpnStore> Drop for VpnStoreGuard<T> {
    fn drop(&mut self) {
        if self.is_transaction {
            let _ = tokio::runtime::Handle::current().block_on(self.rollback_transaction());
        }
    }
}

impl<T: VpnStore> Deref for VpnStoreGuard<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.store
    }
}

impl<T: VpnStore> DerefMut for VpnStoreGuard<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.store
    }
}

#[async_trait::async_trait]
pub trait VpnStoreFactory<T: VpnStore>: Send + Sync + 'static {
    async fn get_vpn_store(&self) -> VpnResult<VpnStoreGuard<T>>;
}
