-- Ruphish release baseline. Fresh installs and direct Gophish conversions share this schema.

CREATE TABLE "attachments" (
  "id" integer primary key autoincrement,
  "template_id" bigint,
  "content" varchar(255),
  "type" varchar(255),
  "name" varchar(255)
);

CREATE TABLE "campaigns" (
  "id" integer primary key autoincrement,
  "user_id" bigint,
  "name" varchar(255) NOT NULL,
  "created_date" datetime,
  "completed_date" datetime,
  "template_id" bigint,
  "page_id" bigint,
  "status" varchar(255),
  "url" varchar(255),
  "smtp_id" bigint,
  "launch_date" DATETIME,
  send_by_date DATETIME
);

CREATE TABLE "email_requests" (
  "id" integer primary key autoincrement,
  "user_id" integer,
  "template_id" integer,
  "page_id" integer,
  "first_name" varchar(255),
  "last_name" varchar(255),
  "email" varchar(255),
  "position" varchar(255),
  "url" varchar(255),
  "r_id" varchar(255),
  "from_address" varchar(255)
);

CREATE TABLE "events" (
  "id" integer primary key autoincrement,
  "campaign_id" bigint,
  "email" varchar(255),
  "time" datetime,
  "kind" varchar(255),
  metadata BLOB,
  recipient_id BIGINT,
  source VARCHAR(255) NOT NULL DEFAULT 'application',
  edited BOOLEAN NOT NULL DEFAULT FALSE,
  revision BIGINT NOT NULL DEFAULT 0,
  create_key VARCHAR(64),
  client TEXT
);

CREATE TABLE "group_targets" (
  "group_id" bigint,
  "target_id" bigint
);

CREATE TABLE "groups" (
  "id" integer primary key autoincrement,
  "user_id" bigint,
  "name" varchar(255),
  "modified_date" datetime
);

CREATE TABLE headers (
  id integer primary key autoincrement,
  key varchar(255),
  value varchar(255),
  "smtp_id" bigint
);

CREATE TABLE "imap" (
  "user_id" bigint,
  "host" varchar(255),
  "port" integer,
  "username" varchar(255),
  "password" varchar(255),
  "modified_date" datetime default CURRENT_TIMESTAMP,
  "tls" BOOLEAN,
  "enabled" BOOLEAN,
  "folder" varchar(255),
  "restrict_domain" varchar(255),
  "delete_reported_campaign_email" BOOLEAN,
  "last_login" datetime,
  "imap_freq" integer,
  ignore_cert_errors BOOLEAN
);

CREATE TABLE "mail_logs" (
  "id" integer primary key autoincrement,
  "campaign_id" integer,
  "user_id" integer,
  "send_date" datetime,
  "send_attempt" integer,
  "r_id" varchar(255),
  "processing" boolean
);

CREATE TABLE "pages" (
  "id" integer primary key autoincrement,
  "user_id" bigint,
  "name" varchar(255),
  "html" varchar(255),
  "modified_date" datetime,
  redirect_url VARCHAR(255)
);

CREATE TABLE "permissions" (
  "id"          INTEGER PRIMARY KEY AUTOINCREMENT,
  "slug"        VARCHAR(255) NOT NULL UNIQUE,
  "name"        VARCHAR(255) NOT NULL UNIQUE,
  "description" VARCHAR(255)
);

CREATE TABLE "results" (
  "id" integer primary key autoincrement,
  "campaign_id" bigint,
  "user_id" bigint,
  "r_id" varchar(255),
  "email" varchar(255),
  "first_name" varchar(255),
  "last_name" varchar(255),
  "status" varchar(255) NOT NULL,
  "ip" varchar(255),
  "latitude" real,
  "longitude" real,
  position VARCHAR(255),
  send_date DATETIME,
  reported boolean default 0,
  modified_date DATETIME
);

CREATE TABLE "role_permissions" (
  "role_id"       INTEGER NOT NULL,
  "permission_id" INTEGER NOT NULL
);

CREATE TABLE "roles" (
  "id"          INTEGER PRIMARY KEY AUTOINCREMENT,
  "slug"        VARCHAR(255) NOT NULL UNIQUE,
  "name"        VARCHAR(255) NOT NULL UNIQUE,
  "description" VARCHAR(255)
);

CREATE TABLE ruphish_auth_state (
  user_id INTEGER PRIMARY KEY NOT NULL,
  epoch INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE ruphish_campaign_closures (
  token VARCHAR(64) NOT NULL PRIMARY KEY,
  actor_id INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  campaign_ids TEXT NOT NULL,
  result TEXT
);

CREATE TABLE ruphish_campaign_domains (
  campaign_id INTEGER PRIMARY KEY,
  domain_id INTEGER NOT NULL
);

CREATE TABLE ruphish_campaign_drafts (
  id TEXT PRIMARY KEY NOT NULL,
  owner_id INTEGER NOT NULL,
  revision INTEGER NOT NULL,
  name TEXT NOT NULL,
  configuration TEXT NOT NULL,
  schema_version INTEGER NOT NULL,
  step INTEGER NOT NULL,
  created INTEGER NOT NULL,
  modified INTEGER NOT NULL,
  campaign_id INTEGER
);

CREATE TABLE ruphish_campaign_groups (
  campaign_id INTEGER NOT NULL,
  ordinal INTEGER NOT NULL,
  group_id INTEGER NOT NULL,
  PRIMARY KEY(campaign_id,ordinal)
);

CREATE TABLE ruphish_campaign_launch (
  owner_id INTEGER NOT NULL,
  key TEXT NOT NULL,
  fingerprint TEXT NOT NULL,
  campaign_id INTEGER NOT NULL,
  PRIMARY KEY(owner_id,key)
);

CREATE TABLE ruphish_campaign_policy (
  track_opens INTEGER NOT NULL DEFAULT 1 CHECK(track_opens IN (0,1)),
  campaign_id INTEGER PRIMARY KEY,
  redirect_url TEXT NOT NULL,
  learning_page_id INTEGER
);

CREATE TABLE ruphish_delivery (
  result_id INTEGER PRIMARY KEY,
  campaign_id INTEGER NOT NULL,
  state TEXT NOT NULL CHECK(state IN ('ready','preparing','submitting','retry','accepted','failed','unknown','blocked','cancelled')),
  due INTEGER NOT NULL,
  retries INTEGER NOT NULL DEFAULT 0 CHECK(retries BETWEEN 0 AND 8),
  generation INTEGER NOT NULL DEFAULT 0,
  lease_until INTEGER NOT NULL DEFAULT 0,
  last_claim INTEGER NOT NULL DEFAULT 0,
  code TEXT NOT NULL DEFAULT '',
  smtp_code INTEGER,
  stage TEXT NOT NULL DEFAULT '',
  profile_revision TEXT NOT NULL DEFAULT ''
);

CREATE TABLE ruphish_delivery_attempt (
  result_id INTEGER NOT NULL,
  generation INTEGER NOT NULL,
  started INTEGER NOT NULL,
  finished INTEGER,
  state TEXT NOT NULL,
  stage TEXT NOT NULL DEFAULT '',
  smtp_code INTEGER,
  message_id TEXT NOT NULL,
  resource_revision TEXT NOT NULL DEFAULT '',
  PRIMARY KEY(result_id,generation)
);

CREATE TABLE ruphish_domain_checks (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  domain_id INTEGER NOT NULL,
  queued_at INTEGER NOT NULL,
  scheduled_at INTEGER NOT NULL,
  started_at INTEGER,
  finished_at INTEGER,
  outcome TEXT CHECK(outcome IN ('passed','failed','timed_out')),
  code TEXT NOT NULL DEFAULT '',
  CHECK((finished_at IS NULL AND outcome IS NULL) OR (finished_at IS NOT NULL AND outcome IS NOT NULL))
);

CREATE TABLE ruphish_domain_claims (
  hostname TEXT PRIMARY KEY,
  owner_id INTEGER NOT NULL
);

CREATE TABLE ruphish_domains (
  id INTEGER PRIMARY KEY,
  owner_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  origin TEXT NOT NULL,
  hostname TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  revision INTEGER NOT NULL DEFAULT 1,
  challenge TEXT NOT NULL,
  challenge_expires INTEGER NOT NULL,
  checked_at INTEGER NOT NULL DEFAULT 0,
  verified_until INTEGER NOT NULL DEFAULT 0,
  control INTEGER NOT NULL DEFAULT 0,
  dns INTEGER NOT NULL DEFAULT 0,
  routing INTEGER NOT NULL DEFAULT 0,
  tls INTEGER NOT NULL DEFAULT 0,
  code TEXT NOT NULL DEFAULT 'domain_not_verified',
  due INTEGER NOT NULL DEFAULT 0,
  lease_until INTEGER NOT NULL DEFAULT 0,
  generation INTEGER NOT NULL DEFAULT 0,
  failures INTEGER NOT NULL DEFAULT 0,
  routing_proof TEXT NOT NULL DEFAULT '',
  UNIQUE(owner_id,origin)
);

CREATE TABLE "ruphish_event_receipts" (
  result_id INTEGER NOT NULL,
  request_id TEXT NOT NULL,
  event_id BIGINT,
  PRIMARY KEY(result_id,request_id)
);

CREATE TABLE ruphish_learning_pages (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  modified_date TEXT NOT NULL,
  html TEXT NOT NULL DEFAULT '',
  public_id TEXT UNIQUE
);

CREATE TABLE ruphish_legacy_queue_issues (
  mail_log_id INTEGER PRIMARY KEY,
  reason TEXT NOT NULL
);

CREATE TABLE ruphish_mailbox_messages (
  mailbox_id INTEGER NOT NULL,
  uidvalidity INTEGER NOT NULL,
  uid INTEGER NOT NULL,
  state TEXT NOT NULL,
  code TEXT NOT NULL,
  acknowledged INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY(mailbox_id,uidvalidity,uid)
);

CREATE TABLE "ruphish_mailbox_runs" (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  mailbox_id INTEGER NOT NULL,
  started INTEGER NOT NULL,
  finished INTEGER,
  code TEXT NOT NULL DEFAULT 'running',
  matched INTEGER NOT NULL DEFAULT 0,
  rejected INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE "ruphish_mailboxes" (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  owner_id INTEGER NOT NULL,
  name TEXT NOT NULL,
  revision INTEGER NOT NULL DEFAULT 1,
  enabled INTEGER NOT NULL,
  host TEXT NOT NULL,
  port INTEGER NOT NULL CHECK(port BETWEEN 1 AND 65535),
  username TEXT NOT NULL,
  password TEXT NOT NULL,
  tls INTEGER NOT NULL,
  folder TEXT NOT NULL,
  restrict_domain TEXT NOT NULL DEFAULT '',
  delete_reported INTEGER NOT NULL DEFAULT 0,
  frequency INTEGER NOT NULL DEFAULT 60,
  uidvalidity INTEGER NOT NULL DEFAULT 0,
  last_uid INTEGER NOT NULL DEFAULT 0,
  initialized INTEGER NOT NULL DEFAULT 0,
  next_check INTEGER NOT NULL DEFAULT 0,
  lease INTEGER NOT NULL DEFAULT 0,
  last_check INTEGER NOT NULL DEFAULT 0,
  last_success INTEGER NOT NULL DEFAULT 0,
  modified_date TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  last_login TEXT NOT NULL DEFAULT '0001-01-01T00:00:00Z',
  failures INTEGER NOT NULL DEFAULT 0,
  code TEXT NOT NULL DEFAULT '',
  ignore_cert_errors INTEGER NOT NULL DEFAULT 0 CHECK(ignore_cert_errors IN (0,1)),
  UNIQUE(owner_id,name)
);

CREATE TABLE ruphish_profile_cooldown (
  profile_id INTEGER PRIMARY KEY,
  until_time INTEGER NOT NULL,
  revision TEXT NOT NULL,
  blocked INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE ruphish_secrets (
  name TEXT PRIMARY KEY NOT NULL,
  value BLOB NOT NULL
);

CREATE TABLE ruphish_sessions (
  id TEXT PRIMARY KEY NOT NULL,
  data TEXT NOT NULL,
  expires INTEGER NOT NULL
);

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

CREATE TABLE ruphish_user_preferences (
  user_id INTEGER PRIMARY KEY NOT NULL,
  language TEXT,
  format_locale TEXT,
  time_zone TEXT,
  show_utc_offset INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE "ruphish_webhook_attempts" (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  job_id INTEGER NOT NULL,
  attempt INTEGER NOT NULL,
  started INTEGER NOT NULL,
  finished INTEGER,
  status INTEGER,
  code TEXT NOT NULL DEFAULT 'running'
);

CREATE TABLE "ruphish_webhook_jobs" (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  webhook_id INTEGER NOT NULL,
  revision INTEGER NOT NULL,
  event_id INTEGER NOT NULL,
  state TEXT NOT NULL DEFAULT 'queued',
  attempt INTEGER NOT NULL DEFAULT 0,
  due INTEGER NOT NULL DEFAULT 0,
  lease INTEGER NOT NULL DEFAULT 0,
  code TEXT NOT NULL DEFAULT '',
  UNIQUE(webhook_id,event_id)
);

CREATE TABLE ruphish_webhook_state (
  webhook_id INTEGER PRIMARY KEY,
  revision INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE smtp (
  id integer primary key autoincrement,
  user_id bigint,
  interface_type varchar(255),
  name varchar(255),
  host varchar(255),
  username varchar(255),
  password varchar(255),
  from_address varchar(255),
  modified_date datetime default CURRENT_TIMESTAMP,
  ignore_cert_errors BOOLEAN
);

CREATE TABLE "targets" (
  "id" integer primary key autoincrement,
  "first_name" varchar(255),
  "last_name" varchar(255),
  "email" varchar(255),
  "position" varchar(255)
);

CREATE TABLE "templates" (
  "id" integer primary key autoincrement,
  "user_id" bigint,
  "name" varchar(255),
  "subject" varchar(255),
  "text" varchar(255),
  "html" varchar(255),
  "modified_date" datetime,
  envelope_sender varchar(255)
);

CREATE TABLE "users" (
  "id" integer primary key autoincrement,
  "username" varchar(255) NOT NULL UNIQUE,
  "hash" varchar(255),
  "api_key" varchar(255) NOT NULL UNIQUE,
  "role_id" INTEGER,
  password_change_required BOOLEAN,
  last_login datetime,
  account_locked BOOLEAN
);

CREATE TABLE "webhooks" (
  "id" integer primary key autoincrement,
  "name" varchar(255),
  "url" varchar(1000),
  "secret" varchar(255),
  "is_active" boolean default 0
);

CREATE UNIQUE INDEX events_create_key ON events(campaign_id,recipient_id,create_key);

CREATE INDEX events_recipient_identity ON events(recipient_id,id);

CREATE INDEX ruphish_campaign_domain ON ruphish_campaign_domains(domain_id,campaign_id);

CREATE INDEX ruphish_campaign_drafts_owner ON ruphish_campaign_drafts(owner_id,id);

CREATE INDEX ruphish_campaign_learning_page ON ruphish_campaign_policy(learning_page_id,campaign_id);

CREATE INDEX ruphish_campaign_page ON campaigns(page_id,status);

CREATE INDEX ruphish_campaign_profile ON campaigns(smtp_id,status);

CREATE INDEX ruphish_campaign_template ON campaigns(template_id,status);

CREATE INDEX ruphish_delivery_campaign ON ruphish_delivery(campaign_id,state);

CREATE INDEX ruphish_delivery_due ON ruphish_delivery(state,due,campaign_id);

CREATE INDEX ruphish_delivery_fairness ON ruphish_delivery(campaign_id,last_claim);

CREATE UNIQUE INDEX ruphish_domain_checks_active ON ruphish_domain_checks(domain_id) WHERE finished_at IS NULL;

CREATE INDEX ruphish_domain_checks_recent ON ruphish_domain_checks(domain_id,id DESC);

CREATE INDEX ruphish_domains_due ON ruphish_domains(due,lease_until);

CREATE INDEX ruphish_events_campaign ON events(campaign_id,id);

CREATE INDEX ruphish_events_campaign_time ON events(campaign_id,time,id);

CREATE INDEX ruphish_events_recipient ON events(campaign_id,email,id);

CREATE INDEX ruphish_group_targets_group ON group_targets(group_id);

CREATE INDEX ruphish_groups_owner ON groups(user_id);

CREATE UNIQUE INDEX ruphish_learning_pages_owner_name ON ruphish_learning_pages(user_id,name);

CREATE INDEX ruphish_mailbox_history ON ruphish_mailbox_runs(mailbox_id,id);

CREATE INDEX ruphish_mailbox_runs_unfinished ON ruphish_mailbox_runs(mailbox_id) WHERE finished IS NULL;

CREATE INDEX ruphish_pages_owner ON pages(user_id);

CREATE INDEX ruphish_results_campaign ON results(campaign_id,id);

CREATE INDEX ruphish_results_rid ON results(r_id);

CREATE INDEX ruphish_sessions_expiry ON ruphish_sessions(expires);

CREATE INDEX ruphish_smtp_owner ON smtp(user_id);

CREATE INDEX ruphish_templates_owner ON templates(user_id);

CREATE INDEX ruphish_webhook_attempts_unfinished ON ruphish_webhook_attempts(job_id) WHERE finished IS NULL;

CREATE INDEX ruphish_webhook_due ON ruphish_webhook_jobs(state,due,lease);

CREATE VIEW event_history AS SELECT id,campaign_id,email,time,CASE kind WHEN 'campaign_created' THEN 'Campaign Created' WHEN 'email_sent' THEN 'Email Sent' WHEN 'email_error' THEN 'Error Sending Email' WHEN 'email_opened' THEN 'Email Opened' WHEN 'link_clicked' THEN 'Clicked Link' WHEN 'form_submitted' THEN 'Submitted Data' WHEN 'email_reported' THEN 'Email Reported' WHEN 'delivery_reconciled' THEN 'Delivery reconciled' ELSE kind END AS message,metadata AS details,recipient_id,source,edited,revision,client FROM events;

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

CREATE TRIGGER ruphish_event_delete AFTER DELETE ON events BEGIN DELETE FROM ruphish_webhook_attempts WHERE job_id IN (SELECT id FROM ruphish_webhook_jobs WHERE event_id=OLD.id); DELETE FROM ruphish_webhook_jobs WHERE event_id=OLD.id; END;

CREATE TRIGGER ruphish_event_insert AFTER INSERT ON events WHEN NEW.source<>'operator' BEGIN
 INSERT INTO ruphish_webhook_jobs(webhook_id,revision,event_id)
 SELECT w.id,s.revision,NEW.id FROM webhooks w JOIN ruphish_webhook_state s ON s.webhook_id=w.id
 WHERE w.is_active=1 AND (SELECT webhooks_enabled FROM ruphish_settings WHERE id=1)=1;
END;

-- Initial installation settings. Legacy conversion derives integration flags after import.
INSERT INTO roles VALUES (1,'admin','Admin','System administrator with full permissions');
INSERT INTO roles VALUES (2,'user','User','User role with edit access to objects and campaigns');
INSERT INTO permissions VALUES (1,'view_objects','View Objects','View objects in Gophish');
INSERT INTO permissions VALUES (2,'modify_objects','Modify Objects','Create and edit objects in Gophish');
INSERT INTO permissions VALUES (3,'modify_system','Modify System','Manage system-wide configuration');
INSERT INTO role_permissions VALUES (1,1);
INSERT INTO role_permissions VALUES (2,1);
INSERT INTO role_permissions VALUES (1,2);
INSERT INTO role_permissions VALUES (2,2);
INSERT INTO role_permissions VALUES (1,3);
INSERT INTO ruphish_settings VALUES (1,1,0,1,86400,21600,0,0,0,NULL);
