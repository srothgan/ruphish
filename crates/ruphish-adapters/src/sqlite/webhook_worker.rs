//! Atomic webhook claims and fenced outcome persistence.
use super::integrations::{Integrations, db, text};
use ruphish_application::{
    integrations::{Worker, work::*},
    resources::Error,
};
use sqlx::Row;
#[async_trait::async_trait]
impl Worker for Integrations {
    async fn tick(&self) -> Result<(), Error> {
        let (hooks, mail) = tokio::join!(self.deliver_webhooks(), self.poll_mailboxes());
        hooks?;
        mail
    }
    async fn deliver_webhooks(&self) -> Result<(), Error> {
        crate::integration_work::webhooks(self, &self.policy).await
    }
    async fn poll_mailboxes(&self) -> Result<(), Error> {
        crate::integration_work::mailboxes(self, &self.policy).await
    }
}
#[async_trait::async_trait]
impl WebhookQueue for Integrations {
    async fn claim_webhook(&self, clock: i64) -> Result<Option<WebhookClaim>, Error> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await.map_err(db)?;
        sqlx::query(crate::sqlite::worker_sql::WEBHOOK)
        .bind(clock)
        .bind(clock)
        .execute(&mut *tx)
        .await
        .map_err(db)?;
        sqlx::query(r#"
                UPDATE ruphish_webhook_jobs
                SET state=CASE WHEN attempt>=8 THEN 'failed' ELSE 'retry' END,code='outcome_unconfirmed'
                WHERE state='running'
                AND lease<=?
            "#).bind(clock).execute(&mut *tx).await.map_err(db)?;
        let row=sqlx::query(r#"
                SELECT j.id AS job,j.event_id,j.attempt,w.url,w.secret,e.campaign_id,e.email,e.time,e.message,e.details
                FROM ruphish_webhook_jobs j
                JOIN webhooks w ON w.id=j.webhook_id
                JOIN ruphish_webhook_state s ON s.webhook_id=w.id
                AND s.revision=j.revision
                JOIN event_history e ON e.id=j.event_id
                WHERE j.state IN ('queued','retry')
                AND j.due<=?
                AND w.is_active=1 AND (SELECT webhooks_enabled FROM ruphish_settings WHERE id=1)
                ORDER BY j.due,j.id LIMIT 1
            "#).bind(clock).fetch_optional(&mut *tx).await.map_err(db)?;
        let Some(r) = row else {
            tx.commit().await.map_err(db)?;
            return Ok(None);
        };
        let id: i64 = r.try_get("job").map_err(db)?;
        let event: i64 = r.try_get("event_id").map_err(db)?;
        let attempt: i64 = r.try_get::<i64, _>("attempt").map_err(db)? + 1;
        sqlx::query("UPDATE ruphish_webhook_jobs SET state='running',attempt=?,lease=? WHERE id=?")
            .bind(attempt)
            .bind(clock + 30)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        let run = sqlx::query(
            "INSERT INTO ruphish_webhook_attempts(job_id,attempt,started) VALUES(?,?,?)",
        )
        .bind(id)
        .bind(attempt)
        .bind(clock)
        .execute(&mut *tx)
        .await
        .map_err(db)?
        .last_insert_rowid();
        // Serialize once per attempt from immutable, allowlisted event fields. Never raw payloads.
        let body = crate::integration_work::webhook_payload(
            r.try_get("campaign_id").map_err(db)?,
            text(&r, "email")?,
            text(&r, "time")?,
            text(&r, "message")?,
            &text(&r, "details")?,
        )?;
        let url = text(&r, "url")?;
        let secret = text(&r, "secret")?;
        tx.commit().await.map_err(db)?;
        Ok(Some(WebhookClaim {
            id,
            run,
            event,
            attempt,
            url,
            secret,
            body,
        }))
    }
    async fn webhook_current(&self, claim: &WebhookClaim, now: i64) -> Result<bool, Error> {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ruphish_webhook_jobs j JOIN webhooks w ON w.id=j.webhook_id JOIN ruphish_webhook_state s ON s.webhook_id=w.id AND s.revision=j.revision JOIN ruphish_webhook_attempts a ON a.job_id=j.id AND a.attempt=j.attempt WHERE j.id=? AND j.attempt=? AND a.id=? AND a.finished IS NULL AND j.state='running' AND j.lease>? AND w.is_active=1 AND (SELECT webhooks_enabled FROM ruphish_settings WHERE id=1))")
            .bind(claim.id).bind(claim.attempt).bind(claim.run).bind(now)
            .fetch_one(&self.pool).await.map_err(db)
    }
    async fn finish_webhook(
        &self,
        claim: &WebhookClaim,
        outcome: WebhookCompletion,
        done: i64,
    ) -> Result<(), Error> {
        let (id, attempt, run) = (claim.id, claim.attempt, claim.run);
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await.map_err(db)?;
        let owned = sqlx::query(
            r#"
                UPDATE ruphish_webhook_jobs
                SET state=?,code=?,due=?,lease=0
                WHERE id=?
                AND attempt=?
                AND state='running'
            "#,
        )
        .bind(outcome.state)
        .bind(outcome.code)
        .bind(outcome.due)
        .bind(id)
        .bind(attempt)
        .execute(&mut *tx)
        .await
        .map_err(db)?
        .rows_affected()
            == 1;
        sqlx::query(
            r#"
                UPDATE ruphish_webhook_attempts
                SET finished=?,status=?,code=?
                WHERE id=?
                AND finished IS NULL
            "#,
        )
        .bind(done)
        .bind(outcome.status.map(i64::from))
        .bind(if owned {
            outcome.code
        } else {
            "configuration_changed"
        })
        .bind(run)
        .execute(&mut *tx)
        .await
        .map_err(db)?;
        tx.commit().await.map_err(db)
    }
}
