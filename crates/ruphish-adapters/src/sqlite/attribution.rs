//! Attribution is written only by authenticated management transactions.
use ruphish_application::resources::Error;
use ruphish_domain::attribution::Attribution;
use sqlx::{Row, SqliteConnection, sqlite::SqliteRow};

pub(super) fn read(row: &SqliteRow, creator: &str) -> Result<Attribution, Error> {
    read_at(
        row,
        creator,
        row.try_get("created_at").map_err(super::resource_error)?,
    )
}

pub(super) fn read_at(
    row: &SqliteRow,
    creator: &str,
    created_at: Option<String>,
) -> Result<Attribution, Error> {
    Ok(Attribution {
        created_at,
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

/// Capture every display dependency before a management transaction commits.
pub(super) async fn saved<T: ruphish_application::attribution::Attributed>(
    c: &mut SqliteConnection,
    record: T,
) -> Result<ruphish_application::attribution::Saved<T>, Error> {
    let ids = ruphish_application::attribution::identity_ids(record.attributions());
    let identities = super::accounts::display_identities(c, &ids).await?;
    Ok(ruphish_application::attribution::Saved { record, identities })
}
