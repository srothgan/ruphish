//! Draft persistence. Execution is created only by the campaign launch transaction.
use super::{Campaigns, storage};
use ruphish_application::{
    auth::Identity,
    campaign::{
        Campaign, Create,
        drafts::{self, Configuration, Draft, Repository},
    },
    resources::Error,
};
use sqlx::{MySqlConnection, Row};
fn decode(row: sqlx::mysql::MySqlRow) -> Result<Draft, Error> {
    let version: i64 = row.try_get("schema_version").map_err(storage)?;
    if version != 1 {
        return Err(Error::Internal);
    }
    let configuration: String = row.try_get("configuration").map_err(storage)?;
    Ok(Draft {
        id: row.try_get("id").map_err(storage)?,
        revision: row.try_get("revision").map_err(storage)?,
        configuration: serde_json::from_str(&configuration).map_err(|_| Error::Internal)?,
        step: row.try_get("step").map_err(storage)?,
        created: row.try_get("created").map_err(storage)?,
        modified: row.try_get("modified").map_err(storage)?,
        campaign_id: row.try_get("campaign_id").map_err(storage)?,
    })
}
pub(super) async fn load(c: &mut MySqlConnection, owner: i64, id: &str) -> Result<Draft, Error> {
    sqlx::query("SELECT * FROM ruphish_campaign_drafts WHERE owner_id=? AND id=?")
        .bind(owner)
        .bind(id)
        .fetch_optional(c)
        .await
        .map_err(storage)?
        .ok_or(Error::NotFound("Campaign draft"))
        .and_then(decode)
}
pub(super) async fn launching(
    c: &mut MySqlConnection,
    owner: i64,
    id: &str,
) -> Result<Draft, Error> {
    sqlx::query("SELECT * FROM ruphish_campaign_drafts WHERE owner_id=? AND id=? FOR UPDATE")
        .bind(owner)
        .bind(id)
        .fetch_optional(c)
        .await
        .map_err(storage)?
        .ok_or(Error::NotFound("Campaign draft"))
        .and_then(decode)
}
#[async_trait::async_trait]
impl Repository for Campaigns {
    async fn save_draft(
        &self,
        caller: Identity,
        id: &str,
        revision: Option<i64>,
        configuration: Configuration,
        step: i64,
        now: i64,
    ) -> Result<Draft, Error> {
        drafts::validate(id, &configuration, step)?;
        let mut tx = crate::mysql::locks::write(&self.pool)
            .await
            .map_err(storage)?;
        super::super::resources::authorize(&mut tx, caller, true).await?;
        let existing = sqlx::query("SELECT * FROM ruphish_campaign_drafts WHERE id=? FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(storage)?;
        if let Some(row) = existing {
            let owner: i64 = row.try_get("owner_id").map_err(storage)?;
            if owner != caller.id {
                return Err(Error::NotFound("Campaign draft"));
            }
            let current = decode(row)?;
            if !drafts::should_write(&current, revision, &configuration, step)? {
                return Ok(current);
            }
            let json = serde_json::to_string(&configuration).map_err(|_| Error::Internal)?;
            sqlx::query("UPDATE ruphish_campaign_drafts SET revision=revision+1,name=?,configuration=?,step=?,modified=? WHERE id=? AND owner_id=?")
                .bind(&configuration.name).bind(json).bind(step).bind(now).bind(id).bind(caller.id).execute(&mut *tx).await.map_err(storage)?;
        } else {
            if revision.is_some() {
                return Err(Error::NotFound("Campaign draft"));
            }
            let json = serde_json::to_string(&configuration).map_err(|_| Error::Internal)?;
            sqlx::query("INSERT INTO ruphish_campaign_drafts(id,owner_id,revision,name,configuration,schema_version,step,created,modified) VALUES(?,?,1,?,?,1,?,?,?)")
                .bind(id).bind(caller.id).bind(&configuration.name).bind(json).bind(step).bind(now).bind(now).execute(&mut *tx).await.map_err(storage)?;
        }
        let saved = load(&mut tx, caller.id, id).await?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::mysql::commit_error(e)))?;
        Ok(saved)
    }
    async fn draft(&self, caller: Identity, id: &str) -> Result<Draft, Error> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        super::super::resources::authorize(&mut tx, caller, false).await?;
        load(&mut tx, caller.id, id).await
    }
    async fn drafts(&self, caller: Identity, after: &str, limit: i64) -> Result<Vec<Draft>, Error> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        super::super::resources::authorize(&mut tx, caller, false).await?;
        sqlx::query("SELECT * FROM ruphish_campaign_drafts WHERE owner_id=? AND campaign_id IS NULL AND id>? ORDER BY id LIMIT ?")
            .bind(caller.id).bind(after).bind(limit.clamp(1,101)).fetch_all(&mut *tx).await.map_err(storage)?.into_iter().map(decode).collect()
    }
    async fn delete_draft(&self, caller: Identity, id: &str, revision: i64) -> Result<(), Error> {
        let mut tx = crate::mysql::locks::write(&self.pool)
            .await
            .map_err(storage)?;
        super::super::resources::authorize(&mut tx, caller, true).await?;
        let existing = launching(&mut tx, caller.id, id).await?;
        if existing.revision != revision || existing.campaign_id.is_some() {
            return Err(Error::Changed);
        }
        sqlx::query("DELETE FROM ruphish_campaign_drafts WHERE id=? AND owner_id=?")
            .bind(id)
            .bind(caller.id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::mysql::commit_error(e)))?;
        Ok(())
    }
    async fn launch_draft(
        &self,
        caller: Identity,
        id: &str,
        revision: i64,
        input: Create,
        now: i64,
    ) -> Result<Campaign, Error> {
        self.create_inner(caller, input, now, Some((id, revision)))
            .await
    }
}
