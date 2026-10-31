//! Creator authorization and account lifecycle against each real provider.
use super::{query_work::Inspector, *};
use ruphish_application::integrations::WebhookEdit;

const ADMIN: auth::Identity = auth::Identity { id: 1, epoch: 0 };
const OWNER: auth::Identity = auth::Identity { id: 2, epoch: 0 };

fn edit(revision: Option<i64>) -> WebhookEdit {
    WebhookEdit {
        name: "Endpoint".into(),
        url: "https://169.254.169.254/events".into(),
        secret: Some("synthetic-signing-secret".into()),
        active: true,
        revision,
    }
}

async fn contract(db: &Database, inspect: &mut Inspector) {
    inspect.execute("INSERT INTO users(id,username,hash,api_key,role_id,password_change_required,account_locked) VALUES(1,'admin','hash','admin-key',1,FALSE,FALSE),(2,'operator','hash','operator-key',2,FALSE,FALSE),(3,'colleague','hash','colleague-key',2,FALSE,FALSE); INSERT INTO ruphish_auth_state VALUES(1,0),(2,0),(3,0); UPDATE ruphish_settings SET webhooks_enabled=TRUE;").await;
    let repo = db.integrations();
    let accounts = db.accounts();
    let own = repo.save_webhook(OWNER, None, edit(None)).await.unwrap();
    let other = repo.save_webhook(ADMIN, None, edit(None)).await.unwrap();
    assert_eq!(repo.webhooks(OWNER).await.unwrap()[0].id, own.id);
    assert_eq!(repo.webhooks(ADMIN).await.unwrap()[0].id, other.id);
    for caller in [ADMIN, auth::Identity { id: 3, epoch: 0 }] {
        assert!(matches!(
            repo.webhook(caller, own.id).await,
            Err(Error::NotFound("Webhook"))
        ));
        assert!(matches!(
            repo.save_webhook(caller, Some(own.id), edit(Some(own.revision)))
                .await,
            Err(Error::NotFound("Webhook"))
        ));
        assert!(matches!(
            repo.validate_webhook(caller, own.id).await,
            Err(Error::NotFound("Webhook"))
        ));
        for revision in [None, Some(own.revision), Some(own.revision + 1)] {
            assert!(matches!(
                repo.delete_webhook(caller, own.id, revision).await,
                Err(Error::NotFound("Webhook"))
            ));
        }
    }
    assert_eq!(
        repo.webhook(OWNER, own.id).await.unwrap().revision,
        own.revision
    );
    assert!(repo.webhook(OWNER, own.id).await.unwrap().has_secret);
    inspect.execute("INSERT INTO campaigns(id,user_id,name,status) VALUES(41,1,'Other creator','In progress'); INSERT INTO results(id,campaign_id,user_id,r_id,email,status) VALUES(41,41,1,'other-owner','person@example.test','Email Sent'); INSERT INTO events(id,campaign_id,recipient_id,email,time,kind,source) VALUES(41,41,41,'person@example.test','2026-01-01T00:00:00Z','link_clicked','public'); DELETE FROM ruphish_webhook_jobs; INSERT INTO ruphish_webhook_jobs(webhook_id,revision,event_id) SELECT webhook_id,revision,41 FROM ruphish_webhook_state;").await;
    // Configuration ownership does not bind delivery to the creator's session or campaigns.
    let locked = accounts
        .write(
            ADMIN,
            auth::Write::Save {
                target: Some(OWNER),
                hash: None,
                edit: auth::Edit {
                    username: "operator".into(),
                    password: String::new(),
                    role: "user".into(),
                    password_change_required: false,
                    account_locked: true,
                },
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert!(repo.webhooks(OWNER).await.is_err());
    repo.deliver_webhooks().await.unwrap();
    assert_eq!(
        inspect
            .scalar("SELECT count(*) FROM ruphish_webhook_attempts WHERE finished IS NOT NULL")
            .await,
        2
    );
    assert_eq!(
        inspect
            .scalar("SELECT count(*) FROM ruphish_webhook_attempts WHERE code='accepted'")
            .await,
        0,
        "outbound policy prevents real traffic to the fixture destination"
    );
    inspect
        .execute("UPDATE ruphish_webhook_jobs SET state='retry',due=0")
        .await;
    let (removed, delivered) = tokio::join!(
        accounts.write(ADMIN, auth::Write::Delete(locked.identity.id)),
        repo.deliver_webhooks()
    );
    removed.unwrap();
    delivered.unwrap();
    for table in ["webhooks", "ruphish_webhook_state", "ruphish_webhook_jobs"] {
        assert_eq!(
            inspect
                .scalar(&format!("SELECT count(*) FROM {table}"))
                .await,
            1,
            "deletion removes only the creator's {table}"
        );
    }
    assert_eq!(inspect.scalar("SELECT count(*) FROM ruphish_webhook_attempts a LEFT JOIN ruphish_webhook_jobs j ON j.id=a.job_id WHERE j.id IS NULL").await, 0);
    assert_eq!(repo.webhook(ADMIN, other.id).await.unwrap().id, other.id);
    assert_eq!(
        inspect
            .scalar("SELECT count(*) FROM campaigns WHERE id=41")
            .await,
        1
    );
    // Deletion remains available while the global feature is disabled.
    inspect
        .execute("UPDATE ruphish_settings SET webhooks_enabled=FALSE")
        .await;
    repo.delete_webhook(ADMIN, other.id, None).await.unwrap();
    for table in [
        "webhooks",
        "ruphish_webhook_state",
        "ruphish_webhook_jobs",
        "ruphish_webhook_attempts",
    ] {
        assert_eq!(
            inspect
                .scalar(&format!("SELECT count(*) FROM {table}"))
                .await,
            0
        );
    }
}

#[tokio::test]
async fn sqlite_webhook_ownership() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("webhooks.db");
    let db = database::open(Config::Sqlite { path: path.clone() }, Policy::default())
        .await
        .unwrap();
    let connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(path),
    )
    .await
    .unwrap();
    contract(&db, &mut Inspector::Sqlite(connection)).await;
    db.close().await;
}

#[tokio::test]
#[ignore = "Requires canonical MySQL fixture"]
async fn mysql_webhook_ownership() {
    let (connection, config) = fixture().await;
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    let mut inspect = Inspector::Mysql(connection);
    contract(&db, &mut inspect).await;
    db.close().await;
    inspect
        .execute(&format!("DROP DATABASE `{}`", config.database))
        .await;
}

#[tokio::test]
#[ignore = "Requires canonical PostgreSQL fixture"]
async fn postgres_webhook_ownership() {
    let (mut admin, config) = postgres_fixture().await;
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    let mut inspect = Inspector::Postgres(postgres_inspect(&config).await);
    contract(&db, &mut inspect).await;
    drop(inspect);
    db.close().await;
    postgres_cleanup(&mut admin, &config).await;
}

#[tokio::test]
#[ignore = "Requires canonical MySQL fixture"]
async fn mysql_legacy_webhook_ownership_requires_an_administrator() {
    let (mut connection, config) = fixture().await;
    latest_legacy(&mut connection).await;
    sqlx::raw_sql("INSERT INTO users(id,username,hash,api_key,role_id) VALUES(5,'normal','hash','normal-key',2),(17,'admin','hash','admin-key',1),(31,'other-admin','hash','other-key',1); INSERT INTO webhooks(id,name,url,secret,is_active) VALUES(71,'Imported','https://receiver.example.test/hook','preserved-secret',1); DELETE FROM role_permissions WHERE permission_id=(SELECT id FROM permissions WHERE slug='modify_system');").execute(&mut connection).await.unwrap();
    assert!(
        database::open(Config::MySql(config.clone()), Policy::default())
            .await
            .is_err()
    );
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM information_schema.tables WHERE table_schema=DATABASE() AND table_name='ruphish_webhook_state'").fetch_one(&mut connection).await.unwrap(), 0);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT secret FROM webhooks WHERE id=71")
            .fetch_one(&mut connection)
            .await
            .unwrap(),
        "preserved-secret"
    );
    sqlx::raw_sql("INSERT INTO role_permissions(role_id,permission_id) SELECT 1,id FROM permissions WHERE slug='modify_system'").execute(&mut connection).await.unwrap();
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT owner_id FROM ruphish_webhook_state WHERE webhook_id=71"
        )
        .fetch_one(&mut connection)
        .await
        .unwrap(),
        17
    );
    assert!(
        db.integrations()
            .webhook(auth::Identity { id: 17, epoch: 0 }, 71)
            .await
            .unwrap()
            .has_secret
    );
    assert!(matches!(
        db.integrations()
            .webhook(auth::Identity { id: 31, epoch: 0 }, 71)
            .await,
        Err(Error::NotFound("Webhook"))
    ));
    db.close().await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE `{}`",
        config.database
    )))
    .execute(&mut connection)
    .await
    .unwrap();
}
