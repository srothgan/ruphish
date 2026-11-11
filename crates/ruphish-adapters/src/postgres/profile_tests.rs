//! Provider-specific transaction setup for shared sender diagnostics.
use sqlx::{PgConnection, PgPool};
async fn write(pool: &PgPool) -> Result<sqlx::Transaction<'static, sqlx::Postgres>, sqlx::Error> {
    super::locks::write(pool).await
}
async fn guard(_connection: &mut PgConnection) -> Result<(), sqlx::Error> {
    super::locks::guard(_connection, "shared-resources").await
}
crate::profile_tests::repository!(PgConnection, PgPool, sqlx::postgres::PgRow, true);
