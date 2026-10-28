CREATE TABLE ruphish_event_changes (
 event_id BIGINT NOT NULL,
 revision BIGINT NOT NULL,
 owner_id BIGINT NOT NULL,
 campaign_id BIGINT NOT NULL,
 recipient_id BIGINT NOT NULL,
 actor_id BIGINT NOT NULL,
 request_id VARCHAR(64) NOT NULL,
 request_text LONGTEXT NOT NULL,
 action VARCHAR(16) NOT NULL,
 performed_at BIGINT NOT NULL,
 before_state LONGTEXT,
 after_state LONGTEXT NOT NULL,
 time TEXT NOT NULL,
 message VARCHAR(255) NOT NULL,
 details TEXT NOT NULL,
 excluded BIGINT NOT NULL,
 PRIMARY KEY(event_id,revision),
 UNIQUE(actor_id,request_id)
);
CREATE INDEX ruphish_event_changes_campaign ON ruphish_event_changes(campaign_id);
CREATE VIEW ruphish_event_records AS
 SELECT h.*,COALESCE(a.revision,0) AS revision,COALESCE(a.excluded,0) AS excluded,
 COALESCE(a.time,h.time) AS effective_time,COALESCE(a.message,h.message) AS effective_message,
 COALESCE(a.details,h.details) AS effective_details
 FROM event_history h LEFT JOIN ruphish_event_changes a ON a.event_id=h.id
 AND a.revision=(SELECT max(latest.revision) FROM ruphish_event_changes latest WHERE latest.event_id=h.id);
CREATE VIEW effective_event_history AS
 SELECT id,campaign_id,email,effective_time AS time,effective_message AS message,effective_details AS details,recipient_id,source,revision
 FROM ruphish_event_records WHERE excluded=0;
