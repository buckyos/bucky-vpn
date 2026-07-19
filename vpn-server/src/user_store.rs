use base58::ToBase58;
use serde::{Deserialize, Serialize};
use sfo_account::{
    Account, AccountErrorCode, AccountResult, AccountStore, DefaultAccountManager, account_err,
    hash_data, into_account_err,
};
use sfo_http::openapi::utoipa;
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::sqlx_store::SqliteConnection;
use vpn_frame::deserialize_u64_from_string;
use vpn_frame::errors::{VpnErrorCode, VpnResult, into_vpn_err};
use vpn_frame::serialize_u64_as_string;
use vpn_frame::server::NetworkGroupId;

pub type UserId = String;
#[derive(Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct User {
    pub id: String,
    #[serde(skip)]
    pub password: String,
    #[serde(
        serialize_with = "serialize_u64_as_string",
        deserialize_with = "deserialize_u64_from_string"
    )]
    pub network_id: NetworkGroupId,
    pub server_id: String,
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
    pool: SqlitePool,
    password_cache: Mutex<HashMap<String, String>>,
}

impl SqliteUserStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            password_cache: Mutex::new(Default::default()),
        }
    }

    pub async fn init_user_store(&self) -> VpnResult<()> {
        let mut conn = SqliteConnection::acquire(&self.pool)
            .await
            .map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        conn.execute(sqlx::query("CREATE TABLE IF NOT EXISTS user (id varchar(64) PRIMARY KEY, network_id integer, server_id TEXT)")).await.map_err(into_vpn_err!(VpnErrorCode::IoError))?;
        Ok(())
    }

    pub fn update_password(&self, account: &str, password: &str) {
        let mut cache = self.password_cache.lock().unwrap();
        cache.insert(account.to_string(), password.to_string());
    }

    pub fn get_password(&self, account: &str) -> Option<String> {
        let cache = self.password_cache.lock().unwrap();
        cache.get(account).cloned()
    }
}

#[async_trait::async_trait]
impl AccountStore<User> for SqliteUserStore {
    async fn get_account(&self, account_id: &<User as Account>::Id) -> AccountResult<Option<User>> {
        let mut conn = SqliteConnection::acquire(&self.pool)
            .await
            .map_err(into_account_err!(AccountErrorCode::IoError))?;
        match conn
            .fetch_one(sqlx::query("SELECT * FROM user WHERE id = ?").bind(account_id))
            .await
        {
            Ok(row) => {
                let id: String = row.get("id");
                let network_id: i64 = row.get("network_id");
                let password = match self.get_password(&id) {
                    Some(p) => p,
                    None => {
                        return Ok(None);
                    }
                };
                let server_id: String = row.get("server_id");
                Ok(Some(User {
                    id,
                    password,
                    network_id: network_id as u64,
                    server_id,
                }))
            }
            Err(sqlx::Error::RowNotFound) => Ok(None),
            Err(e) => Err(account_err!(AccountErrorCode::IoError, "{}", e)),
        }
    }

    async fn get_account_by_name(&self, account_name: &str) -> AccountResult<Option<User>> {
        let mut conn = SqliteConnection::acquire(&self.pool)
            .await
            .map_err(into_account_err!(AccountErrorCode::IoError))?;
        match conn
            .fetch_one(sqlx::query("SELECT * FROM user WHERE id = ?").bind(account_name))
            .await
        {
            Ok(row) => {
                let id: String = row.get("id");
                let network_id: i64 = row.get("network_id");
                let password = match self.get_password(&id) {
                    Some(p) => p,
                    None => {
                        return Ok(None);
                    }
                };
                let server_id: String = row.get("server_id");
                Ok(Some(User {
                    id,
                    password,
                    network_id: network_id as u64,
                    server_id,
                }))
            }
            Err(sqlx::Error::RowNotFound) => Ok(None),
            Err(e) => Err(account_err!(AccountErrorCode::IoError, "{}", e)),
        }
    }

    async fn remove_account(&self, account_id: &<User as Account>::Id) -> AccountResult<()> {
        let mut conn = SqliteConnection::acquire(&self.pool)
            .await
            .map_err(into_account_err!(AccountErrorCode::IoError))?;
        conn.execute(sqlx::query("DELETE FROM user WHERE id = ?").bind(account_id))
            .await
            .map_err(into_account_err!(AccountErrorCode::IoError))?;
        Ok(())
    }

    async fn add_account(&self, account: &User) -> AccountResult<<User as Account>::Id> {
        let mut conn = SqliteConnection::acquire(&self.pool)
            .await
            .map_err(into_account_err!(AccountErrorCode::IoError))?;
        self.update_password(&account.id, &account.password);
        conn.execute(
            sqlx::query("INSERT INTO user (id, network_id, server_id) VALUES (?, ?, ?)")
                .bind(&account.id)
                .bind(account.network_id as i64)
                .bind(account.server_id.as_str()),
        )
        .await
        .map_err(into_account_err!(AccountErrorCode::IoError))?;
        Ok(account.id.clone())
    }

    async fn update_account(&self, account: &User) -> AccountResult<()> {
        self.update_password(&account.id, &account.password);
        Ok(())
    }
}

pub type UserManagerRef = Arc<DefaultAccountManager<User, SqliteUserStore>>;
