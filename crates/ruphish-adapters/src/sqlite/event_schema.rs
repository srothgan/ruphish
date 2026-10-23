//! Stable observation identities and a read-only legacy projection.
use sqlx::SqliteConnection;
pub(super) async fn migrate(c: &mut SqliteConnection) -> Result<(), sqlx::Error> {
    sqlx::raw_sql(r#"
PRAGMA secure_delete=ON;
DROP TRIGGER ruphish_event_insert;
DROP TRIGGER ruphish_event_delete;
ALTER TABLE events RENAME COLUMN message TO kind;
ALTER TABLE events RENAME COLUMN details TO metadata;
ALTER TABLE events ADD COLUMN recipient_id BIGINT;
ALTER TABLE events ADD COLUMN source VARCHAR(255) NOT NULL DEFAULT 'application';
UPDATE events SET kind=CASE kind WHEN 'Campaign Created' THEN 'campaign_created' WHEN 'Email Sent' THEN 'email_sent' WHEN 'Error Sending Email' THEN 'email_error' WHEN 'Email Opened' THEN 'email_opened' WHEN 'Clicked Link' THEN 'link_clicked' WHEN 'Submitted Data' THEN 'form_submitted' WHEN 'Email Reported' THEN 'email_reported' WHEN 'Delivery reconciled' THEN 'delivery_reconciled' ELSE kind END;
UPDATE events SET source=COALESCE((SELECT source FROM ruphish_event_sources s WHERE s.event_id=events.id),'legacy');
UPDATE events SET recipient_id=(SELECT min(r.id) FROM results r WHERE r.campaign_id=events.campaign_id AND r.email=events.email HAVING count(*)=1);
DROP TABLE ruphish_event_sources;
ALTER TABLE ruphish_submission_ids RENAME TO ruphish_event_receipts;
ALTER TABLE ruphish_event_receipts RENAME COLUMN event_id TO request_id;
ALTER TABLE ruphish_event_receipts ADD COLUMN event_id BIGINT;
CREATE INDEX events_recipient_identity ON events(recipient_id,id);
ALTER TABLE pages DROP COLUMN capture_credentials;
ALTER TABLE pages DROP COLUMN capture_passwords;
ALTER TABLE ruphish_campaign_policy DROP COLUMN capture_credentials;
ALTER TABLE ruphish_campaign_policy DROP COLUMN capture_passwords;
CREATE VIEW event_history AS SELECT id,campaign_id,email,time,CASE kind WHEN 'campaign_created' THEN 'Campaign Created' WHEN 'email_sent' THEN 'Email Sent' WHEN 'email_error' THEN 'Error Sending Email' WHEN 'email_opened' THEN 'Email Opened' WHEN 'link_clicked' THEN 'Clicked Link' WHEN 'form_submitted' THEN 'Submitted Data' WHEN 'email_reported' THEN 'Email Reported' WHEN 'delivery_reconciled' THEN 'Delivery reconciled' ELSE kind END AS message,metadata AS details,recipient_id,source FROM events;
CREATE TRIGGER ruphish_event_insert AFTER INSERT ON events BEGIN INSERT INTO ruphish_webhook_jobs(webhook_id,revision,event_id) SELECT w.id,s.revision,NEW.id FROM webhooks w JOIN ruphish_webhook_state s ON s.webhook_id=w.id WHERE w.is_active=1; END;
CREATE TRIGGER ruphish_event_delete AFTER DELETE ON events BEGIN DELETE FROM ruphish_webhook_attempts WHERE job_id IN (SELECT id FROM ruphish_webhook_jobs WHERE event_id=OLD.id); DELETE FROM ruphish_webhook_jobs WHERE event_id=OLD.id; END;
INSERT INTO ruphish_schema VALUES(11,'observations-v1');
"#).execute(&mut *c).await?;
    // Older applications already restricted these values. Rebuild the closed
    // representation during upgrade as well, including hand-imported history.
    let mut after = 0_i64;
    loop {
        let rows: Vec<(i64, Option<String>)> =
            sqlx::query_as("SELECT id,metadata FROM events WHERE id>? ORDER BY id LIMIT 500")
                .bind(after)
                .fetch_all(&mut *c)
                .await?;
        if rows.is_empty() {
            break;
        }
        for (id, metadata) in rows {
            after = id;
            sqlx::query("UPDATE events SET metadata=? WHERE id=?")
                .bind(crate::participant_privacy::details(
                    metadata.as_deref().unwrap_or_default(),
                ))
                .bind(id)
                .execute(&mut *c)
                .await?;
        }
    }
    Ok(())
}
