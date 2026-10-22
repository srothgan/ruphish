//! Reconstruct previous integration table identities for historical upgrade fixtures.
pub async fn restore_version_nine_tables(pool: &sqlx::SqlitePool) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    let triggers: Vec<String> = sqlx::query_scalar("SELECT sql FROM sqlite_master WHERE type='trigger' AND name IN ('ruphish_event_insert','ruphish_event_delete') ORDER BY name").fetch_all(&mut *tx).await?;
    sqlx::raw_sql("DROP TRIGGER ruphish_event_insert; DROP TRIGGER ruphish_event_delete;")
        .execute(&mut *tx)
        .await?;
    for table in [
        "ruphish_mailboxes",
        "ruphish_mailbox_runs",
        "ruphish_webhook_jobs",
        "ruphish_webhook_attempts",
    ] {
        let schema: String =
            sqlx::query_scalar("SELECT sql FROM sqlite_master WHERE type='table' AND name=?")
                .bind(table)
                .fetch_one(&mut *tx)
                .await?;
        let prior = schema
            .replace("AUTOINCREMENT", "")
            .replacen(table, &format!("{table}_v9"), 1);
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("{prior}; INSERT INTO {table}_v9 SELECT * FROM {table}; DROP TABLE {table}; ALTER TABLE {table}_v9 RENAME TO {table};"))).execute(&mut *tx).await?;
    }
    for trigger in triggers {
        sqlx::raw_sql(sqlx::AssertSqlSafe(trigger))
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("CREATE INDEX ruphish_webhook_due ON ruphish_webhook_jobs(state,due,lease)")
        .execute(&mut *tx)
        .await?;
    tx.commit().await
}
