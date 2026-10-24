//! InnoDB ownership for multi-row invariants, scoped to the current transaction.
use sqlx::MySqlConnection;

pub(super) async fn write(
    pool: &sqlx::MySqlPool,
) -> Result<sqlx::Transaction<'static, sqlx::MySql>, sqlx::Error> {
    let mut connection = pool.acquire().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
        .execute(&mut *connection)
        .await?;
    let mut tx = sqlx::Transaction::begin(connection, None).await?;
    sqlx::query("SELECT id FROM ruphish_settings WHERE id=1 FOR SHARE")
        .fetch_one(&mut *tx)
        .await?;
    Ok(tx)
}

pub(super) async fn snapshot(
    pool: &sqlx::MySqlPool,
) -> Result<sqlx::Transaction<'static, sqlx::MySql>, sqlx::Error> {
    let mut connection = pool.acquire().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *connection)
        .await?;
    sqlx::Transaction::begin(connection, None).await
}

/// Serialize one invariant, including the first writer, without an application mutex.
pub(super) async fn guard(c: &mut MySqlConnection, scope: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO ruphish_write_guards(scope) VALUES(?) ON DUPLICATE KEY UPDATE scope=scope",
    )
    .bind(scope)
    .execute(&mut *c)
    .await?;
    sqlx::query("SELECT scope FROM ruphish_write_guards WHERE scope=? FOR UPDATE")
        .bind(scope)
        .fetch_one(c)
        .await?;
    Ok(())
}

/// Account rows fence authorization changes and deletion against owned writes.
pub(super) async fn owner(c: &mut MySqlConnection, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT id FROM users WHERE id=? FOR UPDATE")
        .bind(id)
        .fetch_optional(c)
        .await?;
    Ok(())
}

pub(super) async fn campaign(
    c: &mut MySqlConnection,
    id: i64,
    exclusive: bool,
) -> Result<bool, sqlx::Error> {
    let owner: Option<i64> = sqlx::query_scalar("SELECT user_id FROM campaigns WHERE id=?")
        .bind(id)
        .fetch_optional(&mut *c)
        .await?;
    let Some(owner) = owner else { return Ok(false) };
    sqlx::query("SELECT id FROM users WHERE id=? FOR SHARE")
        .bind(owner)
        .fetch_optional(&mut *c)
        .await?;
    let query = if exclusive {
        "SELECT id FROM campaigns WHERE id=? FOR UPDATE"
    } else {
        "SELECT id FROM campaigns WHERE id=? FOR SHARE"
    };
    Ok(sqlx::query(query)
        .bind(id)
        .fetch_optional(c)
        .await?
        .is_some())
}

/// Domain workers take owner before domain, matching account deletion. Claims
/// skip a deleting/busy owner; completion waits and rechecks the domain afterward.
pub(super) async fn domain_owner(
    c: &mut MySqlConnection,
    id: i64,
    skip_locked: bool,
) -> Result<bool, sqlx::Error> {
    let owner: Option<i64> = sqlx::query_scalar("SELECT owner_id FROM ruphish_domains WHERE id=?")
        .bind(id)
        .fetch_optional(&mut *c)
        .await?;
    let Some(owner) = owner else { return Ok(false) };
    let query = if skip_locked {
        "SELECT id FROM users WHERE id=? FOR SHARE SKIP LOCKED"
    } else {
        "SELECT id FROM users WHERE id=? FOR SHARE"
    };
    Ok(sqlx::query(query)
        .bind(owner)
        .fetch_optional(c)
        .await?
        .is_some())
}

/// Exclusive installation boundary, acquired before any account or resource locks.
pub(super) async fn exclusive_policy(
    pool: &sqlx::MySqlPool,
) -> Result<sqlx::Transaction<'static, sqlx::MySql>, sqlx::Error> {
    let mut c = pool.acquire().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
        .execute(&mut *c)
        .await?;
    let mut tx = sqlx::Transaction::begin(c, None).await?;
    sqlx::query("SELECT id FROM ruphish_settings WHERE id=1 FOR UPDATE")
        .fetch_one(&mut *tx)
        .await?;
    Ok(tx)
}
