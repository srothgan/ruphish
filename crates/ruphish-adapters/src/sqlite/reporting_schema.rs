//! Reporting privacy, provenance and durable integration state.
use sqlx::SqliteConnection;

pub(super) const SCHEMA: &str = r#"
PRAGMA secure_delete=ON;
UPDATE results SET ip='',latitude=0,longitude=0;
CREATE INDEX ruphish_events_campaign_time ON events(campaign_id,time,id);
DROP INDEX ruphish_events_recipient;
CREATE INDEX ruphish_events_recipient ON events(campaign_id,email,id);
CREATE TABLE ruphish_event_sources(event_id INTEGER PRIMARY KEY,source TEXT NOT NULL);
INSERT INTO ruphish_event_sources SELECT id,'legacy' FROM events;
CREATE TABLE ruphish_webhook_state(webhook_id INTEGER PRIMARY KEY,revision INTEGER NOT NULL DEFAULT 1);
INSERT INTO ruphish_webhook_state SELECT id,1 FROM webhooks;
CREATE TABLE ruphish_webhook_jobs(id INTEGER PRIMARY KEY,webhook_id INTEGER NOT NULL,revision INTEGER NOT NULL,event_id INTEGER NOT NULL,state TEXT NOT NULL DEFAULT 'queued',attempt INTEGER NOT NULL DEFAULT 0,due INTEGER NOT NULL DEFAULT 0,lease INTEGER NOT NULL DEFAULT 0,code TEXT NOT NULL DEFAULT '',UNIQUE(webhook_id,event_id));
CREATE INDEX ruphish_webhook_due ON ruphish_webhook_jobs(state,due,lease);
CREATE TABLE ruphish_webhook_attempts(id INTEGER PRIMARY KEY,job_id INTEGER NOT NULL,attempt INTEGER NOT NULL,started INTEGER NOT NULL,finished INTEGER,status INTEGER,code TEXT NOT NULL DEFAULT 'running');
CREATE TRIGGER ruphish_event_insert AFTER INSERT ON events BEGIN
 INSERT INTO ruphish_event_sources VALUES(NEW.id,'application');
 INSERT INTO ruphish_webhook_jobs(webhook_id,revision,event_id) SELECT w.id,s.revision,NEW.id FROM webhooks w JOIN ruphish_webhook_state s ON s.webhook_id=w.id WHERE w.is_active=1;
END;
CREATE TRIGGER ruphish_event_delete AFTER DELETE ON events BEGIN
 DELETE FROM ruphish_webhook_attempts WHERE job_id IN (SELECT id FROM ruphish_webhook_jobs WHERE event_id=OLD.id);
 DELETE FROM ruphish_webhook_jobs WHERE event_id=OLD.id;
 DELETE FROM ruphish_event_sources WHERE event_id=OLD.id;
END;
CREATE TABLE ruphish_mailboxes(id INTEGER PRIMARY KEY,owner_id INTEGER NOT NULL,name TEXT NOT NULL,revision INTEGER NOT NULL DEFAULT 1,enabled INTEGER NOT NULL,host TEXT NOT NULL,port INTEGER NOT NULL CHECK(port BETWEEN 1 AND 65535),username TEXT NOT NULL,password TEXT NOT NULL,tls INTEGER NOT NULL,folder TEXT NOT NULL,restrict_domain TEXT NOT NULL DEFAULT '',delete_reported INTEGER NOT NULL DEFAULT 0,frequency INTEGER NOT NULL DEFAULT 60,uidvalidity INTEGER NOT NULL DEFAULT 0,last_uid INTEGER NOT NULL DEFAULT 0,initialized INTEGER NOT NULL DEFAULT 0,next_check INTEGER NOT NULL DEFAULT 0,lease INTEGER NOT NULL DEFAULT 0,last_check INTEGER NOT NULL DEFAULT 0,last_success INTEGER NOT NULL DEFAULT 0,modified_date TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),last_login TEXT NOT NULL DEFAULT '0001-01-01T00:00:00Z',failures INTEGER NOT NULL DEFAULT 0,code TEXT NOT NULL DEFAULT '',ignore_cert_errors INTEGER NOT NULL DEFAULT 0 CHECK(ignore_cert_errors IN (0,1)),UNIQUE(owner_id,name));
CREATE TABLE ruphish_mailbox_messages(mailbox_id INTEGER NOT NULL,uidvalidity INTEGER NOT NULL,uid INTEGER NOT NULL,state TEXT NOT NULL,code TEXT NOT NULL,acknowledged INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(mailbox_id,uidvalidity,uid));
CREATE TABLE ruphish_mailbox_runs(id INTEGER PRIMARY KEY,mailbox_id INTEGER NOT NULL,started INTEGER NOT NULL,finished INTEGER,code TEXT NOT NULL DEFAULT 'running',matched INTEGER NOT NULL DEFAULT 0,rejected INTEGER NOT NULL DEFAULT 0);
INSERT INTO ruphish_mailboxes(owner_id,name,enabled,host,port,username,password,tls,folder,restrict_domain,delete_reported,frequency,modified_date,last_login,ignore_cert_errors)
 SELECT user_id,'Imported reporting mailbox ' || rowid,CASE WHEN port BETWEEN 1 AND 65535 AND length(host)>0 AND length(username)>0 AND length(password)>0 THEN COALESCE(enabled,0) ELSE 0 END,COALESCE(host,''),CASE WHEN port BETWEEN 1 AND 65535 THEN port ELSE 993 END,COALESCE(username,''),COALESCE(password,''),COALESCE(tls,1),COALESCE(NULLIF(folder,''),'INBOX'),COALESCE(restrict_domain,''),COALESCE(delete_reported_campaign_email,0),CASE WHEN imap_freq>=30 AND imap_freq<=31540000 THEN imap_freq ELSE 60 END,COALESCE(NULLIF(modified_date,''),'0001-01-01T00:00:00Z'),COALESCE(NULLIF(last_login,''),'0001-01-01T00:00:00Z'),COALESCE(ignore_cert_errors,0)<>0 FROM imap;
DELETE FROM imap;
INSERT INTO ruphish_schema VALUES(6,'analysis-integrations-v1');
"#;

pub(super) async fn migrate(c: &mut SqliteConnection) -> Result<(), sqlx::Error> {
    sqlx::raw_sql(SCHEMA).execute(c).await?;
    Ok(())
}
