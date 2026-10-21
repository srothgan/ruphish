//! PostgreSQL ownership for multi-row invariants, scoped to the current transaction.
use sqlx::PgConnection;

pub(super) async fn write(
    pool: &sqlx::PgPool,
) -> Result<sqlx::Transaction<'static, sqlx::Postgres>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

pub(super) async fn snapshot(
    pool: &sqlx::PgPool,
) -> Result<sqlx::Transaction<'static, sqlx::Postgres>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

/// Serialize one invariant, including the first writer, without an application mutex.
pub(super) async fn guard(c: &mut PgConnection, scope: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO ruphish_write_guards(scope) VALUES($1) ON CONFLICT (scope) DO NOTHING",
    )
    .bind(scope)
    .execute(&mut *c)
    .await?;
    sqlx::query("SELECT scope FROM ruphish_write_guards WHERE scope=$1 FOR UPDATE")
        .bind(scope)
        .fetch_one(c)
        .await?;
    Ok(())
}

/// Account rows fence authorization changes and deletion against owned writes.
pub(super) async fn owner(c: &mut PgConnection, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(id)
        .fetch_optional(c)
        .await?;
    Ok(())
}

pub(super) async fn campaign(
    c: &mut PgConnection,
    id: i64,
    exclusive: bool,
) -> Result<bool, sqlx::Error> {
    let owner: Option<i64> = sqlx::query_scalar("SELECT user_id FROM campaigns WHERE id=$1")
        .bind(id)
        .fetch_optional(&mut *c)
        .await?;
    let Some(owner) = owner else { return Ok(false) };
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR SHARE")
        .bind(owner)
        .fetch_optional(&mut *c)
        .await?;
    let query = if exclusive {
        "SELECT id FROM campaigns WHERE id=$1 FOR UPDATE"
    } else {
        "SELECT id FROM campaigns WHERE id=$1 FOR SHARE"
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
    c: &mut PgConnection,
    id: i64,
    skip_locked: bool,
) -> Result<bool, sqlx::Error> {
    let owner: Option<i64> = sqlx::query_scalar("SELECT owner_id FROM ruphish_domains WHERE id=$1")
        .bind(id)
        .fetch_optional(&mut *c)
        .await?;
    let Some(owner) = owner else { return Ok(false) };
    let query = if skip_locked {
        "SELECT id FROM users WHERE id=$1 FOR SHARE SKIP LOCKED"
    } else {
        "SELECT id FROM users WHERE id=$1 FOR SHARE"
    };
    Ok(sqlx::query(query)
        .bind(owner)
        .fetch_optional(c)
        .await?
        .is_some())
}
