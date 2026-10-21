//! Additive domains and public-event migration. No legacy origin is trusted implicitly.
use super::UpgradeError;
use sqlx::{Row, SqliteConnection};

pub(super) async fn migrate(c: &mut SqliteConnection) -> Result<(), UpgradeError> {
    sqlx::raw_sql("CREATE TABLE ruphish_domains(id INTEGER PRIMARY KEY,owner_id INTEGER NOT NULL,name TEXT NOT NULL,origin TEXT NOT NULL,hostname TEXT NOT NULL,enabled INTEGER NOT NULL DEFAULT 1,revision INTEGER NOT NULL DEFAULT 1,challenge TEXT NOT NULL,challenge_expires INTEGER NOT NULL,checked_at INTEGER NOT NULL DEFAULT 0,verified_until INTEGER NOT NULL DEFAULT 0,control INTEGER NOT NULL DEFAULT 0,dns INTEGER NOT NULL DEFAULT 0,routing INTEGER NOT NULL DEFAULT 0,tls INTEGER NOT NULL DEFAULT 0,code TEXT NOT NULL DEFAULT 'domain_not_verified',due INTEGER NOT NULL DEFAULT 0,lease_until INTEGER NOT NULL DEFAULT 0,generation INTEGER NOT NULL DEFAULT 0,failures INTEGER NOT NULL DEFAULT 0,routing_proof TEXT NOT NULL DEFAULT '',UNIQUE(owner_id,origin));
CREATE INDEX ruphish_domains_due ON ruphish_domains(due,lease_until);
CREATE TABLE ruphish_domain_claims(hostname TEXT PRIMARY KEY,owner_id INTEGER NOT NULL);
CREATE TABLE ruphish_campaign_domains(campaign_id INTEGER PRIMARY KEY,domain_id INTEGER NOT NULL);
CREATE TABLE ruphish_submission_ids(result_id INTEGER NOT NULL,event_id TEXT NOT NULL,PRIMARY KEY(result_id,event_id));
CREATE INDEX ruphish_events_recipient ON events(campaign_id,email);
INSERT INTO ruphish_schema VALUES(4,'verified-domains-public-events-v1');").execute(&mut *c).await?;
    let rows = sqlx::query("SELECT id,user_id,url FROM campaigns ORDER BY id")
        .fetch_all(&mut *c)
        .await?;
    let now = ruphish_application::datetime::now().as_second();
    for row in rows {
        let value: String = row.try_get::<Option<String>, _>("url")?.unwrap_or_default();
        let Ok(origin) = ruphish_domain::domains::origin(&value, true) else {
            continue;
        };
        let owner: i64 = row.try_get("user_id")?;
        let mut bytes = [0; 32];
        getrandom::fill(&mut bytes).map_err(|_| {
            UpgradeError::Unrecognized("Domain challenge entropy unavailable".into())
        })?;
        sqlx::query("INSERT OR IGNORE INTO ruphish_domains(owner_id,name,origin,hostname,challenge,challenge_expires) VALUES(?,?,?,?,?,?)").bind(owner).bind(&origin.host).bind(&origin.value).bind(&origin.host).bind(hex::encode(bytes)).bind(now+ruphish_domain::domains::CHALLENGE_LIFETIME).execute(&mut *c).await?;
        sqlx::query("INSERT INTO ruphish_campaign_domains SELECT ?,id FROM ruphish_domains WHERE owner_id=? AND origin=?").bind(row.try_get::<i64,_>("id")?).bind(owner).bind(origin.value).execute(&mut *c).await?;
    }
    Ok(())
}

/// Persist active attempts and a bounded recent history without inventing past timings.
pub(super) async fn verification_history(c: &mut SqliteConnection) -> Result<(), UpgradeError> {
    sqlx::raw_sql("CREATE TABLE ruphish_domain_checks(id INTEGER PRIMARY KEY AUTOINCREMENT,domain_id INTEGER NOT NULL,queued_at INTEGER NOT NULL,scheduled_at INTEGER NOT NULL,started_at INTEGER,finished_at INTEGER,outcome TEXT CHECK(outcome IN ('passed','failed','timed_out')),code TEXT NOT NULL DEFAULT '',CHECK((finished_at IS NULL AND outcome IS NULL) OR (finished_at IS NOT NULL AND outcome IS NOT NULL)));
CREATE UNIQUE INDEX ruphish_domain_checks_active ON ruphish_domain_checks(domain_id) WHERE finished_at IS NULL;
CREATE INDEX ruphish_domain_checks_recent ON ruphish_domain_checks(domain_id,id DESC);
INSERT INTO ruphish_domain_checks(domain_id,queued_at,scheduled_at,finished_at,outcome,code) SELECT id,checked_at,checked_at,checked_at,CASE WHEN code='verified' THEN 'passed' WHEN code='domain_check_timeout' THEN 'timed_out' ELSE 'failed' END,code FROM ruphish_domains WHERE checked_at>0;
INSERT INTO ruphish_domain_checks(domain_id,queued_at,scheduled_at) SELECT id,0,due FROM ruphish_domains WHERE enabled=1 AND checked_at=0;
UPDATE ruphish_domains SET lease_until=0,generation=generation+1,routing_proof='',verified_until=CASE WHEN checked_at>0 AND code!='verified' THEN 0 ELSE verified_until END;
INSERT INTO ruphish_schema VALUES(5,'domain-verification-history-v1');").execute(&mut *c).await?;
    Ok(())
}
