//! Shared SQL for caller- and operation-scoped creation receipts.
macro_rules! repository {
    ($connection:ty, $postgres:expr) => {
        use ruphish_application::{campaign::idempotency::{self, Operation}, resources::Error};
        use sqlx::Row;
        pub(crate) async fn lookup(
            c: &mut $connection, caller: i64, operation: Operation, request: &(String, String),
        ) -> Result<Option<i64>, Error> {
            idempotency::validate(&request.0)?;
            let sql = crate::sql::placeholders("SELECT fingerprint,resource_id FROM ruphish_campaign_creation WHERE owner_id=? AND operation=? AND request_key=?", $postgres);
            let row = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(caller).bind(operation.as_str()).bind(&request.0)
                .fetch_optional(c).await.map_err(super::storage)?;
            row.map(|row| {
                idempotency::reconcile(&request.1, &row.try_get::<String,_>("fingerprint").map_err(super::storage)?)?;
                row.try_get("resource_id").map_err(super::storage)
            }).transpose()
        }
        pub(crate) async fn remember(
            c: &mut $connection, caller: i64, operation: Operation, request: &(String, String), id: i64,
        ) -> Result<(), Error> {
            let sql = crate::sql::placeholders("INSERT INTO ruphish_campaign_creation(owner_id,operation,request_key,fingerprint,resource_id) VALUES(?,?,?,?,?)", $postgres);
            sqlx::query(sqlx::AssertSqlSafe(sql)).bind(caller).bind(operation.as_str()).bind(&request.0).bind(&request.1).bind(id)
                .execute(c).await.map_err(super::storage)?;
            Ok(())
        }
    };
}
pub(crate) use repository;
