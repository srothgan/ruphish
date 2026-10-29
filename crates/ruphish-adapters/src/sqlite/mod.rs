//! SQLite persistence, release schema initialization and direct Gophish conversion.

pub mod accounts;
pub mod campaigns;
pub mod domains;
mod engagement;
pub mod integrations;
pub mod interactions;
pub(crate) mod mailbox_worker;
mod migrations;
pub mod overview;
pub mod previews;
pub mod reporting;
pub mod resources;
pub mod sessions;
mod startup;
mod webhook_worker;

pub use startup::{StartupDatabase, StartupError};

mod legacy;
mod schema;
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use std::path::Path;

/// Latest schema understood by the current compatibility adapter.
pub const LATEST_LEGACY_VERSION: i64 = 20220321133237;

/// A database cannot safely be recognized, copied, or migrated.
#[derive(Debug, thiserror::Error)]
pub enum UpgradeError {
    /// I/O failure or an existing destination; existing files are never overwritten.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Database query or migration failure.
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    /// The version history or schema is unsupported or inconsistent.
    #[error("Unrecognized or inconsistent database: {0}")]
    Unrecognized(String),
}

/// Create a fresh schema without creating bootstrap accounts.
pub async fn create(path: &Path) -> Result<(), UpgradeError> {
    let temporary = temporary_destination(path)?;
    crate::private_file::protect(temporary.path())?;
    let mut connection = connect(temporary.path(), false).await?;
    let migration = migrate(&mut connection, true).await;
    let closed = connection.close().await;
    migration?;
    closed?;
    publish(temporary, path)
}

/// Upgrade a coherent SQLite snapshot to a new path, leaving the source recoverable.
///
/// SQLite VACUUM INTO includes committed WAL data. Publication uses no-clobber file
/// persistence only after validation and migration have succeeded. Stop the old
/// application before cutover so later writes cannot be lost after this snapshot.
pub async fn upgrade_copy(source: &Path, destination: &Path) -> Result<(), UpgradeError> {
    let temporary = temporary_destination(destination)?;
    crate::private_file::protect(temporary.path())?;
    let mut original = connect(source, true).await?;
    let snapshot = sqlx::query("VACUUM INTO ?")
        .bind(temporary.path().to_string_lossy().as_ref())
        .execute(&mut original)
        .await;
    let closed = original.close().await;
    snapshot?;
    closed?;
    let mut copy = connect(temporary.path(), false).await?;
    let migration = migrate(&mut copy, false).await;
    let closed = copy.close().await;
    migration?;
    closed?;
    publish(temporary, destination)
}

fn temporary_destination(destination: &Path) -> Result<tempfile::NamedTempFile, std::io::Error> {
    if destination.try_exists()? {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "destination already exists",
        ));
    }
    tempfile::NamedTempFile::new_in(
        destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )
}

fn publish(temporary: tempfile::NamedTempFile, path: &Path) -> Result<(), UpgradeError> {
    temporary
        .persist_noclobber(path)
        .map_err(|error| error.error)?;
    Ok(())
}

async fn connect(path: &Path, read_only: bool) -> Result<SqliteConnection, sqlx::Error> {
    SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .read_only(read_only)
            .create_if_missing(false),
    )
    .await
}

async fn migrate(connection: &mut SqliteConnection, fresh: bool) -> Result<(), UpgradeError> {
    let mut transaction = connection.begin_with("BEGIN IMMEDIATE").await?;
    if fresh {
        migrations::initialize(&mut transaction).await?;
        schema::validate(&mut transaction).await?;
    } else {
        let source = migrations::inspect(&mut transaction).await?;
        migrations::upgrade(&mut transaction, source).await?;
    }
    transaction.commit().await?;
    Ok(())
}

/// Map SQLite failures without leaking queries, bound values or file names.
pub(crate) fn storage_error(error: sqlx::Error) -> ruphish_application::storage::Error {
    use ruphish_application::storage::Error;
    match error {
        sqlx::Error::PoolTimedOut => Error::Busy,
        sqlx::Error::Database(error) => {
            match error.code().and_then(|code| code.parse::<u32>().ok()) {
                Some(517) => Error::TransactionConflict,
                Some(code) if matches!(code & 0xff, 5 | 6) => Error::Busy,
                Some(code) if code & 0xff == 19 => Error::Integrity,
                Some(code) if code & 0xff == 17 => Error::UnsupportedSchema,
                _ => Error::Unavailable,
            }
        }
        _ => Error::Unavailable,
    }
}

/// Classify statement failures without exposing driver diagnostics.
pub(super) fn resource_error(error: sqlx::Error) -> ruphish_application::resources::Error {
    ruphish_application::resources::Error::Storage(storage_error(error))
}

pub(super) mod settings;

mod event_edit;

mod inventory_sql;
mod worker_sql;
