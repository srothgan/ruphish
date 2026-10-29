//! Transactional account workflows against real SQLite, including failure injection.
#![allow(clippy::unwrap_used, reason = "Integration setup and assertions")]
mod support;
use ruphish_adapters::password_worker::{self, PasswordWorker};
use ruphish_application::auth::{Edit, Error, Service};
use std::{path::Path, sync::Arc};
use support::DatabaseFixture;

struct SessionFault {
    repository: Arc<dyn ruphish_application::auth::Repository>,
    fault: &'static str,
}

#[async_trait::async_trait]
impl ruphish_application::auth::Repository for SessionFault {
    async fn by_id(&self, id: i64) -> Result<ruphish_application::auth::Account, Error> {
        use ruphish_application::storage;
        match self.fault {
            "busy" => Err(storage::Error::Busy.into()),
            "unavailable" => Err(storage::Error::Unavailable.into()),
            "missing" => Err(Error::NotFound),
            _ => {
                let mut account = self.repository.by_id(id).await?;
                account.role.permissions.clear();
                Ok(account)
            }
        }
    }
    async fn by_name(&self, name: &str) -> Result<ruphish_application::auth::Account, Error> {
        self.repository.by_name(name).await
    }
    async fn by_key(&self, key: &str) -> Result<ruphish_application::auth::Account, Error> {
        self.repository.by_key(key).await
    }
    async fn list(
        &self,
        caller: ruphish_application::auth::Identity,
    ) -> Result<Vec<ruphish_application::auth::Account>, Error> {
        self.repository.list(caller).await
    }
    async fn write(
        &self,
        caller: ruphish_application::auth::Identity,
        write: ruphish_application::auth::Write,
    ) -> Result<Option<ruphish_application::auth::Account>, Error> {
        self.repository.write(caller, write).await
    }
}

#[tokio::test]
async fn delivery_authorization_retains_storage_and_permission_classifications() {
    use ruphish_application::{auth, delivery, resources, storage};
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("delivery-auth.db");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/authentication/legacy-accounts.sqlite3"),
        &path,
    )
    .unwrap();
    let db = DatabaseFixture::open(&path).await.unwrap();
    let repository = db.accounts();
    let actor = repository.by_id(1).await.unwrap().identity;
    let resources = resources::Service::new(
        db.resources(),
        Arc::new(ruphish_adapters::resources::Validate),
        Arc::new(ruphish_adapters::imports::Tools {
            policy: Default::default(),
        }),
        db.content_work(),
    );
    for fault in ["busy", "unavailable", "missing", "forbidden"] {
        let accounts = auth::Service::new(
            Arc::new(SessionFault {
                repository: repository.clone(),
                fault,
            }),
            Arc::new(PasswordWorker::default()),
        );
        let delivery = delivery::Service::new(
            accounts,
            resources.clone(),
            Arc::new(ruphish_adapters::smtp::Smtp::new(
                Default::default(),
                String::new(),
            )),
            db.previews(),
        );
        // An absent profile also prevents transport if authorization unexpectedly succeeds.
        let error = delivery
            .connect(actor, delivery::ProfileSelection::Saved(-1))
            .await
            .unwrap_err();
        let observed = match error {
            resources::Error::Storage(storage::Error::Busy) => "busy",
            resources::Error::Storage(storage::Error::Unavailable) => "unavailable",
            resources::Error::Policy(auth::AccountError::Unauthorized) => "missing",
            resources::Error::Policy(auth::AccountError::Forbidden) => "forbidden",
            _ => "wrong classification",
        };
        assert_eq!(observed, fault);
    }
    db.close().await;
}

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
    let db = DatabaseFixture::open(&path).await.unwrap();
    let pool = db.inspection_pool();
    let repo = db.accounts();
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
        Err(Error::Storage(
            ruphish_application::storage::Error::Integrity
        ))
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
    let reopened = DatabaseFixture::open(&path).await.unwrap();
    assert!(reopened.backup_path().is_none());
    let repo = reopened.accounts();
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
    let db = DatabaseFixture::open(&path).await.unwrap();
    let pool = db.inspection_pool();
    sqlx::raw_sql("INSERT INTO users(id,username,hash,api_key,role_id,password_change_required,account_locked) VALUES(100,'surviving-admin','unused','surviving-key',1,0,0); INSERT INTO groups(id,user_id,name) VALUES(101,100,'Other owner'); INSERT INTO group_targets(group_id,target_id) SELECT 101,id FROM targets; CREATE TRIGGER fail_delete BEFORE DELETE ON users WHEN OLD.id=17 BEGIN SELECT RAISE(ABORT,'synthetic late delete failure'); END;").execute(&pool).await.unwrap();
    let repo = db.accounts();
    let actor = repo.by_id(100).await.unwrap();
    let old = repo.by_id(17).await.unwrap();
    sqlx::raw_sql(
        "INSERT INTO ruphish_user_preferences(user_id,language) VALUES(17,'de'),(100,'en')",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(matches!(
        repo.write(actor.identity, ruphish_application::auth::Write::Delete(17))
            .await,
        Err(Error::Storage(
            ruphish_application::storage::Error::Integrity
        ))
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
    assert_eq!(
        repo.by_id(17)
            .await
            .unwrap()
            .preferences
            .language
            .as_deref(),
        Some("de")
    );
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
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM ruphish_user_preferences WHERE user_id=17"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        repo.by_id(100)
            .await
            .unwrap()
            .preferences
            .language
            .as_deref(),
        Some("en")
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
        "UPDATE _sqlx_migrations SET version=999 WHERE version=1",
        "DROP TABLE ruphish_auth_state",
        "ALTER TABLE ruphish_sessions ADD COLUMN unsupported TEXT",
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("accounts.db");
        let db = DatabaseFixture::open(&path).await.unwrap();
        db.bootstrap(
            Some("original-password".into()),
            Some("original-key".into()),
        )
        .await
        .unwrap();
        sqlx::raw_sql(sql)
            .execute(&db.inspection_pool())
            .await
            .unwrap();
        db.close().await;
        let bytes = std::fs::read(&path).unwrap();
        assert!(DatabaseFixture::open(&path).await.is_err(), "{sql}");
        assert_eq!(
            std::fs::read(&path).unwrap(),
            bytes,
            "No partial schema may be mistaken for a fresh database"
        );
    }
}

#[tokio::test]
async fn preferences_survive_reopen_and_failed_writes_preserve_choices() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("preferences.db");
    let db = DatabaseFixture::open(&path).await.unwrap();
    db.bootstrap(
        Some("Preferences fixture password 42!".into()),
        Some("preferences-fixture-key".into()),
    )
    .await
    .unwrap();
    let pool = db.inspection_pool();
    sqlx::raw_sql("UPDATE users SET password_change_required=0;")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    db.close().await;
    let upgraded = DatabaseFixture::open(&path).await.unwrap();
    assert!(!upgraded.has_recovery_backup());
    let pool = upgraded.inspection_pool();
    let service = Service::new(upgraded.accounts(), Arc::new(PasswordWorker::default()));
    let actor = upgraded.accounts().by_name("admin").await.unwrap();
    assert_eq!(actor.api_key, "preferences-fixture-key");
    assert_eq!(actor.preferences, Default::default());
    let selected = ruphish_application::auth::Preferences {
        language: Some("de".into()),
        format_locale: Some("zh-CN".into()),
        time_zone: Some("Europe/Berlin".into()),
        show_utc_offset: false,
    };
    assert_eq!(
        service
            .save_preferences(actor.identity, selected.clone())
            .await
            .unwrap(),
        selected
    );
    sqlx::raw_sql("CREATE TRIGGER fail_preferences BEFORE UPDATE ON ruphish_user_preferences BEGIN SELECT RAISE(ABORT,'synthetic preference storage failure'); END;").execute(&pool).await.unwrap();
    assert!(
        service
            .save_preferences(actor.identity, Default::default())
            .await
            .is_err()
    );
    assert_eq!(
        upgraded
            .accounts()
            .by_id(actor.identity.id)
            .await
            .unwrap()
            .preferences,
        selected
    );
    sqlx::raw_sql("DROP TRIGGER fail_preferences")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    upgraded.close().await;
    let reopened = DatabaseFixture::open(&path).await.unwrap();
    assert_eq!(
        reopened
            .accounts()
            .by_id(actor.identity.id)
            .await
            .unwrap()
            .preferences,
        ruphish_application::auth::Preferences {
            show_utc_offset: false,
            ..selected.clone()
        }
    );
    reopened
        .accounts()
        .write(
            actor.identity,
            ruphish_application::auth::Write::Preferences(selected.clone()),
        )
        .await
        .unwrap();
    reopened.close().await;
    let reopened = DatabaseFixture::open(&path).await.unwrap();
    assert_eq!(
        reopened
            .accounts()
            .by_id(actor.identity.id)
            .await
            .unwrap()
            .preferences,
        selected
    );
    reopened.close().await;
}
