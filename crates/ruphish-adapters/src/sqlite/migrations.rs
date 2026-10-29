//! Release migration history. Gophish recognition and conversion live in `legacy`.
use super::{UpgradeError, legacy, schema};
use sqlx::{
    SqlSafeStr, SqliteConnection,
    migrate::{Migration, MigrationType, Migrator},
};
use std::borrow::Cow;

pub(super) const BASELINE: &str = include_str!("../../migrations/sqlite/1_baseline.sql");

pub(super) fn migrator() -> Migrator {
    Migrator {
        migrations: Cow::Owned(vec![Migration::new(
            1,
            Cow::Borrowed("release baseline"),
            MigrationType::Simple,
            BASELINE.into_sql_str(),
            false,
        )]),
        locking: false,
        ..Migrator::DEFAULT
    }
}

pub(super) async fn initialize(c: &mut SqliteConnection) -> Result<(), UpgradeError> {
    migrator()
        .run_direct(None, &mut *c, false)
        .await
        .map_err(|e| UpgradeError::Unrecognized(format!("Application migration failed: {e}")))?;
    Ok(())
}

/// A recognized source, before any mutation. Old Ruphish histories are unsupported.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Source {
    Application(i64),
    Gophish(i64),
}

pub(super) async fn inspect(c: &mut SqliteConnection) -> Result<Source, UpgradeError> {
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&mut *c)
        .await?;
    if integrity != "ok" {
        return Err(UpgradeError::Unrecognized(integrity));
    }
    let application: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='_sqlx_migrations')",
    )
    .fetch_one(&mut *c)
    .await?;
    if application {
        let rows: Vec<(i64, bool, Vec<u8>)> = sqlx::query_as(
            "SELECT version,success,checksum FROM _sqlx_migrations ORDER BY version",
        )
        .fetch_all(&mut *c)
        .await?;
        let migrations = migrator();
        if rows.is_empty()
            || rows.len() > migrations.migrations.len()
            || !rows.iter().zip(migrations.iter()).all(
                |((version, success, checksum), expected)| {
                    *version == expected.version
                        && *success
                        && checksum.as_slice() == expected.checksum.as_ref()
                },
            )
        {
            return Err(UpgradeError::Unrecognized(
                "Unsupported Ruphish schema history".into(),
            ));
        }
        let version = rows
            .last()
            .map(|row| row.0)
            .ok_or_else(|| UpgradeError::Unrecognized("Empty application history".into()))?;
        schema::validate_version(c, version).await?;
        Ok(Source::Application(version))
    } else {
        legacy::inspect(c).await.map(Source::Gophish)
    }
}

pub(super) async fn upgrade(c: &mut SqliteConnection, source: Source) -> Result<(), UpgradeError> {
    match source {
        Source::Gophish(version) => legacy::convert(c, version).await?,
        Source::Application(_) => initialize(c).await?,
    }
    schema::validate(c).await
}

pub(super) fn current(source: &Source) -> bool {
    matches!(source, Source::Application(version) if migrator().iter().last().is_some_and(|migration| migration.version == *version))
}
