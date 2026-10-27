//! One policy workflow exercised through each real provider's production facade.
use super::*;

async fn account(db: &Database) -> auth::Account {
    db.bootstrap(
        Some("Settings fixture password 42!".into()),
        Some("settings-fixture-key".into()),
    )
    .await
    .unwrap();
    let admin = db.accounts().by_name("admin").await.unwrap();
    db.accounts()
        .write(
            admin.identity,
            auth::Write::Password {
                expected_hash: admin.hash.clone(),
                hash: admin.hash,
            },
        )
        .await
        .unwrap()
        .unwrap()
}

async fn input(db: &Database, actor: auth::Identity, origin: &str) -> campaign::Create {
    let resources = db.resources();
    let group = resources
        .save(
            actor,
            resource(
                "Audience",
                Content::Group(vec![Recipient {
                    email: "participant@example.test".into(),
                    ..Default::default()
                }]),
            ),
            None,
            None,
        )
        .await
        .unwrap();
    let template = resources
        .save(
            actor,
            resource(
                "Email",
                Content::Template(Template {
                    text: "Training {{.URL}}".into(),
                    ..Default::default()
                }),
            ),
            None,
            None,
        )
        .await
        .unwrap();
    let page = resources
        .save(
            actor,
            resource(
                "Page",
                Content::Page(Page {
                    html: "<h1>Training</h1>".into(),
                    ..Default::default()
                }),
            ),
            None,
            None,
        )
        .await
        .unwrap();
    let profile = resources
        .save(
            actor,
            resource(
                "SMTP",
                Content::Profile(Profile {
                    interface_type: "SMTP".into(),
                    host: "127.0.0.1:2525".into(),
                    from_address: "training@example.test".into(),
                    ..Default::default()
                }),
            ),
            None,
            None,
        )
        .await
        .unwrap();
    let select = |r: Resource| campaign::Selection {
        id: r.id,
        revision: Some(r.modified_date),
    };
    campaign::Create {
        name: "Settings workflow".into(),
        groups: vec![select(group)],
        template: select(template),
        page: select(page),
        profile: select(profile),
        url: origin.into(),
        ..Default::default()
    }
}

pub(super) async fn exercise(db: &Database) {
    let admin = account(db).await;
    let who = admin.identity;
    let other = db
        .accounts()
        .write(
            who,
            auth::Write::Save {
                target: None,
                hash: Some(admin.hash),
                edit: auth::Edit {
                    username: "settings-operator".into(),
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
    let settings = db.settings();
    let initial = settings.get(who).await.unwrap();
    assert!(!initial.policy.verification_enabled);
    assert!(initial.policy.verification_expires);
    assert!(!initial.policy.reporting_mailboxes_enabled);
    assert!(!initial.policy.webhooks_enabled);
    assert!(settings.get(other).await.is_err());
    assert!(
        !settings
            .features(other)
            .await
            .unwrap()
            .reporting_mailboxes_enabled
    );
    assert!(
        settings
            .save(other, initial.revision, initial.policy.clone(), 100)
            .await
            .is_err()
    );
    assert!(settings.preview_closure(other, 100).await.is_err());
    let mut invalid = initial.policy.clone();
    invalid.verification_validity_seconds = 1;
    assert!(
        settings
            .save(who, initial.revision, invalid, 100)
            .await
            .is_err()
    );
    assert_eq!(settings.get(who).await.unwrap().revision, initial.revision);
    let now = 1_900_000_000;
    let second_admin = db
        .accounts()
        .write(
            who,
            auth::Write::Save {
                target: None,
                hash: Some(db.accounts().by_name("admin").await.unwrap().hash),
                edit: auth::Edit {
                    username: "policy-reviewer".into(),
                    password: String::new(),
                    role: "admin".into(),
                    password_change_required: false,
                    account_locked: false,
                },
            },
        )
        .await
        .unwrap()
        .unwrap()
        .identity;
    let empty = settings.preview_closure(who, now).await.unwrap();
    assert!(matches!(
        settings
            .close_campaigns(second_admin, &empty.token, now)
            .await,
        Err(Error::NotFound(_))
    ));

    assert_eq!(empty.campaigns, 0);
    assert_eq!(
        settings
            .close_campaigns(who, &empty.token, now)
            .await
            .unwrap()
            .cancelled_deliveries,
        0
    );
    assert!(
        settings
            .close_campaigns(who, &"f".repeat(64), now)
            .await
            .is_err()
    );
    let expired = settings.preview_closure(who, now).await.unwrap();
    assert!(matches!(
        settings
            .close_campaigns(who, &expired.token, expired.expires_at)
            .await,
        Err(Error::Invalid(_))
    ));

    let domains = db.domains();
    let domain = domains
        .save(
            who,
            None,
            domains::Edit {
                name: "Origin".into(),
                origin: "https://policy.example.test".into(),
                enabled: true,
                revision: None,
            },
            now,
        )
        .await
        .unwrap();
    assert!(!domain.verification_required);
    assert_eq!(
        domain.verified_until, 0,
        "off must never manufacture successful evidence"
    );
    assert!(domain.eligible_until > now);
    assert!(domain.active_check.is_none());
    assert!(domains.claim(now).await.unwrap().is_none());
    assert!(matches!(
        domains.request_check(who, domain.id, now).await,
        Err(Error::FeatureDisabled(_))
    ));
    assert!(domains.serves("policy.example.test").await.unwrap());
    let launch = input(db, who, &domain.origin).await;
    let campaigns = db.campaigns();

    assert!(
        campaigns.prepare(other, &launch, now).await.is_err(),
        "disabled verification never bypasses ownership"
    );
    let disabled_domain = domains
        .save(
            who,
            Some(domain.id),
            domains::Edit {
                name: domain.name.clone(),
                origin: domain.origin.clone(),
                enabled: false,
                revision: Some(domain.revision),
            },
            now,
        )
        .await
        .unwrap();
    assert!(campaigns.prepare(who, &launch, now).await.is_err());
    assert!(!domains.serves("policy.example.test").await.unwrap());
    domains
        .save(
            who,
            Some(domain.id),
            domains::Edit {
                name: domain.name.clone(),
                origin: domain.origin.clone(),
                enabled: true,
                revision: Some(disabled_domain.revision),
            },
            now,
        )
        .await
        .unwrap();
    let first = campaigns.create(who, launch.clone(), now).await.unwrap();
    let rid = first.results[0].rid.clone();
    assert!(
        db.interactions()
            .interact("policy.example.test", &rid, interactions::Action::Open, now)
            .await
            .is_ok()
    );
    let mut required = initial.policy.clone();
    required.verification_enabled = true;
    let saved = settings
        .save(who, initial.revision, required.clone(), now)
        .await
        .unwrap();
    assert!(matches!(
        settings
            .save(who, initial.revision, required.clone(), now)
            .await,
        Err(Error::Changed)
    ));
    assert!(campaigns.prepare(who, &launch, now).await.is_err());
    assert!(campaigns.claim(now).await.unwrap().is_none());
    assert!(!domains.serves("policy.example.test").await.unwrap());
    domains.request_check(who, domain.id, now).await.unwrap();
    let obsolete = domains.claim(now).await.unwrap().unwrap();
    let off = settings
        .save(who, saved.revision, initial.policy.clone(), now)
        .await
        .unwrap();
    domains
        .finish(
            &obsolete,
            domains::Observation {
                control: true,
                dns: true,
                routing: true,
                tls: true,
                code: "verified",
            },
            now + 1,
        )
        .await
        .unwrap();
    assert_eq!(
        domains.get(who, domain.id).await.unwrap().verified_until,
        0,
        "disabled claim is fenced"
    );
    let mut policy = required.clone();
    policy.verification_validity_seconds = 3600;
    policy.verification_recheck_seconds = 900;
    let enabled = settings
        .save(who, off.revision, policy.clone(), now + 2)
        .await
        .unwrap();
    domains
        .request_check(who, domain.id, now + 3)
        .await
        .unwrap();
    let claim = domains.claim(now + 3).await.unwrap().unwrap();
    domains
        .finish(
            &claim,
            domains::Observation {
                control: true,
                dns: true,
                routing: true,
                tls: true,
                code: "verified",
            },
            now + 4,
        )
        .await
        .unwrap();
    let evidence = domains.get(who, domain.id).await.unwrap();
    assert_eq!(evidence.expires_at, Some(now + 3604));
    assert!(campaigns.prepare(who, &launch, now + 3604).await.is_err());
    assert!(
        domains.serves("policy.example.test").await.unwrap(),
        "expiry alone preserves links"
    );
    policy.verification_expires = false;
    let no_expiry = settings
        .save(who, enabled.revision, policy.clone(), now + 3604)
        .await
        .unwrap();
    assert!(campaigns.prepare(who, &launch, now + 3605).await.is_ok());
    assert_eq!(domains.get(who, domain.id).await.unwrap().expires_at, None);
    domains
        .request_check(who, domain.id, now + 3605)
        .await
        .unwrap();
    let failed = domains.claim(now + 3605).await.unwrap().unwrap();
    domains
        .finish(
            &failed,
            domains::Observation {
                code: "domain_txt_mismatch",
                ..Default::default()
            },
            now + 3606,
        )
        .await
        .unwrap();
    assert!(campaigns.prepare(who, &launch, now + 3607).await.is_err());
    assert!(!domains.serves("policy.example.test").await.unwrap());
    policy.verification_enabled = false;
    policy.reporting_mailboxes_enabled = false;
    let disabled = settings
        .save(who, no_expiry.revision, policy.clone(), now + 3607)
        .await
        .unwrap();
    assert!(
        !settings
            .features(other)
            .await
            .unwrap()
            .reporting_mailboxes_enabled
    );
    let integrations = db.integrations();
    let hook = integrations
        .save_webhook(
            who,
            None,
            ruphish_application::integrations::WebhookEdit {
                name: "Disabled endpoint".into(),
                url: "http://127.0.0.1:1/never".into(),
                secret: Some("synthetic-secret".into()),
                active: true,
                revision: None,
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        integrations.validate_webhook(who, hook.id).await,
        Err(Error::FeatureDisabled(_))
    ));
    integrations.tick().await.unwrap();
    assert!(
        integrations
            .webhook(who, hook.id)
            .await
            .unwrap()
            .history
            .is_empty()
    );
    let (a, b) = tokio::join!(
        settings.save(who, disabled.revision, policy.clone(), now + 3608),
        settings.save(who, disabled.revision, policy.clone(), now + 3608)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(matches!(a.err().or(b.err()), Some(Error::Changed)));
    // Cross-owner closure includes scheduled campaigns but not durable drafts.
    domains
        .save(
            other,
            None,
            domains::Edit {
                name: "Other origin".into(),
                origin: "https://other-policy.example.test".into(),
                enabled: true,
                revision: None,
            },
            now,
        )
        .await
        .unwrap();
    let mut other_launch = input(db, other, "https://other-policy.example.test").await;
    other_launch.launch = Some(now + 7200);
    let second = campaigns
        .create(other, other_launch, now + 3608)
        .await
        .unwrap();
    campaigns
        .save_draft(
            other,
            "settings-draft-000001",
            None,
            Default::default(),
            0,
            now,
        )
        .await
        .unwrap();
    let review = settings.preview_closure(who, now + 3609).await.unwrap();
    assert_eq!(review.campaigns, 2);
    assert_eq!(review.pending_deliveries, 2);
    assert!(
        settings
            .close_campaigns(other, &review.token, now + 3610)
            .await
            .is_err()
    );
    let third = campaigns
        .create(who, launch.clone(), now + 3610)
        .await
        .unwrap();
    assert!(matches!(
        settings
            .close_campaigns(who, &review.token, now + 3611)
            .await,
        Err(Error::Conflict(_))
    ));
    assert_ne!(
        campaigns.get(who, first.id).await.unwrap().status.as_str(),
        "Completed"
    );
    let review = settings.preview_closure(who, now + 3612).await.unwrap();
    let (first_close, duplicate_close) = tokio::join!(
        settings.close_campaigns(who, &review.token, now + 3613),
        settings.close_campaigns(who, &review.token, now + 3613)
    );
    let result = first_close.unwrap();
    let duplicate = duplicate_close.unwrap();
    assert_eq!(duplicate.completed_at, result.completed_at);
    assert_eq!(duplicate.cancelled_deliveries, result.cancelled_deliveries);
    assert_eq!(result.campaigns, 3);
    assert_eq!(result.cancelled_deliveries, 3);
    for (owner, id) in [(who, first.id), (other, second.id), (who, third.id)] {
        let closed = campaigns.get(owner, id).await.unwrap();
        assert_eq!(closed.status.as_str(), "Completed");
        assert_eq!(closed.results.len(), 1);
    }
    assert!(
        campaigns
            .draft(other, "settings-draft-000001")
            .await
            .is_ok()
    );
    assert!(
        db.interactions()
            .interact(
                "policy.example.test",
                &rid,
                interactions::Action::Open,
                now + 3614
            )
            .await
            .is_err()
    );
    let later = campaigns.create(who, launch, now + 3614).await.unwrap();
    let replay = settings
        .close_campaigns(who, &review.token, now + 4000)
        .await
        .unwrap();
    assert_eq!(replay.completed_at, result.completed_at);
    assert_eq!(replay.campaigns, 3);
    assert_ne!(
        campaigns.get(who, later.id).await.unwrap().status.as_str(),
        "Completed"
    );
}

#[tokio::test]
async fn sqlite_company_settings_and_closure() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("policy.db");
    let config = || Config::Sqlite {
        path: path.clone(),
        migrations_prefix: String::new(),
    };
    let db = database::open(config(), Policy::default()).await.unwrap();
    exercise(&db).await;
    db.close().await;
    let db = database::open(config(), Policy::default()).await.unwrap();
    let who = db.accounts().by_name("admin").await.unwrap().identity;
    assert!(
        !db.settings()
            .get(who)
            .await
            .unwrap()
            .policy
            .reporting_mailboxes_enabled
    );
    db.close().await;
}

#[tokio::test]
#[ignore = "Requires canonical MySQL fixture"]
async fn mysql_company_settings_and_closure() {
    let (mut inspect, config) = fixture().await;
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    exercise(&db).await;
    mysql_policy_serialization(&db, &mut inspect).await;
    db.close().await;
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    let who = db.accounts().by_name("admin").await.unwrap().identity;
    assert!(
        !db.settings()
            .get(who)
            .await
            .unwrap()
            .policy
            .reporting_mailboxes_enabled
    );
    db.close().await;
    sqlx::raw_sql("DROP VIEW ruphish_domain_policy; DROP TABLE ruphish_campaign_closures; DROP TABLE ruphish_settings; DELETE FROM _sqlx_migrations WHERE version=6;").execute(&mut inspect).await.unwrap();
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    assert_upgraded(&db).await;
    db.close().await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE `{}`",
        config.database
    )))
    .execute(&mut inspect)
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "Requires canonical PostgreSQL fixture"]
async fn postgres_company_settings_and_closure() {
    let (mut admin, config) = postgres_fixture().await;
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    exercise(&db).await;
    let mut contention = postgres_inspect(&config).await;
    postgres_policy_serialization(&db, &mut contention).await;
    contention.close().await.unwrap();
    db.close().await;
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    let who = db.accounts().by_name("admin").await.unwrap().identity;
    assert!(
        !db.settings()
            .get(who)
            .await
            .unwrap()
            .policy
            .reporting_mailboxes_enabled
    );
    db.close().await;
    let mut inspect = postgres_inspect(&config).await;
    sqlx::raw_sql("DROP VIEW ruphish_domain_policy; DROP TABLE ruphish_campaign_closures; DROP TABLE ruphish_settings; DELETE FROM _sqlx_migrations WHERE version=6;").execute(&mut inspect).await.unwrap();
    inspect.close().await.unwrap();
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    assert_upgraded(&db).await;
    db.close().await;
    postgres_cleanup(&mut admin, &config).await;
}

#[tokio::test]
async fn sqlite_policy_and_closure_roll_back_all_related_writes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("atomic-policy.db");
    let db = database::open(
        Config::Sqlite {
            path: path.clone(),
            migrations_prefix: String::new(),
        },
        Policy::default(),
    )
    .await
    .unwrap();
    let who = account(&db).await.identity;
    let inspect =
        sqlx::SqlitePool::connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&path))
            .await
            .unwrap();
    let settings = db.settings();
    let before = settings.get(who).await.unwrap();
    let domain = db
        .domains()
        .save(
            who,
            None,
            domains::Edit {
                name: "Rollback".into(),
                origin: "https://rollback.example.test".into(),
                enabled: true,
                revision: None,
            },
            100,
        )
        .await
        .unwrap();
    let launch = input(&db, who, &domain.origin).await;
    let a = db
        .campaigns()
        .create(who, launch.clone(), 100)
        .await
        .unwrap();
    let b = db.campaigns().create(who, launch, 100).await.unwrap();
    sqlx::raw_sql("CREATE TRIGGER reject_policy_effect BEFORE UPDATE ON ruphish_domains BEGIN SELECT RAISE(ABORT,'fixture fault'); END;").execute(&inspect).await.unwrap();
    let mut changed = before.policy.clone();
    changed.verification_enabled = true;
    assert!(
        settings
            .save(who, before.revision, changed, 101)
            .await
            .is_err()
    );
    let after = settings.get(who).await.unwrap();
    assert_eq!(after.policy, before.policy);
    assert_eq!(after.revision, before.revision);
    assert_eq!(
        db.domains()
            .get(who, domain.id)
            .await
            .unwrap()
            .next_check_at,
        domain.next_check_at
    );
    sqlx::raw_sql("DROP TRIGGER reject_policy_effect; CREATE TRIGGER reject_closure_result BEFORE UPDATE ON ruphish_campaign_closures BEGIN SELECT RAISE(ABORT,'fixture fault'); END;").execute(&inspect).await.unwrap();
    let review = settings.preview_closure(who, 102).await.unwrap();
    assert!(
        settings
            .close_campaigns(who, &review.token, 103)
            .await
            .is_err()
    );
    for id in [a.id, b.id] {
        assert_ne!(
            db.campaigns().get(who, id).await.unwrap().status.as_str(),
            "Completed"
        );
        assert_eq!(
            db.campaigns()
                .deliveries(who, id, 0, 10)
                .await
                .unwrap()
                .items[0]
                .state
                .as_str(),
            "ready"
        );
    }
    sqlx::raw_sql("DROP TRIGGER reject_closure_result")
        .execute(&inspect)
        .await
        .unwrap();
    let result = settings
        .close_campaigns(who, &review.token, 104)
        .await
        .unwrap();
    assert_eq!(result.campaigns, 2);
    assert_eq!(result.cancelled_deliveries, 2);
    inspect.close().await;
    db.close().await;
}

const INTEGRATION_CASES: [(bool, bool, bool); 5] = [
    (false, false, false),
    (true, false, false),
    (false, true, false),
    (true, true, false),
    (true, true, true),
];

// Independent pre-settings data: presence controls the global default, while
// every integration keeps its own enabled state and service credentials.
fn integration_presence_sql(mailbox: bool, webhook: bool, enabled: bool) -> String {
    let mut sql = "DELETE FROM ruphish_webhook_attempts; DELETE FROM ruphish_webhook_jobs; DELETE FROM ruphish_webhook_state; DELETE FROM webhooks; DELETE FROM ruphish_mailboxes;".to_owned();
    if mailbox {
        sql.push_str(&format!("INSERT INTO ruphish_mailboxes(id,owner_id,name,enabled,host,port,username,password,tls,folder) VALUES(99,1,'Existing mailbox',{enabled},'127.0.0.1',993,'service','preserved-mailbox-secret',TRUE,'INBOX');"));
    }
    if webhook {
        sql.push_str(&format!("INSERT INTO webhooks(id,name,url,secret,is_active) VALUES(99,'Existing endpoint','http://127.0.0.1:1/events','preserved-service-secret',{enabled}); INSERT INTO ruphish_webhook_state(webhook_id,revision) VALUES(99,1);"));
    }
    sql
}

async fn assert_integration_defaults(
    db: &Database,
    mailbox: bool,
    webhook: bool,
    enabled: bool,
) -> ruphish_domain::settings::Policy {
    let who = db.accounts().by_name("admin").await.unwrap().identity;
    let initial = db.settings().get(who).await.unwrap();
    assert!(
        !initial.policy.verification_enabled,
        "existing accounts and domains never opt into verification"
    );
    assert_eq!(initial.policy.reporting_mailboxes_enabled, mailbox);
    assert_eq!(initial.policy.webhooks_enabled, webhook);
    if mailbox {
        assert_eq!(
            db.integrations().mailbox(who, 99).await.unwrap().enabled,
            enabled
        );
    }
    if webhook {
        assert_eq!(
            db.integrations().webhook(who, 99).await.unwrap().active,
            enabled
        );
    }
    let mut saved = initial.policy;
    saved.reporting_mailboxes_enabled = !mailbox;
    saved.webhooks_enabled = !webhook;
    db.settings()
        .save(who, initial.revision, saved.clone(), 1_900_000_000)
        .await
        .unwrap();
    saved
}

async fn assert_saved_policy(db: &Database, saved: &ruphish_domain::settings::Policy) {
    let who = db.accounts().by_name("admin").await.unwrap().identity;
    assert_eq!(&db.settings().get(who).await.unwrap().policy, saved);
    assert!(!db.domains().list(who).await.unwrap().is_empty());
}

#[tokio::test]
async fn sqlite_settings_upgrade_uses_integration_presence_without_enabling_verification() {
    for (mailbox, webhook, enabled) in INTEGRATION_CASES {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("upgrade.db");
        std::fs::copy(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/observations/ruphish-v10.sqlite3"),
            &path,
        )
        .unwrap();
        let inspect = sqlx::SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::new().filename(&path),
        )
        .await
        .unwrap();
        sqlx::raw_sql(sqlx::AssertSqlSafe(integration_presence_sql(
            mailbox, webhook, enabled,
        )))
        .execute(&inspect)
        .await
        .unwrap();
        let campaigns: i64 = sqlx::query_scalar("SELECT count(*) FROM campaigns")
            .fetch_one(&inspect)
            .await
            .unwrap();
        inspect.close().await;
        let config = || Config::Sqlite {
            path: path.clone(),
            migrations_prefix: String::new(),
        };
        let db = database::open(config(), Policy::default()).await.unwrap();
        assert!(db.backup_path().is_some());
        let saved = assert_integration_defaults(&db, mailbox, webhook, enabled).await;
        db.close().await;
        let db = database::open(config(), Policy::default()).await.unwrap();
        assert_saved_policy(&db, &saved).await;
        let inspect = sqlx::SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::new().filename(&path),
        )
        .await
        .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM campaigns")
                .fetch_one(&inspect)
                .await
                .unwrap(),
            campaigns
        );
        if webhook {
            assert_eq!(
                sqlx::query_scalar::<_, String>("SELECT secret FROM webhooks WHERE id=99")
                    .fetch_one(&inspect)
                    .await
                    .unwrap(),
                "preserved-service-secret"
            );
        }
        if mailbox {
            assert_eq!(
                sqlx::query_scalar::<_, String>(
                    "SELECT password FROM ruphish_mailboxes WHERE id=99"
                )
                .fetch_one(&inspect)
                .await
                .unwrap(),
                "preserved-mailbox-secret"
            );
        }
        inspect.close().await;
        db.close().await;
    }
}

async fn presence_account(db: &Database) {
    let who = account(db).await.identity;
    db.domains()
        .save(
            who,
            None,
            domains::Edit {
                name: "Existing domain".into(),
                origin: "https://existing.example.test".into(),
                enabled: true,
                revision: None,
            },
            1_900_000_000,
        )
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "Requires canonical MySQL fixture"]
async fn mysql_settings_upgrade_uses_integration_presence_without_enabling_verification() {
    let (mut inspect, config) = fixture().await;
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    presence_account(&db).await;
    db.close().await;
    for (mailbox, webhook, enabled) in INTEGRATION_CASES {
        sqlx::raw_sql("DROP VIEW ruphish_domain_policy; DROP TABLE ruphish_campaign_closures; DROP TABLE ruphish_settings; DELETE FROM _sqlx_migrations WHERE version=6;").execute(&mut inspect).await.unwrap();
        sqlx::raw_sql(sqlx::AssertSqlSafe(integration_presence_sql(
            mailbox, webhook, enabled,
        )))
        .execute(&mut inspect)
        .await
        .unwrap();
        let db = database::open(Config::MySql(config.clone()), Policy::default())
            .await
            .unwrap();
        let saved = assert_integration_defaults(&db, mailbox, webhook, enabled).await;
        db.close().await;
        let db = database::open(Config::MySql(config.clone()), Policy::default())
            .await
            .unwrap();
        assert_saved_policy(&db, &saved).await;
        db.close().await;
    }
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE `{}`",
        config.database
    )))
    .execute(&mut inspect)
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "Requires canonical PostgreSQL fixture"]
async fn postgres_settings_upgrade_uses_integration_presence_without_enabling_verification() {
    let (mut admin, config) = postgres_fixture().await;
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    presence_account(&db).await;
    db.close().await;
    for (mailbox, webhook, enabled) in INTEGRATION_CASES {
        let mut inspect = postgres_inspect(&config).await;
        sqlx::raw_sql("DROP VIEW ruphish_domain_policy; DROP TABLE ruphish_campaign_closures; DROP TABLE ruphish_settings; DELETE FROM _sqlx_migrations WHERE version=6;").execute(&mut inspect).await.unwrap();
        sqlx::raw_sql(sqlx::AssertSqlSafe(integration_presence_sql(
            mailbox, webhook, enabled,
        )))
        .execute(&mut inspect)
        .await
        .unwrap();
        inspect.close().await.unwrap();
        let db = database::open(Config::Postgres(config.clone()), Policy::default())
            .await
            .unwrap();
        let saved = assert_integration_defaults(&db, mailbox, webhook, enabled).await;
        db.close().await;
        let db = database::open(Config::Postgres(config.clone()), Policy::default())
            .await
            .unwrap();
        assert_saved_policy(&db, &saved).await;
        db.close().await;
    }
    postgres_cleanup(&mut admin, &config).await;
}

async fn assert_upgraded(db: &Database) {
    let who = db.accounts().by_name("admin").await.unwrap().identity;
    let policy = db.settings().get(who).await.unwrap().policy;
    assert!(!policy.verification_enabled);
    assert!(policy.webhooks_enabled);
    assert!(!policy.reporting_mailboxes_enabled);
    assert_eq!(db.domains().list(who).await.unwrap().len(), 1);
    assert_eq!(db.integrations().webhooks(who).await.unwrap().len(), 1);
    assert_eq!(
        db.campaigns().get(who, 1).await.unwrap().status.as_str(),
        "Completed"
    );
}

async fn contention_setup(
    db: &Database,
) -> (
    auth::Identity,
    auth::Identity,
    ruphish_application::settings::Settings,
    campaign::Create,
    i64,
) {
    let admin = db.accounts().by_name("admin").await.unwrap().identity;
    let other = db
        .accounts()
        .by_name("settings-operator")
        .await
        .unwrap()
        .identity;
    let settings = db.settings().get(admin).await.unwrap();
    let resources = db.resources();
    let selected = |resource: &Resource| campaign::Selection {
        id: resource.id,
        revision: Some(resource.modified_date.clone()),
    };
    let groups = resources.list(other, Kind::Group).await.unwrap();
    let templates = resources.list(other, Kind::Template).await.unwrap();
    let pages = resources.list(other, Kind::Page).await.unwrap();
    let profiles = resources.list(other, Kind::Profile).await.unwrap();
    let launch = campaign::Create {
        name: "Concurrent policy change".into(),
        groups: vec![selected(
            groups.iter().find(|r| r.name == "Audience").unwrap(),
        )],
        template: selected(templates.iter().find(|r| r.name == "Email").unwrap()),
        page: selected(pages.iter().find(|r| r.name == "Page").unwrap()),
        profile: selected(profiles.iter().find(|r| r.name == "SMTP").unwrap()),
        url: "https://other-policy.example.test".into(),
        ..Default::default()
    };
    let domain = db.domains().list(other).await.unwrap()[0].id;
    (admin, other, settings, launch, domain)
}

async fn mysql_policy_serialization(db: &Database, inspect: &mut sqlx::MySqlConnection) {
    let (admin, other, settings, launch, domain) = contention_setup(db).await;
    let mut held = inspect.begin().await.unwrap();
    sqlx::query("SELECT id FROM ruphish_domains WHERE id=? FOR UPDATE")
        .bind(domain)
        .fetch_one(&mut *held)
        .await
        .unwrap();
    let campaigns = db.campaigns();
    let launching =
        tokio::spawn(async move { campaigns.create(other, launch, 1_900_010_000).await });
    tokio::time::timeout(std::time::Duration::from_secs(10),async {
        loop {
            assert!(!launching.is_finished());
            let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM performance_schema.data_locks WHERE OBJECT_SCHEMA=DATABASE() AND OBJECT_NAME='ruphish_domains' AND LOCK_STATUS='WAITING')").fetch_one(&mut *held).await.unwrap();
            if waiting {break;} tokio::task::yield_now().await;
        }
    }).await.unwrap();
    let store = db.settings();
    let mut policy = settings.policy.clone();
    policy.webhooks_enabled = true;
    let saving = tokio::spawn(async move {
        store
            .save(admin, settings.revision, policy, 1_900_010_001)
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(10),async {
        loop {
            assert!(!saving.is_finished(),"policy must wait for an admitted write by another owner");
            let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM performance_schema.data_locks WHERE OBJECT_SCHEMA=DATABASE() AND OBJECT_NAME='ruphish_settings' AND LOCK_STATUS='WAITING')").fetch_one(&mut *held).await.unwrap();
            if waiting {break;} tokio::task::yield_now().await;
        }
    }).await.unwrap();
    held.rollback().await.unwrap();
    launching.await.unwrap().unwrap();
    let saved = saving.await.unwrap().unwrap();
    db.settings()
        .save(admin, saved.revision, settings.policy, 1_900_010_002)
        .await
        .unwrap();
}

async fn postgres_policy_serialization(db: &Database, inspect: &mut sqlx::PgConnection) {
    let (admin, other, settings, launch, domain) = contention_setup(db).await;
    let mut held = inspect.begin().await.unwrap();
    sqlx::query("SELECT id FROM ruphish_domains WHERE id=$1 FOR UPDATE")
        .bind(domain)
        .fetch_one(&mut *held)
        .await
        .unwrap();
    let campaigns = db.campaigns();
    let launching =
        tokio::spawn(async move { campaigns.create(other, launch, 1_900_010_000).await });
    tokio::time::timeout(std::time::Duration::from_secs(10),async {
        loop {
            assert!(!launching.is_finished());
            sqlx::query("SELECT pg_stat_clear_snapshot()").execute(&mut *held).await.unwrap();
            let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND application_name='ruphish' AND wait_event_type='Lock')").fetch_one(&mut *held).await.unwrap();
            if waiting {break;} tokio::task::yield_now().await;
        }
    }).await.unwrap();
    let store = db.settings();
    let mut policy = settings.policy.clone();
    policy.webhooks_enabled = true;
    let saving = tokio::spawn(async move {
        store
            .save(admin, settings.revision, policy, 1_900_010_001)
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(10),async {
        loop {
            assert!(!saving.is_finished(),"policy must wait for an admitted write by another owner");
            sqlx::query("SELECT pg_stat_clear_snapshot()").execute(&mut *held).await.unwrap();
            let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND application_name='ruphish' AND wait_event_type='Lock' AND query LIKE '%ruphish_settings%')").fetch_one(&mut *held).await.unwrap();
            if waiting {break;} tokio::task::yield_now().await;
        }
    }).await.unwrap();
    held.rollback().await.unwrap();
    launching.await.unwrap().unwrap();
    let saved = saving.await.unwrap().unwrap();
    db.settings()
        .save(admin, saved.revision, settings.policy, 1_900_010_002)
        .await
        .unwrap();
}
