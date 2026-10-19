//! Recognize Goose history and upgrade a database copy without altering the original.

pub mod accounts;
mod auth_schema;
mod campaign_schema;
pub mod campaigns;
mod domain_schema;
pub mod domains;
pub mod integrations;
pub mod interactions;
pub(crate) mod mailbox_worker;
mod migrations;
mod participant_privacy;
pub mod previews;
mod report_exports;
pub mod reporting;
mod reporting_schema;
pub mod resources;
pub mod sessions;
mod startup;
mod webhook_worker;

pub use startup::{StartupDatabase, StartupError};

use migrations::MIGRATIONS;
use sqlx::{Connection, Row, SqliteConnection, sqlite::SqliteConnectOptions};
use std::{collections::BTreeMap, path::Path};

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
    #[error("Unrecognized or inconsistent Gophish database: {0}")]
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

fn up(source: &str) -> &str {
    source
        .split("-- +goose Up")
        .nth(1)
        .unwrap_or("")
        .split("-- +goose Down")
        .next()
        .unwrap_or("")
}

async fn migrate(connection: &mut SqliteConnection, fresh: bool) -> Result<(), UpgradeError> {
    let mut transaction = connection.begin_with("BEGIN IMMEDIATE").await?;
    if fresh {
        sqlx::raw_sql("CREATE TABLE goose_db_version(id INTEGER PRIMARY KEY AUTOINCREMENT, version_id INTEGER NOT NULL, is_applied INTEGER NOT NULL, tstamp TIMESTAMP DEFAULT(datetime('now'))); INSERT INTO goose_db_version(version_id,is_applied) VALUES(0,1);").execute(&mut *transaction).await?;
    }
    let count = inspect(&mut transaction, fresh).await?;
    apply_pending(&mut transaction, count).await?;
    transaction.commit().await?;
    Ok(())
}

async fn inspect(connection: &mut SqliteConnection, fresh: bool) -> Result<usize, UpgradeError> {
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&mut *connection)
        .await?;
    if integrity != "ok" {
        return Err(UpgradeError::Unrecognized(integrity));
    }
    let history = sqlx::query("SELECT version_id,is_applied FROM goose_db_version ORDER BY id")
        .fetch_all(&mut *connection)
        .await?;
    if history.is_empty() {
        return Err(UpgradeError::Unrecognized("empty migration history".into()));
    }
    let mut applied = BTreeMap::new();
    for row in history {
        let version: i64 = row.try_get("version_id")?;
        let state: i64 = row.try_get("is_applied")?;
        if !matches!(state, 0 | 1)
            || (version != 0 && !MIGRATIONS.iter().any(|(v, _)| *v == version))
        {
            return Err(UpgradeError::Unrecognized(format!(
                "unknown version/state {version}/{state}"
            )));
        }
        applied.insert(version, state == 1);
    }
    let mut count = 0;
    let mut gap = false;
    for (version, _) in MIGRATIONS {
        if applied.get(version).copied().unwrap_or(false) {
            if gap {
                return Err(UpgradeError::Unrecognized(
                    "migration history has a gap".into(),
                ));
            }
            count += 1;
        } else {
            gap = true;
        }
    }
    if !fresh && count == 0 {
        return Err(UpgradeError::Unrecognized(
            "no application migration applied".into(),
        ));
    }
    validate_schema(connection, count).await?;
    Ok(count)
}

async fn apply_pending(
    connection: &mut SqliteConnection,
    count: usize,
) -> Result<(), UpgradeError> {
    for (version, source) in &MIGRATIONS[count..] {
        if *version == 20160227180335 {
            convert_early_smtp(connection).await?;
        } else {
            sqlx::raw_sql(up(source)).execute(&mut *connection).await?;
        }
        sqlx::query("INSERT INTO goose_db_version(version_id,is_applied) VALUES(?,1)")
            .bind(version)
            .execute(&mut *connection)
            .await?;
    }
    Ok(())
}

async fn convert_early_smtp(connection: &mut SqliteConnection) -> Result<(), UpgradeError> {
    let ambiguous: i64 = sqlx::query_scalar("SELECT count(*) FROM smtp AS s LEFT JOIN campaigns AS c ON c.id=s.campaign_id WHERE c.id IS NULL OR c.user_id IS NULL OR (SELECT count(*) FROM smtp WHERE campaign_id=s.campaign_id)>1").fetch_one(&mut *connection).await?;
    if ambiguous != 0 {
        return Err(UpgradeError::Unrecognized(
            "old SMTP profiles have missing owners or ambiguous campaign associations".into(),
        ));
    }
    // The original migration discards smtp records. Retain every field and association.
    sqlx::raw_sql("ALTER TABLE smtp RENAME TO ruphish_old_smtp; ALTER TABLE campaigns ADD COLUMN smtp_id bigint;").execute(&mut *connection).await?;
    // Use the original table definition for schema compatibility.
    let definition = up(MIGRATIONS[5].1)
        .split("CREATE TABLE smtp")
        .nth(1)
        .ok_or_else(|| UpgradeError::Unrecognized("missing embedded SMTP definition".into()))?;
    // This string is assembled only from an embedded, provenance-checked migration.
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "CREATE TABLE smtp{definition}"
    )))
    .execute(&mut *connection)
    .await?;
    sqlx::raw_sql("INSERT INTO smtp(id,user_id,interface_type,name,host,username,password,from_address,ignore_cert_errors) SELECT s.smtp_id,c.user_id,'SMTP','Migrated SMTP '||s.smtp_id,s.host,s.username,'',s.from_address,s.ignore_cert_errors FROM ruphish_old_smtp AS s JOIN campaigns AS c ON c.id=s.campaign_id; UPDATE campaigns SET smtp_id=(SELECT smtp_id FROM ruphish_old_smtp WHERE campaign_id=campaigns.id); DROP TABLE ruphish_old_smtp;").execute(&mut *connection).await?;
    Ok(())
}

async fn schema(
    connection: &mut SqliteConnection,
) -> Result<BTreeMap<String, Vec<String>>, sqlx::Error> {
    let tables: Vec<String> = sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name != 'goose_db_version' ORDER BY name").fetch_all(&mut *connection).await?;
    let mut result = BTreeMap::new();
    for table in tables {
        let rows = sqlx::query(
            "SELECT name,type,\"notnull\",dflt_value,pk FROM pragma_table_info(?) ORDER BY cid",
        )
        .bind(&table)
        .fetch_all(&mut *connection)
        .await?;
        let columns = rows
            .iter()
            .map(|row| -> Result<String, sqlx::Error> {
                Ok(format!(
                    "{}|{}|{}|{:?}|{}",
                    row.try_get::<String, _>("name")?,
                    row.try_get::<String, _>("type")?,
                    row.try_get::<i64, _>("notnull")?,
                    row.try_get::<Option<String>, _>("dflt_value")?,
                    row.try_get::<i64, _>("pk")?
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        result.insert(table, columns);
    }
    Ok(result)
}

async fn validate_schema(
    connection: &mut SqliteConnection,
    count: usize,
) -> Result<(), UpgradeError> {
    let mut expected = SqliteConnection::connect("sqlite::memory:").await?;
    for (_, source) in &MIGRATIONS[..count] {
        sqlx::raw_sql(up(source)).execute(&mut expected).await?;
    }
    if auth_schema::validate_version(connection).await? {
        sqlx::raw_sql(auth_schema::SCHEMA)
            .execute(&mut expected)
            .await?;
        if auth_schema::version(connection).await? >= 2 {
            sqlx::raw_sql(campaign_schema::SCHEMA)
                .execute(&mut expected)
                .await?;
        }
        if auth_schema::version(connection).await? >= 4 {
            domain_schema::migrate(&mut expected).await?;
        }
        if auth_schema::version(connection).await? >= 5 {
            domain_schema::verification_history(&mut expected).await?;
        }
        if auth_schema::version(connection).await? >= 6 {
            reporting_schema::migrate(&mut expected).await?;
        }
    }
    if schema(connection).await? != schema(&mut expected).await? {
        return Err(UpgradeError::Unrecognized(
            "schema does not match migration history".into(),
        ));
    }
    expected.close().await?;
    Ok(())
}
