//! Startup owns a dedicated connection throughout recognition and migration.
//! Legacy publication is one atomic InnoDB RENAME, retaining the original tables.
use super::{preflight, storage_error};
use crate::database::{Error, MySqlConfig, migrations::MySqlSchema};
use sha2::{Digest, Sha256};
use sqlx::{
    Connection, MySqlConnection, Row, SqlSafeStr,
    migrate::{Migration, MigrationType, Migrator},
};
use std::{borrow::Cow, collections::BTreeSet};

pub(super) const BASELINE: &str = include_str!("../../migrations/mysql/1_application.sql");
const JOURNAL: &str = "_ruphish_upgrade";
pub(super) const STAGE: &str = "rp_stage_";
const ORIGINAL: &str = "rp_original_";

fn baseline() -> Migration {
    Migration::new(
        1,
        Cow::Borrowed("application"),
        MigrationType::Simple,
        BASELINE.into_sql_str(),
        false,
    )
}

fn migrator() -> Migrator {
    Migrator {
        migrations: Cow::Owned(vec![baseline()]),
        locking: false,
        ..Migrator::DEFAULT
    }
}

pub(super) fn tables() -> BTreeSet<String> {
    BASELINE
        .lines()
        .filter_map(|line| line.strip_prefix("CREATE TABLE "))
        .filter_map(|line| line.split_whitespace().next())
        .map(|name| name.trim_matches('`').to_owned())
        .collect()
}

pub(super) async fn names(c: &mut MySqlConnection) -> Result<BTreeSet<String>, Error> {
    sqlx::query_scalar::<_, String>("SELECT TABLE_NAME FROM information_schema.TABLES WHERE TABLE_SCHEMA=DATABASE() ORDER BY TABLE_NAME")
        .fetch_all(c).await.map(|v| v.into_iter().collect()).map_err(storage_error)
}

pub(super) async fn prepare(c: &mut MySqlConnection, config: &MySqlConfig) -> Result<(), Error> {
    let (_, lock) = preflight::acquire(c, config).await?;
    let existing = names(c).await?;
    if existing.contains(JOURNAL) {
        resume(c, config).await?;
    } else if existing.contains("_sqlx_migrations") {
        // SQLx checks unknown, dirty and modified versions. It never blesses them.
        migrator()
            .run_direct(None, &mut *c, false)
            .await
            .map_err(|_| Error::UnsupportedSchema)?;
    } else {
        match preflight::assess(c).await? {
            MySqlSchema::Empty => {
                migrator()
                    .run_direct(None, &mut *c, false)
                    .await
                    .map_err(|_| Error::UnsupportedSchema)?;
            }
            MySqlSchema::Gophish {
                version,
                requires_legacy_timezone,
            } => {
                if requires_legacy_timezone && config.legacy_timezone.is_none() {
                    return Err(Error::Invalid("Pre-UTC Gophish dates require mysql.legacy_timezone (an explicit UTC offset, for example +00:00)".into()));
                }
                super::upgrade::validate_source(c, version).await?;
                sqlx::raw_sql("CREATE TABLE _ruphish_upgrade(id BIGINT NOT NULL PRIMARY KEY, source_version BIGINT NOT NULL, checksum VARCHAR(64) NOT NULL, phase VARCHAR(32) NOT NULL, source_tables LONGTEXT NOT NULL, timezone VARCHAR(6) NOT NULL) ENGINE=InnoDB").execute(&mut *c).await.map_err(storage_error)?;
                sqlx::query("INSERT INTO _ruphish_upgrade VALUES(1,?,?,'preparing',?,?)")
                    .bind(version)
                    .bind(checksum())
                    .bind(serde_json::to_string(&existing).map_err(|_| Error::UnsupportedSchema)?)
                    .bind(config.legacy_timezone.as_deref().unwrap_or("+00:00"))
                    .execute(&mut *c)
                    .await
                    .map_err(storage_error)?;
                resume(c, config).await?;
            }
        }
    }
    validate_current(c).await?;
    preflight::release(c, &lock).await
}

fn checksum() -> String {
    let mut hash = Sha256::new();
    hash.update(BASELINE.as_bytes());
    hash.update(include_bytes!("upgrade.rs"));
    hex::encode(hash.finalize())
}

async fn resume(c: &mut MySqlConnection, _config: &MySqlConfig) -> Result<(), Error> {
    let row = sqlx::query("SELECT source_version,checksum,phase,source_tables,timezone FROM _ruphish_upgrade WHERE id=1")
        .fetch_optional(&mut *c).await.map_err(storage_error)?.ok_or(Error::UnsupportedSchema)?;
    if row
        .try_get::<String, _>("checksum")
        .map_err(storage_error)?
        != checksum()
    {
        return Err(Error::UnsupportedSchema);
    }
    let source: BTreeSet<String> = serde_json::from_str(
        &row.try_get::<String, _>("source_tables")
            .map_err(storage_error)?,
    )
    .map_err(|_| Error::UnsupportedSchema)?;
    if source.is_empty()
        || source
            .iter()
            .any(|s| !identifier(s) || s.starts_with("rp_") || s.starts_with('_'))
    {
        return Err(Error::UnsupportedSchema);
    }
    let phase: String = row.try_get("phase").map_err(storage_error)?;
    let version = row.try_get("source_version").map_err(storage_error)?;
    let zone: String = row.try_get("timezone").map_err(storage_error)?;
    let expected = tables();
    let actual = names(c).await?;
    let published = source
        .iter()
        .all(|name| actual.contains(&format!("{ORIGINAL}{name}")))
        && expected.iter().all(|name| actual.contains(name))
        && !actual.iter().any(|name| name.starts_with(STAGE));
    if !published {
        if !matches!(phase.as_str(), "preparing" | "copied")
            || !source.iter().all(|name| actual.contains(name))
            || actual.iter().any(|name| name.starts_with(ORIGINAL))
        {
            return Err(Error::UnsupportedSchema);
        }
        let allowed: BTreeSet<_> = source
            .iter()
            .cloned()
            .chain(expected.iter().map(|n| format!("{STAGE}{n}")))
            .chain([JOURNAL.to_owned()])
            .collect();
        if !actual.is_subset(&allowed) {
            return Err(Error::UnsupportedSchema);
        }
        // A committed copy can be published after interruption without copying again.
        if phase == "preparing" {
            // Only staging tables owned by this exact, recognized journal are disposable.
            // The legacy source has not been changed; a partial DDL stage is rebuilt.
            for name in expected.iter().rev() {
                let name = format!("{STAGE}{name}");
                if actual.contains(&name) {
                    statement(c, format!("DROP TABLE `{name}`")).await?;
                }
            }
            create_stage(c).await?;
            let mut tx = c.begin().await.map_err(storage_error)?;
            super::upgrade::copy(&mut tx, version, &zone).await?;
            sqlx::query("UPDATE _ruphish_upgrade SET phase='copied' WHERE id=1")
                .execute(&mut *tx)
                .await
                .map_err(storage_error)?;
            tx.commit().await.map_err(|_| Error::UncertainCommit)?;
        }
        let pairs: Vec<_> = source
            .iter()
            .map(|n| format!("`{n}` TO `{ORIGINAL}{n}`"))
            .chain(expected.iter().map(|n| format!("`{STAGE}{n}` TO `{n}`")))
            .collect();
        statement(c, format!("RENAME TABLE {}", pairs.join(","))).await?;
    }
    // The atomic rename is its own durable publication marker. A disconnect between
    // rename and history publication is reconciled from complete table-name sets.
    let migration = baseline();
    sqlx::raw_sql("CREATE TABLE IF NOT EXISTS _sqlx_migrations (version BIGINT PRIMARY KEY, description TEXT NOT NULL, installed_on TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP, success BOOLEAN NOT NULL, checksum BLOB NOT NULL, execution_time BIGINT NOT NULL)").execute(&mut *c).await.map_err(storage_error)?;
    let mut tx = c.begin().await.map_err(storage_error)?;
    let recorded: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM _sqlx_migrations WHERE version=1)")
            .fetch_one(&mut *tx)
            .await
            .map_err(storage_error)?;
    if !recorded {
        sqlx::query("INSERT INTO _sqlx_migrations(version,description,success,checksum,execution_time) VALUES(1,'application',true,?,0)")
            .bind(migration.checksum.as_ref()).execute(&mut *tx).await.map_err(storage_error)?;
    }
    sqlx::query("UPDATE _ruphish_upgrade SET phase='published' WHERE id=1")
        .execute(&mut *tx)
        .await
        .map_err(storage_error)?;
    tx.commit().await.map_err(|_| Error::UncertainCommit)?;
    migrator()
        .run_direct(None, &mut *c, false)
        .await
        .map_err(|_| Error::UnsupportedSchema)?;
    Ok(())
}

async fn create_stage(c: &mut MySqlConnection) -> Result<(), Error> {
    // This transforms identifiers in our embedded DDL only, never runtime SQL or input.
    // Longest first prevents a table name from matching another name's prefix.
    let mut ddl = BASELINE.to_owned();
    let mut known: Vec<_> = tables().into_iter().collect();
    known.sort_by_key(|n| std::cmp::Reverse(n.len()));
    for name in known {
        ddl = ddl.replace(&format!("`{name}`"), &format!("`{STAGE}{name}`"));
        for prefix in ["CREATE TABLE ", "INSERT INTO "] {
            ddl = ddl.replace(
                &format!("{prefix}{name} "),
                &format!("{prefix}{STAGE}{name} "),
            );
            ddl = ddl.replace(
                &format!("{prefix}{name}("),
                &format!("{prefix}{STAGE}{name}("),
            );
        }
    }
    statement(c, ddl).await
}

pub(super) fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

pub(super) async fn statement(c: &mut MySqlConnection, sql: String) -> Result<(), Error> {
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
        .execute(c)
        .await
        .map_err(storage_error)?;
    Ok(())
}

async fn validate_current(c: &mut MySqlConnection) -> Result<(), Error> {
    let actual = names(c).await?;
    let expected = tables();
    if !expected.is_subset(&actual) {
        return Err(Error::UnsupportedSchema);
    }
    let backup = if actual.contains(JOURNAL) {
        let json: String = sqlx::query_scalar(
            "SELECT source_tables FROM _ruphish_upgrade WHERE id=1 AND phase='published'",
        )
        .fetch_one(&mut *c)
        .await
        .map_err(storage_error)?;
        serde_json::from_str::<BTreeSet<String>>(&json).map_err(|_| Error::UnsupportedSchema)?
    } else {
        BTreeSet::new()
    };
    let allowed: BTreeSet<_> = expected
        .into_iter()
        .chain(["_sqlx_migrations".to_owned()])
        .chain(actual.contains(JOURNAL).then(|| JOURNAL.to_owned()))
        .chain(backup.iter().map(|n| format!("{ORIGINAL}{n}")))
        .collect();
    if actual != allowed {
        return Err(Error::UnsupportedSchema);
    }
    let customized:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM information_schema.TRIGGERS WHERE TRIGGER_SCHEMA=DATABASE()) OR EXISTS(SELECT 1 FROM information_schema.REFERENTIAL_CONSTRAINTS WHERE CONSTRAINT_SCHEMA=DATABASE()) OR EXISTS(SELECT 1 FROM information_schema.TABLES WHERE TABLE_SCHEMA=DATABASE() AND (ENGINE<>'InnoDB' OR TABLE_TYPE<>'BASE TABLE'))").fetch_one(&mut *c).await.map_err(storage_error)?;
    if customized {
        return Err(Error::UnsupportedSchema);
    }
    validate_columns(c).await?;
    Ok(())
}

async fn validate_columns(c: &mut MySqlConnection) -> Result<(), Error> {
    // Generated from the embedded baseline on the supported server. Compare all
    // behavioral metadata, not merely column types or migration-history hashes.
    let expected: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("schema.json")).map_err(|_| Error::UnsupportedSchema)?;
    let rows: Vec<String> = sqlx::query_scalar(include_str!("schema_inventory.sql"))
        .fetch_all(c)
        .await
        .map_err(storage_error)?;
    let application = tables();
    let mut actual = Vec::new();
    for row in rows {
        let value: serde_json::Value =
            serde_json::from_str(&row).map_err(|_| Error::UnsupportedSchema)?;
        if value
            .get(1)
            .and_then(|v| v.as_str())
            .is_some_and(|table| application.contains(table))
        {
            actual.push(value);
        }
    }
    // Sort locally: catalog collations differ between supported server versions.
    let key = |value: &serde_json::Value| value.to_string();
    actual.sort_by_key(&key);
    let mut expected = expected;
    expected.sort_by_key(key);
    if actual != expected {
        return Err(Error::UnsupportedSchema);
    }
    Ok(())
}
