//! Attribution is written only by authenticated management transactions.
use ruphish_application::resources::Error;
use ruphish_domain::attribution::Attribution;
use sqlx::{Row, SqliteConnection, sqlite::SqliteRow};

pub(super) fn read(row: &SqliteRow, creator: &str) -> Result<Attribution, Error> {
    Ok(Attribution {
        created_by: row.try_get(creator).map_err(super::resource_error)?,
        edited_by: row.try_get("edited_by").map_err(super::resource_error)?,
        edited_at: row.try_get("edited_at").map_err(super::resource_error)?,
    })
}

pub(super) async fn edit(
    c: &mut SqliteConnection,
    table: &'static str,
    id: i64,
    actor: i64,
    at: &str,
) -> Result<(), Error> {
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "UPDATE {table} SET edited_by=?,edited_at=? WHERE id=?"
    )))
    .bind(actor)
    .bind(at)
    .bind(id)
    .execute(c)
    .await
    .map_err(super::resource_error)?;
    Ok(())
}
