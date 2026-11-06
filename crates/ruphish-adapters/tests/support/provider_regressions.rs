//! Real-provider regression scenarios; driver access is only setup/faults/assertions.
use super::*;
use ruphish_application::{delivery::MessageInput, storage};

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
                Some(original.revision.to_string()),
            )
            .await
            .unwrap();
        let read = db
            .resources()
            .get(actor, Kind::Template, original.id)
            .await
            .unwrap();
        assert!(
            saved.record == read,
            "save response must equal persisted data"
        );
        let Content::Template(template) = &read.content else {
            panic!()
        };
        assert_eq!(template.subject, subject);
        original = saved.record;
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
            matches!(db.resources().save(actor, edit.clone().into(), Some(original.id), Some(original.revision.to_string())).await,
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
            Some(original.revision.to_string()),
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
        track_opens: true,
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
#[ignore = "Requires canonical MySQL fixture and RUPHISH_MYSQL_TEST_PORT"]
async fn mysql_cancelled_work_releases_pool_capacity_before_external_locks() {
    let (mut inspector, mut config) = fixture().await;
    config.max_connections = 1;
    config.acquire_timeout_seconds = 2;
    config.lock_timeout_seconds = 60;
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
    let resources = db.resources();
    let mut edit = original.clone();
    edit.name = "Cancelled replacement".into();
    let pending = tokio::spawn(async move {
        resources
            .save(
                actor,
                edit.clone().into(),
                Some(edit.id),
                Some(edit.revision.to_string()),
            )
            .await
    });
    // Observe the actual server lock wait before cancelling; scheduling or a
    // sleep cannot establish that the operation acquired the pool connection.
    loop {
        assert!(
            !pending.is_finished(),
            "write ended before the lock barrier"
        );
        let waiting: i64 = sqlx::query_scalar("SELECT count(*) FROM performance_schema.data_lock_waits w JOIN performance_schema.data_locks l ON l.ENGINE_LOCK_ID=w.REQUESTING_ENGINE_LOCK_ID AND l.ENGINE=w.ENGINE WHERE l.OBJECT_SCHEMA=?")
            .bind(&config.database)
            .fetch_one(&mut *held)
            .await
            .unwrap();
        if waiting > 0 {
            break;
        }
        tokio::task::yield_now().await;
    }
    pending.abort();
    assert!(matches!(pending.await, Err(error) if error.is_cancelled()));
    // The external lock stays held. A replacement connection can still read
    // committed state; cancellation must not strand the sole pool permit.
    let recovered = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            if db.health().check().await.is_ok() {
                break;
            }
        }
    })
    .await;
    held.rollback().await.unwrap();
    let unchanged = db.resources().get(actor, Kind::Template, original.id).await;
    let closed = tokio::time::timeout(std::time::Duration::from_secs(10), db.close()).await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE `{}`",
        config.database
    )))
    .execute(&mut inspector)
    .await
    .unwrap();
    recovered.expect("cancelled MySQL work retained pool capacity behind an external lock");
    assert!(unchanged.unwrap() == original);
    closed.expect("cancelled MySQL work prevented pool shutdown");
}

#[tokio::test]
async fn sqlite_facade_retains_busy_across_capabilities() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("busy.db");
    let db = database::open(Config::Sqlite { path: path.clone() }, Policy::default())
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
async fn verify_retained_domain(db: &Database, admin: auth::Identity, check: &domains::Check) {
    let domain = db.domains().get(admin, check.domain.id).await.unwrap();
    assert_eq!(domain.code, "verified");
    assert!(domain.verified_until > 111);
    assert_eq!(domain.attribution, check.domain.attribution);
    assert!(
        db.domains()
            .save(admin, None, domain_edit(), 110)
            .await
            .is_err(),
        "preserved origin cannot be registered twice"
    );
}

#[tokio::test]
#[ignore = "Requires canonical PostgreSQL fixture and RUPHISH_POSTGRES_TEST_PORT"]
async fn postgres_account_deletion_preserves_inflight_domain_verification() {
    let (mut admin_connection, mut config) = postgres_fixture().await;
    config.lock_timeout_seconds = 15;
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    let (admin, target, check) = deletion_fixture(&db).await;
    let mut inspector = postgres_inspect(&config).await;
    // Pause deletion at the account row. Domain verification must complete
    // independently while the account transaction is still in flight.
    sqlx::raw_sql("CREATE FUNCTION pause_delete() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock(987654321); RETURN OLD; END $$; CREATE TRIGGER pause_delete BEFORE DELETE ON users FOR EACH ROW EXECUTE FUNCTION pause_delete()")
        .execute(&mut inspector).await.unwrap();
    sqlx::query("SELECT pg_advisory_lock(987654321)")
        .execute(&mut inspector)
        .await
        .unwrap();
    let accounts = db.accounts();
    let deleting =
        tokio::spawn(async move { accounts.write(admin, auth::Write::Delete(target.id)).await });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        assert!(
            !deleting.is_finished(),
            "deletion ended before reaching the test barrier"
        );
        assert!(
            std::time::Instant::now() < deadline,
            "deletion did not reach the test barrier"
        );
        let waiting: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND NOT granted AND objid=987654321 AND database=(SELECT oid FROM pg_database WHERE datname=current_database()))").fetch_one(&mut inspector).await.unwrap();
        if waiting {
            break;
        }
        tokio::task::yield_now().await;
    }
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        db.domains().finish(&check, passed(), 101),
    )
    .await
    .expect("domain completion must not wait for account deletion")
    .unwrap();
    let resources = db.resources();
    let writing = tokio::spawn(async move {
        resources
            .save(
                target,
                resource("Revoked write", Content::Template(Template::default())),
                None,
                None,
            )
            .await
    });
    sqlx::query("SELECT pg_advisory_unlock(987654321)")
        .execute(&mut inspector)
        .await
        .unwrap();
    deleting.await.unwrap().unwrap();
    assert!(matches!(writing.await.unwrap(), Err(Error::Policy(_))));
    assert!(
        db.resources()
            .get_by_name(admin, Kind::Template, "Revoked write")
            .await
            .is_err()
    );
    verify_retained_domain(&db, admin, &check).await;
    inspector.close().await.unwrap();
    db.close().await;
    postgres_cleanup(&mut admin_connection, &config).await;
}

#[tokio::test]
#[ignore = "Requires canonical MySQL fixture and RUPHISH_MYSQL_TEST_PORT"]
async fn mysql_account_deletion_preserves_inflight_domain_verification() {
    let (mut inspector, mut config) = fixture().await;
    config.lock_timeout_seconds = 15;
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    let (admin, target, check) = deletion_fixture(&db).await;
    let barrier = format!("delete_{}", config.database);
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE TRIGGER pause_delete BEFORE DELETE ON users FOR EACH ROW BEGIN IF GET_LOCK('{barrier}',15)=1 THEN DO RELEASE_LOCK('{barrier}'); END IF; END")))
        .execute(&mut inspector).await.unwrap();
    sqlx::query("SELECT GET_LOCK(?,15)")
        .bind(&barrier)
        .execute(&mut inspector)
        .await
        .unwrap();
    let accounts = db.accounts();
    let deleting =
        tokio::spawn(async move { accounts.write(admin, auth::Write::Delete(target.id)).await });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        assert!(
            !deleting.is_finished(),
            "deletion ended before reaching the test barrier"
        );
        assert!(
            std::time::Instant::now() < deadline,
            "deletion did not reach the test barrier"
        );
        let waiting: i64 = sqlx::query_scalar("SELECT count(*) FROM performance_schema.metadata_locks WHERE OBJECT_TYPE='USER LEVEL LOCK' AND OBJECT_NAME=? AND LOCK_STATUS='PENDING'").bind(&barrier).fetch_one(&mut inspector).await.unwrap();
        if waiting > 0 {
            break;
        }
        tokio::task::yield_now().await;
    }
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        db.domains().finish(&check, passed(), 101),
    )
    .await
    .expect("domain completion must not wait for account deletion")
    .unwrap();
    let resources = db.resources();
    let writing = tokio::spawn(async move {
        resources
            .save(
                target,
                resource("Revoked write", Content::Template(Template::default())),
                None,
                None,
            )
            .await
    });
    sqlx::query("SELECT RELEASE_LOCK(?)")
        .bind(&barrier)
        .execute(&mut inspector)
        .await
        .unwrap();
    deleting.await.unwrap().unwrap();
    assert!(matches!(writing.await.unwrap(), Err(Error::Policy(_))));
    assert!(
        db.resources()
            .get_by_name(admin, Kind::Template, "Revoked write")
            .await
            .is_err()
    );
    verify_retained_domain(&db, admin, &check).await;
    db.close().await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE `{}`",
        config.database
    )))
    .execute(&mut inspector)
    .await
    .unwrap();
}
