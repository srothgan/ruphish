//! Display preferences extend the legacy account without rewriting it.
use super::UpgradeError;
use sqlx::SqliteConnection;

pub(super) async fn migrate(connection: &mut SqliteConnection) -> Result<(), UpgradeError> {
    sqlx::raw_sql("CREATE TABLE ruphish_user_preferences(user_id INTEGER PRIMARY KEY NOT NULL, language TEXT, format_locale TEXT, time_zone TEXT); INSERT INTO ruphish_schema VALUES(7,'user-preferences-v1');").execute(connection).await?;
    Ok(())
}

pub(super) async fn offset(connection: &mut SqliteConnection) -> Result<(), UpgradeError> {
    sqlx::raw_sql("ALTER TABLE ruphish_user_preferences ADD COLUMN show_utc_offset INTEGER NOT NULL DEFAULT 1; INSERT INTO ruphish_schema VALUES(8,'utc-offset-preference-v1');").execute(connection).await?;
    Ok(())
}
