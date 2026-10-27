//! Never reuse durable integration identities after deletion. Existing IDs are preserved.
use sqlx::SqliteConnection;

pub(super) async fn migrate(connection: &mut SqliteConnection) -> Result<(), sqlx::Error> {
    sqlx::raw_sql(r#"
DROP TRIGGER ruphish_event_insert;
DROP TRIGGER ruphish_event_delete;
CREATE TABLE ruphish_mailboxes_new(id INTEGER PRIMARY KEY AUTOINCREMENT,owner_id INTEGER NOT NULL,name TEXT NOT NULL,revision INTEGER NOT NULL DEFAULT 1,enabled INTEGER NOT NULL,host TEXT NOT NULL,port INTEGER NOT NULL CHECK(port BETWEEN 1 AND 65535),username TEXT NOT NULL,password TEXT NOT NULL,tls INTEGER NOT NULL,folder TEXT NOT NULL,restrict_domain TEXT NOT NULL DEFAULT '',delete_reported INTEGER NOT NULL DEFAULT 0,frequency INTEGER NOT NULL DEFAULT 60,uidvalidity INTEGER NOT NULL DEFAULT 0,last_uid INTEGER NOT NULL DEFAULT 0,initialized INTEGER NOT NULL DEFAULT 0,next_check INTEGER NOT NULL DEFAULT 0,lease INTEGER NOT NULL DEFAULT 0,last_check INTEGER NOT NULL DEFAULT 0,last_success INTEGER NOT NULL DEFAULT 0,modified_date TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),last_login TEXT NOT NULL DEFAULT '0001-01-01T00:00:00Z',failures INTEGER NOT NULL DEFAULT 0,code TEXT NOT NULL DEFAULT '',ignore_cert_errors INTEGER NOT NULL DEFAULT 0 CHECK(ignore_cert_errors IN (0,1)),UNIQUE(owner_id,name));
INSERT INTO ruphish_mailboxes_new SELECT * FROM ruphish_mailboxes;
DROP TABLE ruphish_mailboxes;
ALTER TABLE ruphish_mailboxes_new RENAME TO ruphish_mailboxes;
CREATE TABLE ruphish_mailbox_runs_new(id INTEGER PRIMARY KEY AUTOINCREMENT,mailbox_id INTEGER NOT NULL,started INTEGER NOT NULL,finished INTEGER,code TEXT NOT NULL DEFAULT 'running',matched INTEGER NOT NULL DEFAULT 0,rejected INTEGER NOT NULL DEFAULT 0);
INSERT INTO ruphish_mailbox_runs_new SELECT * FROM ruphish_mailbox_runs;
DROP TABLE ruphish_mailbox_runs;
ALTER TABLE ruphish_mailbox_runs_new RENAME TO ruphish_mailbox_runs;
CREATE TABLE ruphish_webhook_jobs_new(id INTEGER PRIMARY KEY AUTOINCREMENT,webhook_id INTEGER NOT NULL,revision INTEGER NOT NULL,event_id INTEGER NOT NULL,state TEXT NOT NULL DEFAULT 'queued',attempt INTEGER NOT NULL DEFAULT 0,due INTEGER NOT NULL DEFAULT 0,lease INTEGER NOT NULL DEFAULT 0,code TEXT NOT NULL DEFAULT '',UNIQUE(webhook_id,event_id));
INSERT INTO ruphish_webhook_jobs_new SELECT * FROM ruphish_webhook_jobs;
DROP TABLE ruphish_webhook_jobs;
ALTER TABLE ruphish_webhook_jobs_new RENAME TO ruphish_webhook_jobs;
CREATE TABLE ruphish_webhook_attempts_new(id INTEGER PRIMARY KEY AUTOINCREMENT,job_id INTEGER NOT NULL,attempt INTEGER NOT NULL,started INTEGER NOT NULL,finished INTEGER,status INTEGER,code TEXT NOT NULL DEFAULT 'running');
INSERT INTO ruphish_webhook_attempts_new SELECT * FROM ruphish_webhook_attempts;
DROP TABLE ruphish_webhook_attempts;
ALTER TABLE ruphish_webhook_attempts_new RENAME TO ruphish_webhook_attempts;
CREATE INDEX ruphish_webhook_due ON ruphish_webhook_jobs(state,due,lease);
CREATE TRIGGER ruphish_event_insert AFTER INSERT ON events BEGIN
 INSERT INTO ruphish_event_sources VALUES(NEW.id,'application');
 INSERT INTO ruphish_webhook_jobs(webhook_id,revision,event_id) SELECT w.id,s.revision,NEW.id FROM webhooks w JOIN ruphish_webhook_state s ON s.webhook_id=w.id WHERE w.is_active=1;
END;
CREATE TRIGGER ruphish_event_delete AFTER DELETE ON events BEGIN
 DELETE FROM ruphish_webhook_attempts WHERE job_id IN (SELECT id FROM ruphish_webhook_jobs WHERE event_id=OLD.id);
 DELETE FROM ruphish_webhook_jobs WHERE event_id=OLD.id;
 DELETE FROM ruphish_event_sources WHERE event_id=OLD.id;
END;
INSERT INTO ruphish_schema VALUES(10,'integration-identities-v1');
"#).execute(connection).await?;
    Ok(())
}
