//! Durable webhook claims, persisted attempts and restart recovery.
use super::integrations::{Integrations, db, now, text};
use ruphish_application::resources::Error;
use sqlx::Row;
impl Integrations {
    /// Run a bounded integration batch. Persistence survives process cancellation.
    pub async fn tick(&self) -> Result<(), Error> {
        let (hooks, mail) = tokio::join!(self.deliver_webhooks(), self.poll_mailboxes());
        hooks?;
        mail
    }
    /// Dispatch at most four claimed webhooks without blocking mailbox scheduling.
    pub async fn deliver_webhooks(&self) -> Result<(), Error> {
        for result in futures_util::future::join_all((0..4).map(|_| self.webhook_once())).await {
            result?;
        }
        Ok(())
    }
    /// Poll at most two independently claimed mailboxes concurrently.
    pub async fn poll_mailboxes(&self) -> Result<(), Error> {
        for result in futures_util::future::join_all((0..2).map(|_| self.mailbox_once())).await {
            result?;
        }
        Ok(())
    }
    async fn webhook_once(&self) -> Result<(), Error> {
        let clock = now();
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await.map_err(db)?;
        sqlx::query(
            r#"
                UPDATE ruphish_webhook_attempts
                SET finished=?,code='outcome_unconfirmed'
                WHERE finished IS NULL
                AND job_id IN (SELECT id
                FROM ruphish_webhook_jobs
                WHERE state='running'
                AND lease<=?)
            "#,
        )
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
                JOIN events e ON e.id=j.event_id
                WHERE j.state IN ('queued','retry')
                AND j.due<=?
                AND w.is_active=1
                ORDER BY j.due,j.id LIMIT 1
            "#).bind(clock).fetch_optional(&mut *tx).await.map_err(db)?;
        let Some(r) = row else {
            tx.commit().await.map_err(db)?;
            return Ok(());
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
        let body = serde_json::to_vec(&serde_json::json!({
            "campaign_id": r.try_get::<i64, _>("campaign_id").map_err(db)?,
            "email": text(&r, "email")?,
            "time": text(&r, "time")?,
            "message": text(&r, "message")?,
            "details": super::participant_privacy::details(&text(&r, "details")?),
        }))
        .map_err(|_| Error::Internal)?;
        let url = text(&r, "url")?;
        let secret = text(&r, "secret")?;
        tx.commit().await.map_err(db)?;
        let outcome =
            crate::webhook_delivery::send(&self.policy, &url, &secret, &body, &event.to_string())
                .await;
        let done = now();
        let retry = outcome.code != "accepted"
            && !matches!(
                outcome.code,
                "webhook_invalid_url" | "webhook_signing_failed"
            )
            && ruphish_domain::reporting::retryable(outcome.status)
            && attempt < 8;
        let state = if outcome.code == "accepted" {
            "accepted"
        } else if retry {
            "retry"
        } else {
            "failed"
        };
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
        .bind(state)
        .bind(outcome.code)
        .bind(
            done + ruphish_domain::reporting::retry_delay(
                attempt as u32,
                (id as u32).wrapping_add(attempt as u32),
            ),
        )
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
