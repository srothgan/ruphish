//! Account transactions over the preserved Gophish schema.
use crate::postgres::InsertId;
use ruphish_application::auth::{Account, AccountError, Error, Identity, Repository, Role, Write};
use ruphish_domain::account::preserve_administrator;
use sqlx::{PgConnection, PgPool, Row, postgres::PgRow};

/// SQLx account repository; authorization is rechecked within every write transaction.
#[derive(Clone)]
pub struct Accounts {
    pool: PgPool,
}
impl Accounts {
    /// Compose with the startup-owned pool.
    pub fn new(pool: PgPool) -> Self {
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

pub(super) async fn account(connection: &mut PgConnection, id: i64) -> Result<Account, Error> {
    let row = sqlx::query("SELECT u.*,COALESCE(a.epoch,0) AS epoch FROM users u LEFT JOIN ruphish_auth_state a ON a.user_id=u.id WHERE u.id=$1")
        .bind(id).fetch_optional(&mut *connection).await.map_err(storage)?.ok_or(Error::NotFound)?;
    let role_id: i64 = row.try_get("role_id").map_err(storage)?;
    let role_row = sqlx::query("SELECT id,slug,name,description FROM roles WHERE id=$1")
        .bind(role_id)
        .fetch_optional(&mut *connection)
        .await
        .map_err(storage)?
        .ok_or(Error::RoleNotFound)?;
    let permissions = sqlx::query_scalar("SELECT p.slug FROM permissions p JOIN role_permissions rp ON rp.permission_id=p.id WHERE rp.role_id=$1 ORDER BY p.slug").bind(role_id).fetch_all(&mut *connection).await.map_err(storage)?;
    let text = |name: &str| -> Result<String, Error> {
        Ok(row
            .try_get::<Option<String>, _>(name)
            .map_err(storage)?
            .unwrap_or_default())
    };
    Ok(Account {
        identity: Identity {
            id,
            epoch: row.try_get("epoch").map_err(storage)?,
        },
        username: text("username")?,
        hash: text("hash")?,
        api_key: text("api_key")?,
        role: role(role_row, permissions)?,
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
fn role(row: PgRow, permissions: Vec<String>) -> Result<Role, Error> {
    Ok(Role {
        id: row.try_get("id").map_err(storage)?,
        slug: row.try_get("slug").map_err(storage)?,
        name: row.try_get("name").map_err(storage)?,
        description: row.try_get("description").map_err(storage)?,
        permissions,
    })
}
async fn bump(connection: &mut PgConnection, id: i64) -> Result<(), Error> {
    sqlx::query("INSERT INTO ruphish_auth_state(user_id,epoch) VALUES($1,1) ON CONFLICT (user_id) DO UPDATE SET epoch=ruphish_auth_state.epoch+1").bind(id).execute(connection).await.map_err(storage)?;
    Ok(())
}
async fn remaining_admins(connection: &mut PgConnection) -> Result<i64, Error> {
    sqlx::query_scalar("SELECT count(*) FROM users u JOIN roles r ON r.id=u.role_id WHERE r.slug='admin' AND COALESCE(u.account_locked,FALSE)=FALSE").fetch_one(connection).await.map_err(storage)
}
fn random_key() -> Result<String, Error> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes).map_err(|_| Error::Internal)?;
    Ok(hex::encode(bytes))
}

#[async_trait::async_trait]
impl Repository for Accounts {
    async fn by_id(&self, id: i64) -> Result<Account, Error> {
        let mut tx = super::locks::snapshot(&self.pool).await.map_err(storage)?;
        let result = account(&mut tx, id).await?;
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(result)
    }
    async fn by_name(&self, name: &str) -> Result<Account, Error> {
        let mut connection = super::locks::snapshot(&self.pool).await.map_err(storage)?;
        let id = sqlx::query_scalar("SELECT id FROM users WHERE username=$1")
            .bind(name)
            .fetch_optional(&mut *connection)
            .await
            .map_err(storage)?
            .ok_or(Error::NotFound)?;
        let result = account(&mut connection, id).await?;
        connection
            .commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(result)
    }
    async fn by_key(&self, key: &str) -> Result<Account, Error> {
        if key.is_empty() {
            return Err(Error::InvalidCredentials);
        }
        let mut connection = super::locks::snapshot(&self.pool).await.map_err(storage)?;
        let id = sqlx::query_scalar("SELECT id FROM users WHERE api_key=$1")
            .bind(key)
            .fetch_optional(&mut *connection)
            .await
            .map_err(storage)?
            .ok_or(Error::InvalidCredentials)?;
        let result = account(&mut connection, id).await?;
        connection
            .commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(result)
    }
    async fn list(&self, caller: Identity) -> Result<Vec<Account>, Error> {
        let mut tx = crate::postgres::locks::snapshot(&self.pool)
            .await
            .map_err(storage)?;
        let actor = account(&mut tx, caller.id).await?;
        actor.authenticate(caller)?;
        actor.require("modify_system")?;
        let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM users ORDER BY id")
            .fetch_all(&mut *tx)
            .await
            .map_err(storage)?;
        let mut users = Vec::with_capacity(ids.len());
        for id in ids {
            users.push(account(&mut tx, id).await?);
        }
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(users)
    }
    async fn write(&self, caller: Identity, write: Write) -> Result<Option<Account>, Error> {
        let mut tx = crate::postgres::locks::write(&self.pool)
            .await
            .map_err(storage)?;
        if matches!(&write, Write::Save { .. } | Write::Delete(_)) {
            super::locks::guard(&mut tx, "accounts")
                .await
                .map_err(storage)?;
        }
        let target = match &write {
            Write::Save {
                target: Some(target),
                ..
            } => target.id,
            Write::Delete(id) => *id,
            _ => caller.id,
        };
        for id in [caller.id.min(target), caller.id.max(target)] {
            super::locks::owner(&mut tx, id).await.map_err(storage)?;
        }
        let actor = account(&mut tx, caller.id).await?;
        actor.authenticate(caller)?;
        let result_id = match write {
            Write::Login => {
                sqlx::query("UPDATE users SET last_login=$1 WHERE id=$2")
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
                sqlx::query("UPDATE users SET hash=$1,password_change_required=FALSE WHERE id=$2")
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
                sqlx::query("UPDATE users SET api_key=$1 WHERE id=$2")
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
                    if !actor.permits("modify_system")
                        && (edit.role != existing.role.slug
                            || edit.account_locked != existing.account_locked
                            || edit.password_change_required != existing.password_change_required)
                    {
                        return Err(AccountError::Escalation.into());
                    }
                    Some(existing)
                } else {
                    actor.require("modify_system")?;
                    None
                };
                let role_id: i64 = sqlx::query_scalar("SELECT id FROM roles WHERE slug=$1")
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
                    sqlx::query("UPDATE users SET username=$1,role_id=$2,hash=$3,password_change_required=$4,account_locked=$5 WHERE id=$6")
                        .bind(edit.username).bind(role_id).bind(hash.unwrap_or(existing.hash)).bind(edit.password_change_required).bind(edit.account_locked).bind(existing.identity.id).execute(&mut *tx).await.map_err(storage)?;
                    bump(&mut tx, existing.identity.id).await?;
                    existing.identity.id
                } else {
                    sqlx::query("INSERT INTO users(username,hash,api_key,role_id,password_change_required,last_login,account_locked) VALUES($1,$2,$3,$4,$5,'0001-01-01T00:00:00Z',$6) RETURNING id")
                        .bind(edit.username).bind(hash.ok_or(Error::Internal)?).bind(random_key()?).bind(role_id).bind(edit.password_change_required).bind(edit.account_locked).fetch_one(&mut *tx).await.map_err(storage)?.insert_id().map_err(storage)?
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
                let campaigns: Vec<i64> =
                    sqlx::query_scalar("SELECT id FROM campaigns WHERE user_id=$1")
                        .bind(id)
                        .fetch_all(&mut *tx)
                        .await
                        .map_err(storage)?;
                for campaign in campaigns {
                    super::events::remove_campaign(&mut tx, campaign)
                        .await
                        .map_err(storage)?;
                }
                // All owned records, including legacy queue state, are removed in this transaction.
                // Shared target rows and system-level webhooks have no exclusive account ownership.
                for statement in [
                    "DELETE FROM ruphish_delivery_attempt WHERE result_id IN (SELECT id FROM results WHERE user_id=$1)",
                    "DELETE FROM ruphish_submission_ids WHERE result_id IN (SELECT id FROM results WHERE user_id=$1)",
                    "DELETE FROM ruphish_campaign_domains WHERE campaign_id IN (SELECT id FROM campaigns WHERE user_id=$1)",
                    "DELETE FROM ruphish_domain_claims WHERE owner_id=$1",
                    "DELETE FROM ruphish_domain_checks WHERE domain_id IN (SELECT id FROM ruphish_domains WHERE owner_id=$1)",
                    "DELETE FROM ruphish_domains WHERE owner_id=$1",
                    "DELETE FROM ruphish_delivery WHERE campaign_id IN (SELECT id FROM campaigns WHERE user_id=$1)",
                    "DELETE FROM ruphish_campaign_policy WHERE campaign_id IN (SELECT id FROM campaigns WHERE user_id=$1)",
                    "DELETE FROM ruphish_campaign_groups WHERE campaign_id IN (SELECT id FROM campaigns WHERE user_id=$1)",
                    "DELETE FROM ruphish_campaign_launch WHERE owner_id=$1",
                    "DELETE FROM ruphish_profile_cooldown WHERE profile_id IN (SELECT id FROM smtp WHERE user_id=$1)",
                    "DELETE FROM ruphish_legacy_queue_issues WHERE mail_log_id IN (SELECT id FROM mail_logs WHERE user_id=$1)",
                    "DELETE FROM events WHERE campaign_id IN (SELECT id FROM campaigns WHERE user_id=$1)",
                    "DELETE FROM results WHERE user_id=$1",
                    "DELETE FROM mail_logs WHERE user_id=$1",
                    "DELETE FROM campaigns WHERE user_id=$1",
                    "DELETE FROM attachments WHERE template_id IN (SELECT id FROM templates WHERE user_id=$1)",
                    "DELETE FROM templates WHERE user_id=$1",
                    "DELETE FROM pages WHERE user_id=$1",
                    "DELETE FROM group_targets WHERE group_id IN (SELECT id FROM groups WHERE user_id=$1)",
                    "DELETE FROM groups WHERE user_id=$1",
                    "DELETE FROM headers WHERE smtp_id IN (SELECT id FROM smtp WHERE user_id=$1)",
                    "DELETE FROM smtp WHERE user_id=$1",
                    "DELETE FROM ruphish_mailbox_runs WHERE mailbox_id IN (SELECT id FROM ruphish_mailboxes WHERE owner_id=$1)",
                    "DELETE FROM ruphish_mailbox_messages WHERE mailbox_id IN (SELECT id FROM ruphish_mailboxes WHERE owner_id=$1)",
                    "DELETE FROM ruphish_mailboxes WHERE owner_id=$1",
                    "DELETE FROM imap WHERE user_id=$1",
                    "DELETE FROM email_requests WHERE user_id=$1",
                    "DELETE FROM users WHERE id=$1",
                ] {
                    sqlx::query(statement)
                        .bind(id)
                        .execute(&mut *tx)
                        .await
                        .map_err(storage)?;
                }
                // Keep the generation tombstone so identifier reuse cannot revive an old session.
                bump(&mut tx, id).await?;
                None
            }
        };
        let result = if let Some(id) = result_id {
            Some(account(&mut tx, id).await?)
        } else {
            None
        };
        tx.commit()
            .await
            .map_err(|e| Error::Storage(crate::postgres::commit_error(e)))?;
        Ok(result)
    }
}
