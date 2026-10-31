//! Version-aware data conversion. Source tables remain unchanged for recovery.
use super::{
    migrations::{STAGE, identifier, statement},
    storage_error,
};
use crate::database::Error;
use sqlx::{MySqlConnection, Row};
use std::collections::BTreeMap;

pub(super) async fn validate_source(c: &mut MySqlConnection, version: i64) -> Result<(), Error> {
    // Unsupported ownership is an error, never an invitation to adopt orphaned data.
    for table in ["templates", "pages", "groups", "campaigns"] {
        ensure_zero(c, format!("SELECT count(*) FROM `{table}` t LEFT JOIN users u ON u.id=t.user_id WHERE u.id IS NULL OR t.id<=0")).await?;
    }
    for query in [
        "SELECT count(*) FROM users WHERE id<=0 OR username IS NULL OR username='' OR api_key IS NULL",
        "SELECT count(*) FROM group_targets g LEFT JOIN `groups` p ON p.id=g.group_id LEFT JOIN targets t ON t.id=g.target_id WHERE p.id IS NULL OR t.id IS NULL",
        "SELECT count(*) FROM results r LEFT JOIN campaigns c ON c.id=r.campaign_id WHERE c.id IS NULL OR r.user_id<>c.user_id OR r.user_id IS NULL OR r.id<=0",
        "SELECT count(*) FROM events e LEFT JOIN campaigns c ON c.id=e.campaign_id WHERE c.id IS NULL OR e.id<=0",
        "SELECT count(*) FROM attachments a LEFT JOIN templates t ON t.id=a.template_id WHERE t.id IS NULL OR a.id<=0",
    ] {
        ensure_zero(c, query.to_owned()).await?;
    }
    if version < 20160227180335 {
        ensure_zero(c, "SELECT count(*) FROM smtp s LEFT JOIN campaigns c ON c.id=s.campaign_id WHERE c.id IS NULL OR s.smtp_id<=0 OR (SELECT count(*) FROM smtp d WHERE d.campaign_id=s.campaign_id)<>1".into()).await?;
    } else {
        ensure_zero(c, "SELECT count(*) FROM smtp s LEFT JOIN users u ON u.id=s.user_id WHERE u.id IS NULL OR s.id<=0".into()).await?;
    }
    ensure_zero(c, "SELECT count(*) FROM targets WHERE id<=0".into()).await?;
    if version >= 20170219122503 {
        ensure_zero(
            c,
            "SELECT count(*) FROM headers h LEFT JOIN smtp s ON s.id=h.smtp_id WHERE s.id IS NULL"
                .into(),
        )
        .await?;
    }
    if version >= 20200116000000 {
        ensure_zero(
            c,
            "SELECT count(*) FROM imap i LEFT JOIN users u ON u.id=i.user_id WHERE u.id IS NULL"
                .into(),
        )
        .await?;
    }
    if version >= 20190105192341 {
        ensure_zero(c,"SELECT count(*) FROM role_permissions rp LEFT JOIN roles r ON r.id=rp.role_id LEFT JOIN permissions p ON p.id=rp.permission_id WHERE r.id IS NULL OR p.id IS NULL".into()).await?;
        ensure_zero(
            c,
            "SELECT count(*) FROM users u LEFT JOIN roles r ON r.id=u.role_id WHERE r.id IS NULL"
                .into(),
        )
        .await?;
    }
    Ok(())
}

async fn ensure_zero(c: &mut MySqlConnection, sql: String) -> Result<(), Error> {
    let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .fetch_one(c)
        .await
        .map_err(storage_error)?;
    if count != 0 {
        return Err(Error::UnsupportedSchema);
    }
    Ok(())
}

pub(super) async fn copy(c: &mut MySqlConnection, version: i64, offset: &str) -> Result<(), Error> {
    validate_source(c, version).await?;
    let source = columns(c, false).await?;
    let destination = columns(c, true).await?;
    // Legacy RBAC IDs and permissions are data, not interchangeable seed values.
    if source.contains_key("roles") {
        for table in ["roles", "permissions", "role_permissions"] {
            statement(c, format!("DELETE FROM `{STAGE}{table}`")).await?;
        }
    }
    for (table, fields) in &destination {
        if !source.contains_key(table) || table == "smtp" && version < 20160227180335 {
            continue;
        }
        let known = &source[table];
        let mut expressions = Vec::new();
        let mut names = Vec::new();
        for name in fields.keys() {
            if name == "active_domain" {
                continue;
            }
            names.push(format!("`{name}`"));
            let value = if table == "events" && name == "metadata"
                || table == "results" && name == "ip"
            {
                "''".to_owned()
            } else if table == "results" && matches!(name.as_str(), "latitude" | "longitude") {
                "0".to_owned()
            } else if table == "events" && name == "kind" {
                "CASE message WHEN 'Campaign Created' THEN 'campaign_created' WHEN 'Email Sent' THEN 'email_sent' WHEN 'Error Sending Email' THEN 'email_error' WHEN 'Email Opened' THEN 'email_opened' WHEN 'Clicked Link' THEN 'link_clicked' WHEN 'Submitted Data' THEN 'form_submitted' WHEN 'Email Reported' THEN 'email_reported' WHEN 'Delivery reconciled' THEN 'delivery_reconciled' ELSE message END".into()
            } else if table == "events" && name == "source" {
                "'legacy'".into()
            } else if table == "events" && name == "recipient_id" {
                "(SELECT min(r.id) FROM results r WHERE r.campaign_id=events.campaign_id AND r.email=events.email HAVING count(*)=1)".into()
            } else if table == "events" && matches!(name.as_str(), "edited" | "revision") {
                "0".into()
            } else if table == "events" && name == "create_key" {
                "NULL".into()
            } else if let Some(ty) = known.get(name) {
                if matches!(ty.as_str(), "datetime" | "timestamp") {
                    // DATETIME has no offset. Apply an explicit source interpretation
                    // only before the recorded upstream UTC conversion.
                    let zone = if version < 20170828220440 {
                        offset
                    } else {
                        "+00:00"
                    };
                    if zone.len() != 6
                        || !zone
                            .bytes()
                            .all(|b| b.is_ascii_digit() || matches!(b, b'+' | b'-' | b':'))
                    {
                        return Err(Error::UnsupportedSchema);
                    }
                    format!(
                        "CASE WHEN `{name}` IS NULL OR CAST(`{name}` AS CHAR)<'1000' THEN '0001-01-01T00:00:00Z' ELSE DATE_FORMAT(CONVERT_TZ(`{name}`,'{zone}','+00:00'),'%Y-%m-%dT%H:%i:%sZ') END"
                    )
                } else {
                    format!("`{name}`")
                }
            } else {
                match (table.as_str(), name.as_str()) {
                    ("users","role_id") => "CASE WHEN id=(SELECT id FROM (SELECT id FROM users ORDER BY (username='admin') DESC,id LIMIT 1) candidate) THEN 1 ELSE 2 END".into(),
                    ("campaigns","launch_date") => "COALESCE(DATE_FORMAT(created_date,'%Y-%m-%dT%H:%i:%sZ'),'0001-01-01T00:00:00Z')".into(),
                    (_,"modified_date"|"send_date"|"send_by_date"|"last_login") => "'0001-01-01T00:00:00Z'".into(),
                    (_,"capture_credentials"|"capture_passwords"|"reported"|"password_change_required"|"account_locked"|"ignore_cert_errors") => "0".into(),
                    (_,"smtp_id") => "NULL".into(),
                    _ => "''".into(),
                }
            };
            expressions.push(value);
        }
        statement(
            c,
            format!(
                "INSERT INTO `{STAGE}{table}` ({}) SELECT {} FROM `{table}`",
                names.join(","),
                expressions.join(",")
            ),
        )
        .await?;
        ensure_zero(c, format!("SELECT ABS((SELECT count(*) FROM `{table}`)-(SELECT count(*) FROM `{STAGE}{table}`))")).await?;
    }
    if version < 20160227180335 {
        let ignore = if source["smtp"].contains_key("ignore_cert_errors") {
            "COALESCE(s.ignore_cert_errors,0)"
        } else {
            "0"
        };
        statement(c, format!("INSERT INTO rp_stage_smtp(id,user_id,interface_type,name,host,username,password,from_address,ignore_cert_errors,modified_date) SELECT s.smtp_id,c.user_id,'SMTP',CONCAT('Migrated SMTP ',s.smtp_id),s.host,s.username,'',s.from_address,{ignore},'0001-01-01T00:00:00Z' FROM smtp s JOIN campaigns c ON c.id=s.campaign_id")).await?;
        statement(
            c,
            "UPDATE rp_stage_campaigns c JOIN smtp s ON s.campaign_id=c.id SET c.smtp_id=s.smtp_id"
                .into(),
        )
        .await?;
    }
    if version < 20160605210903 {
        statement(
            c,
            "UPDATE rp_stage_campaigns SET launch_date=created_date".into(),
        )
        .await?;
    }
    if version < 20170104231222 {
        statement(c, "UPDATE rp_stage_results r SET status=CASE WHEN EXISTS(SELECT 1 FROM rp_stage_events e WHERE e.campaign_id=r.campaign_id AND e.email=r.email AND e.kind='form_submitted') THEN 'Submitted Data' WHEN EXISTS(SELECT 1 FROM rp_stage_events e WHERE e.campaign_id=r.campaign_id AND e.email=r.email AND e.kind='link_clicked') THEN 'Clicked Link' ELSE status END WHERE status='Success'".into()).await?;
    }
    if version < 20180524203752 {
        statement(c, "UPDATE rp_stage_results r SET modified_date=COALESCE((SELECT max(e.time) FROM rp_stage_events e WHERE e.campaign_id=r.campaign_id AND e.email=r.email),'0001-01-01T00:00:00Z')".into()).await?;
    }
    initialize_work(c).await?;
    import_domains(c).await?;
    Ok(())
}

async fn columns(
    c: &mut MySqlConnection,
    staged: bool,
) -> Result<BTreeMap<String, BTreeMap<String, String>>, Error> {
    let rows = sqlx::query("SELECT TABLE_NAME,COLUMN_NAME,DATA_TYPE FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=DATABASE() ORDER BY TABLE_NAME,ORDINAL_POSITION").fetch_all(c).await.map_err(storage_error)?;
    let mut result: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for r in rows {
        let table: String = r.try_get("TABLE_NAME").map_err(storage_error)?;
        let name: String = r.try_get("COLUMN_NAME").map_err(storage_error)?;
        if !identifier(&table) || !identifier(&name) {
            return Err(Error::UnsupportedSchema);
        }
        if table.starts_with(STAGE) != staged {
            continue;
        }
        let table = table.strip_prefix(STAGE).unwrap_or(&table).to_owned();
        result
            .entry(table)
            .or_default()
            .insert(name, r.try_get("DATA_TYPE").map_err(storage_error)?);
    }
    Ok(result)
}

async fn initialize_work(c: &mut MySqlConnection) -> Result<(), Error> {
    statement(c, "INSERT INTO rp_stage_ruphish_campaign_policy(campaign_id,redirect_url) SELECT c.id,COALESCE(p.redirect_url,'') FROM rp_stage_campaigns c JOIN rp_stage_pages p ON p.id=c.page_id AND p.user_id=c.user_id;
INSERT INTO rp_stage_ruphish_webhook_state SELECT id,1 FROM rp_stage_webhooks;
INSERT INTO rp_stage_ruphish_legacy_queue_issues SELECT m.id,'Unresolved or ambiguous legacy queue association' FROM rp_stage_mail_logs m WHERE (SELECT count(*) FROM rp_stage_results r JOIN rp_stage_campaigns c ON c.id=r.campaign_id JOIN rp_stage_users u ON u.id=c.user_id WHERE r.r_id=m.r_id AND r.user_id=m.user_id AND r.campaign_id=m.campaign_id AND c.user_id=m.user_id)<>1 OR (SELECT count(*) FROM rp_stage_results r WHERE r.r_id=m.r_id)<>1 OR (SELECT count(*) FROM rp_stage_mail_logs d WHERE d.r_id=m.r_id)<>1 OR COALESCE(m.send_attempt,0) NOT BETWEEN 0 AND 8 OR m.send_date<'1970-01-01';
INSERT INTO rp_stage_ruphish_delivery(result_id,campaign_id,state,due,retries,code) SELECT r.id,m.campaign_id,CASE WHEN c.status='Completed' THEN 'cancelled' WHEN COALESCE(m.processing,0)<>0 THEN 'unknown' WHEN p.campaign_id IS NULL THEN 'blocked' ELSE 'ready' END,TIMESTAMPDIFF(SECOND,'1970-01-01 00:00:00',STR_TO_DATE(LEFT(m.send_date,19),'%Y-%m-%dT%H:%i:%s')),COALESCE(m.send_attempt,0),CASE WHEN COALESCE(m.processing,0)<>0 THEN 'legacy_delivery_outcome_unknown' WHEN p.campaign_id IS NULL THEN 'campaign_page_missing' ELSE '' END FROM rp_stage_mail_logs m JOIN rp_stage_results r ON r.r_id=m.r_id AND r.campaign_id=m.campaign_id AND r.user_id=m.user_id JOIN rp_stage_campaigns c ON c.id=m.campaign_id LEFT JOIN rp_stage_ruphish_campaign_policy p ON p.campaign_id=c.id WHERE m.id NOT IN (SELECT mail_log_id FROM rp_stage_ruphish_legacy_queue_issues);
DELETE FROM rp_stage_mail_logs WHERE id NOT IN (SELECT mail_log_id FROM rp_stage_ruphish_legacy_queue_issues);
INSERT INTO rp_stage_ruphish_mailboxes(owner_id,name,enabled,host,port,username,password,tls,folder,restrict_domain,delete_reported,frequency,modified_date,last_login,ignore_cert_errors)
SELECT user_id,CONCAT('Imported reporting mailbox ',ROW_NUMBER() OVER (ORDER BY user_id,host,username)),CASE WHEN port BETWEEN 1 AND 65535 AND length(host)>0 AND length(username)>0 AND length(password)>0 THEN COALESCE(enabled,0) ELSE 0 END,COALESCE(host,''),CASE WHEN port BETWEEN 1 AND 65535 THEN port ELSE 993 END,COALESCE(username,''),COALESCE(password,''),COALESCE(tls,1),COALESCE(NULLIF(folder,''),'INBOX'),COALESCE(restrict_domain,''),COALESCE(delete_reported_campaign_email,0),CASE WHEN imap_freq BETWEEN 30 AND 31540000 THEN imap_freq ELSE 60 END,COALESCE(modified_date,'0001-01-01T00:00:00Z'),COALESCE(last_login,'0001-01-01T00:00:00Z'),COALESCE(ignore_cert_errors,0)<>0 FROM rp_stage_imap;
DELETE FROM rp_stage_imap;
UPDATE rp_stage_ruphish_settings SET reporting_mailboxes_enabled=EXISTS(SELECT 1 FROM rp_stage_ruphish_mailboxes),webhooks_enabled=EXISTS(SELECT 1 FROM rp_stage_webhooks) WHERE id=1;".into()).await
}

async fn import_domains(c: &mut MySqlConnection) -> Result<(), Error> {
    let rows = sqlx::query("SELECT id,user_id,url FROM rp_stage_campaigns ORDER BY id")
        .fetch_all(&mut *c)
        .await
        .map_err(storage_error)?;
    let now = ruphish_application::datetime::now().as_second();
    for row in rows {
        let value: Option<String> = row.try_get("url").map_err(storage_error)?;
        let Ok(origin) = ruphish_domain::domains::origin(value.as_deref().unwrap_or(""), true)
        else {
            continue;
        };
        let owner: i64 = row.try_get("user_id").map_err(storage_error)?;
        let mut bytes = [0; 32];
        getrandom::fill(&mut bytes).map_err(|_| Error::Unavailable)?;
        sqlx::query("INSERT INTO rp_stage_ruphish_domains(owner_id,name,origin,hostname,challenge,challenge_expires,due) VALUES(?,?,?,?,?,?,?) ON DUPLICATE KEY UPDATE id=id")
            .bind(owner).bind(&origin.host).bind(&origin.value).bind(&origin.host).bind(hex::encode(bytes)).bind(now+ruphish_domain::domains::CHALLENGE_LIFETIME).bind(if origin.verification_supported() { 0 } else { i64::MAX }).execute(&mut *c).await.map_err(storage_error)?;
        sqlx::query("INSERT INTO rp_stage_ruphish_campaign_domains SELECT ?,id FROM rp_stage_ruphish_domains WHERE owner_id=? AND origin=?")
            .bind(row.try_get::<i64,_>("id").map_err(storage_error)?).bind(owner).bind(origin.value).execute(&mut *c).await.map_err(storage_error)?;
    }
    statement(c,"INSERT INTO rp_stage_ruphish_domain_checks(domain_id,queued_at,scheduled_at) SELECT id,0,due FROM rp_stage_ruphish_domains WHERE enabled=1 AND due<9223372036854775807".into()).await
}
