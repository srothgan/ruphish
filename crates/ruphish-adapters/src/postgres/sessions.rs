//! Atomic PostgreSQL session persistence; framework behavior lives in the shared bridge.
use super::storage_error;
use ruphish_application::storage::{Error, SessionRecord, Sessions as Repository};
use sqlx::{PgPool, Row};

pub struct Sessions {
    pool: PgPool,
}
impl Sessions {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
#[async_trait::async_trait]
impl Repository for Sessions {
    async fn cleanup(&self, now: i64) -> Result<(), Error> {
        sqlx::query("DELETE FROM ruphish_sessions WHERE expires<=$1")
            .bind(now)
            .execute(&self.pool)
            .await
            .map_err(storage_error)?;
        Ok(())
    }
    /// Load/create a persistent cookie-protection key atomically, without logging it.
    async fn cookie_key(&self) -> Result<[u8; 64], Error> {
        let mut tx = crate::postgres::locks::write(&self.pool)
            .await
            .map_err(storage_error)?;
        super::locks::guard(&mut tx, "cookie-key")
            .await
            .map_err(storage_error)?;
        let stored: Option<Vec<u8>> =
            sqlx::query_scalar("SELECT value FROM ruphish_secrets WHERE name='cookie-key'")
                .fetch_optional(&mut *tx)
                .await
                .map_err(storage_error)?;
        let key = if let Some(key) = stored {
            if key.len() != 64 {
                return Err(Error::Invalid("Invalid stored cookie key".into()));
            }
            key
        } else {
            let mut key = vec![0; 64];
            getrandom::fill(&mut key)
                .map_err(|_| Error::Invalid("OS randomness unavailable".into()))?;
            sqlx::query("INSERT INTO ruphish_secrets(name,value) VALUES('cookie-key',$1)")
                .bind(&key)
                .execute(&mut *tx)
                .await
                .map_err(storage_error)?;
            key
        };
        tx.commit().await.map_err(super::commit_error)?;
        key.try_into()
            .map_err(|_| Error::Invalid("Invalid stored cookie key".into()))
    }
    async fn create(&self, record: &SessionRecord) -> Result<bool, Error> {
        match sqlx::query("INSERT INTO ruphish_sessions(id,data,expires) VALUES($1,$2,$3)")
            .bind(&record.id)
            .bind(&record.data)
            .bind(record.expires)
            .execute(&self.pool)
            .await
        {
            Ok(_) => Ok(true),
            Err(sqlx::Error::Database(error)) if error.is_unique_violation() => Ok(false),
            Err(error) => Err(storage_error(error)),
        }
    }

    async fn save(&self, record: &SessionRecord, now: i64) -> Result<bool, Error> {
        let changed = sqlx::query(
            "UPDATE ruphish_sessions SET data=$1,expires=$2 WHERE id=$3 AND expires>$4",
        )
        .bind(&record.data)
        .bind(record.expires)
        .bind(&record.id)
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(storage_error)?;
        Ok(changed.rows_affected() == 1)
    }
    async fn load(&self, id: &str, now: i64) -> Result<Option<SessionRecord>, Error> {
        let row =
            sqlx::query("SELECT data,expires FROM ruphish_sessions WHERE id=$1 AND expires>$2")
                .bind(id)
                .bind(now)
                .fetch_optional(&self.pool)
                .await
                .map_err(storage_error)?;
        row.map(|row| {
            Ok(SessionRecord {
                id: id.to_owned(),
                data: row.try_get("data").map_err(storage_error)?,
                expires: row.try_get("expires").map_err(storage_error)?,
            })
        })
        .transpose()
    }
    async fn delete(&self, id: &str) -> Result<(), Error> {
        sqlx::query("DELETE FROM ruphish_sessions WHERE id=$1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(storage_error)?;
        Ok(())
    }
}
