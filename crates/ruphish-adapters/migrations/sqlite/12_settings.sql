CREATE TABLE ruphish_settings (
 id INTEGER NOT NULL PRIMARY KEY CHECK (id=1),
 revision INTEGER NOT NULL,
 verification_enabled INTEGER NOT NULL,
 verification_expires INTEGER NOT NULL,
 verification_validity_seconds INTEGER NOT NULL,
 verification_recheck_seconds INTEGER NOT NULL,
 reporting_mailboxes_enabled INTEGER NOT NULL,
 webhooks_enabled INTEGER NOT NULL,
 updated_at INTEGER NOT NULL,
 updated_by INTEGER
);
INSERT INTO ruphish_settings SELECT 1,1,FALSE,TRUE,86400,21600,EXISTS(SELECT 1 FROM ruphish_mailboxes),EXISTS(SELECT 1 FROM webhooks),0,NULL;
CREATE TABLE ruphish_campaign_closures (
 token VARCHAR(64) NOT NULL PRIMARY KEY,
 actor_id INTEGER NOT NULL,
 expires_at INTEGER NOT NULL,
 campaign_ids TEXT NOT NULL,
 result TEXT
);
CREATE VIEW ruphish_domain_policy AS
 SELECT d.id,
 CASE WHEN NOT s.verification_enabled THEN 9223372036854775807
      WHEN d.verified_until<=0 THEN 0
      WHEN NOT s.verification_expires THEN 9223372036854775807
      ELSE d.checked_at+s.verification_validity_seconds END AS eligible_until,
 (NOT s.verification_enabled OR d.verified_until>0) AS serves,
 s.verification_enabled AS verification_required,
 CASE WHEN s.verification_enabled AND s.verification_expires AND d.verified_until>0
      THEN d.checked_at+s.verification_validity_seconds ELSE NULL END AS expires_at
 FROM ruphish_domains d CROSS JOIN ruphish_settings s WHERE s.id=1;
DROP TRIGGER ruphish_event_insert;
CREATE TRIGGER ruphish_event_insert AFTER INSERT ON events WHEN NEW.source<>'operator' BEGIN
 INSERT INTO ruphish_webhook_jobs(webhook_id,revision,event_id)
 SELECT w.id,s.revision,NEW.id FROM webhooks w JOIN ruphish_webhook_state s ON s.webhook_id=w.id
 WHERE w.is_active=1 AND (SELECT webhooks_enabled FROM ruphish_settings WHERE id=1)=1;
END;
INSERT INTO ruphish_schema VALUES(12,'company-settings-v1');
