use std::sync::Arc;
use base58::ToBase58;
use sfo_account::{account_err, hash_data, into_account_err, Account, AccountErrorCode, AccountResult, AccountStore, DefaultAccountManager};
use serde::{Deserialize, Serialize};
use sfo_http::openapi::utoipa;
use sfo_sql::{Row};
use sfo_sql::errors::SqlErrorCode;
use sfo_sql::sqlite::{sql_query, SqlPool};
use vpn_frame::errors::{into_vpn_err, VpnErrorCode, VpnResult};
use vpn_frame::server::NetworkGroupId;
use vpn_frame::serialize_u64_as_string;
use vpn_frame::deserialize_u64_from_string;

pub type UserId = String;
#[derive(Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct User {
    pub id: String,
    #[serde(skip)]
    pub password: String,
    #[serde(serialize_with = "serialize_u64_as_string", deserialize_with = "deserialize_u64_from_string")]
    pub network_id: NetworkGroupId,
    pub server_id: String,
}

impl User {
    pub fn new(id: &str, password: &str, network_id: NetworkGroupId, server_id: &str) -> Self {
        Self { id: id.to_string(), password: password.to_string(), network_id, server_id: server_id.to_string() }
    }
}
impl Account for User {
    type Id = UserId;

    fn account_id(&self) -> &Self::Id {
        &self.id
    }

    fn account_name(&self) -> &str {
        self.id.as_str()
    }

    fn verify_password(&self, password: &str, salt: &[u8]) -> bool {
        let hash = hash_data(vec![self.password.as_bytes(), salt].as_slice()).to_base58();
        hash.as_str() == password
    }
}

pub struct SqliteUserStore {
    pool: SqlPool,
}

impl SqliteUserStore {
    pub fn new(pool: SqlPool) -> Self {
        Self { pool }
    }

    pub async fn init_user_store(&self) -> VpnResult<()> {
        let mut conn = self.pool.get_conn().await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        conn.execute_sql(sql_query("CREATE TABLE IF NOT EXISTS user (id varchar(64) PRIMARY KEY, password TEXT, network_id integer, server_id TEXT)")).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl AccountStore<User> for SqliteUserStore {
    async fn get_account(&self, account_id: &<User as Account>::Id) -> AccountResult<Option<User>> {
        let mut conn = self.pool.get_conn().await.map_err(into_account_err!(AccountErrorCode::IoError))?;
        match conn.query_one(sql_query("SELECT * FROM user WHERE id = ?").bind(account_id)).await {
            Ok(row) => {
                let id: String = row.get("id");
                let password: String = row.get("password");
                let network_id: i64 = row.get("network_id");
                let server_id: String = row.get("server_id");
                Ok(Some(User { id, password, network_id: network_id as u64, server_id }))
            }
            Err(e) => {
                if e.code() == SqlErrorCode::NotFound {
                    Ok(None)
                } else {
                    Err(account_err!(AccountErrorCode::IoError, "{}", e))
                }
            }
        }
    }

    async fn get_account_by_name(&self, account_name: &str) -> AccountResult<Option<User>> {
        let mut conn = self.pool.get_conn().await.map_err(into_account_err!(AccountErrorCode::IoError))?;
        match conn.query_one(sql_query("SELECT * FROM user WHERE id = ?").bind(account_name)).await {
            Ok(row) => {
                let id: String = row.get("id");
                let password: String = row.get("password");
                let network_id: i64 = row.get("network_id");
                let server_id: String = row.get("server_id");
                Ok(Some(User { id, password, network_id: network_id as u64, server_id }))
            }
            Err(e) => {
                if e.code() == SqlErrorCode::NotFound {
                    Ok(None)
                } else {
                    Err(account_err!(AccountErrorCode::IoError, "{}", e))
                }
            }
        }
    }

    async fn remove_account(&self, account_id: &<User as Account>::Id) -> AccountResult<()> {
        let mut conn = self.pool.get_conn().await.map_err(into_account_err!(AccountErrorCode::IoError))?;
        conn.execute_sql(sql_query("DELETE FROM user WHERE id = ?").bind(account_id)).await.map_err(into_account_err!(AccountErrorCode::IoError))?;
        Ok(())
    }

    async fn add_account(&self, account: &User) -> AccountResult<<User as Account>::Id> {
        let mut conn = self.pool.get_conn().await.map_err(into_account_err!(AccountErrorCode::IoError))?;
        conn.execute_sql(sql_query("INSERT INTO user (id, password, network_id, server_id) VALUES (?, ?, ?, ?)")
            .bind(&account.id)
            .bind(&account.password)
            .bind(account.network_id as i64)
            .bind(account.server_id.as_str())).await.map_err(into_account_err!(AccountErrorCode::IoError))?;
        Ok(account.id.clone())
    }

    async fn update_account(&self, account: &User) -> AccountResult<()> {
        let mut conn = self.pool.get_conn().await.map_err(into_account_err!(AccountErrorCode::IoError))?;
        conn.execute_sql(sql_query("UPDATE user SET password = ? WHERE id = ?").bind(&account.password).bind(&account.id)).await.map_err(into_account_err!(AccountErrorCode::IoError))?;
        Ok(())
    }
}

pub type UserManagerRef = Arc<DefaultAccountManager<User, SqliteUserStore>>;
