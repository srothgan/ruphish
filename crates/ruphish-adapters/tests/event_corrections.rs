//! Corrections preserve original evidence, serialize writers and survive reopen.
#![allow(
    clippy::unwrap_used,
    reason = "Isolated fixtures and independent assertions"
)]
#[path = "support/event_corrections.rs"]
mod scenarios;
mod support;
use ruphish_application::event_edit::Action;
use ruphish_domain::events::Kind;
use scenarios::*;

#[tokio::test]
async fn account_deletion_cleans_owned_corrections_atomically_and_preserves_other_owners() {
    let dir = tempfile::tempdir().unwrap();
    let db = support::DatabaseFixture::open(&dir.path().join("delete.db"), "")
        .await
        .unwrap();
    let pool = db.inspection_pool();
    sqlx::raw_sql(SEED).execute(&pool).await.unwrap();
    let other = ruphish_application::auth::Identity { id: 2, epoch: 0 };
    for (owner, campaign, recipient) in [(OWNER, 10, 11), (other, 20, 21)] {
        db.campaigns()
            .change_event(
                owner,
                owner,
                campaign,
                recipient,
                change(
                    "owned-addition",
                    None,
                    0,
                    Action::Add,
                    Some(Kind::LinkClicked),
                ),
                NOW,
            )
            .await
            .unwrap();
    }
    sqlx::raw_sql("CREATE TRIGGER fail_delete BEFORE DELETE ON users WHEN OLD.id=2 BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").execute(&pool).await.unwrap();
    assert!(
        db.accounts()
            .write(OWNER, ruphish_application::auth::Write::Delete(2))
            .await
            .is_err()
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM ruphish_event_changes")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
    sqlx::raw_sql("DROP TRIGGER fail_delete;")
        .execute(&pool)
        .await
        .unwrap();
    db.accounts()
        .write(OWNER, ruphish_application::auth::Write::Delete(2))
        .await
        .unwrap();
    let owners: Vec<i64> = sqlx::query_scalar("SELECT owner_id FROM ruphish_event_changes")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(owners, [1]);
    db.close().await;
}

#[tokio::test]
async fn audit_pages_are_bounded_without_losing_original_or_current_state() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.db");
    let db = support::DatabaseFixture::open(&path, "").await.unwrap();
    sqlx::raw_sql(SEED)
        .execute(&db.inspection_pool())
        .await
        .unwrap();
    let repo = db.campaigns();
    for revision in 0..52 {
        repo.change_event(
            OWNER,
            OWNER,
            10,
            11,
            change(
                &format!("audit-{revision}"),
                Some(101),
                revision,
                if revision % 2 == 0 {
                    Action::Exclude
                } else {
                    Action::Restore
                },
                None,
            ),
            NOW,
        )
        .await
        .unwrap();
    }
    let first = repo.inspect_event(OWNER, 10, 11, 101, None).await.unwrap();
    assert_eq!(first.audit.len(), 50);
    assert_eq!(first.audit.first().unwrap().revision, 52);
    assert_eq!(first.next_before, Some(3));
    let second = repo
        .inspect_event(OWNER, 10, 11, 101, first.next_before)
        .await
        .unwrap();
    assert_eq!(
        second.audit.iter().map(|a| a.revision).collect::<Vec<_>>(),
        [2, 1]
    );
    assert_eq!(second.record.revision, 52);
    assert_eq!(second.record.original, second.record.current);
    assert!(!second.record.excluded);
    assert_eq!(second.next_before, None);
    db.close().await;
    // Startup must reject a view that silently restores excluded observations.
    use sqlx::Connection;
    let mut c = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(&path),
    )
    .await
    .unwrap();
    sqlx::raw_sql("DROP VIEW effective_event_history; CREATE VIEW effective_event_history AS SELECT * FROM event_history;")
        .execute(&mut c).await.unwrap();
    c.close().await.unwrap();
    assert!(support::DatabaseFixture::open(&path, "").await.is_err());
}

#[tokio::test]
async fn populated_previous_schema_upgrades_with_recoverable_original_and_reopens() {
    use sqlx::Connection;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("previous.db");
    let db = support::DatabaseFixture::open(&path, "").await.unwrap();
    sqlx::raw_sql(SEED)
        .execute(&db.inspection_pool())
        .await
        .unwrap();
    db.close().await;
    let mut c = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(&path),
    )
    .await
    .unwrap();
    sqlx::raw_sql("DROP VIEW effective_event_history; DROP VIEW ruphish_event_records; DROP TABLE ruphish_event_changes; DROP TRIGGER ruphish_event_insert; DELETE FROM ruphish_schema WHERE version=14;").execute(&mut c).await.unwrap();
    let prior = include_str!("../migrations/sqlite/12_settings.sql");
    let trigger = prior
        .split_once("CREATE TRIGGER ruphish_event_insert")
        .unwrap()
        .1
        .split_once("INSERT INTO ruphish_schema")
        .unwrap()
        .0;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "CREATE TRIGGER ruphish_event_insert{trigger}"
    )))
    .execute(&mut c)
    .await
    .unwrap();
    c.close().await.unwrap();
    let db = support::DatabaseFixture::open(&path, "").await.unwrap();
    let backup = db.backup_path().unwrap().to_owned();
    let mut original = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(backup)
            .read_only(true),
    )
    .await
    .unwrap();
    let version: i64 = sqlx::query_scalar("SELECT max(version) FROM ruphish_schema")
        .fetch_one(&mut original)
        .await
        .unwrap();
    assert_eq!(version, 13);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM events")
        .fetch_one(&mut original)
        .await
        .unwrap();
    assert_eq!(count, 5);
    original.close().await.unwrap();
    scenarios::contract(&db).await;
    db.close().await;
    let db = support::DatabaseFixture::open(&path, "").await.unwrap();
    assert_eq!(
        db.campaigns()
            .inspect_event(OWNER, 10, 11, 102, None)
            .await
            .unwrap()
            .record
            .original
            .time,
        "2026-06-01T00:00:01Z"
    );
    db.close().await;
}

#[tokio::test]
async fn corrections_are_individual_audited_replayable_and_persistent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("events.db");
    let db = support::DatabaseFixture::open(&path, "").await.unwrap();
    let pool = db.inspection_pool();
    sqlx::raw_sql(SEED).execute(&pool).await.unwrap();
    sqlx::raw_sql("INSERT INTO webhooks(id,name,url,secret,is_active) VALUES(1,'Fixture','http://127.0.0.1:9/','',1); INSERT INTO ruphish_webhook_state VALUES(1,1); UPDATE ruphish_settings SET webhooks_enabled=1;").execute(&pool).await.unwrap();
    scenarios::contract(&db).await;
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM ruphish_webhook_jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        jobs, 0,
        "Administrative changes never enqueue participant-event webhooks"
    );
    // Original evidence was never rewritten or deleted.
    let original: (String, String) = sqlx::query_as("SELECT time,kind FROM events WHERE id=102")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        original,
        ("2026-06-01T00:00:01Z".into(), "link_clicked".into())
    );
    // Failure after inserting an operator event rolls back the event and audit together.
    sqlx::raw_sql("CREATE TRIGGER reject_event_audit BEFORE INSERT ON ruphish_event_changes BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").execute(&pool).await.unwrap();
    assert!(
        db.campaigns()
            .change_event(
                OWNER,
                OWNER,
                10,
                11,
                change("rollback", None, 0, Action::Add, Some(Kind::EmailOpened)),
                NOW
            )
            .await
            .is_err()
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 6);
    sqlx::raw_sql("DROP TRIGGER reject_event_audit;")
        .execute(&pool)
        .await
        .unwrap();
    // Simulate a later real observation: no recipient-level override suppresses it.
    sqlx::raw_sql("INSERT INTO events(campaign_id,recipient_id,email,time,kind,metadata,source) VALUES(10,11,'alice@example.test','2026-06-03T00:00:00Z','link_clicked','{}','public');").execute(&pool).await.unwrap();
    assert_eq!(
        db.reporting()
            .analysis(
                OWNER,
                10,
                ruphish_application::reporting::Filter {
                    limit: 100,
                    ..Default::default()
                }
            )
            .await
            .unwrap()
            .totals[2],
        1
    );
    let jobs: i64 = sqlx::query_scalar("SELECT count(*) FROM ruphish_webhook_jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        jobs, 1,
        "A later system observation still enqueues its webhook"
    );
    let one = change("race-one", Some(102), 1, Action::Restore, None);
    let two = change("race-two", Some(102), 1, Action::Exclude, None);
    let repo = db.campaigns();
    let (a, b) = tokio::join!(
        repo.change_event(OWNER, OWNER, 10, 11, one, NOW),
        repo.change_event(OWNER, OWNER, 10, 11, two, NOW)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    db.close().await;
    let reopened = support::DatabaseFixture::open(&path, "").await.unwrap();
    let record = reopened
        .campaigns()
        .inspect_event(OWNER, 10, 11, 101, None)
        .await
        .unwrap();
    assert!(record.record.excluded);
    assert_eq!(record.audit.len(), 3);
    reopened.close().await;
}
