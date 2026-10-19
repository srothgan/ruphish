//! Versioned campaign execution state. Old mail logs are imported once, never polled.
use super::UpgradeError;
use sqlx::SqliteConnection;

pub(super) const SCHEMA:&str = "
INSERT INTO ruphish_schema VALUES(2,'campaign-execution-v1');
CREATE INDEX ruphish_results_rid ON results(r_id);
CREATE INDEX ruphish_results_campaign ON results(campaign_id,id);
CREATE INDEX ruphish_events_campaign ON events(campaign_id,id);
CREATE INDEX ruphish_group_targets_group ON group_targets(group_id);

CREATE TABLE ruphish_campaign_groups(campaign_id INTEGER NOT NULL, ordinal INTEGER NOT NULL, group_id INTEGER NOT NULL, PRIMARY KEY(campaign_id,ordinal));
CREATE TABLE ruphish_campaign_policy(campaign_id INTEGER PRIMARY KEY, capture_credentials INTEGER NOT NULL, capture_passwords INTEGER NOT NULL, redirect_url TEXT NOT NULL);
CREATE TABLE ruphish_campaign_launch(owner_id INTEGER NOT NULL, key TEXT NOT NULL, fingerprint TEXT NOT NULL, campaign_id INTEGER NOT NULL, PRIMARY KEY(owner_id,key));
CREATE TABLE ruphish_delivery(result_id INTEGER PRIMARY KEY, campaign_id INTEGER NOT NULL, state TEXT NOT NULL CHECK(state IN ('ready','preparing','submitting','retry','accepted','failed','unknown','blocked','cancelled')), due INTEGER NOT NULL, retries INTEGER NOT NULL DEFAULT 0 CHECK(retries BETWEEN 0 AND 8), generation INTEGER NOT NULL DEFAULT 0, lease_until INTEGER NOT NULL DEFAULT 0, last_claim INTEGER NOT NULL DEFAULT 0, code TEXT NOT NULL DEFAULT '', smtp_code INTEGER, stage TEXT NOT NULL DEFAULT '', profile_revision TEXT NOT NULL DEFAULT '');
CREATE INDEX ruphish_delivery_due ON ruphish_delivery(state,due,campaign_id);
CREATE INDEX ruphish_delivery_campaign ON ruphish_delivery(campaign_id,state);
CREATE TABLE ruphish_delivery_attempt(result_id INTEGER NOT NULL,generation INTEGER NOT NULL,started INTEGER NOT NULL,finished INTEGER,state TEXT NOT NULL,stage TEXT NOT NULL DEFAULT '',smtp_code INTEGER,message_id TEXT NOT NULL,resource_revision TEXT NOT NULL DEFAULT '',PRIMARY KEY(result_id,generation));
CREATE TABLE ruphish_profile_cooldown(profile_id INTEGER PRIMARY KEY,until_time INTEGER NOT NULL,revision TEXT NOT NULL,blocked INTEGER NOT NULL DEFAULT 0);
CREATE TABLE ruphish_legacy_queue_issues(mail_log_id INTEGER PRIMARY KEY,reason TEXT NOT NULL);
";

pub(super) async fn migrate(c: &mut SqliteConnection) -> Result<(), UpgradeError> {
    sqlx::raw_sql(SCHEMA).execute(&mut *c).await?;
    sqlx::raw_sql("INSERT INTO ruphish_campaign_policy SELECT c.id,coalesce(p.capture_credentials,0),coalesce(p.capture_passwords,0),coalesce(p.redirect_url,'') FROM campaigns c JOIN pages p ON p.id=c.page_id AND p.user_id=c.user_id;
INSERT INTO ruphish_legacy_queue_issues SELECT m.id,'Unresolved or ambiguous legacy queue association' FROM mail_logs m WHERE (SELECT count(*) FROM results r JOIN campaigns c ON c.id=r.campaign_id JOIN users u ON u.id=c.user_id WHERE r.r_id=m.r_id AND r.user_id=m.user_id AND r.campaign_id=m.campaign_id AND c.user_id=m.user_id)!=1 OR (SELECT count(*) FROM results all_results WHERE all_results.r_id=m.r_id)!=1 OR (SELECT count(*) FROM mail_logs d WHERE d.r_id=m.r_id)!=1 OR coalesce(m.send_attempt,0) NOT BETWEEN 0 AND 8 OR unixepoch(m.send_date) IS NULL;
INSERT INTO ruphish_delivery(result_id,campaign_id,state,due,retries,code) SELECT r.id,m.campaign_id,CASE WHEN c.status='Completed' THEN 'cancelled' WHEN coalesce(m.processing,0)!=0 THEN 'unknown' WHEN p.campaign_id IS NULL THEN 'blocked' ELSE 'ready' END,unixepoch(m.send_date),coalesce(m.send_attempt,0),CASE WHEN coalesce(m.processing,0)!=0 THEN 'legacy_delivery_outcome_unknown' WHEN p.campaign_id IS NULL THEN 'campaign_page_missing' ELSE '' END FROM mail_logs m JOIN results r ON r.r_id=m.r_id AND r.campaign_id=m.campaign_id AND r.user_id=m.user_id JOIN campaigns c ON c.id=m.campaign_id LEFT JOIN ruphish_campaign_policy p ON p.campaign_id=c.id WHERE m.id NOT IN (SELECT mail_log_id FROM ruphish_legacy_queue_issues);
DELETE FROM mail_logs WHERE id NOT IN (SELECT mail_log_id FROM ruphish_legacy_queue_issues);").execute(&mut *c).await?;
    Ok(())
}
