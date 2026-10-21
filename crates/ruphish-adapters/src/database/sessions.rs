//! One tower-sessions bridge shared by every database provider.
use ruphish_application::storage::{self, SessionRecord};
use std::sync::Arc;
use time::OffsetDateTime;
use tower_sessions_core::{
    session::{Id, Record},
    session_store::{Error, Result},
};

/// Framework adapter backed only by the application session persistence contract.
#[derive(Clone)]
pub struct SessionStore {
    repository: Arc<dyn storage::Sessions>,
}
impl std::fmt::Debug for SessionStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionStore").finish_non_exhaustive()
    }
}
impl SessionStore {
    pub(super) fn new(repository: Arc<dyn storage::Sessions>) -> Self {
        Self { repository }
    }
    /// Delete expired records; reads also enforce expiry before cleanup.
    pub async fn cleanup(&self) -> Result<()> {
        self.repository.cleanup(now()).await.map_err(storage)
    }
    /// Atomically load/create the shared cookie-protection key.
    pub async fn cookie_key(&self) -> Result<[u8; 64]> {
        self.repository.cookie_key().await.map_err(storage)
    }
}
fn now() -> i64 {
    OffsetDateTime::now_utc().unix_timestamp()
}
fn storage(_: storage::Error) -> Error {
    Error::Backend("Session storage unavailable".into())
}
fn encode(record: &Record) -> Result<SessionRecord> {
    Ok(SessionRecord {
        id: record.id.to_string(),
        data: serde_json::to_string(&record.data)
            .map_err(|_| Error::Encode("Invalid session data".into()))?,
        expires: record.expiry_date.unix_timestamp(),
    })
}
#[async_trait::async_trait]
impl tower_sessions_core::SessionStore for SessionStore {
    async fn create(&self, record: &mut Record) -> Result<()> {
        self.cleanup().await?;
        let mut stored = encode(record)?;
        for _ in 0..8 {
            if self.repository.create(&stored).await.map_err(storage)? {
                return Ok(());
            }
            record.id = Id::default();
            stored.id = record.id.to_string();
        }
        Err(Error::Backend("Could not allocate a unique session".into()))
    }
    async fn save(&self, record: &Record) -> Result<()> {
        if self
            .repository
            .save(&encode(record)?, now())
            .await
            .map_err(storage)?
        {
            Ok(())
        } else {
            Err(Error::Backend("Session was revoked or expired".into()))
        }
    }
    async fn load(&self, id: &Id) -> Result<Option<Record>> {
        self.repository
            .load(&id.to_string(), now())
            .await
            .map_err(storage)?
            .map(|record| {
                Ok(Record {
                    id: *id,
                    data: serde_json::from_str(&record.data)
                        .map_err(|_| Error::Decode("Invalid stored session".into()))?,
                    expiry_date: OffsetDateTime::from_unix_timestamp(record.expires)
                        .map_err(|_| Error::Decode("Invalid expiry".into()))?,
                })
            })
            .transpose()
    }
    async fn delete(&self, id: &Id) -> Result<()> {
        self.repository
            .delete(&id.to_string())
            .await
            .map_err(storage)
    }
}
