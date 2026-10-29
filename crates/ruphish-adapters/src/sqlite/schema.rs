//! Verify the complete release schema independently of its migration journal.
use super::UpgradeError;
use sqlx::{Connection, SqliteConnection};

async fn inventory(c: &mut SqliteConnection) -> Result<Vec<(String, String, String)>, sqlx::Error> {
    let rows: Vec<(String, String, String)> = sqlx::query_as("SELECT type,name,sql FROM sqlite_master WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%' AND name<>'_sqlx_migrations' ORDER BY type,name")
        .fetch_all(c).await?;
    Ok(rows
        .into_iter()
        .map(|(kind, name, sql)| (kind, name, sql.replace("\r\n", "\n")))
        .collect())
}

pub(super) async fn validate(c: &mut SqliteConnection) -> Result<(), UpgradeError> {
    validate_version(c, i64::MAX).await
}

pub(super) async fn validate_version(
    c: &mut SqliteConnection,
    version: i64,
) -> Result<(), UpgradeError> {
    let mut expected = SqliteConnection::connect("sqlite::memory:").await?;
    for migration in super::migrations::migrator()
        .iter()
        .filter(|m| m.version <= version)
    {
        sqlx::raw_sql(migration.sql.clone())
            .execute(&mut expected)
            .await?;
    }
    let actual = inventory(c).await?;
    let wanted = inventory(&mut expected).await?;
    expected.close().await?;
    if actual != wanted {
        return Err(UpgradeError::Unrecognized(
            "versioned schema objects differ from their migrations".into(),
        ));
    }
    Ok(())
}
