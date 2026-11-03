//! Draft persistence. Execution is created only by the campaign launch transaction.
use super::{Campaigns, Operation, idempotency, storage};
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
        attribution: super::super::attribution::read_at(
            &row,
            "owner_id",
            Some(super::stamp(row.try_get("created").map_err(storage)?)?),
        )?,
        id: row.try_get("id").map_err(storage)?,
        revision: row.try_get("revision").map_err(storage)?,
        configuration: serde_json::from_str(&configuration).map_err(|_| Error::Internal)?,
        step: row.try_get("step").map_err(storage)?,
        created: row.try_get("created").map_err(storage)?,
        modified: row.try_get("modified").map_err(storage)?,
        campaign_id: row.try_get("campaign_id").map_err(storage)?,
    })
}
pub(super) async fn load(c: &mut MySqlConnection, id: i64) -> Result<Draft, Error> {
    sqlx::query("SELECT * FROM ruphish_campaign_drafts WHERE id=?")
        .bind(id)
        .fetch_optional(c)
        .await
        .map_err(storage)?
        .ok_or(Error::NotFound("Campaign draft"))
        .and_then(decode)
}
pub(super) async fn launching(c: &mut MySqlConnection, id: i64) -> Result<Draft, Error> {
    sqlx::query("SELECT * FROM ruphish_campaign_drafts WHERE id=? FOR UPDATE")
        .bind(id)
        .fetch_optional(c)
        .await
        .map_err(storage)?
        .ok_or(Error::NotFound("Campaign draft"))
        .and_then(decode)
}
#[async_trait::async_trait]
impl Repository for Campaigns {
    async fn create_draft(
        &self,
        caller: Identity,
        request: (String, String),
        configuration: Configuration,
        step: i64,
        now: i64,
    ) -> Result<ruphish_application::attribution::Saved<Draft>, Error> {
        drafts::validate(&configuration, step)?;
        let mut tx = crate::mysql::locks::write(&self.pool)
            .await
            .map_err(storage)?;
        super::super::resources::authorize(&mut tx, caller, true).await?;
        if let Some(id) =
            idempotency::lookup(&mut tx, caller.id, Operation::Draft, &request).await?
        {
            let record = load(&mut tx, id).await?;
            return super::super::attribution::saved(&mut tx, record).await;
        }
        let json = serde_json::to_string(&configuration).map_err(|_| Error::Internal)?;
        let id = i64::try_from(sqlx::query("INSERT INTO ruphish_campaign_drafts(owner_id,revision,name,configuration,schema_version,step,created,modified) VALUES(?,1,?,?,1,?,?,?)")
            .bind(caller.id).bind(&configuration.name).bind(json).bind(step).bind(now).bind(now).execute(&mut *tx).await.map_err(storage)?.last_insert_id()).map_err(|_| Error::Internal)?;
        sqlx::query("UPDATE ruphish_campaign_drafts SET edited_by=?,edited_at=? WHERE id=?")
            .bind(caller.id)
            .bind(super::stamp(now)?)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        idempotency::remember(&mut tx, caller.id, Operation::Draft, &request, id).await?;
        let saved = load(&mut tx, id).await?;
        let saved = super::super::attribution::saved(&mut tx, saved).await?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::mysql::commit_error(e)))?;
        Ok(saved)
    }

    async fn save_draft(
        &self,
        caller: Identity,
        id: i64,
        revision: Option<i64>,
        configuration: Configuration,
        step: i64,
        now: i64,
    ) -> Result<ruphish_application::attribution::Saved<Draft>, Error> {
        drafts::validate(&configuration, step)?;
        let mut tx = crate::mysql::locks::write(&self.pool)
            .await
            .map_err(storage)?;
        super::super::resources::authorize(&mut tx, caller, true).await?;
        let current = launching(&mut tx, id).await?;
        if !drafts::should_write(&current, revision, &configuration, step)? {
            return super::super::attribution::saved(&mut tx, current).await;
        }
        let json = serde_json::to_string(&configuration).map_err(|_| Error::Internal)?;
        sqlx::query("UPDATE ruphish_campaign_drafts SET revision=revision+1,name=?,configuration=?,step=?,modified=? WHERE id=? ")
                .bind(&configuration.name).bind(json).bind(step).bind(now).bind(id).execute(&mut *tx).await.map_err(storage)?;
        sqlx::query("UPDATE ruphish_campaign_drafts SET edited_by=?,edited_at=? WHERE id=?")
            .bind(caller.id)
            .bind(super::stamp(now)?)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        let saved = load(&mut tx, id).await?;
        let saved = super::super::attribution::saved(&mut tx, saved).await?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::mysql::commit_error(e)))?;
        Ok(saved)
    }
    async fn draft(&self, caller: Identity, id: i64) -> Result<Draft, Error> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        super::super::resources::authorize(&mut tx, caller, false).await?;
        load(&mut tx, id).await
    }
    async fn drafts(&self, caller: Identity, after: i64, limit: i64) -> Result<Vec<Draft>, Error> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        super::super::resources::authorize(&mut tx, caller, false).await?;
        sqlx::query("SELECT * FROM ruphish_campaign_drafts WHERE campaign_id IS NULL AND id>? ORDER BY id LIMIT ?")
            .bind(after).bind(limit.clamp(1,101)).fetch_all(&mut *tx).await.map_err(storage)?.into_iter().map(decode).collect()
    }
    async fn delete_draft(&self, caller: Identity, id: i64, revision: i64) -> Result<(), Error> {
        let mut tx = crate::mysql::locks::write(&self.pool)
            .await
            .map_err(storage)?;
        super::super::resources::authorize(&mut tx, caller, true).await?;
        let existing = launching(&mut tx, id).await?;
        if existing.revision != revision || existing.campaign_id.is_some() {
            return Err(Error::Changed);
        }
        sqlx::query("DELETE FROM ruphish_campaign_drafts WHERE id=? ")
            .bind(id)
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
        id: i64,
        revision: i64,
        input: Create,
        now: i64,
    ) -> Result<ruphish_application::attribution::Saved<Campaign>, Error> {
        self.create_inner(caller, input, now, Some((id, revision)))
            .await
    }
}
