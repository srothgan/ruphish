//! Account transactions over the preserved Gophish schema.
use ruphish_application::auth::{Account, Error, Identity, Repository, Role, Write};
use ruphish_domain::account::preserve_administrator;
use sqlx::{Row, SqliteConnection, SqlitePool, sqlite::SqliteRow};

/// SQLx account repository; authorization is rechecked within every write transaction.
#[derive(Clone)]
pub struct Accounts {
    pool: SqlitePool,
}
impl Accounts {
    /// Compose with the startup-owned pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn storage(error: sqlx::Error) -> Error {
    if error
        .as_database_error()
        .is_some_and(|e| e.is_unique_violation())
    {
        Error::Conflict
    } else {
        Error::Storage(super::storage_error(error))
    }
}

pub(super) async fn account(connection: &mut SqliteConnection, id: i64) -> Result<Account, Error> {
    let row = sqlx::query("SELECT u.*,COALESCE(a.epoch,0) AS epoch,p.language,p.format_locale,p.time_zone,p.show_utc_offset FROM users u LEFT JOIN ruphish_auth_state a ON a.user_id=u.id LEFT JOIN ruphish_user_preferences p ON p.user_id=u.id WHERE u.id=?")
        .bind(id).fetch_optional(&mut *connection).await.map_err(storage)?.ok_or(Error::NotFound)?;
    let role_id: i64 = row.try_get("role_id").map_err(storage)?;
    let role_row = sqlx::query("SELECT id,slug,name,description FROM roles WHERE id=?")
        .bind(role_id)
        .fetch_optional(&mut *connection)
        .await
        .map_err(storage)?
        .ok_or(Error::RoleNotFound)?;
    let permissions = sqlx::query_scalar("SELECT p.slug FROM permissions p JOIN role_permissions rp ON rp.permission_id=p.id WHERE rp.role_id=? ORDER BY p.slug").bind(role_id).fetch_all(&mut *connection).await.map_err(storage)?;
    decode_account(row, role(role_row, permissions)?)
}
fn decode_account(row: SqliteRow, role: Role) -> Result<Account, Error> {
    let text = |name: &str| -> Result<String, Error> {
        Ok(row
            .try_get::<Option<String>, _>(name)
            .map_err(storage)?
            .unwrap_or_default())
    };
    Ok(Account {
        preferences: ruphish_application::auth::Preferences {
            language: row.try_get("language").map_err(storage)?,
            format_locale: row.try_get("format_locale").map_err(storage)?,
            time_zone: row.try_get("time_zone").map_err(storage)?,
            show_utc_offset: row
                .try_get::<Option<bool>, _>("show_utc_offset")
                .map_err(storage)?
                .unwrap_or(true),
        },
        identity: Identity {
            id: row.try_get("id").map_err(storage)?,
            epoch: row.try_get("epoch").map_err(storage)?,
        },
        username: text("username")?,
        hash: text("hash")?,
        api_key: text("api_key")?,
        role,
        password_change_required: row
            .try_get::<Option<bool>, _>("password_change_required")
            .map_err(storage)?
            .unwrap_or(false),
        account_locked: row
            .try_get::<Option<bool>, _>("account_locked")
            .map_err(storage)?
            .unwrap_or(false),
        last_login: text("last_login")?,
    })
}
fn role(row: SqliteRow, permissions: Vec<String>) -> Result<Role, Error> {
    Ok(Role {
        id: row.try_get("id").map_err(storage)?,
        slug: row.try_get("slug").map_err(storage)?,
        name: row.try_get("name").map_err(storage)?,
        description: row.try_get("description").map_err(storage)?,
        permissions,
    })
}
async fn bump(connection: &mut SqliteConnection, id: i64) -> Result<(), Error> {
    sqlx::query("INSERT INTO ruphish_auth_state(user_id,epoch) VALUES(?,1) ON CONFLICT(user_id) DO UPDATE SET epoch=epoch+1").bind(id).execute(connection).await.map_err(storage)?;
    Ok(())
}
async fn remaining_admins(connection: &mut SqliteConnection) -> Result<i64, Error> {
    sqlx::query_scalar("SELECT count(*) FROM users u JOIN roles r ON r.id=u.role_id WHERE r.slug='admin' AND COALESCE(u.account_locked,0)=0").fetch_one(connection).await.map_err(storage)
}
fn random_key() -> Result<String, Error> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes).map_err(|_| Error::Internal)?;
    Ok(hex::encode(bytes))
}

#[async_trait::async_trait]
impl Repository for Accounts {
    async fn by_id(&self, id: i64) -> Result<Account, Error> {
        account(&mut *self.pool.acquire().await.map_err(storage)?, id).await
    }
    async fn by_name(&self, name: &str) -> Result<Account, Error> {
        let mut connection = self.pool.acquire().await.map_err(storage)?;
        let id = sqlx::query_scalar("SELECT id FROM users WHERE username=?")
            .bind(name)
            .fetch_optional(&mut *connection)
            .await
            .map_err(storage)?
            .ok_or(Error::NotFound)?;
        account(&mut connection, id).await
    }
    async fn by_key(&self, key: &str) -> Result<Account, Error> {
        if key.is_empty() {
            return Err(Error::InvalidCredentials);
        }
        let mut connection = self.pool.acquire().await.map_err(storage)?;
        let id = sqlx::query_scalar("SELECT id FROM users WHERE api_key=?")
            .bind(key)
            .fetch_optional(&mut *connection)
            .await
            .map_err(storage)?
            .ok_or(Error::InvalidCredentials)?;
        account(&mut connection, id).await
    }
    async fn list(&self, caller: Identity) -> Result<Vec<Account>, Error> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let actor = account(&mut tx, caller.id).await?;
        actor.authenticate(caller)?;
        actor.require("modify_system")?;
        let rows = sqlx::query("SELECT u.*,COALESCE(a.epoch,0) AS epoch,p.language,p.format_locale,p.time_zone,p.show_utc_offset FROM users u LEFT JOIN ruphish_auth_state a ON a.user_id=u.id LEFT JOIN ruphish_user_preferences p ON p.user_id=u.id ORDER BY u.id")
            .fetch_all(&mut *tx).await.map_err(storage)?;
        let role_rows = sqlx::query("SELECT id,slug,name,description FROM roles WHERE EXISTS(SELECT 1 FROM users WHERE users.role_id=roles.id)")
            .fetch_all(&mut *tx).await.map_err(storage)?;
        let permission_rows = sqlx::query("SELECT rp.role_id,p.slug FROM permissions p JOIN role_permissions rp ON rp.permission_id=p.id WHERE EXISTS(SELECT 1 FROM users WHERE users.role_id=rp.role_id) ORDER BY p.slug")
            .fetch_all(&mut *tx).await.map_err(storage)?;
        let mut permissions = std::collections::BTreeMap::<i64, Vec<String>>::new();
        for row in permission_rows {
            permissions
                .entry(row.try_get("role_id").map_err(storage)?)
                .or_default()
                .push(row.try_get("slug").map_err(storage)?);
        }
        let mut roles = std::collections::BTreeMap::new();
        for row in role_rows {
            let id: i64 = row.try_get("id").map_err(storage)?;
            roles.insert(id, role(row, permissions.remove(&id).unwrap_or_default())?);
        }
        let mut users = Vec::with_capacity(rows.len());
        for row in rows {
            let role_id: i64 = row.try_get("role_id").map_err(storage)?;
            let role = roles.get(&role_id).ok_or(Error::RoleNotFound)?.clone();
            users.push(decode_account(row, role)?);
        }
        tx.commit().await.map_err(storage)?;
        Ok(users)
    }
    async fn write(&self, caller: Identity, write: Write) -> Result<Option<Account>, Error> {
        let mut tx = self
            .pool
            .begin_with("BEGIN IMMEDIATE")
            .await
            .map_err(storage)?;
        let actor = account(&mut tx, caller.id).await?;
        actor.authenticate(caller)?;
        let result_id = match write {
            Write::Preferences(preferences) => {
                if actor.password_change_required {
                    return Err(ruphish_application::auth::AccountError::ResetRequired.into());
                }
                preferences.validate()?;
                sqlx::query("INSERT INTO ruphish_user_preferences(user_id,language,format_locale,time_zone,show_utc_offset) VALUES(?,?,?,?,?) ON CONFLICT(user_id) DO UPDATE SET language=excluded.language,format_locale=excluded.format_locale,time_zone=excluded.time_zone,show_utc_offset=excluded.show_utc_offset")
                    .bind(caller.id).bind(preferences.language).bind(preferences.format_locale).bind(preferences.time_zone).bind(preferences.show_utc_offset)
                    .execute(&mut *tx).await.map_err(storage)?;
                Some(caller.id)
            }
            Write::Login => {
                sqlx::query("UPDATE users SET last_login=? WHERE id=?")
                    .bind(ruphish_application::datetime::utc(
                        ruphish_application::datetime::now(),
                    ))
                    .bind(caller.id)
                    .execute(&mut *tx)
                    .await
                    .map_err(storage)?;
                Some(caller.id)
            }
            Write::Password {
                expected_hash,
                hash,
            } => {
                if actor.hash != expected_hash {
                    return Err(Error::Changed);
                }
                sqlx::query("UPDATE users SET hash=?,password_change_required=0 WHERE id=?")
                    .bind(hash)
                    .bind(caller.id)
                    .execute(&mut *tx)
                    .await
                    .map_err(storage)?;
                bump(&mut tx, caller.id).await?;
                Some(caller.id)
            }
            Write::RotateKey => {
                actor.require("modify_objects")?;
                sqlx::query("UPDATE users SET api_key=? WHERE id=?")
                    .bind(random_key()?)
                    .bind(caller.id)
                    .execute(&mut *tx)
                    .await
                    .map_err(storage)?;
                Some(caller.id)
            }
            Write::Save { target, edit, hash } => {
                let existing = if let Some(target) = target {
                    actor.access_account(target.id, true)?;
                    let existing = account(&mut tx, target.id).await?;
                    if existing.identity != target {
                        return Err(Error::Changed);
                    }
                    actor.authorize_edit(&existing, &edit)?;
                    Some(existing)
                } else {
                    actor.require("modify_system")?;
                    None
                };
                let role_id: i64 = sqlx::query_scalar("SELECT id FROM roles WHERE slug=?")
                    .bind(&edit.role)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(storage)?
                    .ok_or(Error::RoleNotFound)?;
                let id = if let Some(existing) = existing {
                    preserve_administrator(
                        existing.role.slug == "admin" && !existing.account_locked,
                        edit.role == "admin" && !edit.account_locked,
                        remaining_admins(&mut tx).await?,
                    )?;
                    sqlx::query("UPDATE users SET username=?,role_id=?,hash=?,password_change_required=?,account_locked=? WHERE id=?")
                        .bind(edit.username).bind(role_id).bind(hash.unwrap_or(existing.hash)).bind(edit.password_change_required).bind(edit.account_locked).bind(existing.identity.id).execute(&mut *tx).await.map_err(storage)?;
                    bump(&mut tx, existing.identity.id).await?;
                    existing.identity.id
                } else {
                    sqlx::query("INSERT INTO users(username,hash,api_key,role_id,password_change_required,last_login,account_locked) VALUES(?,?,?,?,?,'0001-01-01T00:00:00Z',?)")
                        .bind(edit.username).bind(hash.ok_or(Error::Internal)?).bind(random_key()?).bind(role_id).bind(edit.password_change_required).bind(edit.account_locked).execute(&mut *tx).await.map_err(storage)?.last_insert_rowid()
                };
                Some(id)
            }
            Write::Delete(id) => {
                actor.delete_account(id)?;
                let target = account(&mut tx, id).await?;
                preserve_administrator(
                    target.role.slug == "admin" && !target.account_locked,
                    false,
                    remaining_admins(&mut tx).await?,
                )?;
                // All owned records, including legacy queue state, are removed in this transaction.
                // Shared target rows and system-level webhooks have no exclusive account ownership.
                for statement in [
                    "DELETE FROM ruphish_delivery_attempt WHERE result_id IN (SELECT id FROM results WHERE user_id=?)",
                    "DELETE FROM ruphish_event_receipts WHERE result_id IN (SELECT id FROM results WHERE user_id=?)",
                    "DELETE FROM ruphish_campaign_domains WHERE campaign_id IN (SELECT id FROM campaigns WHERE user_id=?)",
                    "DELETE FROM ruphish_domain_claims WHERE owner_id=?",
                    "DELETE FROM ruphish_domain_checks WHERE domain_id IN (SELECT id FROM ruphish_domains WHERE owner_id=?)",
                    "DELETE FROM ruphish_domains WHERE owner_id=?",
                    "DELETE FROM ruphish_delivery WHERE campaign_id IN (SELECT id FROM campaigns WHERE user_id=?)",
                    "DELETE FROM ruphish_campaign_policy WHERE campaign_id IN (SELECT id FROM campaigns WHERE user_id=?)",
                    "DELETE FROM ruphish_campaign_groups WHERE campaign_id IN (SELECT id FROM campaigns WHERE user_id=?)",
                    "DELETE FROM ruphish_campaign_launch WHERE owner_id=?",
                    "DELETE FROM ruphish_profile_cooldown WHERE profile_id IN (SELECT id FROM smtp WHERE user_id=?)",
                    "DELETE FROM ruphish_legacy_queue_issues WHERE mail_log_id IN (SELECT id FROM mail_logs WHERE user_id=?)",
                    "DELETE FROM events WHERE campaign_id IN (SELECT id FROM campaigns WHERE user_id=?)",
                    "DELETE FROM results WHERE user_id=?",
                    "DELETE FROM mail_logs WHERE user_id=?",
                    "DELETE FROM campaigns WHERE user_id=?",
                    "DELETE FROM attachments WHERE template_id IN (SELECT id FROM templates WHERE user_id=?)",
                    "DELETE FROM templates WHERE user_id=?",
                    "DELETE FROM pages WHERE user_id=?",
                    "DELETE FROM group_targets WHERE group_id IN (SELECT id FROM groups WHERE user_id=?)",
                    "DELETE FROM groups WHERE user_id=?",
                    "DELETE FROM headers WHERE smtp_id IN (SELECT id FROM smtp WHERE user_id=?)",
                    "DELETE FROM smtp WHERE user_id=?",
                    "DELETE FROM ruphish_mailbox_runs WHERE mailbox_id IN (SELECT id FROM ruphish_mailboxes WHERE owner_id=?)",
                    "DELETE FROM ruphish_mailbox_messages WHERE mailbox_id IN (SELECT id FROM ruphish_mailboxes WHERE owner_id=?)",
                    "DELETE FROM ruphish_mailboxes WHERE owner_id=?",
                    "DELETE FROM imap WHERE user_id=?",
                    "DELETE FROM email_requests WHERE user_id=?",
                    "DELETE FROM ruphish_user_preferences WHERE user_id=?",
                    "DELETE FROM ruphish_campaign_drafts WHERE owner_id=?",
                    "DELETE FROM users WHERE id=?",
                ] {
                    sqlx::query(statement)
                        .bind(id)
                        .execute(&mut *tx)
                        .await
                        .map_err(storage)?;
                }
                // Keep the generation tombstone so SQLite identifier reuse cannot revive an old session.
                bump(&mut tx, id).await?;
                None
            }
        };
        let result = if let Some(id) = result_id {
            Some(account(&mut tx, id).await?)
        } else {
            None
        };
        tx.commit().await.map_err(storage)?;
        Ok(result)
    }
}
