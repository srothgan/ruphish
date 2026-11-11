//! Provider-specific transaction setup for shared sender diagnostics.
use sqlx::{SqliteConnection, SqlitePool};
async fn write(pool: &SqlitePool) -> Result<sqlx::Transaction<'static, sqlx::Sqlite>, sqlx::Error> {
    pool.begin_with("BEGIN IMMEDIATE").await
}
async fn guard(_connection: &mut SqliteConnection) -> Result<(), sqlx::Error> {
    Ok(())
}
crate::profile_tests::repository!(SqliteConnection, SqlitePool, sqlx::sqlite::SqliteRow, false);
