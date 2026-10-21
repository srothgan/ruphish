//! Additive campaign draft storage, outside the execution tables.
use super::UpgradeError;
use sqlx::SqliteConnection;
pub(super) async fn migrate(c: &mut SqliteConnection) -> Result<(), UpgradeError> {
    sqlx::raw_sql("CREATE TABLE ruphish_campaign_drafts(id TEXT PRIMARY KEY NOT NULL, owner_id INTEGER NOT NULL, revision INTEGER NOT NULL, name TEXT NOT NULL, configuration TEXT NOT NULL, schema_version INTEGER NOT NULL, step INTEGER NOT NULL, created INTEGER NOT NULL, modified INTEGER NOT NULL, campaign_id INTEGER); CREATE INDEX ruphish_campaign_drafts_owner ON ruphish_campaign_drafts(owner_id,id); INSERT INTO ruphish_schema VALUES(9,'campaign-drafts-v1');").execute(c).await?;
    Ok(())
}
