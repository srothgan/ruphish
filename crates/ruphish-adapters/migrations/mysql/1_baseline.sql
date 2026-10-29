-- Ruphish release baseline. Fresh installs and direct Gophish conversions share this schema.

CREATE TABLE `users` (
  `id` BIGINT NOT NULL AUTO_INCREMENT,
  `username` VARCHAR(255) NOT NULL,
  `hash` VARCHAR(255),
  `api_key` VARCHAR(255) NOT NULL,
  `role_id` BIGINT,
  `password_change_required` BOOLEAN,
  `last_login` VARCHAR(40),
  `account_locked` BOOLEAN,
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `templates` (
  `id` BIGINT NOT NULL AUTO_INCREMENT,
  `user_id` BIGINT,
  `name` VARCHAR(255),
  `subject` VARCHAR(255),
  `text` LONGTEXT,
  `html` LONGTEXT,
  `modified_date` VARCHAR(40),
  `envelope_sender` VARCHAR(255),
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `targets` (
  `id` BIGINT NOT NULL AUTO_INCREMENT,
  `first_name` VARCHAR(255),
  `last_name` VARCHAR(255),
  `email` VARCHAR(255),
  `position` VARCHAR(255),
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `results` (
  `id` BIGINT NOT NULL AUTO_INCREMENT,
  `campaign_id` BIGINT,
  `user_id` BIGINT,
  `r_id` VARCHAR(255),
  `email` VARCHAR(255),
  `first_name` VARCHAR(255),
  `last_name` VARCHAR(255),
  `status` VARCHAR(255) NOT NULL,
  `ip` VARCHAR(255),
  `latitude` DOUBLE,
  `longitude` DOUBLE,
  `position` VARCHAR(255),
  `send_date` VARCHAR(40),
  `reported` BOOLEAN DEFAULT 0,
  `modified_date` VARCHAR(40),
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `pages` (
  `id` BIGINT NOT NULL AUTO_INCREMENT,
  `user_id` BIGINT,
  `name` VARCHAR(255),
  `html` LONGTEXT,
  `modified_date` VARCHAR(40),
  `redirect_url` LONGTEXT,
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `groups` (
  `id` BIGINT NOT NULL AUTO_INCREMENT,
  `user_id` BIGINT,
  `name` VARCHAR(255),
  `modified_date` VARCHAR(40),
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `group_targets` (
  `group_id` BIGINT,
  `target_id` BIGINT
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `events` (
  `id` BIGINT NOT NULL AUTO_INCREMENT,
  `campaign_id` BIGINT,
  `email` VARCHAR(255),
  `time` VARCHAR(40),
  `kind` VARCHAR(255),
  `metadata` LONGTEXT,
  recipient_id BIGINT,
  source VARCHAR(255) NOT NULL DEFAULT 'application',
  edited BOOLEAN NOT NULL DEFAULT FALSE,
  revision BIGINT NOT NULL DEFAULT 0,
  create_key VARCHAR(64),
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `campaigns` (
  `id` BIGINT NOT NULL AUTO_INCREMENT,
  `user_id` BIGINT,
  `name` VARCHAR(255) NOT NULL,
  `created_date` VARCHAR(40),
  `completed_date` VARCHAR(40),
  `template_id` BIGINT,
  `page_id` BIGINT,
  `status` VARCHAR(255),
  `url` LONGTEXT,
  `smtp_id` BIGINT,
  `launch_date` VARCHAR(40),
  `send_by_date` VARCHAR(40),
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `attachments` (
  `id` BIGINT NOT NULL AUTO_INCREMENT,
  `template_id` BIGINT,
  `content` LONGTEXT,
  `type` VARCHAR(255),
  `name` VARCHAR(255),
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `smtp` (
  `id` BIGINT NOT NULL AUTO_INCREMENT,
  `user_id` BIGINT,
  `interface_type` VARCHAR(255),
  `name` VARCHAR(255),
  `host` VARCHAR(255),
  `username` VARCHAR(255),
  `password` LONGTEXT,
  `from_address` VARCHAR(255),
  `modified_date` VARCHAR(40) DEFAULT (DATE_FORMAT(CURRENT_TIMESTAMP(6),'%Y-%m-%dT%H:%i:%s.%fZ')),
  `ignore_cert_errors` BOOLEAN,
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `headers` (
  `id` BIGINT NOT NULL AUTO_INCREMENT,
  `key` VARCHAR(255),
  `value` LONGTEXT,
  `smtp_id` BIGINT,
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `mail_logs` (
  `id` BIGINT NOT NULL AUTO_INCREMENT,
  `campaign_id` BIGINT,
  `user_id` BIGINT,
  `send_date` VARCHAR(40),
  `send_attempt` BIGINT,
  `r_id` VARCHAR(255),
  `processing` BOOLEAN,
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `email_requests` (
  `id` BIGINT NOT NULL AUTO_INCREMENT,
  `user_id` BIGINT,
  `template_id` BIGINT,
  `page_id` BIGINT,
  `first_name` VARCHAR(255),
  `last_name` VARCHAR(255),
  `email` VARCHAR(255),
  `position` VARCHAR(255),
  `url` LONGTEXT,
  `r_id` VARCHAR(255),
  `from_address` VARCHAR(255),
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `roles` (
  `id` BIGINT NOT NULL AUTO_INCREMENT,
  `slug` VARCHAR(255) NOT NULL,
  `name` VARCHAR(255) NOT NULL,
  `description` VARCHAR(255),
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `permissions` (
  `id` BIGINT NOT NULL AUTO_INCREMENT,
  `slug` VARCHAR(255) NOT NULL,
  `name` VARCHAR(255) NOT NULL,
  `description` VARCHAR(255),
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `role_permissions` (
  `role_id` BIGINT NOT NULL,
  `permission_id` BIGINT NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `webhooks` (
  `id` BIGINT NOT NULL AUTO_INCREMENT,
  `name` VARCHAR(255),
  `url` LONGTEXT,
  `secret` LONGTEXT,
  `is_active` BOOLEAN DEFAULT 0,
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `imap` (
  `user_id` BIGINT,
  `host` VARCHAR(255),
  `port` BIGINT,
  `username` VARCHAR(255),
  `password` LONGTEXT,
  `modified_date` VARCHAR(40) DEFAULT (DATE_FORMAT(CURRENT_TIMESTAMP(6),'%Y-%m-%dT%H:%i:%s.%fZ')),
  `tls` BOOLEAN,
  `enabled` BOOLEAN,
  `folder` VARCHAR(255),
  `restrict_domain` VARCHAR(255),
  `delete_reported_campaign_email` BOOLEAN,
  `last_login` VARCHAR(40),
  `imap_freq` BIGINT,
  `ignore_cert_errors` BOOLEAN
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `ruphish_auth_state` (
  `user_id` BIGINT NOT NULL,
  `epoch` BIGINT NOT NULL DEFAULT 0,
  PRIMARY KEY (`user_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `ruphish_sessions` (
  `id` VARCHAR(255) NOT NULL,
  `data` LONGTEXT NOT NULL,
  `expires` BIGINT NOT NULL,
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `ruphish_secrets` (
  `name` VARCHAR(255) NOT NULL,
  `value` VARBINARY(64) NOT NULL,
  PRIMARY KEY (`name`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `ruphish_campaign_groups` (
  `campaign_id` BIGINT NOT NULL,
  `ordinal` BIGINT NOT NULL,
  `group_id` BIGINT NOT NULL,
  PRIMARY KEY (`campaign_id`,`ordinal`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `ruphish_campaign_policy` (
  `campaign_id` BIGINT NOT NULL,
  `redirect_url` LONGTEXT NOT NULL,
  learning_page_id bigint DEFAULT NULL,
  PRIMARY KEY (`campaign_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `ruphish_campaign_launch` (
  `owner_id` BIGINT NOT NULL,
  `key` VARCHAR(255) NOT NULL,
  `fingerprint` VARCHAR(255) NOT NULL,
  `campaign_id` BIGINT NOT NULL,
  PRIMARY KEY (`owner_id`,`key`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `ruphish_delivery` (
  `result_id` BIGINT NOT NULL,
  `campaign_id` BIGINT NOT NULL,
  `state` VARCHAR(255) NOT NULL,
  `due` BIGINT NOT NULL,
  `retries` BIGINT NOT NULL DEFAULT 0,
  `generation` BIGINT NOT NULL DEFAULT 0,
  `lease_until` BIGINT NOT NULL DEFAULT 0,
  `last_claim` BIGINT NOT NULL DEFAULT 0,
  `code` VARCHAR(255) NOT NULL DEFAULT '',
  `smtp_code` BIGINT,
  `stage` VARCHAR(255) NOT NULL DEFAULT '',
  `profile_revision` VARCHAR(255) NOT NULL DEFAULT '',
  PRIMARY KEY (`result_id`),
  CHECK (state IN ('ready','preparing','submitting','retry','accepted','failed','unknown','blocked','cancelled')),
  CHECK (retries BETWEEN 0 AND 8)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `ruphish_delivery_attempt` (
  `result_id` BIGINT NOT NULL,
  `generation` BIGINT NOT NULL,
  `started` BIGINT NOT NULL,
  `finished` BIGINT,
  `state` VARCHAR(255) NOT NULL,
  `stage` VARCHAR(255) NOT NULL DEFAULT '',
  `smtp_code` BIGINT,
  `message_id` VARCHAR(255) NOT NULL,
  `resource_revision` VARCHAR(255) NOT NULL DEFAULT '',
  PRIMARY KEY (`result_id`,`generation`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `ruphish_profile_cooldown` (
  `profile_id` BIGINT NOT NULL,
  `until_time` BIGINT NOT NULL,
  `revision` VARCHAR(255) NOT NULL,
  `blocked` BIGINT NOT NULL DEFAULT 0,
  PRIMARY KEY (`profile_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `ruphish_legacy_queue_issues` (
  `mail_log_id` BIGINT NOT NULL,
  `reason` LONGTEXT NOT NULL,
  PRIMARY KEY (`mail_log_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `ruphish_domains` (
  `id` BIGINT NOT NULL AUTO_INCREMENT,
  `owner_id` BIGINT NOT NULL,
  `name` VARCHAR(255) NOT NULL,
  `origin` VARCHAR(512) NOT NULL,
  `hostname` VARCHAR(255) NOT NULL,
  `enabled` BIGINT NOT NULL DEFAULT 1,
  `revision` BIGINT NOT NULL DEFAULT 1,
  `challenge` VARCHAR(255) NOT NULL,
  `challenge_expires` BIGINT NOT NULL,
  `checked_at` BIGINT NOT NULL DEFAULT 0,
  `verified_until` BIGINT NOT NULL DEFAULT 0,
  `control` BIGINT NOT NULL DEFAULT 0,
  `dns` BIGINT NOT NULL DEFAULT 0,
  `routing` BIGINT NOT NULL DEFAULT 0,
  `tls` BIGINT NOT NULL DEFAULT 0,
  `code` VARCHAR(255) NOT NULL DEFAULT 'domain_not_verified',
  `due` BIGINT NOT NULL DEFAULT 0,
  `lease_until` BIGINT NOT NULL DEFAULT 0,
  `generation` BIGINT NOT NULL DEFAULT 0,
  `failures` BIGINT NOT NULL DEFAULT 0,
  `routing_proof` VARCHAR(255) NOT NULL DEFAULT '',
  PRIMARY KEY (`id`),
  UNIQUE KEY domain_origin (`owner_id`,`origin`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `ruphish_domain_claims` (
  `hostname` VARCHAR(255) NOT NULL,
  `owner_id` BIGINT NOT NULL,
  PRIMARY KEY (`hostname`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `ruphish_campaign_domains` (
  `campaign_id` BIGINT NOT NULL,
  `domain_id` BIGINT NOT NULL,
  PRIMARY KEY (`campaign_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `ruphish_domain_checks` (
  `id` BIGINT NOT NULL AUTO_INCREMENT,
  `domain_id` BIGINT NOT NULL,
  `queued_at` BIGINT NOT NULL,
  `scheduled_at` BIGINT NOT NULL,
  `started_at` BIGINT,
  `finished_at` BIGINT,
  `outcome` VARCHAR(255),
  `code` VARCHAR(255) NOT NULL DEFAULT '',
  PRIMARY KEY (`id`),
  `active_domain` BIGINT GENERATED ALWAYS AS (CASE WHEN finished_at IS NULL THEN domain_id ELSE NULL END) STORED,
  UNIQUE KEY active_domain_check (`active_domain`),
  CHECK (outcome IS NULL OR outcome IN ('passed','failed','timed_out')),
  CHECK ((finished_at IS NULL AND outcome IS NULL) OR (finished_at IS NOT NULL AND outcome IS NOT NULL))
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `ruphish_webhook_state` (
  `webhook_id` BIGINT NOT NULL,
  `revision` BIGINT NOT NULL DEFAULT 1,
  PRIMARY KEY (`webhook_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `ruphish_webhook_jobs` (
  `id` BIGINT NOT NULL AUTO_INCREMENT,
  `webhook_id` BIGINT NOT NULL,
  `revision` BIGINT NOT NULL,
  `event_id` BIGINT NOT NULL,
  `state` VARCHAR(255) NOT NULL DEFAULT 'queued',
  `attempt` BIGINT NOT NULL DEFAULT 0,
  `due` BIGINT NOT NULL DEFAULT 0,
  `lease` BIGINT NOT NULL DEFAULT 0,
  `code` VARCHAR(255) NOT NULL DEFAULT '',
  PRIMARY KEY (`id`),
  UNIQUE KEY webhook_event (`webhook_id`,`event_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `ruphish_webhook_attempts` (
  `id` BIGINT NOT NULL AUTO_INCREMENT,
  `job_id` BIGINT NOT NULL,
  `attempt` BIGINT NOT NULL,
  `started` BIGINT NOT NULL,
  `finished` BIGINT,
  `status` BIGINT,
  `code` VARCHAR(255) NOT NULL DEFAULT 'running',
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `ruphish_mailboxes` (
  `id` BIGINT NOT NULL AUTO_INCREMENT,
  `owner_id` BIGINT NOT NULL,
  `name` VARCHAR(255) NOT NULL,
  `revision` BIGINT NOT NULL DEFAULT 1,
  `enabled` BIGINT NOT NULL,
  `host` VARCHAR(255) NOT NULL,
  `port` BIGINT NOT NULL,
  `username` VARCHAR(255) NOT NULL,
  `password` LONGTEXT NOT NULL,
  `tls` BIGINT NOT NULL,
  `folder` VARCHAR(255) NOT NULL,
  `restrict_domain` VARCHAR(255) NOT NULL DEFAULT '',
  `delete_reported` BIGINT NOT NULL DEFAULT 0,
  `frequency` BIGINT NOT NULL DEFAULT 60,
  `uidvalidity` BIGINT NOT NULL DEFAULT 0,
  `last_uid` BIGINT NOT NULL DEFAULT 0,
  `initialized` BIGINT NOT NULL DEFAULT 0,
  `next_check` BIGINT NOT NULL DEFAULT 0,
  `lease` BIGINT NOT NULL DEFAULT 0,
  `last_check` BIGINT NOT NULL DEFAULT 0,
  `last_success` BIGINT NOT NULL DEFAULT 0,
  `modified_date` VARCHAR(40) NOT NULL DEFAULT (DATE_FORMAT(CURRENT_TIMESTAMP(6),'%Y-%m-%dT%H:%i:%s.%fZ')),
  `last_login` VARCHAR(40) NOT NULL DEFAULT '0001-01-01T00:00:00Z',
  `failures` BIGINT NOT NULL DEFAULT 0,
  `code` VARCHAR(255) NOT NULL DEFAULT '',
  `ignore_cert_errors` BOOLEAN NOT NULL DEFAULT FALSE,
  PRIMARY KEY (`id`),
  UNIQUE KEY mailbox_name (`owner_id`,`name`),
  CHECK (port BETWEEN 1 AND 65535)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `ruphish_mailbox_messages` (
  `mailbox_id` BIGINT NOT NULL,
  `uidvalidity` BIGINT NOT NULL,
  `uid` BIGINT NOT NULL,
  `state` VARCHAR(255) NOT NULL,
  `code` VARCHAR(255) NOT NULL,
  `acknowledged` BIGINT NOT NULL DEFAULT 0,
  PRIMARY KEY (`mailbox_id`,`uidvalidity`,`uid`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `ruphish_mailbox_runs` (
  `id` BIGINT NOT NULL AUTO_INCREMENT,
  `mailbox_id` BIGINT NOT NULL,
  `started` BIGINT NOT NULL,
  `finished` BIGINT,
  `code` VARCHAR(255) NOT NULL DEFAULT 'running',
  `matched` BIGINT NOT NULL DEFAULT 0,
  `rejected` BIGINT NOT NULL DEFAULT 0,
  PRIMARY KEY (`id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `ruphish_write_guards` (
  `scope` VARCHAR(255) NOT NULL,
  PRIMARY KEY (`scope`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE `ruphish_event_receipts` (
  `result_id` BIGINT NOT NULL,
  `request_id` VARCHAR(255) NOT NULL,
  event_id BIGINT,
  PRIMARY KEY (`result_id`,`request_id`)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE ruphish_user_preferences (
  user_id BIGINT NOT NULL PRIMARY KEY,
  language VARCHAR(128),
  format_locale VARCHAR(128),
  time_zone VARCHAR(128),
  show_utc_offset BOOLEAN NOT NULL DEFAULT TRUE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

CREATE TABLE ruphish_campaign_drafts (
 id VARCHAR(64) COLLATE utf8mb4_0900_bin NOT NULL PRIMARY KEY,
 owner_id BIGINT NOT NULL,
 revision BIGINT NOT NULL,
 name VARCHAR(255) COLLATE utf8mb4_0900_bin NOT NULL,
 configuration LONGTEXT NOT NULL,
 schema_version BIGINT NOT NULL,
 step BIGINT NOT NULL,
 created BIGINT NOT NULL,
 modified BIGINT NOT NULL,
 campaign_id BIGINT
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

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

CREATE TABLE ruphish_campaign_closures (
 token VARCHAR(64) CHARACTER SET ascii COLLATE ascii_bin NOT NULL PRIMARY KEY,
 actor_id BIGINT NOT NULL,
 expires_at BIGINT NOT NULL,
 campaign_ids LONGTEXT NOT NULL,
 result LONGTEXT
) ENGINE=InnoDB;

CREATE TABLE ruphish_learning_pages (id bigint NOT NULL AUTO_INCREMENT PRIMARY KEY, user_id bigint NOT NULL, name varchar(255) NOT NULL, modified_date varchar(255) NOT NULL, html longtext DEFAULT NULL, public_id varchar(32) DEFAULT NULL, UNIQUE KEY ruphish_learning_pages_public_id(public_id), UNIQUE KEY ruphish_learning_pages_owner_name(user_id,name)) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_0900_bin;

-- Indexes
CREATE UNIQUE INDEX `users_0` ON `users` (username);
CREATE UNIQUE INDEX `users_1` ON `users` (api_key);
CREATE INDEX `templates_0` ON `templates` (user_id,name);
CREATE INDEX `pages_0` ON `pages` (user_id,name);
CREATE INDEX `groups_0` ON `groups` (user_id,name);
CREATE INDEX `smtp_0` ON `smtp` (user_id,name);
CREATE INDEX `results_0` ON `results` (r_id);
CREATE INDEX `results_1` ON `results` (campaign_id,id);
CREATE INDEX `events_0` ON `events` (campaign_id,time,id);
CREATE INDEX `events_1` ON `events` (campaign_id,email,id);
CREATE INDEX `group_targets_0` ON `group_targets` (group_id);
CREATE INDEX `group_targets_1` ON `group_targets` (target_id);
CREATE INDEX `attachments_0` ON `attachments` (template_id);
CREATE INDEX `headers_0` ON `headers` (smtp_id);
CREATE INDEX `ruphish_sessions_0` ON `ruphish_sessions` (expires);
CREATE INDEX `ruphish_delivery_0` ON `ruphish_delivery` (state,due,campaign_id);
CREATE INDEX `ruphish_delivery_1` ON `ruphish_delivery` (campaign_id,state);
CREATE INDEX `ruphish_domains_0` ON `ruphish_domains` (due,lease_until);
CREATE INDEX `ruphish_domain_checks_0` ON `ruphish_domain_checks` (domain_id,id);
CREATE INDEX `ruphish_webhook_jobs_0` ON `ruphish_webhook_jobs` (state,due,lease);
CREATE INDEX events_recipient_identity ON events(recipient_id,id);
CREATE UNIQUE INDEX events_create_key ON events(campaign_id,recipient_id,create_key);
CREATE INDEX ruphish_campaign_drafts_owner ON ruphish_campaign_drafts(owner_id, id);
CREATE INDEX ruphish_mailbox_runs_unfinished ON ruphish_mailbox_runs(mailbox_id,finished);
CREATE INDEX ruphish_webhook_attempts_unfinished ON ruphish_webhook_attempts(job_id,finished);
CREATE INDEX ruphish_delivery_fairness ON ruphish_delivery(campaign_id,last_claim);
CREATE INDEX ruphish_campaign_template ON campaigns(template_id,status);
CREATE INDEX ruphish_campaign_page ON campaigns(page_id,status);
CREATE INDEX ruphish_campaign_profile ON campaigns(smtp_id,status);
CREATE INDEX ruphish_campaign_learning_page ON ruphish_campaign_policy(learning_page_id,campaign_id);
CREATE INDEX ruphish_mailbox_history ON ruphish_mailbox_runs(mailbox_id,id);
CREATE INDEX ruphish_campaign_domain ON ruphish_campaign_domains(domain_id,campaign_id);

-- Initial data
INSERT INTO ruphish_write_guards VALUES ('accounts'),('cookie-key');
INSERT INTO roles(id,slug,name,description) VALUES (1,'admin','Admin','System administrator with full permissions'),(2,'user','User','User role with edit access to objects and campaigns');
INSERT INTO permissions(id,slug,name,description) VALUES (1,'view_objects','View Objects','View objects'),(2,'modify_objects','Modify Objects','Create and edit objects'),(3,'modify_system','Modify System','Manage system-wide configuration');
INSERT INTO role_permissions VALUES (1,1),(1,2),(1,3),(2,1),(2,2);
INSERT INTO ruphish_settings VALUES (1,1,FALSE,TRUE,86400,21600,FALSE,FALSE,0,NULL);

-- Views
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

CREATE VIEW event_history AS SELECT id,campaign_id,email,time,CASE kind WHEN 'campaign_created' THEN 'Campaign Created' WHEN 'email_sent' THEN 'Email Sent' WHEN 'email_error' THEN 'Error Sending Email' WHEN 'email_opened' THEN 'Email Opened' WHEN 'link_clicked' THEN 'Clicked Link' WHEN 'form_submitted' THEN 'Submitted Data' WHEN 'email_reported' THEN 'Email Reported' WHEN 'delivery_reconciled' THEN 'Delivery reconciled' ELSE kind END AS message,metadata AS details,recipient_id,source,edited,revision FROM events;
