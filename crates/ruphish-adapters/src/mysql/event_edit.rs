//! Provider-specific transaction and generated identity mechanics.
crate::event_edit::repository!(sqlx::MySqlConnection, false);
fn storage(error: sqlx::Error) -> Error {
    super::resource_error(error)
}
async fn begin(pool: &sqlx::MySqlPool) -> Result<sqlx::Transaction<'_, sqlx::MySql>, Error> {
    super::locks::write(pool).await.map_err(storage)
}
async fn commit(tx: sqlx::Transaction<'_, sqlx::MySql>) -> Result<(), Error> {
    tx.commit()
        .await
        .map_err(|e| Error::Storage(super::commit_error(e)))
}
async fn insert(
    c: &mut sqlx::MySqlConnection,
    campaign: i64,
    recipient: i64,
    fact: &Fact,
    create_key: &str,
) -> Result<i64, Error> {
    let ruphish_domain::events::RecordedKind::Known(kind) = fact.kind else {
        return Err(Error::Internal);
    };
    let details = crate::event_edit::details(fact)?;
    let row = sqlx::query(sqlx::AssertSqlSafe(crate::sql::placeholders("INSERT INTO events(campaign_id,recipient_id,email,time,kind,metadata,source,create_key) SELECT ?,id,email,?,?,?,'operator',? FROM results WHERE id=?", false)))
        .bind(campaign).bind(&fact.time).bind(kind.key()).bind(details).bind(create_key).bind(recipient).execute(c).await.map_err(storage)?;
    use super::InsertId;
    row.insert_id().map_err(storage)
}
