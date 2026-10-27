//! Real-provider regression scenarios; driver access is only setup/faults/assertions.
use super::*;
use ruphish_application::{delivery::MessageInput, storage};
use std::time::Duration;

async fn exact_text(db: &Database) {
    let (actor, mut original) = replacement_fixture(db).await;
    for subject in [
        "a".repeat(255),
        "é".repeat(255),
        "🦀".repeat(255),
        format!("{} ", "x".repeat(254)),
    ] {
        let mut edit = original.clone();
        let Content::Template(template) = &mut edit.content else {
            panic!()
        };
        template.subject = subject.clone();
        let saved = db
            .resources()
            .save(
                actor,
                edit.into(),
                Some(original.id),
                Some(original.modified_date.clone()),
            )
            .await
            .unwrap();
        let read = db
            .resources()
            .get(actor, Kind::Template, original.id)
            .await
            .unwrap();
        assert!(saved == read, "save response must equal persisted data");
        let Content::Template(template) = &read.content else {
            panic!()
        };
        assert_eq!(template.subject, subject);
        original = saved;
    }
    for subject in [
        "a".repeat(256),
        "é".repeat(256),
        "🦀".repeat(256),
        format!("{} ", "x".repeat(255)),
        format!("{}  ", "é".repeat(255)),
    ] {
        let mut edit = original.clone();
        edit.name = "Must not replace the existing name".into();
        let Content::Template(template) = &mut edit.content else {
            panic!()
        };
        template.subject = subject;
        template.attachments.clear();
        assert!(
            matches!(db.resources().save(actor, edit.clone().into(), Some(original.id), Some(original.modified_date.clone())).await,
            Err(Error::Invalid(code)) if code.starts_with("storage_text_invalid:"))
        );
        assert!(
            db.resources()
                .get(actor, Kind::Template, original.id)
                .await
                .unwrap()
                == original,
            "rejected replacement must preserve body, attachments and revision"
        );
        assert!(matches!(
            db.resources().save(actor, edit.into(), None, None).await,
            Err(Error::Invalid(_))
        ));
        assert_eq!(
            db.resources()
                .list(actor, Kind::Template)
                .await
                .unwrap()
                .len(),
            1
        );
    }
}

#[tokio::test]
#[ignore = "Requires canonical MySQL fixture and RUPHISH_MYSQL_TEST_PORT"]
async fn mysql_non_strict_server_preserves_exact_text_or_rejects_atomically() {
    let (mut inspector, mut config) = fixture().await;
    config.max_connections = 1;
    let mode: String = sqlx::query_scalar("SELECT @@GLOBAL.sql_mode")
        .fetch_one(&mut inspector)
        .await
        .unwrap();
    sqlx::query("SET GLOBAL sql_mode=''")
        .execute(&mut inspector)
        .await
        .unwrap();
    let opened = database::open(Config::MySql(config.clone()), Policy::default()).await;
    // Restore before asserting so a failed startup cannot contaminate other fixtures.
    sqlx::query("SET GLOBAL sql_mode=?")
        .bind(mode)
        .execute(&mut inspector)
        .await
        .unwrap();
    let db = opened.unwrap();
    exact_text(&db).await;
    // A field without resource validation proves session strictness itself, rather
    // than letting the Rust length check mask an unsafe MySQL session.
    let admin = db.accounts().by_name("admin").await.unwrap();
    let result = db
        .accounts()
        .write(
            admin.identity,
            auth::Write::Save {
                target: None,
                hash: Some("x".repeat(256)),
                edit: auth::Edit {
                    username: "strict-mode-probe".into(),
                    password: String::new(),
                    role: "user".into(),
                    password_change_required: false,
                    account_locked: false,
                },
            },
        )
        .await;
    assert!(
        result.is_err(),
        "non-strict server defaults must not permit data truncation"
    );
    assert!(matches!(
        db.accounts().by_name("strict-mode-probe").await,
        Err(auth::Error::NotFound)
    ));
    db.close().await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE `{}`",
        config.database
    )))
    .execute(&mut inspector)
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "Requires canonical PostgreSQL fixture and RUPHISH_POSTGRES_TEST_PORT"]
async fn postgres_character_boundaries_preserve_exact_text_or_reject_atomically() {
    let (mut admin, config) = postgres_fixture().await;
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    exact_text(&db).await;
    db.close().await;
    postgres_cleanup(&mut admin, &config).await;
}

async fn busy_operations(db: &Database, actor: auth::Identity, original: &Resource) {
    let result = db
        .resources()
        .save(
            actor,
            original.clone().into(),
            Some(original.id),
            Some(original.modified_date.clone()),
        )
        .await;
    assert!(matches!(result, Err(Error::Storage(storage::Error::Busy))));
    assert!(matches!(
        db.accounts().write(actor, auth::Write::RotateKey).await,
        Err(auth::Error::Storage(storage::Error::Busy))
    ));
    assert!(matches!(
        db.domains().save(actor, None, domain_edit(), 100).await,
        Err(Error::Storage(storage::Error::Busy))
    ));
    let message = MessageInput {
        message_id: None,
        recipient: Recipient::default(),
        template: Template::default(),
        template_id: original.id,
        page_id: 0,
        profile: Profile::default(),
        url: String::new(),
        rid: "busy-preview".into(),
    };
    assert!(matches!(
        db.previews().store(actor, &message).await,
        Err(Error::Storage(storage::Error::Busy))
    ));
}

#[tokio::test]
#[ignore = "Requires canonical MySQL fixture and RUPHISH_MYSQL_TEST_PORT"]
async fn mysql_facade_retains_busy_across_capabilities() {
    let (mut inspector, mut config) = fixture().await;
    config.lock_timeout_seconds = 1;
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    let (actor, original) = replacement_fixture(&db).await;
    let mut held = inspector.begin().await.unwrap();
    sqlx::query("SELECT id FROM users WHERE id=? FOR UPDATE")
        .bind(actor.id)
        .fetch_one(&mut *held)
        .await
        .unwrap();
    busy_operations(&db, actor, &original).await;
    held.rollback().await.unwrap();
    assert!(
        db.resources()
            .get(actor, Kind::Template, original.id)
            .await
            .unwrap()
            == original
    );
    assert!(db.domains().list(actor).await.unwrap().is_empty());
    assert!(
        db.accounts()
            .write(actor, auth::Write::RotateKey)
            .await
            .is_ok()
    );
    db.close().await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE `{}`",
        config.database
    )))
    .execute(&mut inspector)
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "Requires canonical PostgreSQL fixture and RUPHISH_POSTGRES_TEST_PORT"]
async fn postgres_facade_retains_busy_across_capabilities() {
    let (mut admin, mut config) = postgres_fixture().await;
    config.lock_timeout_seconds = 1;
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    let (actor, original) = replacement_fixture(&db).await;
    let mut inspector = postgres_inspect(&config).await;
    let mut held = inspector.begin().await.unwrap();
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(actor.id)
        .fetch_one(&mut *held)
        .await
        .unwrap();
    busy_operations(&db, actor, &original).await;
    held.rollback().await.unwrap();
    assert!(
        db.resources()
            .get(actor, Kind::Template, original.id)
            .await
            .unwrap()
            == original
    );
    assert!(db.domains().list(actor).await.unwrap().is_empty());
    assert!(
        db.accounts()
            .write(actor, auth::Write::RotateKey)
            .await
            .is_ok()
    );
    inspector.close().await.unwrap();
    db.close().await;
    postgres_cleanup(&mut admin, &config).await;
}

#[tokio::test]
async fn sqlite_facade_retains_busy_across_capabilities() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("busy.db");
    let db = database::open(
        Config::Sqlite {
            path: path.clone(),
            migrations_prefix: String::new(),
        },
        Policy::default(),
    )
    .await
    .unwrap();
    let (actor, original) = replacement_fixture(&db).await;
    let mut inspector = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(&path),
    )
    .await
    .unwrap();
    let held = inspector.begin_with("BEGIN IMMEDIATE").await.unwrap();
    busy_operations(&db, actor, &original).await;
    held.rollback().await.unwrap();
    assert!(
        db.resources()
            .get(actor, Kind::Template, original.id)
            .await
            .unwrap()
            == original
    );
    assert!(db.domains().list(actor).await.unwrap().is_empty());
    assert!(
        db.accounts()
            .write(actor, auth::Write::RotateKey)
            .await
            .is_ok()
    );
    inspector.close().await.unwrap();
    db.close().await;
}

fn domain_edit() -> domains::Edit {
    domains::Edit {
        name: "Reusable domain".into(),
        origin: "https://reusable.example.test".into(),
        enabled: true,
        revision: None,
    }
}
fn passed() -> domains::Observation {
    domains::Observation {
        control: true,
        dns: true,
        routing: true,
        tls: true,
        code: "verified",
    }
}
async fn deletion_fixture(db: &Database) -> (auth::Identity, auth::Identity, domains::Check) {
    let (admin, _) = replacement_fixture(db).await;
    let settings = db.settings().get(admin).await.unwrap();
    let mut policy = settings.policy;
    policy.verification_enabled = true;
    db.settings()
        .save(admin, settings.revision, policy, 100)
        .await
        .unwrap();

    let target = db
        .accounts()
        .write(
            admin,
            auth::Write::Save {
                target: None,
                hash: Some("fixture-only".into()),
                edit: auth::Edit {
                    username: "departing".into(),
                    password: String::new(),
                    role: "user".into(),
                    password_change_required: false,
                    account_locked: false,
                },
            },
        )
        .await
        .unwrap()
        .unwrap()
        .identity;
    db.domains()
        .save(target, None, domain_edit(), 100)
        .await
        .unwrap();
    let check = db.domains().claim(100).await.unwrap().unwrap();
    (admin, target, check)
}
async fn verify_reuse(db: &Database, admin: auth::Identity) {
    let domain = db
        .domains()
        .save(admin, None, domain_edit(), 110)
        .await
        .unwrap();
    let check = db.domains().claim(110).await.unwrap().unwrap();
    db.domains().finish(&check, passed(), 111).await.unwrap();
    let domain = db.domains().get(admin, domain.id).await.unwrap();
    assert_eq!(domain.code, "verified");
    assert!(domain.verified_until > 111);
}

#[tokio::test]
#[ignore = "Requires canonical PostgreSQL fixture and RUPHISH_POSTGRES_TEST_PORT"]
async fn postgres_account_deletion_fences_inflight_domain_verification() {
    let (mut admin_connection, mut config) = postgres_fixture().await;
    config.lock_timeout_seconds = 15;
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    let (admin, target, check) = deletion_fixture(&db).await;
    let mut inspector = postgres_inspect(&config).await;
    // Pause the actual account-delete operation after claims cleanup but before
    // domain deletion. Verification updates do not execute this DELETE trigger.
    sqlx::raw_sql("CREATE FUNCTION pause_delete() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock(987654321); RETURN OLD; END $$; CREATE TRIGGER pause_delete BEFORE DELETE ON ruphish_domain_checks FOR EACH ROW EXECUTE FUNCTION pause_delete()")
        .execute(&mut inspector).await.unwrap();
    sqlx::query("SELECT pg_advisory_lock(987654321)")
        .execute(&mut inspector)
        .await
        .unwrap();
    let accounts = db.accounts();
    let deleting =
        tokio::spawn(async move { accounts.write(admin, auth::Write::Delete(target.id)).await });
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let waiting: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND NOT granted AND objid=987654321 AND database=(SELECT oid FROM pg_database WHERE datname=current_database()))").fetch_one(&mut inspector).await.unwrap();
            if waiting { break; }
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    assert!(
        db.domains().claim(200).await.unwrap().is_none(),
        "claim must skip a deleting owner"
    );
    let domains = db.domains();
    let finishing = tokio::spawn(async move { domains.finish(&check, passed(), 101).await });
    // Both operations must be blocked now: deletion on the test barrier,
    // verification on the live owner's row. No timing-based success assumption.
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            assert!(!finishing.is_finished(), "verification bypassed account deletion");
            let waiters: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND application_name='ruphish' AND wait_event_type='Lock'").fetch_one(&mut inspector).await.unwrap();
            if waiters >= 2 { break; }
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    sqlx::query("SELECT pg_advisory_unlock(987654321)")
        .execute(&mut inspector)
        .await
        .unwrap();
    deleting.await.unwrap().unwrap();
    finishing.await.unwrap().unwrap();
    let orphan: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ruphish_domain_claims WHERE owner_id=$1")
            .bind(target.id)
            .fetch_one(&mut inspector)
            .await
            .unwrap();
    assert_eq!(orphan, 0);
    verify_reuse(&db, admin).await;
    inspector.close().await.unwrap();
    db.close().await;
    postgres_cleanup(&mut admin_connection, &config).await;
}

#[tokio::test]
#[ignore = "Requires canonical MySQL fixture and RUPHISH_MYSQL_TEST_PORT"]
async fn mysql_account_deletion_fences_inflight_domain_verification() {
    let (mut inspector, mut config) = fixture().await;
    config.lock_timeout_seconds = 15;
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    let (admin, target, check) = deletion_fixture(&db).await;
    let barrier = format!("delete_{}", config.database);
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE TRIGGER pause_delete BEFORE DELETE ON ruphish_domain_checks FOR EACH ROW BEGIN IF GET_LOCK('{barrier}',15)=1 THEN DO RELEASE_LOCK('{barrier}'); END IF; END")))
        .execute(&mut inspector).await.unwrap();
    sqlx::query("SELECT GET_LOCK(?,15)")
        .bind(&barrier)
        .execute(&mut inspector)
        .await
        .unwrap();
    let accounts = db.accounts();
    let deleting =
        tokio::spawn(async move { accounts.write(admin, auth::Write::Delete(target.id)).await });
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let waiting: i64 = sqlx::query_scalar("SELECT count(*) FROM performance_schema.metadata_locks WHERE OBJECT_TYPE='USER LEVEL LOCK' AND OBJECT_NAME=? AND LOCK_STATUS='PENDING'").bind(&barrier).fetch_one(&mut inspector).await.unwrap();
            if waiting > 0 { break; }
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    assert!(
        db.domains().claim(200).await.unwrap().is_none(),
        "claim must skip a deleting owner"
    );
    let domains = db.domains();
    let finishing = tokio::spawn(async move { domains.finish(&check, passed(), 101).await });
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            assert!(!finishing.is_finished(), "verification bypassed account deletion");
            let waiting: i64 = sqlx::query_scalar("SELECT count(*) FROM performance_schema.data_lock_waits w JOIN performance_schema.data_locks l ON l.ENGINE_LOCK_ID=w.REQUESTING_ENGINE_LOCK_ID AND l.ENGINE=w.ENGINE WHERE l.OBJECT_SCHEMA=DATABASE() AND l.OBJECT_NAME='users'").fetch_one(&mut inspector).await.unwrap();
            if waiting > 0 { break; }
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    sqlx::query("SELECT RELEASE_LOCK(?)")
        .bind(&barrier)
        .execute(&mut inspector)
        .await
        .unwrap();
    deleting.await.unwrap().unwrap();
    finishing.await.unwrap().unwrap();
    let orphan: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ruphish_domain_claims WHERE owner_id=?")
            .bind(target.id)
            .fetch_one(&mut inspector)
            .await
            .unwrap();
    assert_eq!(orphan, 0);
    verify_reuse(&db, admin).await;
    db.close().await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE `{}`",
        config.database
    )))
    .execute(&mut inspector)
    .await
    .unwrap();
}
