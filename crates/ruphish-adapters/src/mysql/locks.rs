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

/// Account rows fence authorization changes and deletion against authenticated writes.
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
