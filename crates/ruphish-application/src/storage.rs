//! Provider-independent persistence failures and session records.

/// Safe persistence errors: never carry SQL statements, connection strings or values.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Invalid configuration or an unsupported/inconsistent schema.
    #[error("{0}")]
    Invalid(String),
    /// The operating system rejected a storage operation.
    #[error("Storage filesystem operation failed: {0:?}")]
    Io(std::io::ErrorKind),
    /// The bounded wait for storage capacity or a lock expired.
    #[error("Storage is busy or unavailable; the wait limit was reached")]
    Busy,
    /// A driver operation failed without a safe, more specific classification.
    #[error("Storage operation failed")]
    Unavailable,
    /// Server rolled back a conflicting transaction; callers still own retry policy.
    #[error("Storage transaction conflicted with another operation")]
    TransactionConflict,
    /// A database constraint rejected a write.
    #[error("Storage integrity constraint was not satisfied")]
    Integrity,
    /// An unrecognized, incomplete, modified or newer schema cannot be used.
    #[error("Storage schema is unsupported or inconsistent")]
    UnsupportedSchema,
    /// Commit acknowledgement was lost; retrying could repeat a committed operation.
    #[error("Storage commit outcome is unknown; reconcile before retrying")]
    UncertainCommit,
}

/// Opaque serialized session payload; database providers do not interpret it.
pub struct SessionRecord {
    /// Framework-generated identifier.
    pub id: String,
    /// Serialized data owned by the session bridge.
    pub data: String,
    /// Expiry in Unix seconds.
    pub expires: i64,
}

/// Atomic persistence operations used by the shared session framework bridge.
#[async_trait::async_trait]
pub trait Sessions: Send + Sync {
    /// Remove expired records.
    async fn cleanup(&self, now: i64) -> Result<(), Error>;
    /// Load or create the persistent cookie key atomically.
    async fn cookie_key(&self) -> Result<[u8; 64], Error>;
    /// Insert without replacing another session; false means identifier collision.
    async fn create(&self, record: &SessionRecord) -> Result<bool, Error>;
    /// Update only a still-live record; false means revoked, expired or absent.
    async fn save(&self, record: &SessionRecord, now: i64) -> Result<bool, Error>;
    /// Return only a non-expired session.
    async fn load(&self, id: &str, now: i64) -> Result<Option<SessionRecord>, Error>;
    /// Delete idempotently; subsequent saves must not resurrect it.
    async fn delete(&self, id: &str) -> Result<(), Error>;
}
