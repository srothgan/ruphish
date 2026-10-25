//! Independently versioned Ruphish authentication tables; legacy Goose SQL is immutable.
use super::UpgradeError;
use sqlx::SqliteConnection;

pub(super) const LATEST_VERSION: i64 = 13;

pub(super) const SCHEMA: &str = "
CREATE TABLE ruphish_schema(version INTEGER PRIMARY KEY NOT NULL, name TEXT NOT NULL);
INSERT INTO ruphish_schema VALUES(1,'authentication-v1');
CREATE TABLE ruphish_auth_state(user_id INTEGER PRIMARY KEY NOT NULL, epoch INTEGER NOT NULL DEFAULT 0);
CREATE TABLE ruphish_sessions(id TEXT PRIMARY KEY NOT NULL, data TEXT NOT NULL, expires INTEGER NOT NULL);
CREATE INDEX ruphish_sessions_expiry ON ruphish_sessions(expires);
CREATE TABLE ruphish_secrets(name TEXT PRIMARY KEY NOT NULL, value BLOB NOT NULL);
";

pub(super) async fn exists(connection: &mut SqliteConnection) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='ruphish_schema')",
    )
    .fetch_one(connection)
    .await
}

pub(super) async fn validate_version(
    connection: &mut SqliteConnection,
) -> Result<bool, UpgradeError> {
    if !exists(connection).await? {
        return Ok(false);
    }
    let entries: Vec<(i64, String)> =
        sqlx::query_as("SELECT version,name FROM ruphish_schema ORDER BY version")
            .fetch_all(connection)
            .await?;
    let history = [
        (1, "authentication-v1".to_owned()),
        (2, "campaign-execution-v1".to_owned()),
        (3, "participant-privacy-v1".to_owned()),
        (4, "verified-domains-public-events-v1".to_owned()),
        (5, "domain-verification-history-v1".to_owned()),
        (6, "analysis-integrations-v1".to_owned()),
        (7, "user-preferences-v1".to_owned()),
        (8, "utc-offset-preference-v1".to_owned()),
        (9, "campaign-drafts-v1".to_owned()),
        (10, "integration-identities-v1".to_owned()),
        (11, "observations-v1".to_owned()),
        (12, "company-settings-v1".to_owned()),
        (13, "learning-pages-v1".to_owned()),
    ];
    if entries.is_empty() || !history.starts_with(&entries) {
        return Err(UpgradeError::Unrecognized(
            "Unsupported Ruphish schema history".into(),
        ));
    }
    Ok(true)
}

pub(super) async fn version(connection: &mut SqliteConnection) -> Result<i64, sqlx::Error> {
    if !exists(connection).await? {
        return Ok(0);
    }
    sqlx::query_scalar("SELECT max(version) FROM ruphish_schema")
        .fetch_one(connection)
        .await
}

pub(super) async fn migrate(connection: &mut SqliteConnection) -> Result<(), UpgradeError> {
    if !validate_version(connection).await? {
        sqlx::raw_sql(SCHEMA).execute(&mut *connection).await?;
    }
    if version(connection).await? == 1 {
        super::campaign_schema::migrate(connection).await?;
    }
    if version(connection).await? == 2 {
        // Remove all opaque legacy details; preserving them could retain login data.
        // The verified original remains in the explicitly reported recovery backup.
        sqlx::raw_sql("PRAGMA secure_delete=ON; UPDATE events SET details=''; INSERT INTO ruphish_schema VALUES(3,'participant-privacy-v1');").execute(&mut *connection).await?;
    }
    if version(connection).await? == 3 {
        super::domain_schema::migrate(connection).await?;
    }
    if version(connection).await? == 4 {
        super::domain_schema::verification_history(connection).await?;
    }
    if version(connection).await? == 5 {
        super::reporting_schema::migrate(connection).await?;
    }
    if version(connection).await? == 6 {
        super::preferences_schema::migrate(connection).await?;
    }
    if version(connection).await? == 7 {
        super::preferences_schema::offset(connection).await?;
    }
    if version(connection).await? == 8 {
        super::draft_schema::migrate(connection).await?;
    }
    if version(connection).await? == 9 {
        super::identity_schema::migrate(connection).await?;
    }
    if version(connection).await? == 10 {
        super::event_schema::migrate(connection).await?;
    }
    if version(connection).await? == 11 {
        super::settings_schema::migrate(connection).await?;
    }
    if version(connection).await? == 12 {
        sqlx::raw_sql(include_str!(
            "../../migrations/sqlite/13_learning_pages.sql"
        ))
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}
