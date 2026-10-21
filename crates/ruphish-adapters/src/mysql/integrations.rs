//! Transactional integration resources; secrets stay inside adapter-owned records.
use crate::mysql::InsertId;
use crate::outbound::Policy;
use ruphish_application::{auth::Identity, integrations::*, resources::Error};
use sqlx::{MySqlConnection, MySqlPool, Row, mysql::MySqlRow};

#[derive(Clone)]
/// Integration configuration and bounded external operations.
pub struct Integrations {
    pub(super) pool: MySqlPool,
    pub(super) policy: Policy,
}
impl Integrations {
    /// Attach the startup pool and shared destination policy.
    pub fn new(pool: MySqlPool, policy: Policy) -> Self {
        Self { pool, policy }
    }
}
pub(super) fn db(error: sqlx::Error) -> Error {
    crate::mysql::resource_error(error)
}
pub(super) fn invalid(code: &str) -> Error {
    Error::Invalid(code.into())
}
pub(super) fn now() -> i64 {
    ruphish_application::datetime::now().as_second()
}
pub(super) fn text(r: &MySqlRow, key: &str) -> Result<String, Error> {
    Ok(r.try_get::<Option<String>, _>(key)
        .map_err(db)?
        .unwrap_or_default())
}
async fn authorize(
    c: &mut MySqlConnection,
    who: Identity,
    write: bool,
    admin: bool,
) -> Result<(), Error> {
    super::resources::authorize(c, who, write).await?;
    if admin {
        super::accounts::account(c, who.id)
            .await
            .map_err(Error::from)?
            .require("modify_system")?;
    }
    Ok(())
}
fn webhook_row(r: MySqlRow) -> Result<Webhook, Error> {
    Ok(Webhook {
        id: r.try_get("id").map_err(db)?,
        name: text(&r, "name")?,
        url: text(&r, "url")?,
        active: r.try_get("is_active").map_err(db)?,
        revision: r.try_get("revision").map_err(db)?,
        has_secret: !text(&r, "secret")?.is_empty(),
        history: vec![],
    })
}
fn mailbox_row(r: MySqlRow) -> Result<Mailbox, Error> {
    Ok(Mailbox {
        id: r.try_get("id").map_err(db)?,
        name: text(&r, "name")?,
        enabled: r.try_get("enabled").map_err(db)?,
        host: text(&r, "host")?,
        port: r.try_get::<i64, _>("port").map_err(db)? as u16,
        username: text(&r, "username")?,
        has_password: !text(&r, "password")?.is_empty(),
        tls: r.try_get("tls").map_err(db)?,
        folder: text(&r, "folder")?,
        restrict_domain: text(&r, "restrict_domain")?,
        delete_reported: r.try_get("delete_reported").map_err(db)?,
        frequency: r.try_get::<i64, _>("frequency").map_err(db)? as u32,
        revision: r.try_get("revision").map_err(db)?,
        last_check: r.try_get("last_check").map_err(db)?,
        last_success: r.try_get("last_success").map_err(db)?,
        modified_date: text(&r, "modified_date")?.replacen(' ', "T", 1),
        last_login: text(&r, "last_login")?.replacen(' ', "T", 1),
        next_check: r.try_get("next_check").map_err(db)?,
        running: r.try_get::<i64, _>("lease").map_err(db)? > now(),
        code: text(&r, "code")?,
        history: vec![],
    })
}
#[async_trait::async_trait]
impl Repository for Integrations {
    async fn webhooks(&self, who: Identity) -> Result<Vec<Webhook>, Error> {
        let mut tx = crate::mysql::locks::snapshot(&self.pool)
            .await
            .map_err(db)?;
        authorize(&mut tx, who, false, true).await?;
        sqlx::query(
            r#"
                SELECT w.*,s.revision
                FROM webhooks w
                JOIN ruphish_webhook_state s ON s.webhook_id=w.id
                ORDER BY w.name,w.id
            "#,
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(db)?
        .into_iter()
        .map(webhook_row)
        .collect()
    }
    async fn webhook(&self, who: Identity, id: i64) -> Result<Webhook, Error> {
        let mut tx = crate::mysql::locks::snapshot(&self.pool)
            .await
            .map_err(db)?;
        authorize(&mut tx, who, false, true).await?;
        let mut value = webhook_row(
            sqlx::query(
                r#"
                SELECT w.*,s.revision
                FROM webhooks w
                JOIN ruphish_webhook_state s ON s.webhook_id=w.id
                WHERE w.id=?
            "#,
            )
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db)?
            .ok_or(Error::NotFound("Webhook"))?,
        )?;
        for r in sqlx::query(
            r#"
                SELECT a.*,j.event_id
                FROM ruphish_webhook_attempts a
                JOIN ruphish_webhook_jobs j ON j.id=a.job_id
                WHERE j.webhook_id=?
                ORDER BY a.id DESC LIMIT 30
            "#,
        )
        .bind(id)
        .fetch_all(&mut *tx)
        .await
        .map_err(db)?
        {
            value.history.push(Run {
                event_id: r.try_get("event_id").map_err(db)?,
                attempt: r.try_get("attempt").map_err(db)?,
                id: r.try_get("id").map_err(db)?,
                started: r.try_get("started").map_err(db)?,
                finished: r.try_get("finished").map_err(db)?,
                status: r.try_get("status").map_err(db)?,
                code: text(&r, "code")?,
                matched: 0,
                rejected: 0,
            });
        }
        Ok(value)
    }
    async fn save_webhook(
        &self,
        who: Identity,
        id: Option<i64>,
        e: WebhookEdit,
    ) -> Result<Webhook, Error> {
        crate::integration_work::validate_webhook_edit(&e)?;
        let mut tx = crate::mysql::locks::write(&self.pool).await.map_err(db)?;
        authorize(&mut tx, who, true, true).await?;
        let id = if let Some(id) = id {
            let r = sqlx::query(
                r#"
                SELECT secret,revision
                FROM webhooks w
                JOIN ruphish_webhook_state s ON s.webhook_id=w.id
                WHERE w.id=? FOR UPDATE
            "#,
            )
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db)?
            .ok_or(Error::NotFound("Webhook"))?;
            if e.revision != Some(r.try_get("revision").map_err(db)?) {
                return Err(Error::Changed);
            }
            let secret = e.secret.unwrap_or(text(&r, "secret")?);
            if secret.is_empty() {
                return Err(invalid("webhook_secret_required"));
            }
            sqlx::query("UPDATE webhooks SET name=?,url=?,secret=?,is_active=? WHERE id=?")
                .bind(&e.name)
                .bind(&e.url)
                .bind(secret)
                .bind(e.active)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(db)?;
            sqlx::query("UPDATE ruphish_webhook_state SET revision=revision+1 WHERE webhook_id=?")
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(db)?;
            sqlx::query(
                r#"
                UPDATE ruphish_webhook_jobs
                SET state='cancelled',code='configuration_changed'
                WHERE webhook_id=?
                AND state IN ('queued','retry','running')
            "#,
            )
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
            id
        } else {
            let secret = e.secret.ok_or_else(|| invalid("webhook_secret_required"))?;
            let id = sqlx::query("INSERT INTO webhooks(name,url,secret,is_active) VALUES(?,?,?,?)")
                .bind(&e.name)
                .bind(&e.url)
                .bind(secret)
                .bind(e.active)
                .execute(&mut *tx)
                .await
                .map_err(db)?
                .insert_id()
                .map_err(db)?;
            sqlx::query("INSERT INTO ruphish_webhook_state VALUES(?,1)")
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(db)?;
            id
        };
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::mysql::commit_error(e)))?;
        self.webhook(who, id).await
    }
    async fn delete_webhook(&self, who: Identity, id: i64, revision: i64) -> Result<(), Error> {
        let mut tx = crate::mysql::locks::write(&self.pool).await.map_err(db)?;
        authorize(&mut tx, who, true, true).await?;
        sqlx::query("SELECT id FROM webhooks WHERE id=? FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db)?;
        if sqlx::query("DELETE FROM ruphish_webhook_state WHERE webhook_id=? AND revision=?")
            .bind(id)
            .bind(revision)
            .execute(&mut *tx)
            .await
            .map_err(db)?
            .rows_affected()
            != 1
        {
            return Err(Error::Changed);
        }
        sqlx::query(
            r#"
                DELETE
                FROM ruphish_webhook_attempts
                WHERE job_id IN (SELECT id
                FROM ruphish_webhook_jobs
                WHERE webhook_id=?)
            "#,
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(db)?;
        sqlx::query("DELETE FROM ruphish_webhook_jobs WHERE webhook_id=?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        sqlx::query("DELETE FROM webhooks WHERE id=?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::mysql::commit_error(e)))
    }
    async fn validate_webhook(&self, who: Identity, id: i64) -> Result<(), Error> {
        let mut tx = crate::mysql::locks::write(&self.pool).await.map_err(db)?;
        authorize(&mut tx, who, true, true).await?;
        let r = sqlx::query("SELECT url,secret FROM webhooks WHERE id=?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db)?
            .ok_or(Error::NotFound("Webhook"))?;
        let url = text(&r, "url")?;
        let secret = text(&r, "secret")?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::mysql::commit_error(e)))?;
        crate::integration_work::validate_webhook(&self.policy, &url, &secret).await
    }

    async fn mailboxes(&self, who: Identity) -> Result<Vec<Mailbox>, Error> {
        let mut tx = crate::mysql::locks::snapshot(&self.pool)
            .await
            .map_err(db)?;
        authorize(&mut tx, who, false, false).await?;
        sqlx::query("SELECT * FROM ruphish_mailboxes WHERE owner_id=? ORDER BY name,id")
            .bind(who.id)
            .fetch_all(&mut *tx)
            .await
            .map_err(db)?
            .into_iter()
            .map(mailbox_row)
            .collect()
    }
    async fn mailbox(&self, who: Identity, id: i64) -> Result<Mailbox, Error> {
        let mut tx = crate::mysql::locks::snapshot(&self.pool)
            .await
            .map_err(db)?;
        authorize(&mut tx, who, false, false).await?;
        let mut value = mailbox_row(
            sqlx::query("SELECT * FROM ruphish_mailboxes WHERE id=? AND owner_id=?")
                .bind(id)
                .bind(who.id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(db)?
                .ok_or(Error::NotFound("Reporting mailbox"))?,
        )?;
        for r in sqlx::query(
            "SELECT * FROM ruphish_mailbox_runs WHERE mailbox_id=? ORDER BY id DESC LIMIT 30",
        )
        .bind(id)
        .fetch_all(&mut *tx)
        .await
        .map_err(db)?
        {
            value.history.push(Run {
                event_id: None,
                attempt: None,
                id: r.try_get("id").map_err(db)?,
                started: r.try_get("started").map_err(db)?,
                finished: r.try_get("finished").map_err(db)?,
                status: None,
                code: text(&r, "code")?,
                matched: r.try_get("matched").map_err(db)?,
                rejected: r.try_get("rejected").map_err(db)?,
            });
        }
        Ok(value)
    }
    async fn save_mailbox(
        &self,
        who: Identity,
        id: Option<i64>,
        e: MailboxEdit,
    ) -> Result<Mailbox, Error> {
        crate::integration_work::validate_mailbox_edit(&e)?;
        let mut tx = crate::mysql::locks::write(&self.pool).await.map_err(db)?;
        authorize(&mut tx, who, true, false).await?;
        let duplicate: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM ruphish_mailboxes WHERE owner_id=? AND name=? AND id!=?)",
        )
        .bind(who.id)
        .bind(&e.name)
        .bind(id.unwrap_or(0))
        .fetch_one(&mut *tx)
        .await
        .map_err(db)?;
        if duplicate {
            return Err(invalid("mailbox_name_exists"));
        }
        let id = if let Some(id) = id {
            let r =
                sqlx::query("SELECT * FROM ruphish_mailboxes WHERE id=? AND owner_id=? FOR UPDATE")
                    .bind(id)
                    .bind(who.id)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(db)?
                    .ok_or(Error::NotFound("Reporting mailbox"))?;
            if e.revision != Some(r.try_get("revision").map_err(db)?) {
                return Err(Error::Changed);
            }
            let changed = text(&r, "host")? != e.host
                || text(&r, "folder")? != e.folder
                || text(&r, "username")? != e.username
                || r.try_get::<i64, _>("port").map_err(db)? != i64::from(e.port);
            let password = e.password.unwrap_or(text(&r, "password")?);
            if password.is_empty() {
                return Err(invalid("mailbox_password_required"));
            }
            sqlx::query(
                r#"
                UPDATE ruphish_mailboxes
                SET name=?,enabled=?,host=?,port=?,username=?,password=?,tls=?,folder=?,
                    restrict_domain=?,delete_reported=?,frequency=?,revision=revision+1,
                    lease=0,next_check=0,failures=0,code='',modified_date=DATE_FORMAT(UTC_TIMESTAMP(6),'%Y-%m-%dT%H:%i:%s.%fZ'),
                    initialized=CASE WHEN ? THEN 0 ELSE initialized END,
                    uidvalidity=CASE WHEN ? THEN 0 ELSE uidvalidity END,
                    last_uid=CASE WHEN ? THEN 0 ELSE last_uid END
                WHERE id=?
            "#,
            )
            .bind(&e.name)
            .bind(e.enabled)
            .bind(&e.host)
            .bind(i64::from(e.port))
            .bind(&e.username)
            .bind(password)
            .bind(e.tls)
            .bind(&e.folder)
            .bind(&e.restrict_domain)
            .bind(e.delete_reported)
            .bind(i64::from(e.frequency))
            .bind(changed)
            .bind(changed)
            .bind(changed)
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
            if changed {
                sqlx::query("DELETE FROM ruphish_mailbox_messages WHERE mailbox_id=?")
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(db)?;
            }
            id
        } else {
            let password = e
                .password
                .ok_or_else(|| invalid("mailbox_password_required"))?;
            sqlx::query(r#"
                INSERT INTO ruphish_mailboxes(owner_id,name,enabled,host,port,username,password,tls,folder,restrict_domain,delete_reported,frequency)
                VALUES(?,?,?,?,?,?,?,?,?,?,?,?)
            "#)
.bind(who.id)
.bind(&e.name)
.bind(e.enabled)
.bind(&e.host)
.bind(i64::from(e.port))
.bind(&e.username)
.bind(password)
.bind(e.tls)
.bind(&e.folder)
.bind(&e.restrict_domain)
.bind(e.delete_reported)
.bind(i64::from(e.frequency))
.execute(&mut *tx)
.await
.map_err(db)?
.insert_id().map_err(db)?
        };
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::mysql::commit_error(e)))?;
        self.mailbox(who, id).await
    }
    async fn delete_mailbox(&self, who: Identity, id: i64, revision: i64) -> Result<(), Error> {
        let mut tx = crate::mysql::locks::write(&self.pool).await.map_err(db)?;
        authorize(&mut tx, who, true, false).await?;
        if sqlx::query("DELETE FROM ruphish_mailboxes WHERE id=? AND owner_id=? AND revision=?")
            .bind(id)
            .bind(who.id)
            .bind(revision)
            .execute(&mut *tx)
            .await
            .map_err(db)?
            .rows_affected()
            != 1
        {
            return Err(Error::Changed);
        }
        for table in ["ruphish_mailbox_messages", "ruphish_mailbox_runs"] {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "DELETE FROM {table} WHERE mailbox_id=?"
            )))
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        }
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::mysql::commit_error(e)))
    }
    async fn validate_mailbox(&self, who: Identity, id: i64) -> Result<(), Error> {
        let mut tx = crate::mysql::locks::write(&self.pool).await.map_err(db)?;
        authorize(&mut tx, who, true, false).await?;
        let r = sqlx::query("SELECT * FROM ruphish_mailboxes WHERE id=? AND owner_id=? FOR UPDATE")
            .bind(id)
            .bind(who.id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db)?
            .ok_or(Error::NotFound("Reporting mailbox"))?;
        let config = super::mailbox_worker::config(&r)?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::mysql::commit_error(e)))?;
        crate::integration_work::validate_mailbox(&self.policy, &config).await
    }
    async fn validate_mailbox_settings(
        &self,
        who: Identity,
        edit: MailboxEdit,
    ) -> Result<(), Error> {
        crate::integration_work::validate_mailbox_edit(&edit)?;
        let mut tx = crate::mysql::locks::write(&self.pool).await.map_err(db)?;
        authorize(&mut tx, who, true, false).await?;
        // Only reuse the saved secret for the same account and endpoint.
        let password = match edit.password {
            Some(password) => password,
            None => sqlx::query_scalar::<_, String>(
                r#"
                SELECT password
                FROM ruphish_mailboxes
                WHERE owner_id=?
                AND host=?
                AND port=?
                AND username=?
                ORDER BY id LIMIT 1
            "#,
            )
            .bind(who.id)
            .bind(&edit.host)
            .bind(i64::from(edit.port))
            .bind(&edit.username)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db)?
            .ok_or_else(|| invalid("mailbox_password_required"))?,
        };
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::mysql::commit_error(e)))?;
        let config = ruphish_application::integrations::work::MailboxConfig {
            id: 0,
            owner: who.id,
            revision: 0,
            host: edit.host,
            port: edit.port,
            username: edit.username,
            password,
            tls: edit.tls,
            folder: edit.folder,
            restrict: edit.restrict_domain,
            delete: false,
            validity: 0,
            last: 0,
            initialized: false,
        };
        crate::integration_work::validate_mailbox(&self.policy, &config).await
    }
    async fn resume_mailbox(&self, who: Identity, id: i64, revision: i64) -> Result<(), Error> {
        let mut tx = crate::mysql::locks::write(&self.pool).await.map_err(db)?;
        authorize(&mut tx, who, true, false).await?;
        let changed = sqlx::query(r#"
                UPDATE ruphish_mailboxes
                SET initialized=0,uidvalidity=0,last_uid=0,next_check=0,lease=0,revision=revision+1,code=''
                WHERE id=?
                AND owner_id=?
                AND revision=?
                AND code='mailbox_uidvalidity_changed'
            "#)
            .bind(id).bind(who.id).bind(revision).execute(&mut *tx).await.map_err(db)?.rows_affected();
        if changed != 1 {
            return Err(Error::Changed);
        }
        sqlx::query("DELETE FROM ruphish_mailbox_messages WHERE mailbox_id=?")
            .bind(id)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::mysql::commit_error(e)))
    }
    async fn check_mailbox(&self, who: Identity, id: i64) -> Result<(), Error> {
        let mut tx = crate::mysql::locks::write(&self.pool).await.map_err(db)?;
        authorize(&mut tx, who, true, false).await?;
        let row = sqlx::query("SELECT enabled FROM ruphish_mailboxes WHERE id=? AND owner_id=?")
            .bind(id)
            .bind(who.id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(db)?
            .ok_or(Error::NotFound("Reporting mailbox"))?;
        if !row.try_get::<bool, _>("enabled").map_err(db)? {
            return Err(invalid("mailbox_disabled"));
        }
        sqlx::query("UPDATE ruphish_mailboxes SET next_check=0 WHERE id=? AND lease<=?")
            .bind(id)
            .bind(now())
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::mysql::commit_error(e)))
    }
}
