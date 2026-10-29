//! Reconstruct previous integration table identities for historical upgrade fixtures.
pub async fn restore_version_ten_tables(pool: &sqlx::SqlitePool) -> Result<(), sqlx::Error> {
    use sqlx::Connection;
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/observations/ruphish-v10.sqlite3");
    let mut reference = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(path)
            .read_only(true),
    )
    .await?;
    let mut tx = pool.begin().await?;
    sqlx::raw_sql(include_str!("performance_versions/sqlite.sql"))
        .execute(&mut *tx)
        .await?;
    // Copy through the historical projection into independently recorded schemas.
    // Preserve populated history, policies and retry tombstones while restoring v10.
    for (table, source) in [
        ("events", "event_history"),
        ("pages", "pages"),
        ("ruphish_campaign_policy", "ruphish_campaign_policy"),
        ("ruphish_submission_ids", "ruphish_event_receipts"),
        ("ruphish_event_sources", "events"),
    ] {
        let schema: String =
            sqlx::query_scalar("SELECT sql FROM sqlite_master WHERE type='table' AND name=?")
                .bind(table)
                .fetch_one(&mut reference)
                .await?;
        let columns: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_table_info(?) ORDER BY cid")
                .bind(table)
                .fetch_all(&mut reference)
                .await?;
        let projection = columns
            .iter()
            .map(|column| match (table, column.as_str()) {
                (_, "capture_credentials" | "capture_passwords") => "0",
                ("ruphish_submission_ids", "event_id") => "request_id",
                ("ruphish_event_sources", "event_id") => "id",
                _ => column.as_str(),
            })
            .collect::<Vec<_>>()
            .join(",");
        let prior = schema.replacen(table, &format!("fixture_{table}"), 1);
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "{prior}; INSERT INTO fixture_{table} SELECT {projection} FROM {source};"
        )))
        .execute(&mut *tx)
        .await?;
    }
    sqlx::raw_sql("DROP TRIGGER ruphish_event_insert; DROP TRIGGER ruphish_event_delete; DROP TABLE ruphish_learning_pages; DROP VIEW ruphish_domain_policy; DROP TABLE ruphish_campaign_closures; DROP TABLE ruphish_settings; DROP VIEW event_history; DROP TABLE events; DROP TABLE pages; DROP TABLE ruphish_campaign_policy; DROP TABLE ruphish_event_receipts;")
        .execute(&mut *tx).await?;
    for table in [
        "events",
        "pages",
        "ruphish_campaign_policy",
        "ruphish_submission_ids",
        "ruphish_event_sources",
    ] {
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "ALTER TABLE fixture_{table} RENAME TO {table}"
        )))
        .execute(&mut *tx)
        .await?;
    }
    let definitions: Vec<String> = sqlx::query_scalar("SELECT sql FROM sqlite_master WHERE tbl_name='events' AND type IN ('index','trigger') AND sql IS NOT NULL ORDER BY name")
        .fetch_all(&mut reference).await?;
    for definition in definitions {
        sqlx::raw_sql(sqlx::AssertSqlSafe(definition))
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("DELETE FROM ruphish_schema WHERE version>=11")
        .execute(&mut *tx)
        .await?;
    tx.commit().await
}

pub async fn restore_version_nine_tables(pool: &sqlx::SqlitePool) -> Result<(), sqlx::Error> {
    restore_version_ten_tables(pool).await?;
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
