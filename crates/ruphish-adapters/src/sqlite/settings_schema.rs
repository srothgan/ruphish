//! Installation policy and replayable administrative campaign closure.
use super::UpgradeError;
use sqlx::SqliteConnection;
pub(super) async fn migrate(c: &mut SqliteConnection) -> Result<(), UpgradeError> {
    sqlx::raw_sql(include_str!("../../migrations/sqlite/12_settings.sql"))
        .execute(c)
        .await?;
    Ok(())
}
