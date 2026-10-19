//! Transactional account workflows against real SQLite, including failure injection.
#![allow(clippy::unwrap_used, reason = "Integration setup and assertions")]
use ruphish_adapters::{
    password_worker::{self, PasswordWorker},
    sqlite::{StartupDatabase, accounts::Accounts},
};
use ruphish_application::auth::{Edit, Error, Repository, Service};
use std::{path::Path, sync::Arc};

#[tokio::test]
async fn imported_accounts_lookups_roles_and_password_failure_are_atomic() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("accounts.db");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/authentication/legacy-accounts.sqlite3"),
        &path,
    )
    .unwrap();
    let db = StartupDatabase::open(&path, "").await.unwrap();
    let pool = db.pool();
    let repo = Arc::new(Accounts::new(pool.clone()));
    let service = Service::new(repo.clone(), Arc::new(PasswordWorker::default()));
    let original = repo.by_id(1).await.unwrap();
    assert_eq!(
        repo.by_name("admin").await.unwrap().identity,
        original.identity
    );
    assert_eq!(
        repo.by_key(&original.api_key).await.unwrap().identity,
        original.identity
    );
    assert!(matches!(repo.by_id(999).await, Err(Error::NotFound)));
    assert!(matches!(repo.by_name("absent").await, Err(Error::NotFound)));
    assert!(matches!(
        repo.by_key("invalid").await,
        Err(Error::InvalidCredentials)
    ));
    for permission in ["view_objects", "modify_objects", "modify_system"] {
        assert!(original.permits(permission));
    }
    assert!(!original.permits("invented-permission"));
    let created = service
        .save(
            original.identity,
            None,
            Edit {
                username: "ordinary".into(),
                password: "ordinary-password".into(),
                role: "user".into(),
                password_change_required: false,
                account_locked: false,
            },
        )
        .await
        .unwrap();
    assert_eq!(created.role.slug, "user");
    assert!(created.permits("view_objects"));
    assert!(created.permits("modify_objects"));
    assert!(!created.permits("modify_system"));
    assert_eq!(created.api_key.len(), 64);
    assert_ne!(created.api_key, original.api_key);
    assert!(created.hash.starts_with("$argon2id$"));
    assert!(original.hash.starts_with("$2"));
    assert!(matches!(
        service
            .save(
                original.identity,
                None,
                Edit {
                    username: "unsupported".into(),
                    password: "ordinary-password".into(),
                    role: "unknown".into(),
                    password_change_required: false,
                    account_locked: false
                }
            )
            .await,
        Err(Error::RoleNotFound)
    ));
    sqlx::raw_sql("CREATE TRIGGER fail_generation BEFORE INSERT ON ruphish_auth_state BEGIN SELECT RAISE(ABORT,'synthetic failure after password update'); END;").execute(&pool).await.unwrap();
    assert!(matches!(
        service
            .change_password(
                original.identity,
                Some("FixtureOnlyPassword42!".into()),
                "new-password".into(),
                "new-password"
            )
            .await,
        Err(Error::Internal)
    ));
    let unchanged = repo.by_id(1).await.unwrap();
    assert_eq!(unchanged.hash, original.hash);
    assert_eq!(unchanged.identity, original.identity);
    assert!(service.session(original.identity).await.is_ok());
    sqlx::raw_sql("DROP TRIGGER fail_generation;")
        .execute(&pool)
        .await
        .unwrap();
    let changed = service
        .change_password(
            original.identity,
            Some("FixtureOnlyPassword42!".into()),
            "new-password".into(),
            "new-password",
        )
        .await
        .unwrap();
    assert!(changed.hash.starts_with("$argon2id$"));
    assert!(password_worker::verify("new-password", &changed.hash));
    assert!(service.session(original.identity).await.is_err());
    assert_eq!(
        repo.by_id(created.identity.id).await.unwrap().hash,
        created.hash
    );
    // Both requests begin with the same valid identity and password. Only one
    // may commit; the loser must not overwrite the winning password or epoch.
    let (first, second) = tokio::join!(
        service.change_password(
            changed.identity,
            Some("new-password".into()),
            "concurrent-password-one".into(),
            "concurrent-password-one"
        ),
        service.change_password(
            changed.identity,
            Some("new-password".into()),
            "concurrent-password-two".into(),
            "concurrent-password-two"
        )
    );
    assert_ne!(first.is_ok(), second.is_ok());
    let (winner, accepted, rejected) = match (first, second) {
        (Ok(winner), Err(_)) => (winner, "concurrent-password-one", "concurrent-password-two"),
        (Err(_), Ok(winner)) => (winner, "concurrent-password-two", "concurrent-password-one"),
        _ => unreachable!("Exactly one password transaction must succeed"),
    };
    assert!(password_worker::verify(accepted, &winner.hash));
    assert!(!password_worker::verify(rejected, &winner.hash));
    assert_eq!(winner.identity.epoch, changed.identity.epoch + 1);
    assert_eq!(repo.by_id(1).await.unwrap().hash, winner.hash);
    db.close().await;
    let reopened = StartupDatabase::open(&path, "").await.unwrap();
    assert!(reopened.backup_path().is_none());
    let repo = Accounts::new(reopened.pool());
    assert_eq!(repo.by_id(1).await.unwrap().hash, winner.hash);
    reopened.close().await;
}

#[tokio::test]
async fn deletion_rolls_back_owned_records_then_preserves_other_owners_and_shared_targets() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("accounts.db");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/compatibility/20220321133237.sqlite3"),
        &path,
    )
    .unwrap();
    let db = StartupDatabase::open(&path, "").await.unwrap();
    let pool = db.pool();
    sqlx::raw_sql("INSERT INTO users(id,username,hash,api_key,role_id,password_change_required,account_locked) VALUES(100,'surviving-admin','unused','surviving-key',1,0,0); INSERT INTO groups(id,user_id,name) VALUES(101,100,'Other owner'); INSERT INTO group_targets(group_id,target_id) SELECT 101,id FROM targets; CREATE TRIGGER fail_delete BEFORE DELETE ON users WHEN OLD.id=17 BEGIN SELECT RAISE(ABORT,'synthetic late delete failure'); END;").execute(&pool).await.unwrap();
    let repo = Accounts::new(pool.clone());
    let actor = repo.by_id(100).await.unwrap();
    let old = repo.by_id(17).await.unwrap();
    assert!(matches!(
        repo.write(actor.identity, ruphish_application::auth::Write::Delete(17))
            .await,
        Err(Error::Internal)
    ));
    for table in [
        "campaigns",
        "events",
        "templates",
        "attachments",
        "pages",
        "groups",
        "group_targets",
        "smtp",
        "results",
        "targets",
    ] {
        assert!(
            sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!(
                "SELECT count(*) FROM {table}"
            )))
            .fetch_one(&pool)
            .await
            .unwrap()
                > 0,
            "{table} rolled back"
        );
    }
    sqlx::raw_sql("DROP TRIGGER fail_delete;")
        .execute(&pool)
        .await
        .unwrap();
    repo.write(actor.identity, ruphish_application::auth::Write::Delete(17))
        .await
        .unwrap();
    for table in [
        "campaigns",
        "events",
        "templates",
        "attachments",
        "pages",
        "smtp",
        "results",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(format!(
                "SELECT count(*) FROM {table}"
            )))
            .fetch_one(&pool)
            .await
            .unwrap(),
            0,
            "{table} cleaned up"
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM groups WHERE user_id=100")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM targets")
            .fetch_one(&pool)
            .await
            .unwrap()
            > 0
    );
    // A manual restore/import reusing a deleted identifier must not revive prior identities.
    sqlx::query("INSERT INTO users(id,username,hash,api_key,role_id,password_change_required,account_locked) VALUES(17,'reused-id','unused','new-key',2,0,0)").execute(&pool).await.unwrap();
    assert!(
        repo.by_id(17)
            .await
            .unwrap()
            .authenticate(old.identity)
            .is_err()
    );
    db.close().await;
}

#[tokio::test]
async fn unsupported_auth_history_and_partial_schemas_are_refused_without_reinitializing() {
    for sql in [
        "UPDATE ruphish_schema SET version=999 WHERE version=(SELECT max(version) FROM ruphish_schema)",
        "DROP TABLE ruphish_auth_state",
        "ALTER TABLE ruphish_sessions ADD COLUMN unsupported TEXT",
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("accounts.db");
        let db = StartupDatabase::open(&path, "").await.unwrap();
        db.bootstrap(
            Some("original-password".into()),
            Some("original-key".into()),
        )
        .await
        .unwrap();
        sqlx::raw_sql(sql).execute(&db.pool()).await.unwrap();
        db.close().await;
        let bytes = std::fs::read(&path).unwrap();
        assert!(StartupDatabase::open(&path, "").await.is_err(), "{sql}");
        assert_eq!(
            std::fs::read(&path).unwrap(),
            bytes,
            "No partial schema may be mistaken for a fresh database"
        );
    }
}
