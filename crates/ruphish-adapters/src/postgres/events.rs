//! Event provenance and webhook outbox changes share the event writer's transaction.
use sqlx::PgConnection;

pub(super) async fn publish(
    c: &mut PgConnection,
    event: i64,
    source: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO ruphish_event_sources(event_id,source) VALUES($1,$2)")
        .bind(event)
        .bind(source)
        .execute(&mut *c)
        .await?;
    sqlx::query("INSERT INTO ruphish_webhook_jobs(webhook_id,revision,event_id) SELECT w.id,s.revision,$1 FROM webhooks w JOIN ruphish_webhook_state s ON s.webhook_id=w.id WHERE w.is_active=TRUE").bind(event).execute(c).await?;
    Ok(())
}

pub(super) async fn remove_campaign(
    c: &mut PgConnection,
    campaign: i64,
) -> Result<(), sqlx::Error> {
    for statement in [
        "DELETE FROM ruphish_webhook_attempts WHERE job_id IN (SELECT j.id FROM ruphish_webhook_jobs j JOIN events e ON e.id=j.event_id WHERE e.campaign_id=$1)",
        "DELETE FROM ruphish_webhook_jobs WHERE event_id IN (SELECT id FROM events WHERE campaign_id=$1)",
        "DELETE FROM ruphish_event_sources WHERE event_id IN (SELECT id FROM events WHERE campaign_id=$1)",
    ] {
        sqlx::query(statement)
            .bind(campaign)
            .execute(&mut *c)
            .await?;
    }
    Ok(())
}
