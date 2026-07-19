use log::LevelFilter;
use sqlx::pool::PoolConnection;
use sqlx::sqlite::{
    SqliteArguments, SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions,
    SqliteQueryResult, SqliteRow,
};
use sqlx::{ConnectOptions, Sqlite, SqlitePool};
use std::str::FromStr;
use std::time::Duration;

const SQLITE_TIMEOUT: Duration = Duration::from_secs(300);
const SQLITE_SLOW_STATEMENT_THRESHOLD: Duration = Duration::from_secs(10);

pub(crate) type SqliteQuery<'q> =
    sqlx::query::Query<'q, Sqlite, SqliteArguments<'q>>;

pub(crate) async fn open_sqlite_pool(
    uri: &str,
    max_connections: u32,
    journal_mode: Option<SqliteJournalMode>,
) -> Result<SqlitePool, sqlx::Error> {
    log::info!("open pool {} max_connections {}", uri, max_connections);

    let pool_options = SqlitePoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(SQLITE_TIMEOUT)
        .min_connections(0)
        .idle_timeout(SQLITE_TIMEOUT);

    let mut connect_options = SqliteConnectOptions::from_str(uri)?
        .busy_timeout(SQLITE_TIMEOUT)
        .create_if_missing(true);

    if let Some(journal_mode) = journal_mode {
        connect_options = connect_options.journal_mode(journal_mode);
    }

    #[cfg(target_os = "ios")]
    {
        connect_options = connect_options.serialized(true);
    }

    connect_options = connect_options
        .log_statements(LevelFilter::Off)
        .log_slow_statements(LevelFilter::Off, SQLITE_SLOW_STATEMENT_THRESHOLD);

    pool_options.connect_with(connect_options).await
}

pub(crate) struct SqliteConnection {
    connection: PoolConnection<Sqlite>,
}

impl SqliteConnection {
    pub(crate) fn new(connection: PoolConnection<Sqlite>) -> Self {
        Self { connection }
    }

    pub(crate) async fn acquire(pool: &SqlitePool) -> Result<Self, sqlx::Error> {
        pool.acquire().await.map(Self::new)
    }

    pub(crate) async fn execute(
        &mut self,
        query: SqliteQuery<'_>,
    ) -> Result<SqliteQueryResult, sqlx::Error> {
        query.execute(&mut *self.connection).await
    }

    pub(crate) async fn fetch_one(
        &mut self,
        query: SqliteQuery<'_>,
    ) -> Result<SqliteRow, sqlx::Error> {
        query.fetch_one(&mut *self.connection).await
    }

    pub(crate) async fn fetch_all(
        &mut self,
        query: SqliteQuery<'_>,
    ) -> Result<Vec<SqliteRow>, sqlx::Error> {
        query.fetch_all(&mut *self.connection).await
    }

    pub(crate) async fn begin_transaction(&mut self) -> Result<(), sqlx::Error> {
        self.execute(sqlx::query("BEGIN")).await?;
        Ok(())
    }

    pub(crate) async fn commit_transaction(&mut self) -> Result<(), sqlx::Error> {
        self.execute(sqlx::query("COMMIT")).await?;
        Ok(())
    }

    pub(crate) async fn rollback_transaction(&mut self) -> Result<(), sqlx::Error> {
        self.execute(sqlx::query("ROLLBACK")).await?;
        Ok(())
    }

    pub(crate) fn close_on_drop(&mut self) {
        self.connection.close_on_drop();
    }
}
