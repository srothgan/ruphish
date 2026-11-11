//! Provider-specific transaction setup for shared sender diagnostics.
use sqlx::{MySqlConnection, MySqlPool};
async fn write(pool: &MySqlPool) -> Result<sqlx::Transaction<'static, sqlx::MySql>, sqlx::Error> {
    crate::database::mysql::locks::write(pool).await
}
async fn guard(_connection: &mut MySqlConnection) -> Result<(), sqlx::Error> {
    crate::database::mysql::locks::guard(_connection, "shared-resources").await
}
crate::profile_diagnostics::repository!(MySqlConnection, MySqlPool, sqlx::mysql::MySqlRow, false);
