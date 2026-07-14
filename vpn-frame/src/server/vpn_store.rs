use crate::errors::{VpnErrorCode, VpnResult, vpn_err};
use crate::server::{NetworkId, NetworkStore, NodeId, NodeStore};
use crate::{
    NodeTrafficReport, PersistedTrafficStats, PnServerInfo, ProxyNodeApproval,
    ProxyNodeApprovalStatus, ProxyTrafficReport, ProxyTrafficReportApplyResult,
    ProxyTrafficReportResp,
};
use std::ops::{Deref, DerefMut};

#[async_trait::async_trait]
pub trait VpnStore: NetworkStore + NodeStore + PnStore {
    async fn begin_transaction(&mut self) -> VpnResult<()>;
    async fn commit_transaction(&mut self) -> VpnResult<()>;
    async fn rollback_transaction(&mut self) -> VpnResult<()>;
}

#[async_trait::async_trait]
pub trait PnStore: Send {
    async fn apply_node_traffic_report(
        &mut self,
        _pn_node_id: &NodeId,
        _report: &NodeTrafficReport,
    ) -> VpnResult<ProxyTrafficReportApplyResult> {
        Err(vpn_err!(
            VpnErrorCode::Failed,
            "node traffic report persistence is not supported by this store"
        ))
    }

    async fn add_pn_traffic_delta(
        &mut self,
        _node_id: &NodeId,
        _tx_bytes: u64,
        _rx_bytes: u64,
    ) -> VpnResult<()> {
        Err(vpn_err!(
            VpnErrorCode::Failed,
            "pn traffic persistence is not supported by this store"
        ))
    }

    async fn ensure_proxy_node_pending(&mut self, _pn_server: &PnServerInfo) -> VpnResult<()> {
        Err(vpn_err!(
            VpnErrorCode::Failed,
            "proxy node approval persistence is not supported by this store"
        ))
    }

    async fn set_proxy_node_approval(
        &mut self,
        _pn_server: &PnServerInfo,
        _status: ProxyNodeApprovalStatus,
        _comment: Option<&str>,
    ) -> VpnResult<()> {
        Err(vpn_err!(
            VpnErrorCode::Failed,
            "proxy node approval persistence is not supported by this store"
        ))
    }

    async fn is_proxy_node_approved(&mut self, _pn_server: &PnServerInfo) -> VpnResult<bool> {
        Err(vpn_err!(
            VpnErrorCode::Failed,
            "proxy node approval persistence is not supported by this store"
        ))
    }

    async fn list_proxy_node_approvals(&mut self) -> VpnResult<Vec<ProxyNodeApproval>> {
        Err(vpn_err!(
            VpnErrorCode::Failed,
            "proxy node approval persistence is not supported by this store"
        ))
    }

    async fn apply_proxy_traffic_report(
        &mut self,
        _pn_node_id: &NodeId,
        _report: &ProxyTrafficReport,
    ) -> VpnResult<ProxyTrafficReportResp> {
        Err(vpn_err!(
            VpnErrorCode::Failed,
            "proxy traffic report persistence is not supported by this store"
        ))
    }

    async fn get_proxy_traffic_total(
        &mut self,
        _network_id: &NetworkId,
        _source_id: &NodeId,
        _dest_id: &NodeId,
    ) -> VpnResult<PersistedTrafficStats> {
        Err(vpn_err!(
            VpnErrorCode::Failed,
            "proxy traffic report persistence is not supported by this store"
        ))
    }
}

pub struct VpnStoreGuard<T: VpnStore> {
    store: T,
    is_transaction: bool,
}

impl<T: VpnStore> VpnStoreGuard<T> {
    pub fn new(store: T) -> Self {
        Self {
            store,
            is_transaction: false,
        }
    }

    pub async fn begin_transaction(&mut self) -> VpnResult<()> {
        self.store.begin_transaction().await?;
        self.is_transaction = true;
        Ok(())
    }

    pub async fn commit_transaction(&mut self) -> VpnResult<()> {
        self.store.commit_transaction().await?;
        self.is_transaction = false;
        Ok(())
    }

    pub async fn rollback_transaction(&mut self) -> VpnResult<()> {
        let result = self.store.rollback_transaction().await;
        self.is_transaction = false;
        result
    }

    pub async fn finish_transaction<R>(&mut self, operation: VpnResult<R>) -> VpnResult<R> {
        match operation {
            Ok(value) => match self.commit_transaction().await {
                Ok(()) => Ok(value),
                Err(commit_err) => match self.rollback_transaction().await {
                    Ok(()) => Err(commit_err),
                    Err(rollback_err) => Err(vpn_err!(
                        VpnErrorCode::IoError,
                        "commit failed: {}; rollback failed: {}",
                        commit_err,
                        rollback_err
                    )),
                },
            },
            Err(operation_err) => match self.rollback_transaction().await {
                Ok(()) => Err(operation_err),
                Err(rollback_err) => Err(vpn_err!(
                    VpnErrorCode::IoError,
                    "transaction operation failed: {}; rollback failed: {}",
                    operation_err,
                    rollback_err
                )),
            },
        }
    }

    pub async fn with_transaction<R, F>(&mut self, operation: F) -> VpnResult<R>
    where
        F: for<'a> AsyncFnOnce(&'a mut T) -> VpnResult<R>,
    {
        self.begin_transaction().await?;
        let result = operation(&mut self.store).await;
        self.finish_transaction(result).await
    }
}

impl<T: VpnStore> Drop for VpnStoreGuard<T> {
    fn drop(&mut self) {
        if self.is_transaction {
            log::error!("vpn store guard dropped with an active transaction; underlying connection must discard or roll back the transaction");
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
