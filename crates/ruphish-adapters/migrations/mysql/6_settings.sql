CREATE TABLE ruphish_settings (
 id BIGINT NOT NULL PRIMARY KEY CHECK (id=1),
 revision BIGINT NOT NULL,
 verification_enabled BOOLEAN NOT NULL,
 verification_expires BOOLEAN NOT NULL,
 verification_validity_seconds BIGINT NOT NULL,
 verification_recheck_seconds BIGINT NOT NULL,
 reporting_mailboxes_enabled BOOLEAN NOT NULL,
 webhooks_enabled BOOLEAN NOT NULL,
 updated_at BIGINT NOT NULL,
 updated_by BIGINT
) ENGINE=InnoDB;
INSERT INTO ruphish_settings SELECT 1,1,FALSE,TRUE,86400,21600,EXISTS(SELECT 1 FROM ruphish_mailboxes),EXISTS(SELECT 1 FROM webhooks),0,NULL;
CREATE TABLE ruphish_campaign_closures (
 token VARCHAR(64) CHARACTER SET ascii COLLATE ascii_bin NOT NULL PRIMARY KEY,
 actor_id BIGINT NOT NULL,
 expires_at BIGINT NOT NULL,
 campaign_ids LONGTEXT NOT NULL,
 result LONGTEXT
) ENGINE=InnoDB;
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
