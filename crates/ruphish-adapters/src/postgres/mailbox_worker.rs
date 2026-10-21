//! Atomic mailbox claims, report checkpoints and acknowledgement persistence.
use super::integrations::{Integrations, db, text};
use crate::postgres::InsertId;
use ruphish_application::{
    integrations::work::{MailboxClaim, MailboxConfig as Config, MailboxQueue},
    resources::Error,
};
use sqlx::{Row, postgres::PgRow};
pub(super) fn config(r: &PgRow) -> Result<Config, Error> {
    Ok(Config {
        id: r.try_get("id").map_err(db)?,
        owner: r.try_get("owner_id").map_err(db)?,
        revision: r.try_get("revision").map_err(db)?,
        host: text(r, "host")?,
        port: r.try_get::<i64, _>("port").map_err(db)? as u16,
        username: text(r, "username")?,
        password: text(r, "password")?,
        tls: r.try_get("tls").map_err(db)?,
        folder: text(r, "folder")?,
        restrict: text(r, "restrict_domain")?,
        delete: r.try_get("delete_reported").map_err(db)?,
        validity: r.try_get("uidvalidity").map_err(db)?,
        last: r.try_get::<i64, _>("last_uid").map_err(db)? as u32,
        initialized: r.try_get("initialized").map_err(db)?,
    })
}
#[async_trait::async_trait]
impl MailboxQueue for Integrations {
    async fn claim_mailbox(&self, clock: i64) -> Result<Option<MailboxClaim>, Error> {
        let mut tx = crate::postgres::locks::write(&self.pool)
            .await
            .map_err(db)?;
        let owner:Option<i64> = sqlx::query_scalar("SELECT m.owner_id FROM ruphish_mailboxes m WHERE m.enabled=TRUE AND m.next_check<=$1 AND m.lease<=$2 AND EXISTS(SELECT 1 FROM users u WHERE u.id=m.owner_id AND COALESCE(u.account_locked,FALSE)=FALSE AND EXISTS(SELECT 1 FROM role_permissions rp JOIN permissions p ON p.id=rp.permission_id WHERE rp.role_id=u.role_id AND p.slug='modify_objects')) ORDER BY m.next_check,m.id LIMIT 1").bind(clock).bind(clock).fetch_optional(&mut *tx).await.map_err(db)?;
        let Some(owner) = owner else {
            tx.commit()
                .await
                .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
            return Ok(None);
        };
        sqlx::query("SELECT id FROM users WHERE id=$1 FOR SHARE")
            .bind(owner)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db)?;
        let row = sqlx::query(
            r#"
                SELECT m.*
                FROM ruphish_mailboxes m
                WHERE m.owner_id=$1 AND m.enabled=TRUE
                AND m.next_check<=$2
                AND m.lease<=$3
                AND EXISTS(SELECT 1 FROM users u WHERE u.id=m.owner_id AND COALESCE(u.account_locked,FALSE)=FALSE AND EXISTS(SELECT 1 FROM role_permissions rp JOIN permissions p ON p.id=rp.permission_id WHERE rp.role_id=u.role_id AND p.slug='modify_objects'))
                ORDER BY m.next_check,m.id LIMIT 1 FOR UPDATE SKIP LOCKED
            "#,
        )
        .bind(owner)
        .bind(clock)
        .bind(clock)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db)?;
        let Some(r) = row else {
            tx.commit()
                .await
                .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
            return Ok(None);
        };
        let c = config(&r)?;
        sqlx::query("UPDATE ruphish_mailbox_runs SET finished=$1,code='interrupted' WHERE mailbox_id=$2 AND finished IS NULL").bind(clock).bind(c.id).execute(&mut *tx).await.map_err(db)?;
        let frequency = u32::try_from(r.try_get::<i64, _>("frequency").map_err(db)?)
            .map_err(|_| Error::Internal)?;
        let failures = u32::try_from(r.try_get::<i64, _>("failures").map_err(db)?)
            .map_err(|_| Error::Internal)?;
        sqlx::query("UPDATE ruphish_mailboxes SET lease=$1 WHERE id=$2")
            .bind(clock + 60)
            .bind(c.id)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        let run = sqlx::query(
            "INSERT INTO ruphish_mailbox_runs(mailbox_id,started) VALUES($1,$2) RETURNING id",
        )
        .bind(c.id)
        .bind(clock)
        .fetch_one(&mut *tx)
        .await
        .map_err(db)?
        .insert_id()
        .map_err(db)?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(Some(MailboxClaim {
            config: c,
            run,
            lease_until: clock + 60,
            frequency,
            failures,
        }))
    }
    async fn finish_mailbox(
        &self,
        claim: &MailboxClaim,
        code: &'static str,
        next_check: i64,
        done: i64,
    ) -> Result<(), Error> {
        let (c, run) = (&claim.config, claim.run);
        let mut tx = crate::postgres::locks::write(&self.pool)
            .await
            .map_err(db)?;
        let changed = sqlx::query(
            r#"
                UPDATE ruphish_mailboxes
                SET lease=0,last_check=$1,
                    last_success=CASE WHEN $2='passed' THEN $3 ELSE last_success END,
                    code=$4,failures=CASE WHEN $5='passed' THEN 0 ELSE failures+1 END,
                    next_check=$6
                WHERE id=$7
                AND revision=$8
                AND lease=$9
            "#,
        )
        .bind(done)
        .bind(code)
        .bind(done)
        .bind(code)
        .bind(code)
        .bind(next_check)
        .bind(c.id)
        .bind(c.revision)
        .bind(claim.lease_until)
        .execute(&mut *tx)
        .await
        .map_err(db)?
        .rows_affected()
            == 1;
        sqlx::query(
            "UPDATE ruphish_mailbox_runs SET finished=$1,code=$2 WHERE id=$3 AND finished IS NULL",
        )
        .bind(done)
        .bind(if changed {
            code
        } else {
            "configuration_changed"
        })
        .bind(run)
        .execute(&mut *tx)
        .await
        .map_err(db)?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))
    }
    async fn mailbox_login(&self, c: &Config) -> Result<(), &'static str> {
        sqlx::query("UPDATE ruphish_mailboxes SET last_login=to_char(clock_timestamp() AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') WHERE id=$1 AND revision=$2")
            .bind(c.id).bind(c.revision).execute(&self.pool).await.map_err(|_| "mailbox_storage_failed")?;
        Ok(())
    }
    async fn initialize_mailbox(
        &self,
        c: &Config,
        validity: i64,
        next: u32,
    ) -> Result<(), &'static str> {
        if !c.initialized {
            sqlx::query(
                r#"
                UPDATE ruphish_mailboxes
                SET initialized=TRUE,uidvalidity=$1,last_uid=$2
                WHERE id=$3
                AND revision=$4
            "#,
            )
            .bind(validity)
            .bind(i64::from(next.saturating_sub(1)))
            .bind(c.id)
            .bind(c.revision)
            .execute(&self.pool)
            .await
            .map_err(|_| "mailbox_storage_failed")?;
            return Ok(());
        }
        Ok(())
    }
    async fn pending_acknowledgements(
        &self,
        c: &Config,
        validity: i64,
    ) -> Result<Vec<u32>, &'static str> {
        let pending: Vec<i64> = sqlx::query_scalar(
            r#"
                SELECT uid
                FROM ruphish_mailbox_messages
                WHERE mailbox_id=$1
                AND uidvalidity=$2
                AND state='matched'
                AND acknowledged=FALSE
                ORDER BY uid LIMIT 25
            "#,
        )
        .bind(c.id)
        .bind(validity)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| "mailbox_storage_failed")?;
        pending
            .into_iter()
            .map(|uid| u32::try_from(uid).map_err(|_| "mailbox_storage_failed"))
            .collect()
    }
    async fn record_report(
        &self,
        c: &Config,
        run: i64,
        validity: i64,
        uid: u32,
        identified: Result<(String, String), &'static str>,
    ) -> Result<bool, &'static str> {
        let mut tx = crate::postgres::locks::write(&self.pool)
            .await
            .map_err(|_| "mailbox_storage_failed")?;
        sqlx::query("SELECT id FROM users WHERE id=$1 FOR SHARE")
            .bind(c.owner)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|_| "mailbox_storage_failed")?;
        sqlx::query("SELECT id FROM ruphish_mailboxes WHERE id=$1 FOR UPDATE")
            .bind(c.id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|_| "mailbox_storage_failed")?;
        let valid: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(SELECT 1
                FROM ruphish_mailboxes m
                JOIN users u ON u.id=m.owner_id
                WHERE m.id=$1
                AND m.revision=$2
                AND m.enabled=TRUE
                AND COALESCE(u.account_locked,FALSE)=FALSE AND EXISTS(SELECT 1 FROM role_permissions rp JOIN permissions p ON p.id=rp.permission_id WHERE rp.role_id=u.role_id AND p.slug='modify_objects'))
            "#,
            )
            .bind(c.id)
            .bind(c.revision)
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| "mailbox_storage_failed")?;
        if !valid {
            return Err("configuration_changed");
        }
        let existing: bool = sqlx::query_scalar(
            r#"
                SELECT EXISTS(SELECT 1
                FROM ruphish_mailbox_messages
                WHERE mailbox_id=$1
                AND uidvalidity=$2
                AND uid=$3)
            "#,
        )
        .bind(c.id)
        .bind(validity)
        .bind(i64::from(uid))
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| "mailbox_storage_failed")?;
        let mut result = None;
        let mut code = identified
            .as_ref()
            .err()
            .copied()
            .unwrap_or("mailbox_unmatched");
        if let Ok((rid, origin)) = identified {
            let rows = sqlx::query(
                r#"
                SELECT r.id,r.campaign_id,r.email,c.url
                FROM results r
                JOIN campaigns c ON c.id=r.campaign_id
                AND c.user_id=r.user_id
                WHERE r.r_id=$1
                AND r.user_id=$2
            "#,
            )
            .bind(rid)
            .bind(c.owner)
            .fetch_all(&mut *tx)
            .await
            .map_err(|_| "mailbox_storage_failed")?;
            if rows.len() == 1 {
                let r = &rows[0];
                let url: String = r.try_get("url").map_err(|_| "mailbox_storage_failed")?;
                if url::Url::parse(&url).is_ok_and(|u| u.origin().ascii_serialization() == origin) {
                    result = Some((
                        r.try_get::<i64, _>("id")
                            .map_err(|_| "mailbox_storage_failed")?,
                        r.try_get::<i64, _>("campaign_id")
                            .map_err(|_| "mailbox_storage_failed")?,
                        r.try_get::<String, _>("email")
                            .map_err(|_| "mailbox_storage_failed")?,
                    ));
                    code = "matched";
                }
            }
        }
        if !existing {
            if let Some((result, campaign, email)) = &result {
                sqlx::query("SELECT id FROM results WHERE id=$1 FOR UPDATE")
                    .bind(result)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(|_| "mailbox_storage_failed")?;
                let stamp =
                    ruphish_application::datetime::utc(ruphish_application::datetime::now());
                let event = sqlx::query(
                    r#"
                INSERT INTO events(campaign_id,email,time,message,details)
                VALUES($1,$2,$3,'Email Reported','') RETURNING id"#,
                )
                .bind(campaign)
                .bind(email)
                .bind(&stamp)
                .fetch_one(&mut *tx)
                .await
                .map_err(|_| "mailbox_storage_failed")?
                .insert_id()
                .map_err(|_| "mailbox_storage_failed")?;
                super::events::publish(&mut tx, event, "mailbox")
                    .await
                    .map_err(|_| "mailbox_storage_failed")?;
                sqlx::query("UPDATE results SET reported=TRUE,modified_date=$1 WHERE id=$2")
                    .bind(stamp)
                    .bind(result)
                    .execute(&mut *tx)
                    .await
                    .map_err(|_| "mailbox_storage_failed")?;
            }
            sqlx::query("INSERT INTO ruphish_mailbox_messages VALUES($1,$2,$3,$4,$5,FALSE)")
                .bind(c.id)
                .bind(validity)
                .bind(i64::from(uid))
                .bind(if result.is_some() {
                    "matched"
                } else {
                    "rejected"
                })
                .bind(code)
                .execute(&mut *tx)
                .await
                .map_err(|_| "mailbox_storage_failed")?;
        }
        sqlx::query(
            "UPDATE ruphish_mailboxes SET last_uid=GREATEST(last_uid,$1) WHERE id=$2 AND revision=$3",
        )
        .bind(i64::from(uid))
        .bind(c.id)
        .bind(c.revision)
        .execute(&mut *tx)
        .await
        .map_err(|_| "mailbox_storage_failed")?;
        if !existing {
            sqlx::query(
                "UPDATE ruphish_mailbox_runs SET matched=matched+$1,rejected=rejected+$2 WHERE id=$3",
            )
            .bind(i64::from(result.is_some()))
            .bind(i64::from(result.is_none()))
            .bind(run)
            .execute(&mut *tx)
            .await
            .map_err(|_| "mailbox_storage_failed")?;
        }
        tx.commit().await.map_err(|_| "mailbox_storage_failed")?;
        Ok(result.is_some())
    }
    async fn mailbox_current(&self, c: &Config) -> Result<(), &'static str> {
        let current: bool = sqlx::query_scalar(
            r#"
                SELECT EXISTS(SELECT 1
                FROM ruphish_mailboxes m
                JOIN users u ON u.id=m.owner_id
                WHERE m.id=$1
                AND m.revision=$2
                AND m.enabled=TRUE
                AND COALESCE(u.account_locked,FALSE)=FALSE AND EXISTS(SELECT 1 FROM role_permissions rp JOIN permissions p ON p.id=rp.permission_id WHERE rp.role_id=u.role_id AND p.slug='modify_objects'))
            "#,
        )
        .bind(c.id)
        .bind(c.revision)
        .fetch_one(&self.pool)
        .await
        .map_err(|_| "mailbox_storage_failed")?;
        if current {
            Ok(())
        } else {
            Err("configuration_changed")
        }
    }
    async fn acknowledge_report(
        &self,
        c: &Config,
        validity: i64,
        uid: u32,
    ) -> Result<(), &'static str> {
        sqlx::query(
            r#"
                UPDATE ruphish_mailbox_messages
                SET acknowledged=TRUE
                WHERE mailbox_id=$1
                AND uidvalidity=$2
                AND uid=$3
            "#,
        )
        .bind(c.id)
        .bind(validity)
        .bind(i64::from(uid))
        .execute(&self.pool)
        .await
        .map_err(|_| "mailbox_storage_failed")?;
        Ok(())
    }
}
