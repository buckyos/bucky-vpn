use crate::sqlite_store_factory::SqliteStoreFactory;
use p2p_frame::error::{P2pErrorCode, P2pResult, p2p_err};
use p2p_frame::networks::ValidateResult;
use p2p_frame::pn::{PnConnectionValidateContext, PnConnectionValidator};
use std::collections::HashSet;
use std::sync::Arc;
use vpn_frame::server::{NetworkStore, NodeId, VpnStoreFactory};

pub struct SqlitePnConnectionValidator {
    store_factory: Arc<SqliteStoreFactory>,
}

impl SqlitePnConnectionValidator {
    pub fn new(store_factory: Arc<SqliteStoreFactory>) -> Arc<Self> {
        Arc::new(Self { store_factory })
    }
}

#[async_trait::async_trait]
impl PnConnectionValidator for SqlitePnConnectionValidator {
    async fn validate(&self, ctx: &PnConnectionValidateContext) -> P2pResult<ValidateResult> {
        let source_node_id = NodeId::from(ctx.from.as_slice());
        let target_node_id = NodeId::from(ctx.to.as_slice());
        let mut store = self.store_factory.get_vpn_store().await.map_err(|err| {
            p2p_err!(
                P2pErrorCode::InternalError,
                "open vpn store for pn validation failed: code={:?} msg={}",
                err.code(),
                err.msg()
            )
        })?;

        let source_groups = store
            .get_joined_network_group(&source_node_id)
            .await
            .map_err(|err| {
                p2p_err!(
                    P2pErrorCode::InternalError,
                    "query pn source groups failed: code={:?} msg={}",
                    err.code(),
                    err.msg()
                )
            })?;
        let target_groups = store
            .get_joined_network_group(&target_node_id)
            .await
            .map_err(|err| {
                p2p_err!(
                    P2pErrorCode::InternalError,
                    "query pn target groups failed: code={:?} msg={}",
                    err.code(),
                    err.msg()
                )
            })?;

        let allowed_target_groups = target_groups
            .iter()
            .filter(|joined| joined.allow_join)
            .map(|joined| joined.group_id)
            .collect::<HashSet<_>>();

        let has_common_allowed_group = source_groups.iter().any(|joined| {
            joined.allow_join && allowed_target_groups.contains(&joined.group_id)
        });

        if has_common_allowed_group {
            Ok(ValidateResult::Accept)
        } else {
            Ok(ValidateResult::Reject(format!(
                "pn connection requires source={} and target={} to share an allowed group",
                ctx.from, ctx.to
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p2p_frame::networks::TunnelPurpose;
    use p2p_frame::p2p_identity::P2pId;
    use p2p_frame::pn::PnChannelKind;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use vpn_frame::server::JoinedNode;

    async fn new_test_validator() -> (
        Arc<SqliteStoreFactory>,
        Arc<SqlitePnConnectionValidator>,
        PathBuf,
    ) {
        let db_dir = std::env::temp_dir().join(format!(
            "bucky-vpn-server-pn-validator-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&db_dir).unwrap();

        let db_path = db_dir.join("vpn.db");
        let store_factory =
            Arc::new(SqliteStoreFactory::create(db_path.to_str().unwrap()).await.unwrap());
        {
            let mut store = store_factory.get_vpn_store().await.unwrap();
            store.init_db().await.unwrap();
        }

        let validator = SqlitePnConnectionValidator::new(store_factory.clone());
        (store_factory, validator, db_dir)
    }

    async fn add_joined_node(
        store_factory: &Arc<SqliteStoreFactory>,
        group_id: u64,
        node_byte: u8,
        allow_join: bool,
    ) {
        let mut store = store_factory.get_vpn_store().await.unwrap();
        store
            .add_joined_node(&JoinedNode {
                group_id,
                node_id: NodeId::from(vec![node_byte; 32].as_slice()),
                allow_join,
                name: format!("node-{node_byte}"),
                comment: String::new(),
            })
            .await
            .unwrap();
    }

    fn new_validate_context(source_byte: u8, target_byte: u8) -> PnConnectionValidateContext {
        PnConnectionValidateContext {
            from: P2pId::from(vec![source_byte; 32]),
            to: P2pId::from(vec![target_byte; 32]),
            tunnel_id: 1u32.into(),
            kind: PnChannelKind::Stream,
            purpose: TunnelPurpose::from_value(&2000u16).unwrap(),
        }
    }

    #[tokio::test]
    async fn accept_when_nodes_share_allowed_group() {
        let (store_factory, validator, db_dir) = new_test_validator().await;
        add_joined_node(&store_factory, 1, 1, true).await;
        add_joined_node(&store_factory, 1, 2, true).await;

        let result = validator.validate(&new_validate_context(1, 2)).await.unwrap();
        assert!(matches!(result, ValidateResult::Accept));

        drop(validator);
        drop(store_factory);
        let _ = std::fs::remove_dir_all(db_dir);
    }

    #[tokio::test]
    async fn reject_when_nodes_do_not_share_group() {
        let (store_factory, validator, db_dir) = new_test_validator().await;
        add_joined_node(&store_factory, 1, 1, true).await;
        add_joined_node(&store_factory, 2, 2, true).await;

        let result = validator.validate(&new_validate_context(1, 2)).await.unwrap();
        assert!(matches!(result, ValidateResult::Reject(_)));

        drop(validator);
        drop(store_factory);
        let _ = std::fs::remove_dir_all(db_dir);
    }

    #[tokio::test]
    async fn reject_when_source_allow_join_is_false() {
        let (store_factory, validator, db_dir) = new_test_validator().await;
        add_joined_node(&store_factory, 1, 1, false).await;
        add_joined_node(&store_factory, 1, 2, true).await;

        let result = validator.validate(&new_validate_context(1, 2)).await.unwrap();
        assert!(matches!(result, ValidateResult::Reject(_)));

        drop(validator);
        drop(store_factory);
        let _ = std::fs::remove_dir_all(db_dir);
    }

    #[tokio::test]
    async fn reject_when_target_allow_join_is_false() {
        let (store_factory, validator, db_dir) = new_test_validator().await;
        add_joined_node(&store_factory, 1, 1, true).await;
        add_joined_node(&store_factory, 1, 2, false).await;

        let result = validator.validate(&new_validate_context(1, 2)).await.unwrap();
        assert!(matches!(result, ValidateResult::Reject(_)));

        drop(validator);
        drop(store_factory);
        let _ = std::fs::remove_dir_all(db_dir);
    }
}
