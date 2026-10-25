//! Read-only provider probes using the same pools as application operations.

use ruphish_application::{health::Storage, storage::Error};

pub(super) enum Probe {
    Sqlite(sqlx::SqlitePool),
    MySql(sqlx::MySqlPool),
    Postgres(sqlx::PgPool),
}

#[async_trait::async_trait]
impl Storage for Probe {
    async fn check(&self) -> Result<(), Error> {
        // A constant SELECT would not establish access to the application schema.
        // Only read the singleton's identifier; never load its configuration values.
        let query = "SELECT id FROM ruphish_settings WHERE id = 1";
        let found = match self {
            Self::Sqlite(pool) => sqlx::query(query)
                .fetch_optional(pool)
                .await
                .map(|r| r.is_some()),
            Self::MySql(pool) => sqlx::query(query)
                .fetch_optional(pool)
                .await
                .map(|r| r.is_some()),
            Self::Postgres(pool) => sqlx::query(query)
                .fetch_optional(pool)
                .await
                .map(|r| r.is_some()),
        };
        match found {
            Ok(true) => Ok(()),
            Ok(false) => Err(Error::UnsupportedSchema),
            Err(_) => Err(Error::Unavailable),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "Isolated pool fixture and assertions")]
    use super::*;
    use ruphish_application::health::Service;
    use std::{sync::Arc, time::Duration};

    #[tokio::test]
    async fn health_deadline_includes_pool_acquisition_and_releases_capacity() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::raw_sql("CREATE TABLE ruphish_settings(id INTEGER PRIMARY KEY); INSERT INTO ruphish_settings VALUES(1)")
            .execute(&pool).await.unwrap();
        let service = Service::new(Arc::new(Probe::Sqlite(pool.clone())));
        service.started();
        let held = pool.acquire().await.unwrap();
        assert!(
            !tokio::time::timeout(Duration::from_secs(3), service.ready())
                .await
                .unwrap()
        );
        drop(held);
        assert!(service.ready().await);
        sqlx::query("DELETE FROM ruphish_settings")
            .execute(&pool)
            .await
            .unwrap();
        assert!(!service.ready().await);
        pool.close().await;
    }
}
