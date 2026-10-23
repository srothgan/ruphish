-- Preserve history and existing retry identities.
ALTER TABLE events RENAME COLUMN message TO kind;
ALTER TABLE events RENAME COLUMN details TO metadata;
ALTER TABLE events ADD COLUMN recipient_id BIGINT;
ALTER TABLE events ADD COLUMN source VARCHAR(255) COLLATE "C" NOT NULL DEFAULT 'application';
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
CREATE VIEW event_history AS SELECT id,campaign_id,email,time,CASE kind WHEN 'campaign_created' THEN 'Campaign Created' WHEN 'email_sent' THEN 'Email Sent' WHEN 'email_error' THEN 'Error Sending Email' WHEN 'email_opened' THEN 'Email Opened' WHEN 'link_clicked' THEN 'Clicked Link' WHEN 'form_submitted' THEN 'Submitted Data' WHEN 'email_reported' THEN 'Email Reported' WHEN 'delivery_reconciled' THEN 'Delivery reconciled' ELSE kind END::text AS message,metadata AS details,recipient_id,source FROM events;
