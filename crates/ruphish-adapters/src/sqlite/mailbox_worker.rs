//! Stable mailbox checkpoints and report persistence before external acknowledgement.
use super::integrations::{Integrations, db, now, text};
use ruphish_application::resources::Error;
use sqlx::{Row, sqlite::SqliteRow};
pub(crate) struct Config {
    pub id: i64,
    pub owner: i64,
    pub revision: i64,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub tls: bool,
    pub folder: String,
    pub restrict: String,
    pub delete: bool,
    pub validity: i64,
    pub last: u32,
    pub initialized: bool,
}
impl Config {
    pub(crate) fn read(r: &SqliteRow) -> Result<Self, Error> {
        Ok(Self {
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
}
impl Integrations {
    pub(super) async fn mailbox_once(&self) -> Result<(), Error> {
        let clock = now();
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await.map_err(db)?;
        sqlx::query(
            r#"
                UPDATE ruphish_mailbox_runs
                SET finished=?,code='interrupted'
                WHERE finished IS NULL
                AND mailbox_id IN (SELECT id
                FROM ruphish_mailboxes
                WHERE lease<=?)
            "#,
        )
        .bind(clock)
        .bind(clock)
        .execute(&mut *tx)
        .await
        .map_err(db)?;
        let row = sqlx::query(
            r#"
                SELECT m.*
                FROM ruphish_mailboxes m
                JOIN users u ON u.id=m.owner_id
                WHERE m.enabled=1
                AND m.next_check<=?
                AND m.lease<=?
                AND COALESCE(u.account_locked,0)=0 AND EXISTS(SELECT 1 FROM role_permissions rp JOIN permissions p ON p.id=rp.permission_id WHERE rp.role_id=u.role_id AND p.slug='modify_objects')
                ORDER BY m.next_check,m.id LIMIT 1
            "#,
        )
        .bind(clock)
        .bind(clock)
        .fetch_optional(&mut *tx)
        .await
        .map_err(db)?;
        let Some(r) = row else {
            tx.commit().await.map_err(db)?;
            return Ok(());
        };
        let c = Config::read(&r)?;
        sqlx::query("UPDATE ruphish_mailboxes SET lease=? WHERE id=?")
            .bind(clock + 60)
            .bind(c.id)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        let run = sqlx::query("INSERT INTO ruphish_mailbox_runs(mailbox_id,started) VALUES(?,?)")
            .bind(c.id)
            .bind(clock)
            .execute(&mut *tx)
            .await
            .map_err(db)?
            .last_insert_rowid();
        tx.commit().await.map_err(db)?;
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            self.poll_mailbox(&c, run),
        )
        .await
        .unwrap_or(Err("mailbox_timeout"));
        let code = result.err().unwrap_or("passed");
        let done = now();
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await.map_err(db)?;
        let changed = sqlx::query(
            r#"
                UPDATE ruphish_mailboxes
                SET lease=0,last_check=?,
                    last_success=CASE WHEN ?='passed' THEN ? ELSE last_success END,
                    code=?,failures=CASE WHEN ?='passed' THEN 0 ELSE failures+1 END,
                    next_check=?+CASE WHEN ?='passed' THEN frequency
                        ELSE min(3600,60*(1 << min(failures,6))) END
                WHERE id=?
                AND revision=?
                AND lease=?
            "#,
        )
        .bind(done)
        .bind(code)
        .bind(done)
        .bind(code)
        .bind(code)
        .bind(done)
        .bind(code)
        .bind(c.id)
        .bind(c.revision)
        .bind(clock + 60)
        .execute(&mut *tx)
        .await
        .map_err(db)?
        .rows_affected()
            == 1;
        sqlx::query(
            "UPDATE ruphish_mailbox_runs SET finished=?,code=? WHERE id=? AND finished IS NULL",
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
        tx.commit().await.map_err(db)
    }
    async fn poll_mailbox(&self, c: &Config, run: i64) -> Result<(), &'static str> {
        let (mut session, mailbox) = crate::mailbox_transport::connect(&self.policy, c).await?;
        sqlx::query("UPDATE ruphish_mailboxes SET last_login=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=? AND revision=?")
            .bind(c.id).bind(c.revision).execute(&self.pool).await.map_err(|_| "mailbox_storage_failed")?;
        let validity = i64::from(mailbox.uid_validity.ok_or("mailbox_uidvalidity_missing")?);
        let next = mailbox.uid_next.ok_or("mailbox_uidnext_missing")?;
        if !c.initialized {
            sqlx::query(
                r#"
                UPDATE ruphish_mailboxes
                SET initialized=1,uidvalidity=?,last_uid=?
                WHERE id=?
                AND revision=?
            "#,
            )
            .bind(validity)
            .bind(i64::from(next.saturating_sub(1)))
            .bind(c.id)
            .bind(c.revision)
            .execute(&self.pool)
            .await
            .map_err(|_| "mailbox_storage_failed")?;
            let _ = session.logout().await;
            return Ok(());
        }
        if c.validity != validity {
            return Err("mailbox_uidvalidity_changed");
        }
        // Retry only committed reports whose acknowledgement was interrupted.
        let pending: Vec<i64> = sqlx::query_scalar(
            r#"
                SELECT uid
                FROM ruphish_mailbox_messages
                WHERE mailbox_id=?
                AND uidvalidity=?
                AND state='matched'
                AND acknowledged=0
                ORDER BY uid LIMIT 25
            "#,
        )
        .bind(c.id)
        .bind(validity)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| "mailbox_storage_failed")?;
        for uid in pending {
            self.still_current(c).await?;
            crate::mailbox_transport::acknowledge(&mut session, uid as u32, c.delete).await?;
            self.ack(c, validity, uid).await?;
        }
        if next <= c.last.saturating_add(1) {
            let _ = session.logout().await;
            return Ok(());
        }
        let uids =
            crate::mailbox_transport::search(&mut session, c.last.saturating_add(1), next - 1)
                .await?;
        for uid in uids {
            let identified = match crate::mailbox_transport::fetch(&mut session, uid).await {
                Ok(bytes) => crate::mailbox_transport::identify(&bytes, &c.restrict),
                Err(code @ ("mailbox_message_too_large" | "mailbox_message_missing")) => Err(code),
                Err(code) => return Err(code),
            };
            let mut tx = self
                .pool
                .begin_with("BEGIN IMMEDIATE")
                .await
                .map_err(|_| "mailbox_storage_failed")?;
            let valid: bool = sqlx::query_scalar(
                r#"
                SELECT EXISTS(SELECT 1
                FROM ruphish_mailboxes m
                JOIN users u ON u.id=m.owner_id
                WHERE m.id=?
                AND m.revision=?
                AND m.enabled=1
                AND COALESCE(u.account_locked,0)=0 AND EXISTS(SELECT 1 FROM role_permissions rp JOIN permissions p ON p.id=rp.permission_id WHERE rp.role_id=u.role_id AND p.slug='modify_objects'))
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
                WHERE mailbox_id=?
                AND uidvalidity=?
                AND uid=?)
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
                WHERE r.r_id=?
                AND r.user_id=?
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
                    if url::Url::parse(&url)
                        .is_ok_and(|u| u.origin().ascii_serialization() == origin)
                    {
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
                    let stamp = time::OffsetDateTime::now_utc()
                        .format(&time::format_description::well_known::Rfc3339)
                        .map_err(|_| "mailbox_storage_failed")?;
                    let event = sqlx::query(
                        r#"
                INSERT INTO events(campaign_id,email,time,message,details)
                VALUES(?,?,?,'Email Reported','')
            "#,
                    )
                    .bind(campaign)
                    .bind(email)
                    .bind(&stamp)
                    .execute(&mut *tx)
                    .await
                    .map_err(|_| "mailbox_storage_failed")?
                    .last_insert_rowid();
                    sqlx::query(
                        "UPDATE ruphish_event_sources SET source='mailbox' WHERE event_id=?",
                    )
                    .bind(event)
                    .execute(&mut *tx)
                    .await
                    .map_err(|_| "mailbox_storage_failed")?;
                    sqlx::query("UPDATE results SET reported=1,modified_date=? WHERE id=?")
                        .bind(stamp)
                        .bind(result)
                        .execute(&mut *tx)
                        .await
                        .map_err(|_| "mailbox_storage_failed")?;
                }
                sqlx::query("INSERT INTO ruphish_mailbox_messages VALUES(?,?,?,?,?,0)")
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
                "UPDATE ruphish_mailboxes SET last_uid=max(last_uid,?) WHERE id=? AND revision=?",
            )
            .bind(i64::from(uid))
            .bind(c.id)
            .bind(c.revision)
            .execute(&mut *tx)
            .await
            .map_err(|_| "mailbox_storage_failed")?;
            if !existing {
                sqlx::query("UPDATE ruphish_mailbox_runs SET matched=matched+?,rejected=rejected+? WHERE id=?")
                    .bind(i64::from(result.is_some())).bind(i64::from(result.is_none())).bind(run)
                    .execute(&mut *tx).await.map_err(|_| "mailbox_storage_failed")?;
            }
            tx.commit().await.map_err(|_| "mailbox_storage_failed")?;
            if result.is_some() {
                self.still_current(c).await?;
                crate::mailbox_transport::acknowledge(&mut session, uid, c.delete).await?;
                self.ack(c, validity, i64::from(uid)).await?;
            }
        }
        let _ = session.logout().await;
        Ok(())
    }
    async fn still_current(&self, c: &Config) -> Result<(), &'static str> {
        let current: bool = sqlx::query_scalar(
            r#"
                SELECT EXISTS(SELECT 1
                FROM ruphish_mailboxes m
                JOIN users u ON u.id=m.owner_id
                WHERE m.id=?
                AND m.revision=?
                AND m.enabled=1
                AND COALESCE(u.account_locked,0)=0 AND EXISTS(SELECT 1 FROM role_permissions rp JOIN permissions p ON p.id=rp.permission_id WHERE rp.role_id=u.role_id AND p.slug='modify_objects'))
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
    async fn ack(&self, c: &Config, validity: i64, uid: i64) -> Result<(), &'static str> {
        sqlx::query(
            r#"
                UPDATE ruphish_mailbox_messages
                SET acknowledged=1
                WHERE mailbox_id=?
                AND uidvalidity=?
                AND uid=?
            "#,
        )
        .bind(c.id)
        .bind(validity)
        .bind(uid)
        .execute(&self.pool)
        .await
        .map_err(|_| "mailbox_storage_failed")?;
        Ok(())
    }
}
