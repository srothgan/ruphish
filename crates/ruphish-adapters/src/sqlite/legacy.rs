//! Recognize immutable Gophish source shapes and convert directly to the release schema.
use super::{UpgradeError, migrations};
use sqlx::{Row, SqliteConnection};
use std::collections::BTreeMap;

type Column = (String, String, i64, Option<String>, i64);
#[derive(serde::Deserialize)]
struct Shape {
    version: i64,
    tables: BTreeMap<String, Vec<Column>>,
}

fn unsupported() -> UpgradeError {
    UpgradeError::Unrecognized("Unsupported legacy schema or history; pre-release Ruphish databases require a fresh installation".into())
}

pub(super) async fn inspect(c: &mut SqliteConnection) -> Result<i64, UpgradeError> {
    let shapes: Vec<Shape> =
        serde_json::from_str(include_str!("legacy-shapes.json")).map_err(|_| unsupported())?;
    let tables: Vec<String> = sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name<>'goose_db_version' ORDER BY name").fetch_all(&mut *c).await?;
    if tables
        .iter()
        .any(|name| name.starts_with("ruphish_") || name.starts_with('_'))
    {
        return Err(unsupported());
    }
    let history: Vec<(i64, i64)> =
        sqlx::query_as("SELECT version_id,is_applied FROM goose_db_version ORDER BY id")
            .fetch_all(&mut *c)
            .await?;
    if history.is_empty() {
        return Err(unsupported());
    }
    let mut applied = BTreeMap::new();
    for (version, state) in history {
        if !matches!(state, 0 | 1) || (version != 0 && !shapes.iter().any(|s| s.version == version))
        {
            return Err(unsupported());
        }
        applied.insert(version, state == 1);
    }
    let count = shapes
        .iter()
        .take_while(|s| applied.get(&s.version) == Some(&true))
        .count();
    if count == 0
        || shapes[count..]
            .iter()
            .any(|s| applied.get(&s.version) == Some(&true))
    {
        return Err(unsupported());
    }
    let shape = &shapes[count - 1];
    if !tables.iter().eq(shape.tables.keys()) {
        return Err(unsupported());
    }
    let custom: bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type IN ('trigger','view') OR (type='index' AND sql IS NOT NULL))").fetch_one(&mut *c).await?;
    if custom {
        return Err(unsupported());
    }
    for (table, expected) in &shape.tables {
        let actual: Vec<Column> = sqlx::query_as(
            "SELECT name,type,\"notnull\",dflt_value,pk FROM pragma_table_info(?) ORDER BY cid",
        )
        .bind(table)
        .fetch_all(&mut *c)
        .await?;
        if &actual != expected {
            return Err(unsupported());
        }
    }
    Ok(shape.version)
}

async fn statement(c: &mut SqliteConnection, sql: String) -> Result<(), UpgradeError> {
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql)).execute(c).await?;
    Ok(())
}

fn instant(column: &str, version: i64) -> String {
    if version < 20170827141312 {
        format!("STRFTIME('%Y-%m-%d %H:%M:%S+00:00', DATETIME({column}, 'utc'))")
    } else {
        column.to_owned()
    }
}

pub(super) async fn convert(c: &mut SqliteConnection, version: i64) -> Result<(), UpgradeError> {
    // The caller holds BEGIN IMMEDIATE and has published a verified recovery snapshot.
    if inspect(c).await? != version {
        return Err(unsupported());
    }
    if version < 20160227180335 {
        let ambiguous:i64=sqlx::query_scalar("SELECT count(*) FROM smtp s LEFT JOIN campaigns c ON c.id=s.campaign_id WHERE c.id IS NULL OR c.user_id IS NULL OR (SELECT count(*) FROM smtp WHERE campaign_id=s.campaign_id)>1").fetch_one(&mut *c).await?;
        if ambiguous != 0 {
            return Err(unsupported());
        }
    }
    if version >= 20191104103306 {
        let orphaned: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM webhooks) AND NOT EXISTS(SELECT 1 FROM users u JOIN role_permissions rp ON rp.role_id=u.role_id JOIN permissions p ON p.id=rp.permission_id WHERE p.slug='modify_system')")
            .fetch_one(&mut *c).await?;
        if orphaned {
            return Err(UpgradeError::Unrecognized("Legacy webhooks require an existing administrator to assign configuration ownership".into()));
        }
    }
    let tables:Vec<String>=sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name<>'goose_db_version' ORDER BY name").fetch_all(&mut *c).await?;
    let mut source = BTreeMap::new();
    for table in &tables {
        // Names were checked against the embedded source inventory above.
        let fields: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_table_info(?) ORDER BY cid")
                .bind(table)
                .fetch_all(&mut *c)
                .await?;
        source.insert(table.clone(), fields);
        statement(
            c,
            format!("ALTER TABLE \"{table}\" RENAME TO \"gophish_{table}\";"),
        )
        .await?;
    }
    sqlx::raw_sql("PRAGMA secure_delete=ON; DROP TABLE goose_db_version;")
        .execute(&mut *c)
        .await?;
    migrations::initialize(c).await?;
    if source.contains_key("roles") {
        sqlx::raw_sql("DELETE FROM role_permissions; DELETE FROM permissions; DELETE FROM roles;")
            .execute(&mut *c)
            .await?;
    }
    for (table, known) in &source {
        if table == "smtp" && version < 20160227180335 {
            continue;
        }
        let fields: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_table_info(?) ORDER BY cid")
                .bind(table)
                .fetch_all(&mut *c)
                .await?;
        let mut names = Vec::new();
        let mut values = Vec::new();
        for name in &fields {
            let value=match (table.as_str(),name.as_str()) {
                ("events","metadata") | ("results","ip") => Some("''".into()),
                ("results","latitude"|"longitude") => Some("0".into()),
                ("events","kind") => Some("CASE message WHEN 'Campaign Created' THEN 'campaign_created' WHEN 'Email Sent' THEN 'email_sent' WHEN 'Error Sending Email' THEN 'email_error' WHEN 'Email Opened' THEN 'email_opened' WHEN 'Clicked Link' THEN 'link_clicked' WHEN 'Submitted Data' THEN 'form_submitted' WHEN 'Email Reported' THEN 'email_reported' WHEN 'Delivery reconciled' THEN 'delivery_reconciled' ELSE message END".into()),
                ("events","source") => Some("'legacy'".into()),
                ("events","recipient_id") => Some("(SELECT min(r.id) FROM gophish_results r WHERE r.campaign_id=gophish_events.campaign_id AND r.email=gophish_events.email HAVING count(*)=1)".into()),
                ("users","role_id") if !known.contains(name) => Some("CASE WHEN id=(SELECT id FROM gophish_users ORDER BY (username='admin') DESC,id LIMIT 1) THEN 1 ELSE 2 END".into()),
                ("results","status") if version<20170104220731 => Some("CASE WHEN status='Success' AND EXISTS(SELECT 1 FROM gophish_events e WHERE e.campaign_id=gophish_results.campaign_id AND e.email=gophish_results.email AND e.message='Submitted Data') THEN 'Submitted Data' WHEN status='Success' AND EXISTS(SELECT 1 FROM gophish_events e WHERE e.campaign_id=gophish_results.campaign_id AND e.email=gophish_results.email AND e.message='Clicked Link') THEN 'Clicked Link' ELSE status END".into()),
                ("campaigns","smtp_id") if version<20160227180335 => Some("(SELECT smtp_id FROM gophish_smtp WHERE campaign_id=gophish_campaigns.id)".into()),
                ("campaigns","launch_date") if !known.contains(name) => Some(instant("created_date",version)),
                ("results","modified_date") if !known.contains(name) => Some(instant("(SELECT max(time) FROM gophish_events e WHERE e.email=gophish_results.email AND e.campaign_id=gophish_results.campaign_id)",version)),
                _ if known.contains(name) => Some(if matches!(name.as_str(),"created_date"|"completed_date"|"launch_date"|"modified_date"|"time") { instant(&format!("\"{name}\""),version) } else { format!("\"{name}\"") }),
                _ => None,
            };
            if let Some(value) = value {
                names.push(format!("\"{name}\""));
                values.push(value);
            }
        }
        statement(
            c,
            format!(
                "INSERT INTO \"{table}\" ({}) SELECT {} FROM \"gophish_{table}\";",
                names.join(","),
                values.join(",")
            ),
        )
        .await?;
        let difference:i64=sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT abs((SELECT count(*) FROM \"{table}\")-(SELECT count(*) FROM \"gophish_{table}\"))"))).fetch_one(&mut *c).await?;
        if difference != 0 {
            return Err(unsupported());
        }
    }
    if version < 20160227180335 {
        let ignore = if source
            .get("smtp")
            .is_some_and(|s| s.iter().any(|f| f == "ignore_cert_errors"))
        {
            "s.ignore_cert_errors"
        } else {
            "NULL"
        };
        statement(c,format!("INSERT INTO smtp(id,user_id,interface_type,name,host,username,password,from_address,ignore_cert_errors) SELECT s.smtp_id,c.user_id,'SMTP','Migrated SMTP '||s.smtp_id,s.host,s.username,'',s.from_address,{ignore} FROM gophish_smtp s JOIN gophish_campaigns c ON c.id=s.campaign_id;")).await?;
    }
    initialize_work(c).await?;
    import_domains(c).await?;
    // Preserve AUTOINCREMENT high-water marks, including deleted source identities.
    for table in &tables {
        sqlx::query("UPDATE sqlite_sequence SET seq=max(seq,coalesce((SELECT seq FROM sqlite_sequence WHERE name=?),0)) WHERE name=?")
            .bind(format!("gophish_{table}")).bind(table).execute(&mut *c).await?;
        statement(c, format!("DROP TABLE \"gophish_{table}\";")).await?;
    }
    Ok(())
}

async fn initialize_work(c: &mut SqliteConnection) -> Result<(), UpgradeError> {
    let skipped: i64 = sqlx::query_scalar("SELECT count(*) FROM imap WHERE user_id IS NULL OR user_id<=0 OR typeof(user_id)<>'integer' OR NOT EXISTS(SELECT 1 FROM users u WHERE u.id=imap.user_id)")
        .fetch_one(&mut *c).await?;
    crate::database::migrations::warn_skipped_mailboxes(skipped);
    // Gophish account deletion leaves IMAP rows behind. Recovery data retains
    // unusable settings; the application imports only settings with a valid owner.
    sqlx::raw_sql(r#"
INSERT INTO ruphish_campaign_policy(campaign_id,redirect_url) SELECT c.id,coalesce(p.redirect_url,'') FROM campaigns c JOIN pages p ON p.id=c.page_id AND p.user_id=c.user_id;
INSERT INTO ruphish_legacy_queue_issues SELECT m.id,'Unresolved or ambiguous legacy queue association' FROM mail_logs m WHERE (SELECT count(*) FROM results r JOIN campaigns c ON c.id=r.campaign_id JOIN users u ON u.id=c.user_id WHERE r.r_id=m.r_id AND r.user_id=m.user_id AND r.campaign_id=m.campaign_id AND c.user_id=m.user_id)!=1 OR (SELECT count(*) FROM results all_results WHERE all_results.r_id=m.r_id)!=1 OR (SELECT count(*) FROM mail_logs d WHERE d.r_id=m.r_id)!=1 OR coalesce(m.send_attempt,0) NOT BETWEEN 0 AND 8 OR unixepoch(m.send_date) IS NULL;
INSERT INTO ruphish_delivery(result_id,campaign_id,state,due,retries,code) SELECT r.id,m.campaign_id,CASE WHEN c.status='Completed' THEN 'cancelled' WHEN coalesce(m.processing,0)!=0 THEN 'unknown' WHEN p.campaign_id IS NULL THEN 'blocked' ELSE 'ready' END,unixepoch(m.send_date),coalesce(m.send_attempt,0),CASE WHEN coalesce(m.processing,0)!=0 THEN 'legacy_delivery_outcome_unknown' WHEN p.campaign_id IS NULL THEN 'campaign_page_missing' ELSE '' END FROM mail_logs m JOIN results r ON r.r_id=m.r_id AND r.campaign_id=m.campaign_id AND r.user_id=m.user_id JOIN campaigns c ON c.id=m.campaign_id LEFT JOIN ruphish_campaign_policy p ON p.campaign_id=c.id WHERE m.id NOT IN (SELECT mail_log_id FROM ruphish_legacy_queue_issues);
DELETE FROM mail_logs WHERE id NOT IN (SELECT mail_log_id FROM ruphish_legacy_queue_issues);
INSERT INTO ruphish_webhook_state(webhook_id,revision,owner_id) SELECT id,1,(SELECT min(u.id) FROM users u JOIN role_permissions rp ON rp.role_id=u.role_id JOIN permissions p ON p.id=rp.permission_id WHERE p.slug='modify_system') FROM webhooks;
INSERT INTO ruphish_mailboxes(owner_id,name,enabled,host,port,username,password,tls,folder,restrict_domain,delete_reported,frequency,modified_date,last_login,ignore_cert_errors)
 SELECT user_id,'Imported reporting mailbox ' || rowid,CASE WHEN port BETWEEN 1 AND 65535 AND length(host)>0 AND length(username)>0 AND length(password)>0 THEN COALESCE(enabled,0) ELSE 0 END,COALESCE(host,''),CASE WHEN port BETWEEN 1 AND 65535 THEN port ELSE 993 END,COALESCE(username,''),COALESCE(password,''),COALESCE(tls,1),COALESCE(NULLIF(folder,''),'INBOX'),COALESCE(restrict_domain,''),COALESCE(delete_reported_campaign_email,0),CASE WHEN imap_freq>=30 AND imap_freq<=31540000 THEN imap_freq ELSE 60 END,COALESCE(NULLIF(modified_date,''),'0001-01-01T00:00:00Z'),COALESCE(NULLIF(last_login,''),'0001-01-01T00:00:00Z'),COALESCE(ignore_cert_errors,0)<>0 FROM imap WHERE user_id>0 AND typeof(user_id)='integer' AND EXISTS(SELECT 1 FROM users u WHERE u.id=imap.user_id);
DELETE FROM imap;
UPDATE ruphish_settings SET reporting_mailboxes_enabled=EXISTS(SELECT 1 FROM ruphish_mailboxes),webhooks_enabled=EXISTS(SELECT 1 FROM webhooks) WHERE id=1;
"#).execute(c).await?;
    Ok(())
}

async fn import_domains(c: &mut SqliteConnection) -> Result<(), UpgradeError> {
    let rows = sqlx::query("SELECT id,user_id,url FROM campaigns ORDER BY id")
        .fetch_all(&mut *c)
        .await?;
    let now = ruphish_application::datetime::now().as_second();
    for row in rows {
        let owner = row
            .try_get::<Option<i64>, _>("user_id")?
            .filter(|owner| *owner > 0)
            .ok_or_else(|| {
                UpgradeError::Unrecognized(
                    "Legacy campaigns require a positive owner identity".into(),
                )
            })?;
        let value: Option<String> = row.try_get("url")?;
        let Ok(origin) =
            ruphish_domain::domains::origin(value.as_deref().unwrap_or_default(), true)
        else {
            continue;
        };
        let mut bytes = [0; 32];
        getrandom::fill(&mut bytes).map_err(|_| {
            UpgradeError::Unrecognized("Domain challenge entropy unavailable".into())
        })?;
        sqlx::query("INSERT OR IGNORE INTO ruphish_domains(owner_id,name,origin,hostname,challenge,challenge_expires,due) VALUES(?,?,?,?,?,?,?)").bind(owner).bind(&origin.host).bind(&origin.value).bind(&origin.host).bind(hex::encode(bytes)).bind(now+ruphish_domain::domains::CHALLENGE_LIFETIME).bind(if origin.verification_supported() { 0 } else { i64::MAX }).execute(&mut *c).await?;
        sqlx::query("INSERT INTO ruphish_campaign_domains SELECT ?,id FROM ruphish_domains WHERE owner_id=? AND origin=?").bind(row.try_get::<i64,_>("id")?).bind(owner).bind(origin.value).execute(&mut *c).await?;
    }
    sqlx::query("INSERT INTO ruphish_domain_checks(domain_id,queued_at,scheduled_at) SELECT id,0,due FROM ruphish_domains WHERE enabled=1 AND due<9223372036854775807").execute(c).await?;
    Ok(())
}
