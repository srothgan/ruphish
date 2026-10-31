//! Persisted company policy and administrator-only campaign closure.
use ruphish_application::{auth::Identity, resources::Error, settings::*};
use sqlx::{Row, SqliteConnection, SqlitePool, sqlite::SqliteRow};

/// Provider implementation sharing the startup-owned pool.
pub struct SettingsStore {
    pool: SqlitePool,
}
impl SettingsStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}
fn storage(e: sqlx::Error) -> Error {
    super::resource_error(e)
}
fn decode(r: SqliteRow) -> Result<Settings, Error> {
    Ok(Settings {
        revision: r.try_get("revision").map_err(storage)?,
        updated_at: r.try_get("updated_at").map_err(storage)?,
        updated_by: r.try_get("updated_by").map_err(storage)?,
        policy: Policy {
            verification_enabled: r.try_get("verification_enabled").map_err(storage)?,
            verification_expires: r.try_get("verification_expires").map_err(storage)?,
            verification_validity_seconds: r
                .try_get("verification_validity_seconds")
                .map_err(storage)?,
            verification_recheck_seconds: r
                .try_get("verification_recheck_seconds")
                .map_err(storage)?,
            reporting_mailboxes_enabled: r
                .try_get("reporting_mailboxes_enabled")
                .map_err(storage)?,
            webhooks_enabled: r.try_get("webhooks_enabled").map_err(storage)?,
        },
    })
}
pub(super) async fn load(c: &mut SqliteConnection) -> Result<Settings, Error> {
    decode(
        sqlx::query("SELECT * FROM ruphish_settings WHERE id=1")
            .fetch_one(c)
            .await
            .map_err(storage)?,
    )
}
pub(super) async fn require_verification(c: &mut SqliteConnection) -> Result<(), Error> {
    if !load(c).await?.policy.verification_enabled {
        return Err(Error::FeatureDisabled("domain_verification"));
    }
    Ok(())
}
pub(super) async fn require_integration(
    c: &mut SqliteConnection,
    mailbox: bool,
) -> Result<(), Error> {
    let p = load(c).await?.policy;
    if if mailbox {
        p.reporting_mailboxes_enabled
    } else {
        p.webhooks_enabled
    } {
        Ok(())
    } else {
        Err(Error::FeatureDisabled(if mailbox {
            "reporting_mailboxes"
        } else {
            "webhooks"
        }))
    }
}
async fn authorize(c: &mut SqliteConnection, caller: Identity, write: bool) -> Result<(), Error> {
    super::resources::authorize(c, caller, write)
        .await?
        .require("modify_system")?;
    Ok(())
}
async fn active(c: &mut SqliteConnection) -> Result<Vec<i64>, Error> {
    sqlx::query_scalar(
        "SELECT id FROM campaigns WHERE status IS NULL OR status<>'Completed' ORDER BY id",
    )
    .fetch_all(c)
    .await
    .map_err(storage)
}
async fn pending(c: &mut SqliteConnection) -> Result<i64, Error> {
    sqlx::query_scalar("SELECT count(*) FROM ruphish_delivery d JOIN campaigns c ON c.id=d.campaign_id WHERE (c.status IS NULL OR c.status<>'Completed') AND d.state IN ('ready','retry','preparing','blocked')").fetch_one(c).await.map_err(storage)
}
#[async_trait::async_trait]
impl Repository for SettingsStore {
    /// Read policy after checking administrator permission.
    async fn get(&self, caller: Identity) -> Result<Settings, Error> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        authorize(&mut tx, caller, false).await?;
        let value = load(&mut tx).await?;
        tx.commit().await.map_err(storage)?;
        Ok(value)
    }
    /// Read presentation flags after authenticating the caller.
    async fn features(&self, caller: Identity) -> Result<Features, Error> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        super::accounts::account(&mut tx, caller.id)
            .await
            .map_err(Error::from)?
            .authenticate(caller)?;
        let p = load(&mut tx).await?.policy;
        tx.commit().await.map_err(storage)?;
        Ok(Features::from(&p))
    }
    /// Atomically replace policy at the expected revision.
    async fn save(
        &self,
        caller: Identity,
        revision: i64,
        policy: Policy,
        now: i64,
    ) -> Result<Settings, Error> {
        policy.validation().finish().map_err(Error::Validation)?;
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage)?;
        authorize(&mut tx, caller, true).await?;
        let old = load(&mut tx).await?;
        if old.revision != revision {
            return Err(Error::Changed);
        }
        let verification_changed = old.policy.verification_enabled != policy.verification_enabled
            || old.policy.verification_expires != policy.verification_expires
            || old.policy.verification_validity_seconds != policy.verification_validity_seconds
            || old.policy.verification_recheck_seconds != policy.verification_recheck_seconds;
        sqlx::query("UPDATE ruphish_settings SET revision=revision+1,verification_enabled=?,verification_expires=?,verification_validity_seconds=?,verification_recheck_seconds=?,reporting_mailboxes_enabled=?,webhooks_enabled=?,updated_at=?,updated_by=? WHERE id=1")
            .bind(policy.verification_enabled).bind(policy.verification_expires)
            .bind(policy.verification_validity_seconds).bind(policy.verification_recheck_seconds)
            .bind(policy.reporting_mailboxes_enabled).bind(policy.webhooks_enabled).bind(now).bind(caller.id)
            .execute(&mut *tx).await.map_err(storage)?;
        if verification_changed {
            sqlx::query(
                "UPDATE ruphish_domains SET generation=generation+1,lease_until=0,routing_proof=''",
            )
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
            sqlx::query("DELETE FROM ruphish_domain_checks WHERE finished_at IS NULL")
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
            let domains: Vec<(i64, i64, String)> =
                sqlx::query_as("SELECT id,checked_at,origin FROM ruphish_domains ORDER BY id")
                    .fetch_all(&mut *tx)
                    .await
                    .map_err(storage)?;
            for (id, checked, origin_url) in domains {
                let due = if !ruphish_domain::domains::origin(&origin_url, false)
                    .is_ok_and(|o| o.verification_supported())
                {
                    i64::MAX
                } else if policy.verification_enabled {
                    checked
                        .saturating_add(policy.refresh_delay(id))
                        .max(now + id.wrapping_mul(97).rem_euclid(301))
                } else {
                    0
                };
                sqlx::query("UPDATE ruphish_domains SET due=? WHERE id=?")
                    .bind(due)
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(storage)?;
            }
        }
        if old.policy.webhooks_enabled && !policy.webhooks_enabled {
            sqlx::query("UPDATE ruphish_webhook_jobs SET state='cancelled',code='feature_disabled',lease=0 WHERE state IN ('queued','retry','running')").execute(&mut *tx).await.map_err(storage)?;
        }
        if old.policy.reporting_mailboxes_enabled != policy.reporting_mailboxes_enabled {
            sqlx::query("UPDATE ruphish_mailboxes SET revision=revision+1,lease=0,next_check=0")
                .execute(&mut *tx)
                .await
                .map_err(storage)?;
            sqlx::query("UPDATE ruphish_mailbox_runs SET finished=?,code='configuration_changed' WHERE finished IS NULL").bind(now).execute(&mut *tx).await.map_err(storage)?;
        }
        let result = load(&mut tx).await?;
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }
    /// Create an expiring review token for all active campaigns.
    async fn preview_closure(&self, caller: Identity, now: i64) -> Result<ClosurePreview, Error> {
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage)?;
        authorize(&mut tx, caller, true).await?;
        let ids = active(&mut tx).await?;
        let campaigns = ids.len() as i64;
        let pending_deliveries = pending(&mut tx).await?;
        let mut bytes = [0; 32];
        getrandom::fill(&mut bytes).map_err(|_| Error::Internal)?;
        let token = hex::encode(bytes);
        let expires_at = now + 300;
        sqlx::query("DELETE FROM ruphish_campaign_closures WHERE expires_at<=? AND result IS NULL")
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        sqlx::query("INSERT INTO ruphish_campaign_closures(token,actor_id,expires_at,campaign_ids) VALUES(?,?,?,?)")
            .bind(&token).bind(caller.id).bind(expires_at).bind(serde_json::to_string(&ids).map_err(|_| Error::Internal)?)
            .execute(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(ClosurePreview {
            token,
            expires_at,
            campaigns,
            pending_deliveries,
        })
    }
    /// Complete exactly the reviewed set, or replay its saved outcome.
    async fn close_campaigns(
        &self,
        caller: Identity,
        token: &str,
        now: i64,
    ) -> Result<ClosureResult, Error> {
        if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::Invalid("closure_invalid_token".into()));
        }
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage)?;
        authorize(&mut tx, caller, true).await?;
        let row=sqlx::query("SELECT campaign_ids,expires_at,result FROM ruphish_campaign_closures WHERE token=? AND actor_id=?")
            .bind(token).bind(caller.id).fetch_optional(&mut *tx).await.map_err(storage)?.ok_or(Error::NotFound("Confirmation"))?;
        if let Some(result) = row
            .try_get::<Option<String>, _>("result")
            .map_err(storage)?
        {
            return serde_json::from_str(&result).map_err(|_| Error::Internal);
        }
        if row.try_get::<i64, _>("expires_at").map_err(storage)? <= now {
            return Err(Error::Invalid("closure_confirmation_expired".into()));
        }
        let ids: Vec<i64> =
            serde_json::from_str(&row.try_get::<String, _>("campaign_ids").map_err(storage)?)
                .map_err(|_| Error::Internal)?;
        if ids != active(&mut tx).await? {
            return Err(Error::Conflict("Campaign set"));
        }
        let result = ClosureResult {
            campaigns: ids.len() as i64,
            cancelled_deliveries: pending(&mut tx).await?,
            completed_at: now,
        };
        for id in ids {
            super::campaigns::complete_transaction(&mut tx, id, now).await?;
        }
        sqlx::query("UPDATE ruphish_campaign_closures SET result=? WHERE token=?")
            .bind(serde_json::to_string(&result).map_err(|_| Error::Internal)?)
            .bind(token)
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }
}
