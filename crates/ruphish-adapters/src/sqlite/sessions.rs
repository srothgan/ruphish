//! Persistent tower-sessions storage with explicit create and non-resurrecting saves.
use sqlx::{Row, SqlitePool};
use time::OffsetDateTime;
use tower_sessions_core::{
    SessionStore,
    session::{Id, Record},
    session_store::{Error, Result},
};

/// SQLite storage shared by independent admin processes without a credential cache.
#[derive(Debug, Clone)]
pub struct Sessions {
    pool: SqlitePool,
}
impl Sessions {
    /// Use an already prepared application database.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
    /// Delete expired records; expiry is enforced on every load even before cleanup.
    pub async fn cleanup(&self) -> Result<()> {
        sqlx::query("DELETE FROM ruphish_sessions WHERE expires<=?")
            .bind(OffsetDateTime::now_utc().unix_timestamp())
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }
    /// Load/create a persistent cookie-protection key atomically, without logging it.
    pub async fn cookie_key(&self) -> Result<[u8; 64]> {
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage)?;
        let stored: Option<Vec<u8>> =
            sqlx::query_scalar("SELECT value FROM ruphish_secrets WHERE name='cookie-key'")
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage)?;
        let key = if let Some(key) = stored {
            if key.len() != 64 {
                return Err(Error::Backend("Invalid stored cookie key".into()));
            }
            key
        } else {
            let mut key = vec![0; 64];
            getrandom::fill(&mut key)
                .map_err(|_| Error::Backend("OS randomness unavailable".into()))?;
            sqlx::query("INSERT INTO ruphish_secrets(name,value) VALUES('cookie-key',?)")
                .bind(&key)
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
            key
        };
        tx.commit().await.map_err(storage)?;
        key.try_into()
            .map_err(|_| Error::Backend("Invalid stored cookie key".into()))
    }
}
fn storage(_: sqlx::Error) -> Error {
    Error::Backend("Session storage unavailable".into())
}
fn encode(record: &Record) -> Result<String> {
    serde_json::to_string(&record.data).map_err(|_| Error::Encode("Invalid session data".into()))
}

#[async_trait::async_trait]
impl SessionStore for Sessions {
    async fn create(&self, record: &mut Record) -> Result<()> {
        self.cleanup().await?;
        let data = encode(record)?;
        for _ in 0..8 {
            let inserted = sqlx::query("INSERT INTO ruphish_sessions(id,data,expires) VALUES(?,?,?) ON CONFLICT(id) DO NOTHING")
                .bind(record.id.to_string()).bind(&data).bind(record.expiry_date.unix_timestamp()).execute(&self.pool).await.map_err(storage)?;
            if inserted.rows_affected() == 1 {
                return Ok(());
            }
            record.id = Id::default();
        }
        Err(Error::Backend("Could not allocate a unique session".into()))
    }
    async fn save(&self, record: &Record) -> Result<()> {
        let changed =
            sqlx::query("UPDATE ruphish_sessions SET data=?,expires=? WHERE id=? AND expires>?")
                .bind(encode(record)?)
                .bind(record.expiry_date.unix_timestamp())
                .bind(record.id.to_string())
                .bind(OffsetDateTime::now_utc().unix_timestamp())
                .execute(&self.pool)
                .await
                .map_err(storage)?;
        if changed.rows_affected() != 1 {
            return Err(Error::Backend("Session was revoked or expired".into()));
        }
        Ok(())
    }
    async fn load(&self, id: &Id) -> Result<Option<Record>> {
        let row = sqlx::query("SELECT data,expires FROM ruphish_sessions WHERE id=? AND expires>?")
            .bind(id.to_string())
            .bind(OffsetDateTime::now_utc().unix_timestamp())
            .fetch_optional(&self.pool)
            .await
            .map_err(storage)?;
        row.map(|row| {
            let text: String = row.try_get("data").map_err(storage)?;
            let expires: i64 = row.try_get("expires").map_err(storage)?;
            Ok(Record {
                id: *id,
                data: serde_json::from_str(&text)
                    .map_err(|_| Error::Decode("Invalid stored session".into()))?,
                expiry_date: OffsetDateTime::from_unix_timestamp(expires)
                    .map_err(|_| Error::Decode("Invalid expiry".into()))?,
            })
        })
        .transpose()
    }
    async fn delete(&self, id: &Id) -> Result<()> {
        sqlx::query("DELETE FROM ruphish_sessions WHERE id=?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(storage)?;
        Ok(())
    }
}
