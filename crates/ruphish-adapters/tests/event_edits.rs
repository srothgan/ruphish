//! Direct mutations preserve ownership, atomicity and persistence without an audit store.
#![allow(clippy::unwrap_used, reason = "Independent fixture expectations")]
#[path = "support/event_edits.rs"]
mod scenarios;
mod support;
use ruphish_application::event_edit::Action;
use ruphish_domain::events::Kind;
use scenarios::*;

#[tokio::test]
async fn event_rows_are_mutated_deleted_and_persisted_without_audit_tables() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.db");
    let db = support::DatabaseFixture::open(&path).await.unwrap();
    let pool = db.inspection_pool();
    sqlx::raw_sql(SEED).execute(&pool).await.unwrap();
    sqlx::raw_sql("INSERT INTO webhooks(id,name,url,secret,is_active) VALUES(1,'Fixture','http://127.0.0.1:9/','',1); INSERT INTO ruphish_webhook_state(webhook_id,revision,owner_id) VALUES(1,1,1); UPDATE ruphish_settings SET webhooks_enabled=1; INSERT INTO ruphish_webhook_jobs(webhook_id,revision,event_id) VALUES(1,1,101),(1,1,102);").execute(&pool).await.unwrap();
    scenarios::contract(&db).await;
    let row: (String, bool) = sqlx::query_as("SELECT kind,edited FROM events WHERE id=102")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row, ("form_submitted".into(), true));
    let deleted: i64 = sqlx::query_scalar("SELECT count(*) FROM events WHERE id=101")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(deleted, 0);
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM ruphish_webhook_jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(jobs, 0);
    let tables: Vec<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE name LIKE '%event%'")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(!tables.iter().any(|name| name.contains("changes")
        || name.contains("records")
        || name.contains("effective")));
    sqlx::raw_sql("CREATE TRIGGER reject_event_edit BEFORE UPDATE ON events BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").execute(&pool).await.unwrap();
    assert!(
        db.campaigns()
            .change_event(
                OWNER,
                10,
                11,
                change(
                    "rollback",
                    Some(104),
                    0,
                    Action::Edit,
                    Some(Kind::EmailOpened)
                ),
                NOW
            )
            .await
            .is_err()
    );
    let unchanged: (String, bool) = sqlx::query_as("SELECT kind,edited FROM events WHERE id=104")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(unchanged, ("link_clicked".into(), false));
    sqlx::raw_sql("DROP TRIGGER reject_event_edit; INSERT INTO events(id,campaign_id,recipient_id,email,time,kind,metadata,source) VALUES(900,10,11,'alice@example.test','2026-06-03T00:00:00Z','email_opened','{}','public');").execute(&pool).await.unwrap();
    let auto: (bool, i64) = sqlx::query_as("SELECT edited,revision FROM events WHERE id=900")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(auto, (false, 0));
    db.close().await;
    let reopened = support::DatabaseFixture::open(&path).await.unwrap();
    let events = reopened
        .campaigns()
        .recipient_history(OWNER, 10, 11, None, None)
        .await
        .unwrap()
        .events;
    assert!(!events.iter().any(|e| e.id == 101));
    assert!(
        events
            .iter()
            .find(|e| e.id == 102)
            .unwrap()
            .record
            .as_ref()
            .unwrap()
            .edited
    );
    reopened.close().await;
}

#[tokio::test]
async fn event_schema_reopens_with_lf_and_crlf_without_rewriting_history() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("line-endings.db");
    let db = support::DatabaseFixture::open(&path).await.unwrap();
    let pool = db.inspection_pool();
    sqlx::raw_sql(SEED).execute(&pool).await.unwrap();
    let definition: String =
        sqlx::query_scalar("SELECT sql FROM sqlite_master WHERE name='ruphish_event_insert'")
            .fetch_one(&pool)
            .await
            .unwrap();
    let definition = definition.replace("\r\n", "\n");
    assert!(definition.contains('\n'));
    db.close().await;

    for newline in ["\n", "\r\n"] {
        use sqlx::Connection;
        let mut raw = sqlx::SqliteConnection::connect_with(
            &sqlx::sqlite::SqliteConnectOptions::new().filename(&path),
        )
        .await
        .unwrap();
        sqlx::query("DROP TRIGGER ruphish_event_insert")
            .execute(&mut raw)
            .await
            .unwrap();
        let stored = definition.replace('\n', newline);
        sqlx::raw_sql(sqlx::AssertSqlSafe(stored.clone()))
            .execute(&mut raw)
            .await
            .unwrap();
        raw.close().await.unwrap();

        let reopened = support::DatabaseFixture::open(&path).await.unwrap();
        let pool = reopened.inspection_pool();
        let after: String =
            sqlx::query_scalar("SELECT sql FROM sqlite_master WHERE name='ruphish_event_insert'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            after, stored,
            "startup must not rewrite the stored definition"
        );
        let history = reopened
            .campaigns()
            .recipient_history(OWNER, 10, 11, None, None)
            .await
            .unwrap();
        let mut ids = history.events.iter().map(|e| e.id).collect::<Vec<_>>();
        ids.sort_unstable();
        assert_eq!(ids, [101, 102, 103, 104, 105]);
        reopened.close().await;
    }
}

#[tokio::test]
async fn event_retry_index_and_webhook_trigger_drift_are_rejected_and_recoverable() {
    use sqlx::Connection;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("schema.db");
    let db = support::DatabaseFixture::open(&path).await.unwrap();
    db.close().await;
    let mut raw = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(&path),
    )
    .await
    .unwrap();
    for (name, damage) in [
        ("events_create_key", "DROP INDEX events_create_key"),
        (
            "ruphish_event_insert",
            "DROP TRIGGER ruphish_event_insert; CREATE TRIGGER ruphish_event_insert AFTER INSERT ON events BEGIN SELECT 1; END",
        ),
    ] {
        let definition: String = sqlx::query_scalar("SELECT sql FROM sqlite_master WHERE name=?")
            .bind(name)
            .fetch_one(&mut raw)
            .await
            .unwrap();
        sqlx::raw_sql(damage).execute(&mut raw).await.unwrap();
        assert!(support::DatabaseFixture::open(&path).await.is_err());
        if name == "ruphish_event_insert" {
            sqlx::query("DROP TRIGGER ruphish_event_insert")
                .execute(&mut raw)
                .await
                .unwrap();
        }
        sqlx::raw_sql(sqlx::AssertSqlSafe(definition))
            .execute(&mut raw)
            .await
            .unwrap();
        let recovered = support::DatabaseFixture::open(&path).await.unwrap();
        recovered.close().await;
    }
    raw.close().await.unwrap();
}
