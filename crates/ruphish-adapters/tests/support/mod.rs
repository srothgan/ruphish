//! Independent SQL access is only for fixture setup, fault injection and inspection.
//! Every operation under test goes through the same factory/capabilities as production.
use ruphish_adapters::{
    database::{self, Config, Database, Error},
    outbound::Policy,
};
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::{ops::Deref, path::Path, time::Duration};

pub struct DatabaseFixture {
    provider: Database,
    inspection: SqlitePool,
}
impl DatabaseFixture {
    pub async fn open(path: &Path, migrations_prefix: &str) -> Result<Self, Error> {
        let provider = database::open(
            Config::Sqlite {
                path: path.to_owned(),
                migrations_prefix: migrations_prefix.to_owned(),
            },
            Policy::default(),
        )
        .await?;
        let inspection = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_lazy_with(
                SqliteConnectOptions::new()
                    .filename(path)
                    .create_if_missing(false)
                    .busy_timeout(Duration::from_secs(5)),
            );
        Ok(Self {
            provider,
            inspection,
        })
    }
    // No database operation under test may receive this connection.
    #[allow(
        dead_code,
        reason = "Shared fixture helper; migration tests inspect files directly"
    )]
    pub fn inspection_pool(&self) -> SqlitePool {
        self.inspection.clone()
    }
    pub async fn close(self) {
        self.inspection.close().await;
        self.provider.close().await;
    }
}
impl Deref for DatabaseFixture {
    type Target = Database;
    fn deref(&self) -> &Database {
        &self.provider
    }
}
