//! Shared observable contracts, exercised through the public provider facade.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "Independent integration fixtures and assertions must fail immediately on an unexpected outcome"
)]
use ruphish_adapters::{
    database::{self, Config, Database, MySqlConfig},
    outbound::Policy,
};
use ruphish_application::{auth, campaign, domains, interactions, reporting, resources::*};
use sqlx::{
    Connection, MySqlConnection,
    mysql::{MySqlConnectOptions, MySqlSslMode},
};
use std::path::PathBuf;

#[path = "support/reporting_filters.rs"]
mod reporting_filters;

#[path = "support/resource_summaries.rs"]
mod resource_summaries;

#[tokio::test]
async fn sqlite_resource_summaries() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("summaries.db");
    let db = database::open(Config::Sqlite { path: path.clone() }, Policy::default())
        .await
        .unwrap();
    let mut inspect = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(&path),
    )
    .await
    .unwrap();
    sqlx::raw_sql(resource_summaries::SEED)
        .execute(&mut inspect)
        .await
        .unwrap();
    resource_summaries::contract(&db).await;
    inspect.close().await.unwrap();
    db.close().await;
}

#[tokio::test]
#[ignore = "Requires canonical MySQL fixture"]
async fn mysql_resource_summaries() {
    let (mut inspect, config) = fixture().await;
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    let seed = resource_summaries::SEED.replace("INSERT INTO groups", "INSERT INTO `groups`");
    sqlx::raw_sql(sqlx::AssertSqlSafe(seed))
        .execute(&mut inspect)
        .await
        .unwrap();
    resource_summaries::contract(&db).await;
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
async fn postgres_resource_summaries() {
    let (mut admin, config) = postgres_fixture().await;
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    let mut inspect = postgres_inspect(&config).await;
    sqlx::raw_sql(resource_summaries::SEED)
        .execute(&mut inspect)
        .await
        .unwrap();
    resource_summaries::contract(&db).await;
    inspect.close().await.unwrap();
    db.close().await;
    postgres_cleanup(&mut admin, &config).await;
}

#[path = "support/event_edits.rs"]
mod event_edits;

#[tokio::test]
#[ignore = "Requires canonical MySQL fixture"]
async fn mysql_event_edits_preserve_history_and_reopen() {
    let (mut inspect, config) = fixture().await;
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    sqlx::raw_sql(event_edits::SEED)
        .execute(&mut inspect)
        .await
        .unwrap();
    db.close().await;
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    event_edits::contract(&db).await;
    db.close().await;
    let reopened = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    let history = reopened
        .campaigns()
        .recipient_history(event_edits::OWNER, 10, 11, None, None)
        .await
        .unwrap();
    assert!(!history.events.iter().any(|e| e.id == 101));
    assert!(
        history
            .events
            .iter()
            .find(|e| e.id == 102)
            .unwrap()
            .record
            .as_ref()
            .unwrap()
            .edited
    );
    reopened.close().await;
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
async fn postgres_event_edits_preserve_history_and_reopen() {
    let (mut admin, config) = postgres_fixture().await;
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    let mut inspect = postgres_inspect(&config).await;
    sqlx::raw_sql(event_edits::SEED)
        .execute(&mut inspect)
        .await
        .unwrap();
    sqlx::query("SELECT setval(pg_get_serial_sequence('events','id'),105)")
        .execute(&mut inspect)
        .await
        .unwrap();
    db.close().await;
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    event_edits::contract(&db).await;
    db.close().await;
    let reopened = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    let history = reopened
        .campaigns()
        .recipient_history(event_edits::OWNER, 10, 11, None, None)
        .await
        .unwrap();
    assert!(!history.events.iter().any(|e| e.id == 101));
    assert!(
        history
            .events
            .iter()
            .find(|e| e.id == 102)
            .unwrap()
            .record
            .as_ref()
            .unwrap()
            .edited
    );
    reopened.close().await;
    inspect.close().await.unwrap();
    postgres_cleanup(&mut admin, &config).await;
}

#[path = "support/campaign_origins.rs"]
mod campaign_origins;
#[path = "support/open_tracking.rs"]
mod open_tracking;

#[path = "support/campaign_drafts.rs"]
mod campaign_drafts;
#[path = "support/company_settings.rs"]
mod company_settings;
#[path = "support/health.rs"]
mod health;
#[path = "support/learning_pages.rs"]
mod learning_pages;
#[path = "support/mysql_proxy.rs"]
mod mysql_proxy;
#[path = "support/observation_history.rs"]
mod observation_history;

#[tokio::test]
#[ignore = "Requires canonical MySQL fixture and RUPHISH_MYSQL_TEST_PORT"]
async fn mysql_baseline_preserves_populated_history_and_retry_tombstones() {
    let (mut inspect, config) = fixture().await;
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    db.close().await;
    sqlx::raw_sql(observation_history::HISTORY)
        .execute(&mut inspect)
        .await
        .unwrap();
    for _ in 0..2 {
        let db = database::open(Config::MySql(config.clone()), Policy::default())
            .await
            .unwrap();
        observation_history::verify_history(
            sqlx::query_as(observation_history::READ_HISTORY)
                .fetch_all(&mut inspect)
                .await
                .unwrap(),
        );
        let receipt: (i64, String, Option<i64>) =
            sqlx::query_as("SELECT result_id,request_id,event_id FROM ruphish_event_receipts")
                .fetch_one(&mut inspect)
                .await
                .unwrap();
        assert_eq!(receipt, (11, "a".repeat(32), None));
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
#[ignore = "Requires canonical PostgreSQL fixture and RUPHISH_POSTGRES_TEST_PORT"]
async fn postgres_baseline_preserves_populated_history_and_retry_tombstones() {
    let (mut admin, config) = postgres_fixture().await;
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    db.close().await;
    let mut inspect = postgres_inspect(&config).await;
    sqlx::raw_sql(observation_history::HISTORY)
        .execute(&mut inspect)
        .await
        .unwrap();
    for _ in 0..2 {
        let db = database::open(Config::Postgres(config.clone()), Policy::default())
            .await
            .unwrap();
        observation_history::verify_history(
            sqlx::query_as(observation_history::READ_HISTORY)
                .fetch_all(&mut inspect)
                .await
                .unwrap(),
        );
        let receipt: (i64, String, Option<i64>) =
            sqlx::query_as("SELECT result_id,request_id,event_id FROM ruphish_event_receipts")
                .fetch_one(&mut inspect)
                .await
                .unwrap();
        assert_eq!(receipt, (11, "a".repeat(32), None));
        db.close().await;
    }
    inspect.close().await.unwrap();
    postgres_cleanup(&mut admin, &config).await;
}
#[path = "support/postgres_proxy.rs"]
mod postgres_proxy;
#[path = "support/provider_regressions.rs"]
mod provider_regressions;
#[path = "support/sorting.rs"]
mod sorting;

fn resource(name: &str, content: Content) -> Input {
    Resource {
        id: 0,
        name: name.into(),
        modified_date: String::new(),
        content,
    }
    .into()
}

async fn profile_credentials(
    resources: &std::sync::Arc<dyn Repository>,
    actor: auth::Identity,
    other: auth::Identity,
    original: Resource,
) -> Resource {
    let id = original.id;
    let first = resources
        .save(
            actor,
            original.map_password(|_| Some("first-secret".into())),
            Some(id),
            None,
        )
        .await
        .unwrap();
    let replacement = resources
        .save(
            actor,
            first
                .clone()
                .map_password(|_| Some("replacement-secret".into())),
            Some(id),
            Some(first.modified_date.clone()),
        )
        .await
        .unwrap();
    assert!(matches!(
        resources
            .save(
                actor,
                first.clone().map_password(|_| None),
                Some(id),
                Some(first.modified_date.clone())
            )
            .await,
        Err(Error::Changed)
    ));
    // An unconditional unrelated edit based on an older snapshot must retain the latest password.
    let retained = resources
        .save(actor, first.map_password(|_| None), Some(id), None)
        .await
        .unwrap();
    assert!(matches!(&retained.content, Content::Profile(p) if p.password == "replacement-secret"));
    assert!(matches!(
        resources
            .save(
                other,
                retained
                    .clone()
                    .map_password(|_| Some("foreign-secret".into())),
                Some(id),
                None
            )
            .await,
        Err(Error::NotFound("SMTP"))
    ));
    let unchanged = resources.get(actor, Kind::Profile, id).await.unwrap();
    assert!(
        matches!(&unchanged.content, Content::Profile(p) if p.password == "replacement-secret")
    );
    assert_ne!(retained.modified_date, replacement.modified_date);
    let cleared = resources
        .save(
            actor,
            retained.map_password(|_| Some(String::new())),
            Some(id),
            Some(unchanged.modified_date),
        )
        .await
        .unwrap();
    assert!(matches!(&cleared.content, Content::Profile(p) if p.password.is_empty()));
    let persisted = resources.get(actor, Kind::Profile, id).await.unwrap();
    assert!(
        persisted == cleared,
        "clearing must persist, not only change the save response"
    );
    persisted
}

async fn workflows(db: &Database) {
    workflows_with_commit_fault(db, None).await;
}

async fn workflows_with_commit_fault(db: &Database, fault: Option<&std::sync::atomic::AtomicBool>) {
    let accounts = db.accounts();
    db.bootstrap(
        Some("Provider fixture password 42!".into()),
        Some("fixture-api-key".into()),
    )
    .await
    .unwrap();
    let admin = accounts.by_name("admin").await.unwrap();
    let admin = accounts
        .write(
            admin.identity,
            auth::Write::Password {
                expected_hash: admin.hash.clone(),
                hash: admin.hash,
            },
        )
        .await
        .unwrap()
        .unwrap();
    let actor = admin.identity;
    let settings = db.settings().get(actor).await.unwrap();
    let mut policy = settings.policy;
    policy.verification_enabled = true;
    policy.webhooks_enabled = true;
    policy.reporting_mailboxes_enabled = true;
    db.settings()
        .save(actor, settings.revision, policy, 0)
        .await
        .unwrap();
    assert_eq!(
        accounts.by_key("fixture-api-key").await.unwrap().identity,
        actor
    );
    assert!(
        accounts.by_key("FIXTURE-API-KEY").await.is_err(),
        "API key comparison is exact"
    );
    assert!(
        accounts
            .write(actor, auth::Write::Delete(actor.id))
            .await
            .is_err(),
        "self deletion is forbidden"
    );
    let session = db.sessions();
    let (a, b) = tokio::join!(session.cookie_key(), session.cookie_key());
    assert_eq!(
        a.unwrap(),
        b.unwrap(),
        "concurrent startups share one persistent cookie key"
    );
    use tower_sessions_core::{SessionStore as _, session::Record};
    let mut record = Record {
        id: Default::default(),
        data: Default::default(),
        expiry_date: time::OffsetDateTime::now_utc() + time::Duration::hours(1),
    };
    session.create(&mut record).await.unwrap();
    session.save(&record).await.unwrap();
    assert!(session.load(&record.id).await.unwrap().is_some());
    session.delete(&record.id).await.unwrap();
    assert!(
        session.save(&record).await.is_err(),
        "late save cannot resurrect a logged-out session"
    );
    assert!(session.load(&record.id).await.unwrap().is_none());
    let normal = accounts
        .write(
            actor,
            auth::Write::Save {
                target: None,
                hash: Some(admin.hash.clone()),
                edit: auth::Edit {
                    username: "normal".into(),
                    password: String::new(),
                    role: "user".into(),
                    password_change_required: false,
                    account_locked: false,
                },
            },
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(normal.preferences, auth::Preferences::default());
    let overrides = auth::Preferences {
        language: Some("de".into()),
        format_locale: Some("zh-CN".into()),
        time_zone: Some("Europe/Berlin".into()),
        show_utc_offset: false,
    };
    let changed = accounts
        .write(normal.identity, auth::Write::Preferences(overrides.clone()))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        changed.identity, normal.identity,
        "presentation changes must not revoke sessions"
    );
    assert_eq!(
        changed.last_login, normal.last_login,
        "preferences never rewrite recorded timestamps"
    );
    assert_eq!(
        db.accounts()
            .by_id(normal.identity.id)
            .await
            .unwrap()
            .preferences,
        overrides
    );
    assert_eq!(
        accounts.by_id(actor.id).await.unwrap().preferences,
        auth::Preferences::default(),
        "other accounts remain isolated"
    );
    for (field, invalid) in [
        (
            "language",
            auth::Preferences {
                language: Some("zh".into()),
                ..overrides.clone()
            },
        ),
        (
            "format_locale",
            auth::Preferences {
                format_locale: Some("not_a_locale".into()),
                ..overrides.clone()
            },
        ),
        (
            "time_zone",
            auth::Preferences {
                time_zone: Some("Mars/Unknown".into()),
                ..overrides.clone()
            },
        ),
    ] {
        assert!(matches!(
            accounts
                .write(normal.identity, auth::Write::Preferences(invalid))
                .await,
            Err(auth::Error::Validation(report)) if report.issues().iter().any(|issue| issue.field == field)
        ));
        assert_eq!(
            accounts
                .by_id(normal.identity.id)
                .await
                .unwrap()
                .preferences,
            overrides
        );
    }
    let stale = auth::Identity {
        epoch: normal.identity.epoch + 1,
        ..normal.identity
    };
    assert!(
        accounts
            .write(stale, auth::Write::Preferences(Default::default()))
            .await
            .is_err()
    );
    accounts
        .write(
            normal.identity,
            auth::Write::Preferences(Default::default()),
        )
        .await
        .unwrap();
    assert_eq!(
        accounts
            .by_id(normal.identity.id)
            .await
            .unwrap()
            .preferences,
        auth::Preferences::default()
    );

    let listed = accounts.list(actor).await.unwrap();
    assert!(
        listed
            .windows(2)
            .all(|pair| pair[0].identity.id < pair[1].identity.id)
    );
    let listed_normal = listed
        .iter()
        .find(|a| a.identity == normal.identity)
        .unwrap();
    assert_eq!(listed_normal.username, normal.username);
    assert_eq!(listed_normal.role.slug, "user");
    assert!(listed_normal.permits("view_objects"));
    assert!(listed_normal.permits("modify_objects"));
    assert!(!listed_normal.permits("modify_system"));
    assert_eq!(listed_normal.preferences, auth::Preferences::default());
    assert!(accounts.list(normal.identity).await.is_err());

    let integrations = db.integrations();
    use ruphish_application::integrations::{MailboxEdit, WebhookEdit};
    let hook_edit = |revision| WebhookEdit {
        name: "Local denied webhook".into(),
        url: "https://169.254.169.254/hook".into(),
        secret: Some("fixture-hook-secret".into()),
        active: true,
        revision,
    };
    assert!(
        integrations
            .save_webhook(normal.identity, None, hook_edit(None))
            .await
            .is_err(),
        "only administrators configure webhooks"
    );
    let hook = integrations
        .save_webhook(actor, None, hook_edit(None))
        .await
        .unwrap();
    let mail_edit = |revision| MailboxEdit {
        name: "Reporting".into(),
        enabled: true,
        host: "169.254.169.254".into(),
        port: 1,
        username: "service".into(),
        password: Some("fixture-mail-secret".into()),
        tls: true,
        ignore_cert_errors: false,
        folder: "INBOX".into(),
        restrict_domain: String::new(),
        delete_reported: false,
        frequency: 60,
        revision,
    };
    let blocked = accounts
        .write(
            actor,
            auth::Write::Save {
                target: None,
                hash: Some(admin.hash.clone()),
                edit: auth::Edit {
                    username: "locked-mail-owner".into(),
                    password: String::new(),
                    role: "user".into(),
                    password_change_required: false,
                    account_locked: false,
                },
            },
        )
        .await
        .unwrap()
        .unwrap();
    let blocked_mailbox = integrations
        .save_mailbox(blocked.identity, None, mail_edit(None))
        .await
        .unwrap();
    accounts
        .write(
            actor,
            auth::Write::Save {
                target: Some(blocked.identity),
                hash: None,
                edit: auth::Edit {
                    username: "locked-mail-owner".into(),
                    password: String::new(),
                    role: "user".into(),
                    password_change_required: false,
                    account_locked: true,
                },
            },
        )
        .await
        .unwrap();
    let mailbox = integrations
        .save_mailbox(actor, None, mail_edit(None))
        .await
        .unwrap();
    assert!(
        !mailbox.modified_date.starts_with("0001-"),
        "a newly created mailbox records its creation timestamp"
    );
    assert!(
        integrations
            .mailbox(normal.identity, mailbox.id)
            .await
            .is_err()
    );
    assert!(
        integrations
            .save_mailbox(actor, None, mail_edit(None))
            .await
            .is_err(),
        "duplicate names are rejected"
    );
    let mut bypass = mail_edit(Some(mailbox.revision));
    bypass.ignore_cert_errors = true;
    let mailbox = integrations
        .save_mailbox(actor, Some(mailbox.id), bypass)
        .await
        .unwrap();
    assert!(mailbox.ignore_cert_errors);
    assert!(
        integrations
            .mailbox(actor, mailbox.id)
            .await
            .unwrap()
            .ignore_cert_errors
    );
    let (a, b) = tokio::join!(
        integrations.save_mailbox(actor, Some(mailbox.id), mail_edit(Some(mailbox.revision))),
        integrations.save_mailbox(actor, Some(mailbox.id), mail_edit(Some(mailbox.revision)))
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(matches!(
        a.as_ref().err().or(b.as_ref().err()),
        Some(Error::Changed)
    ));
    integrations.poll_mailboxes().await.unwrap();
    let mailbox = integrations.mailbox(actor, mailbox.id).await.unwrap();
    assert!(!mailbox.running);
    assert!(!mailbox.ignore_cert_errors);
    assert_eq!(mailbox.history.len(), 1);
    assert!(mailbox.history[0].finished.is_some());
    assert_ne!(
        mailbox.code, "passed",
        "link-local is denied before connecting"
    );
    accounts
        .write(actor, auth::Write::Delete(blocked.identity.id))
        .await
        .unwrap();
    assert!(
        integrations
            .mailbox(actor, blocked_mailbox.id)
            .await
            .is_err()
    );
    let resources = db.resources();
    let recipient = Recipient {
        email: "élise@example.test".into(),
        first_name: "Élise".into(),
        last_name: "Müller".into(),
        position: "=1+1".into(),
    };
    let group_input = resource("Engineering", Content::Group(vec![recipient]));
    let (a, b) = tokio::join!(
        resources.save(actor, group_input.clone(), None, None),
        resources.save(actor, group_input, None, None)
    );
    assert_eq!(
        usize::from(a.is_ok()) + usize::from(b.is_ok()),
        1,
        "same-owner duplicate creates have one winner"
    );
    assert!(matches!(
        a.as_ref().err().or(b.as_ref().err()),
        Some(Error::Conflict("Group"))
    ));
    let group = a.or(b).unwrap();
    assert!(
        matches!(
            resources.get(normal.identity, Kind::Group, group.id).await,
            Err(Error::NotFound("Group"))
        ),
        "guessed IDs cannot cross owner boundaries"
    );
    assert!(matches!(
        resources
            .delete(normal.identity, Kind::Group, group.id, None)
            .await,
        Err(Error::NotFound("Group"))
    ));
    let template = resources
        .save(
            actor,
            resource(
                "Invitation",
                Content::Template(Template {
                    subject: "Hello {{.FirstName}}".into(),
                    html: format!("<p>{}</p>", "é".repeat(40_000)),
                    text: "Hello {{.FirstName}}".into(),
                    attachments: vec![Attachment {
                        name: "guide.txt".into(),
                        mime_type: "text/plain".into(),
                        content: "SGVsbG8=".into(),
                    }],
                    ..Template::default()
                }),
            ),
            None,
            None,
        )
        .await
        .unwrap();
    let page = resources.save(actor,resource("Welcome",Content::Page(Page {
        html:"<h1>Hello {{.FirstName}}</h1><form><input name='password' type='password'></form>".into(),..Page::default()
    })),None,None).await.unwrap();
    let profile = resources
        .save(
            actor,
            resource(
                "Local SMTP",
                Content::Profile(Profile {
                    interface_type: "SMTP".into(),
                    host: "127.0.0.1:2525".into(),
                    from_address: "training@example.test".into(),
                    headers: vec![Header {
                        key: "X-Training".into(),
                        value: "yes".into(),
                    }],
                    ..Profile::default()
                }),
            ),
            None,
            None,
        )
        .await
        .unwrap();
    let profile = profile_credentials(&resources, actor, normal.identity, profile).await;
    let mut change = group.clone();
    change.name = "Engineering updated".into();
    let (a, b) = tokio::join!(
        resources.save(
            actor,
            change.clone().into(),
            Some(group.id),
            Some(group.modified_date.clone())
        ),
        resources.save(
            actor,
            change.into(),
            Some(group.id),
            Some(group.modified_date.clone())
        )
    );
    assert_eq!(
        usize::from(a.is_ok()) + usize::from(b.is_ok()),
        1,
        "stale editors cannot both win"
    );
    assert!(matches!(
        a.as_ref().err().or(b.as_ref().err()),
        Some(Error::Changed)
    ));
    let group = a.or(b).unwrap();
    assert_ne!(
        group.modified_date,
        resources
            .get(actor, Kind::Template, template.id)
            .await
            .unwrap()
            .modified_date
    );
    let mut now = 1_800_000_000;
    let domains = db.domains();
    let domain = domains
        .save(
            actor,
            None,
            domains::Edit {
                name: "Training".into(),
                origin: "https://training.example.test".into(),
                enabled: true,
                revision: None,
            },
            now,
        )
        .await
        .unwrap();
    let (a, b) = tokio::join!(domains.claim(now), domains.claim(now));
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(
        usize::from(a.is_some()) + usize::from(b.is_some()),
        1,
        "one verification lease per domain"
    );
    let claim = a.or(b).unwrap();
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
            now,
        )
        .await
        .unwrap();
    assert!(domains.get(actor, domain.id).await.unwrap().verified_until > now);
    let selection = |r: &Resource| campaign::Selection {
        id: r.id,
        revision: Some(r.modified_date.clone()),
    };
    let input = campaign::Create {
        name: "Provider acceptance".into(),
        groups: vec![selection(&group)],
        template: selection(&template),
        page: selection(&page),
        profile: selection(&profile),
        url: "https://training.example.test".into(),
        idempotency: Some(("launch-once".into(), "request-fingerprint".into())),
        ..campaign::Create::default()
    };
    campaign_origins::exercise(db, actor, input.clone(), now).await;
    open_tracking::exercise(db, actor, input.clone(), now).await;
    client_observations::exercise(db, actor, input.clone(), now).await;
    learning_pages::exercise(db, actor, normal.identity, input.clone(), now).await;
    campaign_drafts::exercise(db, actor, normal.identity, input.clone(), now, fault).await;
    let campaigns = db.campaigns();
    let review = campaigns.prepare(actor, &input, now).await.unwrap();
    assert_eq!(review.recipients.len(), 1);
    assert_eq!(review.template.id, template.id);
    assert_eq!(review.page.id, page.id);
    assert_eq!(review.profile.id, profile.id);
    assert!(
        matches!(
            campaigns.prepare(normal.identity, &input, now).await,
            Err(Error::NotFound(_))
        ),
        "resource IDs never bypass owner-scoped selection"
    );
    let mut stale = input.clone();
    stale.template.revision = Some("unreviewed-revision".into());
    assert!(matches!(
        campaigns.create(actor, stale, now).await,
        Err(Error::Changed)
    ));
    let mut missing = input.clone();
    missing.template.id = i64::MAX;
    missing.template.revision = None;
    assert!(matches!(
        campaigns.prepare(actor, &missing, now).await,
        Err(Error::NotFound(_))
    ));
    assert!(
        campaigns.list(actor).await.unwrap().is_empty(),
        "review must not persist a campaign"
    );
    if let Some(fault) = fault {
        fault.store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(matches!(
            campaigns.create(actor, input.clone(), now).await,
            Err(Error::Storage(
                ruphish_application::storage::Error::UncertainCommit
            ))
        ));
        assert!(
            !fault.load(std::sync::atomic::Ordering::SeqCst),
            "fault must observe a real COMMIT acknowledgement"
        );
        assert_eq!(
            campaigns.list(actor).await.unwrap().len(),
            1,
            "the committed launch survives its lost response"
        );
    }
    let (a, b) = tokio::join!(
        campaigns.create(actor, input.clone(), now),
        campaigns.create(actor, input.clone(), now)
    );
    let campaign = a.unwrap();
    assert_eq!(
        campaign.id,
        b.unwrap().id,
        "idempotent launch returns the same campaign"
    );
    assert_eq!(campaign.results.len(), 1);
    reporting_filters::contract(db, actor, campaign.id, campaign.results[0].id).await;
    assert!(matches!(
        resources
            .delete(actor, Kind::Template, template.id, None)
            .await,
        Err(Error::InUse)
    ));
    let (left, right) = tokio::join!(campaigns.claim(now), campaigns.claim(now));
    let left = left.unwrap();
    let right = right.unwrap();
    assert_eq!(
        usize::from(left.is_some()) + usize::from(right.is_some()),
        1,
        "competing delivery workers lease one recipient only once"
    );
    let abandoned = left.or(right).unwrap();
    assert!(campaigns.release(&abandoned, now).await.unwrap());
    let expired = campaigns.claim(now).await.unwrap().unwrap();
    assert!(
        !campaigns.arm(&abandoned, now).await.unwrap(),
        "released lease is fenced"
    );
    now += 901;
    let claim = campaigns.claim(now).await.unwrap().unwrap();
    assert!(
        claim.generation > expired.generation,
        "abandoned preparation is safely reclaimed"
    );
    assert!(!campaigns.arm(&expired, now).await.unwrap());
    campaigns.finish(&expired, Ok(()), now).await.unwrap();
    assert_eq!(
        campaigns
            .deliveries(actor, campaign.id, 0, 50)
            .await
            .unwrap()
            .items[0]
            .state
            .as_str(),
        "preparing",
        "late completion cannot overwrite the new lease"
    );
    assert!(campaigns.arm(&claim, now).await.unwrap());
    if let Some(fault) = fault {
        fault.store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(matches!(
            campaigns.finish(&claim, Ok(()), now).await,
            Err(Error::Storage(
                ruphish_application::storage::Error::UncertainCommit
            ))
        ));
        assert!(!fault.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(
            campaigns
                .deliveries(actor, campaign.id, 0, 50)
                .await
                .unwrap()
                .items[0]
                .state
                .as_str(),
            "accepted"
        );
        assert!(
            campaigns.claim(now).await.unwrap().is_none(),
            "accepted SMTP work must never be reclaimed for resending"
        );
    }
    let (left, right) = tokio::join!(
        campaigns.finish(&claim, Ok(()), now),
        campaigns.finish(&claim, Ok(()), now)
    );
    left.unwrap();
    right.unwrap();
    let public = db.interactions();
    let rid = &campaign.results[0].rid;
    public
        .interact(
            "https://training.example.test",
            rid,
            interactions::Action::Open,
            now + 1,
        )
        .await
        .unwrap();
    public
        .interact(
            "https://training.example.test",
            rid,
            interactions::Action::View {
                client: Default::default(),
            },
            now + 2,
        )
        .await
        .unwrap();
    let submit = || interactions::Action::Submit {
        client: Default::default(),
        event_id: "a".repeat(32),
        password_present: true,
    };
    let (a, b) = tokio::join!(
        public.interact("https://training.example.test", rid, submit(), now + 3),
        public.interact("https://training.example.test", rid, submit(), now + 3)
    );
    a.unwrap();
    b.unwrap();
    let context = campaigns.header(actor, campaign.id).await.unwrap();
    assert_eq!(context.id, campaign.id);
    assert_eq!(context.total, 1);
    let overview = db.overview().snapshot(actor, now).await.unwrap();
    assert!(overview.attention.campaign_ids.len() <= 10);
    let analysis = db
        .reporting()
        .analysis(
            actor,
            campaign.id,
            reporting::Filter {
                limit: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        analysis.totals,
        [1, 1, 1, 1, 0],
        "transport retries are acknowledged without duplicating events"
    );
    assert_eq!(analysis.progression, [1, 1, 1, 1, 0]);
    assert_eq!(analysis.recipients.len(), 1);
    assert_eq!(
        analysis
            .events
            .iter()
            .filter(|e| e.password_present == Some(true))
            .count(),
        1
    );
    let filter = || reporting::Filter {
        recipients: vec![claim.result_id],
        from: Some(now + 3),
        until: Some(now + 4),
        event: vec![ruphish_domain::events::Kind::FormSubmitted],
        source: Some(ruphish_domain::reporting::Source::Public),
        search: "EXAMPLE.TEST".into(),
        limit: 50,
        ..Default::default()
    };
    let filtered = db
        .reporting()
        .analysis(actor, campaign.id, filter())
        .await
        .unwrap();
    assert_eq!(filtered.totals, [0, 0, 0, 1, 0]);
    assert_eq!(filtered.recipients.len(), 1);
    assert_eq!(filtered.events.len(), 1);
    let recipient_csv = db
        .reporting()
        .export(actor, campaign.id, filter(), "csv", "recipients")
        .await
        .unwrap();
    let mut reader = csv::Reader::from_reader(recipient_csv.bytes.as_slice());
    let headers = reader.headers().unwrap().clone();
    let row = reader.records().next().unwrap().unwrap();
    let field = |name: &str| &row[headers.iter().position(|h| h == name).unwrap()];
    assert_eq!(field("First Name"), "Élise");
    assert_eq!(field("Last Name"), "Müller");
    assert_eq!(field("Position"), "'=1+1");
    assert_eq!(
        field("Send Date (UTC)"),
        ruphish_application::datetime::Timestamp::from_second(now)
            .unwrap()
            .to_string()
    );
    assert_eq!(
        field("Modified Date (UTC)"),
        ruphish_application::datetime::Timestamp::from_second(now + 3)
            .unwrap()
            .to_string()
    );
    let mut invalid = filter();
    invalid.recipients = vec![i64::MAX];
    assert!(matches!(
        db.reporting().analysis(actor, campaign.id, invalid).await,
        Err(Error::NotFound("Recipient"))
    ));
    let csv = db
        .reporting()
        .export(actor, campaign.id, filter(), "csv", "events")
        .await
        .unwrap();
    let csv = String::from_utf8(csv.bytes).unwrap();
    assert!(csv.contains("Submitted Data"));
    assert!(!csv.contains("fixture-mail-secret"));
    let excel = db
        .reporting()
        .export(actor, campaign.id, filter(), "xlsx", "recipients")
        .await
        .unwrap();
    assert!(excel.bytes.starts_with(b"PK"));
    for action in [
        interactions::Action::Open,
        interactions::Action::View {
            client: Default::default(),
        },
        interactions::Action::Submit {
            client: Default::default(),
            event_id: "b".repeat(32),
            password_present: false,
        },
        interactions::Action::Submit {
            client: Default::default(),
            event_id: "c".repeat(32),
            password_present: true,
        },
    ] {
        public
            .interact("https://training.example.test", rid, action, now + 5)
            .await
            .unwrap();
    }
    let repeated = db
        .reporting()
        .analysis(
            actor,
            campaign.id,
            reporting::Filter {
                limit: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(repeated.totals, [1, 2, 2, 3, 0]);
    assert_eq!(repeated.unique, [1, 1, 1, 1, 0]);
    assert_eq!(repeated.progression, [1, 1, 1, 1, 0]);
    let recipient = db
        .reporting()
        .analysis(
            actor,
            campaign.id,
            reporting::Filter {
                recipient: Some(claim.result_id),
                limit: 50,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(recipient.totals, [1, 2, 2, 3, 0]);
    let history = campaigns
        .recipient_history(actor, campaign.id, claim.result_id, None, None)
        .await
        .unwrap();
    assert_eq!(
        history
            .events
            .iter()
            .filter(|event| event.message == "Submitted Data")
            .count(),
        3
    );
    let window = db
        .overview()
        .posture(
            actor,
            ruphish_application::overview::Window::Last30Days,
            now + 10,
        )
        .await
        .unwrap();
    assert_eq!(window.counts.delivered, 1);
    assert_eq!(window.previous.unwrap().counts.delivered, 0);
    integrations.deliver_webhooks().await.unwrap();
    let hook = integrations.webhook(actor, hook.id).await.unwrap();
    assert!(!hook.history.is_empty());
    assert!(hook.history.iter().all(|a| a.finished.is_some()));
    let (a, b) = tokio::join!(
        integrations.save_webhook(actor, Some(hook.id), hook_edit(Some(hook.revision))),
        integrations.save_webhook(actor, Some(hook.id), hook_edit(Some(hook.revision)))
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(matches!(
        a.as_ref().err().or(b.as_ref().err()),
        Some(Error::Changed)
    ));
    let hook = integrations.webhook(actor, hook.id).await.unwrap();
    assert!(matches!(
        integrations
            .delete_webhook(actor, hook.id, Some(hook.revision + 1))
            .await,
        Err(Error::Changed)
    ));
    assert!(integrations.webhook(actor, hook.id).await.is_ok());
    assert!(matches!(
        integrations
            .delete_mailbox(actor, mailbox.id, Some(mailbox.revision + 1))
            .await,
        Err(Error::Changed)
    ));
    assert!(integrations.mailbox(actor, mailbox.id).await.is_ok());
    integrations
        .delete_webhook(actor, hook.id, Some(hook.revision))
        .await
        .unwrap();
    assert!(integrations.webhook(actor, hook.id).await.is_err());
    integrations
        .delete_mailbox(actor, mailbox.id, Some(mailbox.revision))
        .await
        .unwrap();
    assert!(integrations.mailbox(actor, mailbox.id).await.is_err());
    assert!(matches!(
        integrations
            .delete_webhook(actor, hook.id, Some(hook.revision))
            .await,
        Err(Error::NotFound("Webhook"))
    ));
    assert!(matches!(
        integrations.delete_mailbox(actor, mailbox.id, None).await,
        Err(Error::NotFound("Reporting mailbox"))
    ));
    let legacy_hook = integrations
        .save_webhook(actor, None, hook_edit(None))
        .await
        .unwrap();
    integrations
        .delete_webhook(actor, legacy_hook.id, None)
        .await
        .unwrap();
    let legacy_mailbox = integrations
        .save_mailbox(actor, None, mail_edit(None))
        .await
        .unwrap();
    integrations
        .delete_mailbox(actor, legacy_mailbox.id, None)
        .await
        .unwrap();

    let metrics = db
        .overview()
        .posture(actor, ruphish_application::overview::Window::All, now + 10)
        .await
        .unwrap();
    assert_eq!(metrics.counts.delivered, 1);
    assert_eq!(metrics.counts.credentials_entered, 1);
    assert_eq!(
        db.overview()
            .snapshot(actor, now + 10)
            .await
            .unwrap()
            .recent
            .len(),
        1
    );
    campaigns
        .complete(actor, campaign.id, now + 11)
        .await
        .unwrap();
    assert!(
        public
            .interact(
                "https://training.example.test",
                rid,
                interactions::Action::Open,
                now + 12
            )
            .await
            .is_err()
    );
    assert_eq!(
        db.reporting()
            .analysis(
                actor,
                campaign.id,
                reporting::Filter {
                    limit: 50,
                    ..Default::default()
                }
            )
            .await
            .unwrap()
            .totals,
        [1, 2, 2, 3, 0]
    );
    let mut uncertain_input = input;
    uncertain_input.name = "Uncertain submission".into();
    uncertain_input.idempotency = None;
    let uncertain = campaigns
        .create(actor, uncertain_input, now + 20)
        .await
        .unwrap();
    let lease = campaigns.claim(now + 20).await.unwrap().unwrap();
    assert!(campaigns.arm(&lease, now + 20).await.unwrap());
    assert!(
        campaigns.claim(now + 1222).await.unwrap().is_none(),
        "expired submission must never be automatically resent"
    );
    let state = campaigns
        .deliveries(actor, uncertain.id, 0, 50)
        .await
        .unwrap();
    assert_eq!(state.items[0].state.as_str(), "unknown");
    assert!(
        campaigns
            .resolve(
                actor,
                uncertain.id,
                state.items[0].id,
                state.items[0].generation + 1,
                "resend_unknown",
                now + 1224
            )
            .await
            .is_err()
    );
    campaigns
        .resolve(
            actor,
            uncertain.id,
            state.items[0].id,
            state.items[0].generation,
            "resend_unknown",
            now + 1224,
        )
        .await
        .unwrap();
    campaigns.finish(&lease, Ok(()), now + 1224).await.unwrap();
    assert_eq!(
        campaigns
            .deliveries(actor, uncertain.id, 0, 50)
            .await
            .unwrap()
            .items[0]
            .state
            .as_str(),
        "ready",
        "an outcome from the pre-reconciliation generation cannot overwrite the operator decision"
    );
    let retry = campaigns.claim(now + 1224).await.unwrap().unwrap();
    assert!(campaigns.arm(&retry, now + 1224).await.unwrap());
    campaigns.finish(&retry, Ok(()), now + 1224).await.unwrap();
    let history = campaigns
        .recipient_history(actor, uncertain.id, retry.result_id, None, None)
        .await
        .unwrap();
    assert_eq!(history.attempts.len(), 2);
    assert_eq!(history.attempts[0].state.as_str(), "accepted");
    assert_eq!(history.attempts[1].state.as_str(), "unknown");
    campaigns.delete(actor, uncertain.id).await.unwrap();
    assert!(campaigns.get(actor, uncertain.id).await.is_err());
}

#[tokio::test]
async fn sqlite_provider_contract() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");
    let db = database::open(Config::Sqlite { path: path.clone() }, Policy::default())
        .await
        .unwrap();
    workflows(&db).await;
    let peer = database::open(Config::Sqlite { path: path.clone() }, Policy::default())
        .await
        .unwrap();
    last_admin_race(&db, &peer).await;
    peer.close().await;
    db.close().await;
    // Reopen populated release tables, preserving campaign policy and learning-page defaults.
    let mut previous = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(&path),
    )
    .await
    .unwrap();
    sqlx::raw_sql("UPDATE ruphish_campaign_policy SET redirect_url='https://example.test/preserved-after-upgrade';").execute(&mut previous).await.unwrap();
    previous.close().await.unwrap();
    let db = database::open(Config::Sqlite { path }, Policy::default())
        .await
        .unwrap();
    assert!(db.bootstrap(None, None).await.unwrap().is_none());
    let actor = db.accounts().by_name("admin").await.unwrap().identity;
    let upgraded = db.campaigns().list(actor).await.unwrap();
    assert_eq!(upgraded.len(), 1);
    assert_eq!(
        upgraded[0].policy.as_ref().unwrap().redirect_url,
        "https://example.test/preserved-after-upgrade"
    );
    assert_eq!(upgraded[0].policy.as_ref().unwrap().learning_page_id, None);
    assert!(
        db.resources()
            .list(actor, Kind::LearningPage)
            .await
            .unwrap()
            .is_empty()
    );

    let draft = db
        .campaigns()
        .draft(
            db.accounts().by_name("admin").await.unwrap().identity,
            "draft-survives-restart-01",
        )
        .await
        .unwrap();
    assert_eq!(draft.step, 2);
    assert_eq!(draft.configuration, Default::default());

    assert_eq!(
        db.campaigns()
            .list(db.accounts().by_name("admin").await.unwrap().identity)
            .await
            .unwrap()
            .len(),
        1
    );
    db.close().await;
}

#[tokio::test]
#[ignore = "Requires the canonical MySQL acceptance service and RUPHISH_MYSQL_TEST_PORT"]
async fn mysql_provider_contract() {
    let (mut inspector, config) = fixture().await;
    let (first, second) = tokio::join!(
        database::open(Config::MySql(config.clone()), Policy::default()),
        database::open(Config::MySql(config.clone()), Policy::default())
    );
    let db = first.unwrap();
    let peer = second.unwrap();
    workflows(&db).await;
    last_admin_race(&db, &peer).await;
    peer.close().await;
    db.close().await;
    // Reopen populated release tables, preserving campaign policy and learning-page defaults.
    sqlx::raw_sql("UPDATE ruphish_campaign_policy SET redirect_url='https://example.test/preserved-after-upgrade';").execute(&mut inspector).await.unwrap();
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    assert!(db.bootstrap(None, None).await.unwrap().is_none());
    let actor = db.accounts().by_name("admin").await.unwrap().identity;
    let upgraded = db.campaigns().list(actor).await.unwrap();
    assert_eq!(upgraded.len(), 1);
    assert_eq!(
        upgraded[0].policy.as_ref().unwrap().redirect_url,
        "https://example.test/preserved-after-upgrade"
    );
    assert_eq!(upgraded[0].policy.as_ref().unwrap().learning_page_id, None);
    assert!(
        db.resources()
            .list(actor, Kind::LearningPage)
            .await
            .unwrap()
            .is_empty()
    );

    let draft = db
        .campaigns()
        .draft(
            db.accounts().by_name("admin").await.unwrap().identity,
            "draft-survives-restart-01",
        )
        .await
        .unwrap();
    assert_eq!(draft.step, 2);
    assert_eq!(draft.configuration, Default::default());

    assert_eq!(
        db.campaigns()
            .list(db.accounts().by_name("admin").await.unwrap().identity)
            .await
            .unwrap()
            .len(),
        1
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

async fn fixture() -> (MySqlConnection, MySqlConfig) {
    let port = std::env::var("RUPHISH_MYSQL_TEST_PORT")
        .expect("Set RUPHISH_MYSQL_TEST_PORT")
        .parse()
        .unwrap();
    let ca =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/acceptance/data/mysql/ca.pem");
    let options = MySqlConnectOptions::new()
        .host("127.0.0.1")
        .port(port)
        .username("root")
        .password("ruphish-mysql-fixture-only")
        .ssl_mode(MySqlSslMode::VerifyIdentity)
        .ssl_ca(&ca);
    let mut inspector = MySqlConnection::connect_with(&options).await.unwrap();
    let mut random = [0; 12];
    getrandom::fill(&mut random).unwrap();
    let name = format!("ruphish_contract_{}", hex::encode(random));
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "CREATE DATABASE `{name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_0900_bin; USE `{name}`"
    )))
    .execute(&mut inspector)
    .await
    .unwrap();
    let mut config = MySqlConfig::new(
        "127.0.0.1".into(),
        name,
        "root".into(),
        "ruphish-mysql-fixture-only".into(),
    );
    config.port = port;
    config.ca_path = Some(ca);
    (inspector, config)
}

async fn last_admin_race(db: &Database, peer: &Database) {
    let peer_accounts = peer.accounts();
    let accounts = db.accounts();
    let admin = accounts.by_name("admin").await.unwrap();
    let edit = |name: &str, role: &str| auth::Edit {
        username: name.into(),
        password: String::new(),
        role: role.into(),
        password_change_required: false,
        account_locked: false,
    };
    let second = accounts
        .write(
            admin.identity,
            auth::Write::Save {
                target: None,
                edit: edit("second-admin", "admin"),
                hash: Some(admin.hash.clone()),
            },
        )
        .await
        .unwrap()
        .unwrap();
    let (a, b) = tokio::join!(
        accounts.write(
            admin.identity,
            auth::Write::Save {
                target: Some(admin.identity),
                edit: edit("admin", "user"),
                hash: None
            }
        ),
        peer_accounts.write(
            second.identity,
            auth::Write::Save {
                target: Some(second.identity),
                edit: edit("second-admin", "user"),
                hash: None
            }
        )
    );
    assert_eq!(
        usize::from(a.is_ok()) + usize::from(b.is_ok()),
        1,
        "two demotions must not remove the last administrator"
    );
    let first = accounts.by_name("admin").await.unwrap();
    let second = accounts.by_name("second-admin").await.unwrap();
    assert_eq!(
        usize::from(first.role.slug == "admin") + usize::from(second.role.slug == "admin"),
        1
    );
}

async fn replacement_fixture(db: &Database) -> (auth::Identity, Resource) {
    db.bootstrap(Some("Atomic fixture password 42!".into()), None)
        .await
        .unwrap();
    let user = db.accounts().by_name("admin").await.unwrap();
    let user = db
        .accounts()
        .write(
            user.identity,
            auth::Write::Password {
                expected_hash: user.hash.clone(),
                hash: user.hash,
            },
        )
        .await
        .unwrap()
        .unwrap();
    let original = db
        .resources()
        .save(
            user.identity,
            resource(
                "Original",
                Content::Template(Template {
                    text: "Original body".into(),
                    attachments: vec![Attachment {
                        name: "original.txt".into(),
                        mime_type: "text/plain".into(),
                        content: "YQ==".into(),
                    }],
                    ..Default::default()
                }),
            ),
            None,
            None,
        )
        .await
        .unwrap();
    (user.identity, original)
}

async fn assert_failed_replacement_is_atomic(
    db: &Database,
    actor: auth::Identity,
    original: Resource,
) {
    let mut edit = original.clone();
    edit.name = "Changed parent".into();
    edit.content = Content::Template(Template {
        text: "Changed body".into(),
        attachments: vec![
            Attachment {
                name: "accepted.txt".into(),
                mime_type: "text/plain".into(),
                content: "Yg==".into(),
            },
            Attachment {
                name: "rejected.txt".into(),
                mime_type: "text/plain".into(),
                content: "cmVqZWN0".into(),
            },
        ],
        ..Default::default()
    });
    assert!(
        db.resources()
            .save(
                actor,
                edit.into(),
                Some(original.id),
                Some(original.modified_date.clone())
            )
            .await
            .is_err()
    );
    let actual = db
        .resources()
        .get(actor, Kind::Template, original.id)
        .await
        .unwrap();
    assert!(
        actual == original,
        "failure after parent update and child deletion must roll back the entire replacement, including its revision"
    );
    let mut next = original.clone();
    next.name = "Subsequent successful edit".into();
    assert!(
        db.resources()
            .save(
                actor,
                next.into(),
                Some(original.id),
                Some(original.modified_date)
            )
            .await
            .is_ok(),
        "a failed transaction must release its locks"
    );
}

#[tokio::test]
async fn sqlite_rolls_back_partial_resource_replacement() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("state.db");
    let db = database::open(Config::Sqlite { path: path.clone() }, Policy::default())
        .await
        .unwrap();
    let (actor, original) = replacement_fixture(&db).await;
    let mut inspector = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(path),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TRIGGER fixture_reject_attachment BEFORE INSERT ON attachments WHEN NEW.content='cmVqZWN0' BEGIN SELECT RAISE(ABORT,'fixture rejection'); END;").execute(&mut inspector).await.unwrap();
    assert_failed_replacement_is_atomic(&db, actor, original).await;
    db.close().await;
}

#[tokio::test]
#[ignore = "Requires the canonical MySQL acceptance service and RUPHISH_MYSQL_TEST_PORT"]
async fn mysql_rolls_back_partial_resource_replacement() {
    let (mut inspector, config) = fixture().await;
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    let (actor, original) = replacement_fixture(&db).await;
    sqlx::raw_sql("CREATE TRIGGER fixture_reject_attachment BEFORE INSERT ON attachments FOR EACH ROW BEGIN IF NEW.content='cmVqZWN0' THEN SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT='fixture rejection'; END IF; END").execute(&mut inspector).await.unwrap();
    assert_failed_replacement_is_atomic(&db, actor, original).await;
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
#[ignore = "Requires the canonical MySQL acceptance service and RUPHISH_MYSQL_TEST_PORT"]
async fn mysql_upgrades_every_legacy_version_preserving_identity_and_recoverable_originals() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    let mut scripts: Vec<_> = std::fs::read_dir(root.join("mysql-legacy"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "sql"))
        .collect();
    scripts.sort();
    let password: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("gophish-v0.12.1/password.json")).unwrap())
            .unwrap();
    for count in 1..=scripts.len() {
        let (mut inspector, mut config) = fixture().await;
        config.legacy_timezone = Some("+02:00".into());
        sqlx::raw_sql("SET SESSION time_zone='+00:00'; CREATE TABLE goose_db_version(id BIGINT PRIMARY KEY AUTO_INCREMENT,version_id BIGINT NOT NULL,is_applied BOOLEAN NOT NULL,tstamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP); INSERT INTO goose_db_version(version_id,is_applied) VALUES(0,1)").execute(&mut inspector).await.unwrap();
        for script in &scripts[..count] {
            let version: i64 = script
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .split('_')
                .next()
                .unwrap()
                .parse()
                .unwrap();
            let source = std::fs::read_to_string(script).unwrap();
            let up = source
                .split("-- +goose Up")
                .nth(1)
                .unwrap()
                .split("-- +goose Down")
                .next()
                .unwrap();
            sqlx::raw_sql(sqlx::AssertSqlSafe(up))
                .execute(&mut inspector)
                .await
                .unwrap();
            sqlx::query("INSERT INTO goose_db_version(version_id,is_applied) VALUES(?,1)")
                .bind(version)
                .execute(&mut inspector)
                .await
                .unwrap();
        }
        let version: i64 = scripts[count - 1]
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .split('_')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        sqlx::query("INSERT INTO users(id,username,hash,api_key) VALUES(17,'legacy-admin',?,'legacy-api-key')").bind(password["bcrypt_hash"].as_str().unwrap()).execute(&mut inspector).await.unwrap();
        if version >= 20190105192341 {
            sqlx::query("UPDATE users SET role_id=(SELECT id FROM roles WHERE slug='admin')")
                .execute(&mut inspector)
                .await
                .unwrap();
        }
        sqlx::raw_sql("INSERT INTO templates(id,user_id,name,subject,text,html,modified_date) VALUES(9,17,'Legacy email','Hello','Plain text','<h1>Hello</h1>','2020-01-01 00:00:00'); INSERT INTO pages(id,user_id,name,html,modified_date) VALUES(10,17,'Legacy page','<h1>Welcome</h1>','2020-01-01 00:00:00'); INSERT INTO `groups`(id,user_id,name,modified_date) VALUES(21,17,'Shared group','2020-01-01 00:00:00'),(22,17,'Shared group','2020-01-01 00:00:00'); INSERT INTO targets(id,first_name,last_name,email,position) VALUES(31,'Elise','','elise@example.test','Engineer'); INSERT INTO group_targets VALUES(21,31),(22,31); INSERT INTO campaigns(id,user_id,name,created_date,completed_date,template_id,page_id,status,url) VALUES(13,17,'Legacy campaign','2020-01-01 00:00:00',NULL,9,10,'In progress','https://training.example.test'); INSERT INTO results(id,campaign_id,user_id,r_id,email,first_name,last_name,status,ip,latitude,longitude) VALUES(41,13,17,'legacy-rid','elise@example.test','Elise','','Email Opened','192.0.2.99',51.5,8.5); INSERT INTO events(id,campaign_id,email,time,message) VALUES(51,13,'elise@example.test','2020-01-01 00:00:00','Email Opened')").execute(&mut inspector).await.unwrap();
        let imported_origin = [
            "https://training.example.test",
            "http://training.example.test:8080",
            "https://192.0.2.20:8443",
            "http://[2001:db8::20]:8080",
        ][count % 4];
        let imported_url = format!("{imported_origin}/training?source=legacy");
        sqlx::query("UPDATE campaigns SET url=? WHERE id=13")
            .bind(&imported_url)
            .execute(&mut inspector)
            .await
            .unwrap();
        if version >= 20160131153104 {
            sqlx::query("UPDATE events SET details=?")
                .bind("credential-value-canary")
                .execute(&mut inspector)
                .await
                .unwrap();
        }
        if version < 20160227180335 {
            sqlx::query("INSERT INTO smtp(smtp_id,campaign_id,host,username,from_address) VALUES(11,13,'smtp.example.test','service-account','training@example.test')").execute(&mut inspector).await.unwrap();
        } else {
            sqlx::raw_sql("INSERT INTO smtp(id,user_id,name,interface_type,host,username,password,from_address,modified_date) VALUES(11,17,'Legacy SMTP','SMTP','smtp.example.test','service-account','service-secret','training@example.test','2020-01-01 00:00:00'); UPDATE campaigns SET smtp_id=11").execute(&mut inspector).await.unwrap();
        }
        if version < 20170828220440 {
            let mut ambiguous = config.clone();
            ambiguous.legacy_timezone = None;
            assert!(
                database::open(Config::MySql(ambiguous), Policy::default())
                    .await
                    .is_err(),
                "version {version}: never guess a pre-UTC timezone"
            );
        }
        let db = database::open(Config::MySql(config.clone()), Policy::default())
            .await
            .unwrap_or_else(|e| panic!("upgrade version {version}: {e}"));
        let track_opens: bool = sqlx::query_scalar(
            "SELECT track_opens FROM ruphish_campaign_policy WHERE campaign_id=13",
        )
        .fetch_one(&mut inspector)
        .await
        .unwrap();
        assert!(
            track_opens,
            "migrated {version} defaults to enabled without authored tracker markup"
        );
        let client_count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM events WHERE client IS NOT NULL")
                .fetch_one(&mut inspector)
                .await
                .unwrap();
        assert_eq!(
            client_count, 0,
            "historical {version} has no collected client categories"
        );
        assert!(db.bootstrap(None, None).await.unwrap().is_none());

        let user = db.accounts().by_name("legacy-admin").await.unwrap();
        assert_eq!(user.identity.id, 17);
        assert_eq!(user.hash, password["bcrypt_hash"].as_str().unwrap());
        use auth::Passwords;
        assert!(
            ruphish_adapters::password_worker::PasswordWorker::default()
                .verify(password["plaintext"].as_str().unwrap().into(), user.hash)
                .await
                .unwrap()
        );
        assert_eq!(
            db.resources()
                .list(user.identity, Kind::Group)
                .await
                .unwrap()
                .len(),
            2,
            "legacy duplicate names are preserved"
        );
        let group = db
            .resources()
            .get(user.identity, Kind::Group, 21)
            .await
            .unwrap();
        let Content::Group(targets) = group.content else {
            panic!("group content")
        };
        assert_eq!(targets[0].email, "elise@example.test");
        let campaign = db.campaigns().get(user.identity, 13).await.unwrap();
        assert_eq!(campaign.events[0].id, 51);
        assert_eq!(campaign.results[0].rid, "legacy-rid");
        assert!(campaign.events[0].details.is_empty());
        assert!(campaign.results[0].ip.is_empty());
        assert_eq!(campaign.results[0].latitude, 0.0);
        assert_eq!(
            campaign.created,
            if version < 20170828220440 {
                "2019-12-31T22:00:00Z"
            } else {
                "2020-01-01T00:00:00Z"
            }
        );
        let domains = db.domains().list(user.identity).await.unwrap();
        assert_eq!(domains.len(), 1);
        assert_eq!(domains[0].verified_until, 0);
        assert_eq!(domains[0].origin, imported_origin);
        assert_eq!(campaign.url, imported_url);
        assert!(
            domains[0].eligible_until > 0,
            "fresh migration policy does not require verification"
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT url FROM rp_original_campaigns WHERE id=13")
                .fetch_one(&mut inspector)
                .await
                .unwrap(),
            imported_url
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM ruphish_campaign_domains WHERE campaign_id=13"
            )
            .fetch_one(&mut inspector)
            .await
            .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT ip FROM rp_original_results WHERE id=41")
                .fetch_one(&mut inspector)
                .await
                .unwrap(),
            "192.0.2.99"
        );
        if version >= 20160131153104 {
            assert_eq!(
                sqlx::query_scalar::<_, Vec<u8>>(
                    "SELECT details FROM rp_original_events WHERE id=51"
                )
                .fetch_one(&mut inspector)
                .await
                .unwrap(),
                b"credential-value-canary"
            );
        }
        db.close().await;
        let db = database::open(Config::MySql(config.clone()), Policy::default())
            .await
            .unwrap();
        assert_eq!(
            db.campaigns().get(user.identity, 13).await.unwrap().created,
            campaign.created,
            "restart must not reapply timezone conversion"
        );
        db.close().await;
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "DROP DATABASE `{}`",
            config.database
        )))
        .execute(&mut inspector)
        .await
        .unwrap();
        println!("Upgraded and reopened legacy MySQL version {version}");
    }
}

async fn latest_legacy(c: &mut MySqlConnection) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/mysql-legacy");
    let mut scripts: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "sql"))
        .collect();
    scripts.sort();
    sqlx::raw_sql("CREATE TABLE goose_db_version(id BIGINT PRIMARY KEY AUTO_INCREMENT,version_id BIGINT NOT NULL,is_applied BOOLEAN NOT NULL,tstamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP); INSERT INTO goose_db_version(version_id,is_applied) VALUES(0,1)").execute(&mut *c).await.unwrap();
    for script in scripts {
        let version: i64 = script
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .split('_')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let source = std::fs::read_to_string(script).unwrap();
        let up = source
            .split("-- +goose Up")
            .nth(1)
            .unwrap()
            .split("-- +goose Down")
            .next()
            .unwrap();
        sqlx::raw_sql(sqlx::AssertSqlSafe(up))
            .execute(&mut *c)
            .await
            .unwrap();
        sqlx::query("INSERT INTO goose_db_version(version_id,is_applied) VALUES(?,1)")
            .bind(version)
            .execute(&mut *c)
            .await
            .unwrap();
    }
}

#[tokio::test]
#[ignore = "Requires the canonical MySQL acceptance service and RUPHISH_MYSQL_TEST_PORT"]
async fn mysql_failed_publication_resumes_without_losing_or_duplicating_legacy_data() {
    let (mut inspector, mut config) = fixture().await;
    latest_legacy(&mut inspector).await;
    sqlx::raw_sql("INSERT INTO users(id,username,hash,api_key,role_id) VALUES(17,'legacy','hash-preserved','key-preserved',1); INSERT INTO `groups`(id,user_id,name,modified_date) VALUES(21,17,'Recovered group','2020-01-01 00:00:00')").execute(&mut inspector).await.unwrap();
    let username = format!("u_{}", &config.database[17..]);
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE USER '{username}'@'%' IDENTIFIED BY 'fixture-migration-only'; GRANT SELECT,INSERT,UPDATE,DELETE,CREATE,DROP,INDEX,CREATE VIEW,SHOW VIEW ON `{}`.* TO '{username}'@'%'",config.database))).execute(&mut inspector).await.unwrap();
    config.username = username.clone();
    config.password = Some("fixture-migration-only".into());
    assert!(
        database::open(Config::MySql(config.clone()), Policy::default())
            .await
            .is_err(),
        "missing ALTER privilege must prevent publication"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT phase FROM _ruphish_upgrade")
            .fetch_one(&mut inspector)
            .await
            .unwrap(),
        "copied"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT name FROM `groups` WHERE id=21")
            .fetch_one(&mut inspector)
            .await
            .unwrap(),
        "Recovered group",
        "source is still live and untouched after failed atomic rename"
    );
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "GRANT ALTER ON `{}`.* TO '{username}'@'%'",
        config.database
    )))
    .execute(&mut inspector)
    .await
    .unwrap();
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    let actor = db.accounts().by_name("legacy").await.unwrap();
    assert_eq!(actor.hash, "hash-preserved");
    let groups = db
        .resources()
        .list(actor.identity, Kind::Group)
        .await
        .unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].id, 21);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM rp_original_groups WHERE id=21")
            .fetch_one(&mut inspector)
            .await
            .unwrap(),
        1
    );
    db.close().await;
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    assert!(db.bootstrap(None, None).await.unwrap().is_none());

    db.close().await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE `{}`; DROP USER '{username}'@'%'",
        config.database
    )))
    .execute(&mut inspector)
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "Requires the canonical MySQL acceptance service and RUPHISH_MYSQL_TEST_PORT"]
async fn mysql_rejects_schema_drift_dirty_history_and_unknown_versions_without_reinitializing() {
    let (mut inspector, config) = fixture().await;
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    let (actor, original) = replacement_fixture(&db).await;
    db.close().await;
    for change in [
        "UPDATE _sqlx_migrations SET success=0",
        "UPDATE _sqlx_migrations SET version=999 WHERE version=1",
        "UPDATE _sqlx_migrations SET checksum=UNHEX(REPEAT('00',48))",
    ] {
        sqlx::query("CREATE TEMPORARY TABLE fixture_history AS SELECT * FROM _sqlx_migrations")
            .execute(&mut inspector)
            .await
            .unwrap();
        sqlx::raw_sql(sqlx::AssertSqlSafe(change))
            .execute(&mut inspector)
            .await
            .unwrap();
        assert!(
            matches!(
                database::open(Config::MySql(config.clone()), Policy::default()).await,
                Err(database::Error::UnsupportedSchema)
            ),
            "{change}"
        );
        sqlx::raw_sql("DELETE FROM _sqlx_migrations; INSERT INTO _sqlx_migrations SELECT * FROM fixture_history; DROP TEMPORARY TABLE fixture_history").execute(&mut inspector).await.unwrap();
    }
    for (change, restore) in [
        (
            "ALTER TABLE ruphish_webhook_jobs ALTER COLUMN state SET DEFAULT 'accepted'",
            "ALTER TABLE ruphish_webhook_jobs ALTER COLUMN state SET DEFAULT 'queued'",
        ),
        (
            "ALTER TABLE templates MODIFY id BIGINT NOT NULL",
            "ALTER TABLE templates MODIFY id BIGINT NOT NULL AUTO_INCREMENT",
        ),
        (
            "ALTER TABLE ruphish_domain_checks MODIFY active_domain BIGINT GENERATED ALWAYS AS (NULL) STORED",
            "ALTER TABLE ruphish_domain_checks MODIFY active_domain BIGINT GENERATED ALWAYS AS (CASE WHEN finished_at IS NULL THEN domain_id ELSE NULL END) STORED",
        ),
        (
            "ALTER TABLE ruphish_delivery ALTER CHECK ruphish_delivery_chk_1 NOT ENFORCED",
            "ALTER TABLE ruphish_delivery ALTER CHECK ruphish_delivery_chk_1 ENFORCED",
        ),
        (
            "ALTER TABLE ruphish_delivery DROP CHECK ruphish_delivery_chk_2",
            "ALTER TABLE ruphish_delivery ADD CONSTRAINT ruphish_delivery_chk_2 CHECK (retries BETWEEN 0 AND 8)",
        ),
        (
            "ALTER TABLE ruphish_event_receipts DROP PRIMARY KEY",
            "ALTER TABLE ruphish_event_receipts ADD PRIMARY KEY(result_id,request_id)",
        ),
        (
            "ALTER TABLE users MODIFY api_key VARCHAR(255) COLLATE utf8mb4_0900_ai_ci NOT NULL",
            "ALTER TABLE users MODIFY api_key VARCHAR(255) COLLATE utf8mb4_0900_bin NOT NULL",
        ),
        (
            "ALTER TABLE group_targets ENGINE=MyISAM",
            "ALTER TABLE group_targets ENGINE=InnoDB",
        ),
        (
            "CREATE TABLE unexpected_table(id BIGINT)",
            "DROP TABLE unexpected_table",
        ),
        (
            "ALTER TABLE templates DROP COLUMN text",
            "ALTER TABLE templates ADD COLUMN text LONGTEXT AFTER subject",
        ),
        (
            "ALTER TABLE users DROP INDEX users_0",
            "CREATE UNIQUE INDEX users_0 ON users(username)",
        ),
    ] {
        sqlx::raw_sql(sqlx::AssertSqlSafe(change))
            .execute(&mut inspector)
            .await
            .unwrap();
        assert!(
            matches!(
                database::open(Config::MySql(config.clone()), Policy::default()).await,
                Err(database::Error::UnsupportedSchema)
            ),
            "{change}"
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(restore))
            .execute(&mut inspector)
            .await
            .unwrap();
    }
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    assert_eq!(
        db.accounts().by_name("admin").await.unwrap().identity,
        actor
    );
    assert_eq!(
        db.resources()
            .get(actor, Kind::Template, original.id)
            .await
            .unwrap()
            .name,
        original.name
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
#[ignore = "Requires the canonical MySQL acceptance service and RUPHISH_MYSQL_TEST_PORT"]
async fn mysql_legacy_queue_quarantines_ambiguity_and_retains_service_configuration() {
    let (mut inspector, config) = fixture().await;
    latest_legacy(&mut inspector).await;
    sqlx::raw_sql("INSERT INTO users(id,username,hash,api_key,role_id) VALUES(17,'legacy','preserved-hash','preserved-key',1); INSERT INTO pages(id,user_id,name,html,capture_credentials,capture_passwords,redirect_url) VALUES(10,17,'Page','<p>Training</p>',1,1,''); INSERT INTO campaigns(id,user_id,name,page_id,status,url) VALUES(13,17,'Queue',10,'In progress','https://training.example.test'); INSERT INTO results(id,campaign_id,user_id,r_id,email,status) VALUES(41,13,17,'pending','pending@example.test','Sending'),(42,13,17,'inflight','inflight@example.test','Sending'),(43,13,17,'duplicate','a@example.test','Sending'),(44,13,17,'duplicate','b@example.test','Sending'); INSERT INTO mail_logs(id,campaign_id,user_id,send_date,send_attempt,r_id,processing) VALUES(51,13,17,'2026-01-01 00:00:00',2,'pending',0),(52,13,17,'2026-01-01 01:00:00',3,'inflight',1),(53,13,17,'2026-01-01 02:00:00',0,'duplicate',0),(54,13,17,'2026-01-01 03:00:00',99,'missing',0); INSERT INTO imap(user_id,host,port,username,password,tls,enabled,folder,restrict_domain,delete_reported_campaign_email,imap_freq,modified_date,last_login) VALUES(17,'imap.example.test',993,'service','service-secret',1,1,'Reports','example.test',0,120,'2020-01-01 00:00:00','2020-01-02 00:00:00')").execute(&mut inspector).await.unwrap();
    sqlx::query("UPDATE imap SET ignore_cert_errors=1")
        .execute(&mut inspector)
        .await
        .unwrap();
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    let actor = db.accounts().by_name("legacy").await.unwrap().identity;
    let settings = db.settings().get(actor).await.unwrap();
    assert!(!settings.policy.verification_enabled);
    let mut policy = settings.policy;
    policy.verification_enabled = true;
    db.settings()
        .save(actor, settings.revision, policy, 1767236400)
        .await
        .unwrap();
    let deliveries = db.campaigns().deliveries(actor, 13, 0, 50).await.unwrap();
    assert_eq!(deliveries.items.len(), 2);
    assert_eq!(
        (
            deliveries.items[0].state.as_str(),
            deliveries.items[0].retries,
            deliveries.items[0].due
        ),
        ("ready", 2, 1767225600)
    );
    assert_eq!(
        (
            deliveries.items[1].state.as_str(),
            deliveries.items[1].retries
        ),
        ("unknown", 3)
    );
    assert_eq!(deliveries.items[1].code, "legacy_delivery_outcome_unknown");
    assert!(
        db.campaigns().claim(1767236400).await.unwrap().is_none(),
        "imported domains do not bypass verification"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM ruphish_legacy_queue_issues")
            .fetch_one(&mut inspector)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM rp_original_mail_logs")
            .fetch_one(&mut inspector)
            .await
            .unwrap(),
        4
    );
    let mailboxes = db.integrations().mailboxes(actor).await.unwrap();
    assert_eq!(mailboxes.len(), 1);
    let mailbox = &mailboxes[0];
    assert!(mailbox.enabled && mailbox.has_password && mailbox.tls && mailbox.ignore_cert_errors);
    assert_eq!(mailbox.folder, "Reports");
    assert_eq!(mailbox.frequency, 120);
    assert_eq!(mailbox.last_login, "2020-01-02T00:00:00Z");
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT password FROM ruphish_mailboxes")
            .fetch_one(&mut inspector)
            .await
            .unwrap(),
        "service-secret"
    );
    assert!(db.has_recovery_backup());
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
async fn postgres_provider_contract() {
    let (mut inspector, config) = postgres_fixture().await;
    let (first, second) = tokio::join!(
        database::open(Config::Postgres(config.clone()), Policy::default()),
        database::open(Config::Postgres(config.clone()), Policy::default())
    );
    let db = first.unwrap();
    let peer = second.unwrap();
    workflows(&db).await;
    last_admin_race(&db, &peer).await;
    peer.close().await;
    db.close().await;
    // Reopen populated release tables, preserving campaign policy and learning-page defaults.
    let mut previous = postgres_inspect(&config).await;
    sqlx::raw_sql("UPDATE ruphish_campaign_policy SET redirect_url='https://example.test/preserved-after-upgrade';").execute(&mut previous).await.unwrap();
    previous.close().await.unwrap();
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    assert!(db.bootstrap(None, None).await.unwrap().is_none());
    let actor = db.accounts().by_name("admin").await.unwrap().identity;
    let upgraded = db.campaigns().list(actor).await.unwrap();
    assert_eq!(upgraded.len(), 1);
    assert_eq!(
        upgraded[0].policy.as_ref().unwrap().redirect_url,
        "https://example.test/preserved-after-upgrade"
    );
    assert_eq!(upgraded[0].policy.as_ref().unwrap().learning_page_id, None);
    assert!(
        db.resources()
            .list(actor, Kind::LearningPage)
            .await
            .unwrap()
            .is_empty()
    );

    let draft = db
        .campaigns()
        .draft(
            db.accounts().by_name("admin").await.unwrap().identity,
            "draft-survives-restart-01",
        )
        .await
        .unwrap();
    assert_eq!(draft.step, 2);
    assert_eq!(draft.configuration, Default::default());

    assert_eq!(
        db.campaigns()
            .list(db.accounts().by_name("admin").await.unwrap().identity)
            .await
            .unwrap()
            .len(),
        1
    );
    db.close().await;
    postgres_cleanup(&mut inspector, &config).await;
}
async fn postgres_fixture() -> (sqlx::PgConnection, database::PostgresConfig) {
    let port = std::env::var("RUPHISH_POSTGRES_TEST_PORT")
        .expect("explicit PostgreSQL fixture port")
        .parse()
        .unwrap();
    let ca = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/acceptance/data/postgres/ca.pem")
        .canonicalize()
        .unwrap();
    let options = sqlx::postgres::PgConnectOptions::new_without_pgpass()
        .host("127.0.0.1")
        .port(port)
        .username("postgres")
        .password("ruphish-postgres-fixture-only")
        .database("postgres")
        .ssl_mode(sqlx::postgres::PgSslMode::VerifyFull)
        .ssl_root_cert(&ca);
    let mut inspector = sqlx::PgConnection::connect_with(&options).await.unwrap();
    let mut bytes = [0; 8];
    getrandom::fill(&mut bytes).unwrap();
    let name = format!("rp_{}", hex::encode(bytes));
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "CREATE ROLE {name} LOGIN PASSWORD 'synthetic-provider-password'"
    )))
    .execute(&mut inspector)
    .await
    .unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "CREATE DATABASE {name} OWNER {name} ENCODING 'UTF8'"
    )))
    .execute(&mut inspector)
    .await
    .unwrap();
    let mut config = database::PostgresConfig::new(
        "127.0.0.1".into(),
        name.clone(),
        name,
        "synthetic-provider-password".into(),
    );
    config.port = port;
    config.ca_path = Some(ca);
    config.lock_timeout_seconds = 2;
    (inspector, config)
}
async fn postgres_cleanup(c: &mut sqlx::PgConnection, config: &database::PostgresConfig) {
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE {} WITH (FORCE)",
        config.database
    )))
    .execute(&mut *c)
    .await
    .unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP ROLE {}",
        config.username
    )))
    .execute(c)
    .await
    .unwrap();
}

async fn postgres_inspect(config: &database::PostgresConfig) -> sqlx::PgConnection {
    sqlx::PgConnection::connect_with(
        &sqlx::postgres::PgConnectOptions::new_without_pgpass()
            .host(&config.host)
            .port(config.port)
            .database(&config.database)
            .username(&config.username)
            .password("synthetic-provider-password")
            .ssl_mode(sqlx::postgres::PgSslMode::VerifyFull)
            .ssl_root_cert(config.ca_path.as_ref().unwrap())
            .options([("search_path", "ruphish,pg_catalog")]),
    )
    .await
    .unwrap()
}
#[tokio::test]
#[ignore = "Requires canonical PostgreSQL fixture and RUPHISH_POSTGRES_TEST_PORT"]
async fn postgres_partial_write_rollback_and_pool_recovery() {
    let (mut admin, mut config) = postgres_fixture().await;
    config.max_connections = 1;
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    let (actor, original) = replacement_fixture(&db).await;
    let mut inspect = postgres_inspect(&config).await;
    sqlx::query(
        "ALTER TABLE attachments ADD CONSTRAINT test_reject CHECK (name <> 'rejected.txt')",
    )
    .execute(&mut inspect)
    .await
    .unwrap();
    assert_failed_replacement_is_atomic(&db, actor, original).await;
    // A failed statement aborts the transaction, but must not poison the next borrower.
    assert!(
        !db.resources()
            .list(actor, Kind::Template)
            .await
            .unwrap()
            .is_empty()
    );
    inspect.close().await.unwrap();
    db.close().await;
    postgres_cleanup(&mut admin, &config).await;
}
#[tokio::test]
#[ignore = "Requires canonical PostgreSQL fixture and RUPHISH_POSTGRES_TEST_PORT"]
async fn postgres_lock_timeout_does_not_write_or_poison_connection() {
    let (mut admin, mut config) = postgres_fixture().await;
    config.lock_timeout_seconds = 1;
    config.max_connections = 1;
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    let (actor, original) = replacement_fixture(&db).await;
    let mut inspect = postgres_inspect(&config).await;
    let mut held = inspect.begin().await.unwrap();
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(actor.id)
        .fetch_one(&mut *held)
        .await
        .unwrap();
    let mut edit = original.clone();
    edit.name = "Must not persist".into();

    let result = db
        .resources()
        .save(
            actor,
            edit.into(),
            Some(original.id),
            Some(original.modified_date.clone()),
        )
        .await;
    assert!(matches!(
        result,
        Err(Error::Storage(ruphish_application::storage::Error::Busy))
    ));

    held.rollback().await.unwrap();
    assert!(
        db.resources()
            .get(actor, Kind::Template, original.id)
            .await
            .unwrap()
            == original
    );
    inspect.close().await.unwrap();
    db.close().await;
    postgres_cleanup(&mut admin, &config).await;
}
#[tokio::test]
#[ignore = "Requires canonical PostgreSQL fixture and RUPHISH_POSTGRES_TEST_PORT"]
async fn postgres_rejects_modified_history_unknown_schema_and_bad_tls() {
    let (mut admin, config) = postgres_fixture().await;
    let mut invalid = config.clone();
    invalid.password = Some("wrong-fixture-password".into());
    assert!(
        database::open(Config::Postgres(invalid), Policy::default())
            .await
            .is_err()
    );
    let mut invalid = config.clone();
    invalid.ca_path = None;
    assert!(
        database::open(Config::Postgres(invalid), Policy::default())
            .await
            .is_err()
    );
    let mut inspect = postgres_inspect(&config).await;
    sqlx::query("CREATE TABLE public.unrelated(value text)")
        .execute(&mut inspect)
        .await
        .unwrap();
    sqlx::query("INSERT INTO public.unrelated VALUES('keep me')")
        .execute(&mut inspect)
        .await
        .unwrap();
    assert!(
        database::open(Config::Postgres(config.clone()), Policy::default())
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT value FROM public.unrelated")
            .fetch_one(&mut inspect)
            .await
            .unwrap(),
        "keep me"
    );
    sqlx::query("DROP TABLE public.unrelated")
        .execute(&mut inspect)
        .await
        .unwrap();
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    db.close().await;
    sqlx::query("UPDATE _sqlx_migrations SET checksum=decode('00','hex')")
        .execute(&mut inspect)
        .await
        .unwrap();
    assert!(
        database::open(Config::Postgres(config.clone()), Policy::default())
            .await
            .is_err()
    );
    inspect.close().await.unwrap();
    postgres_cleanup(&mut admin, &config).await;
}

#[tokio::test]
#[ignore = "Requires canonical PostgreSQL fixture and RUPHISH_POSTGRES_TEST_PORT"]
async fn postgres_schema_drift_is_rejected_and_restoration_recovers() {
    let (mut admin, config) = postgres_fixture().await;
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    db.close().await;
    let mut inspect = postgres_inspect(&config).await;
    for (damage, repair) in [
        (
            "ALTER TABLE users ADD COLUMN unexpected text",
            "ALTER TABLE users DROP COLUMN unexpected",
        ),
        (
            "DROP INDEX users_0",
            "CREATE UNIQUE INDEX users_0 ON users(username)",
        ),
        (
            "ALTER TABLE ruphish_delivery ALTER COLUMN generation SET DEFAULT 99",
            "ALTER TABLE ruphish_delivery ALTER COLUMN generation SET DEFAULT 0",
        ),
        (
            "ALTER TABLE users ALTER COLUMN username DROP NOT NULL",
            "ALTER TABLE users ALTER COLUMN username SET NOT NULL",
        ),
        (
            "UPDATE _sqlx_migrations SET success=FALSE",
            "UPDATE _sqlx_migrations SET success=TRUE",
        ),
        (
            "UPDATE _sqlx_migrations SET version=9999 WHERE version=1",
            "UPDATE _sqlx_migrations SET version=1 WHERE version=9999",
        ),
    ] {
        sqlx::raw_sql(sqlx::AssertSqlSafe(damage.to_owned()))
            .execute(&mut inspect)
            .await
            .unwrap();
        assert!(
            database::open(Config::Postgres(config.clone()), Policy::default())
                .await
                .is_err(),
            "schema drift was accepted: {damage}"
        );
        sqlx::raw_sql(sqlx::AssertSqlSafe(repair.to_owned()))
            .execute(&mut inspect)
            .await
            .unwrap();
        database::open(Config::Postgres(config.clone()), Policy::default())
            .await
            .unwrap()
            .close()
            .await;
    }
    inspect.close().await.unwrap();
    postgres_cleanup(&mut admin, &config).await;
}
#[tokio::test]
#[ignore = "Requires canonical PostgreSQL fixture and RUPHISH_POSTGRES_TEST_PORT"]
async fn postgres_failed_ddl_rolls_back_and_startup_releases_migration_lock() {
    let (mut admin, config) = postgres_fixture().await;
    let opts = sqlx::postgres::PgConnectOptions::new_without_pgpass()
        .host(&config.host)
        .port(config.port)
        .database(&config.database)
        .username("postgres")
        .password("ruphish-postgres-fixture-only")
        .ssl_mode(sqlx::postgres::PgSslMode::VerifyFull)
        .ssl_root_cert(config.ca_path.as_ref().unwrap());
    let mut inspect = sqlx::PgConnection::connect_with(&opts).await.unwrap();
    // Independent fault injection after the second application table is created.
    sqlx::raw_sql("CREATE FUNCTION public.reject_second_table() RETURNS event_trigger LANGUAGE plpgsql AS $$ BEGIN IF EXISTS(SELECT 1 FROM pg_event_trigger_ddl_commands() WHERE object_identity='ruphish.templates') THEN RAISE EXCEPTION 'fixture interruption'; END IF; END $$; CREATE EVENT TRIGGER fixture_ddl_failure ON ddl_command_end EXECUTE FUNCTION public.reject_second_table();").execute(&mut inspect).await.unwrap();
    assert!(
        database::open(Config::Postgres(config.clone()), Policy::default())
            .await
            .is_err()
    );
    let app_tables:i64=sqlx::query_scalar("SELECT count(*) FROM pg_class WHERE relnamespace='ruphish'::regnamespace AND relkind='r' AND relname<>'_sqlx_migrations'").fetch_one(&mut inspect).await.unwrap();
    assert_eq!(
        app_tables, 0,
        "transactional DDL must undo the earlier users table too"
    );
    sqlx::raw_sql(
        "DROP EVENT TRIGGER fixture_ddl_failure; DROP FUNCTION public.reject_second_table();",
    )
    .execute(&mut inspect)
    .await
    .unwrap();
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    assert!(
        db.bootstrap(Some("Recovery password 42!".into()), None)
            .await
            .unwrap()
            .is_some()
    );
    db.close().await;
    inspect.close().await.unwrap();
    postgres_cleanup(&mut admin, &config).await;
}
#[tokio::test]
#[ignore = "Requires canonical PostgreSQL fixture and RUPHISH_POSTGRES_TEST_PORT"]
async fn postgres_exact_revisions_and_invalid_text_preserve_original() {
    let (mut admin, config) = postgres_fixture().await;
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    let (actor, mut original) = replacement_fixture(&db).await;
    let mut inspect = postgres_inspect(&config).await;
    let revision = "2099-01-01T00:00:00.123456789Z";
    sqlx::query("UPDATE templates SET modified_date=$1 WHERE id=$2")
        .bind(revision)
        .bind(original.id)
        .execute(&mut inspect)
        .await
        .unwrap();
    original.modified_date = revision.into();
    let mut edit = original.clone();
    edit.name = "Next revision".into();
    let saved = db
        .resources()
        .save(actor, edit.into(), Some(original.id), Some(revision.into()))
        .await
        .unwrap();
    assert_eq!(saved.modified_date, "2099-01-01T00:00:00.12345679Z");
    assert!(matches!(
        db.resources()
            .save(
                actor,
                original.clone().into(),
                Some(original.id),
                Some(revision.into())
            )
            .await,
        Err(Error::Changed)
    ));
    let mut invalid = saved.clone();
    invalid.name = "bad\0name".into();
    assert!(
        matches!(db.resources().save(actor,invalid.into(),Some(saved.id),Some(saved.modified_date.clone())).await,Err(Error::Invalid(code)) if code.starts_with("storage_text_invalid:"))
    );
    assert!(
        db.resources()
            .get(actor, Kind::Template, saved.id)
            .await
            .unwrap()
            == saved
    );
    inspect.close().await.unwrap();
    db.close().await;
    postgres_cleanup(&mut admin, &config).await;
}

#[tokio::test]
#[ignore = "Requires canonical PostgreSQL fixture and RUPHISH_POSTGRES_TEST_PORT"]
async fn postgres_lost_commit_launch_and_delivery_are_reconciled_without_duplicates() {
    let (mut admin, config) = postgres_fixture().await;
    let proxy = postgres_proxy::CommitProxy::start(config.port).await;
    let mut through_proxy = config.clone();
    through_proxy.port = proxy.port;
    through_proxy.tls = database::PostgresTls::Disabled;
    through_proxy.ca_path = None;
    through_proxy.max_connections = 1;
    let db = database::open(Config::Postgres(through_proxy), Policy::default())
        .await
        .unwrap();
    workflows_with_commit_fault(&db, Some(&proxy.armed)).await;
    db.close().await;
    drop(proxy);
    postgres_cleanup(&mut admin, &config).await;
}

#[tokio::test]
#[ignore = "Requires canonical MySQL fixture and RUPHISH_MYSQL_TEST_PORT"]
async fn mysql_lost_commit_launch_and_delivery_are_reconciled_without_duplicates() {
    let (mut admin, config) = fixture().await;
    let proxy = mysql_proxy::CommitProxy::start(config.port).await;
    let mut through_proxy = config.clone();
    through_proxy.port = proxy.port;
    through_proxy.tls = database::MySqlTls::Disabled;
    through_proxy.ca_path = None;
    through_proxy.max_connections = 1;
    let db = database::open(Config::MySql(through_proxy), Policy::default())
        .await
        .unwrap();
    workflows_with_commit_fault(&db, Some(&proxy.armed)).await;
    db.close().await;
    drop(proxy);
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE `{}`",
        config.database
    )))
    .execute(&mut admin)
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "Requires canonical PostgreSQL fixture and RUPHISH_POSTGRES_TEST_PORT"]
async fn postgres_lost_commit_acknowledgement_is_uncertain_and_not_replayed() {
    let (mut admin, config) = postgres_fixture().await;
    let proxy = postgres_proxy::CommitProxy::start(config.port).await;
    let mut through_proxy = config.clone();
    through_proxy.port = proxy.port;
    through_proxy.tls = database::PostgresTls::Disabled;
    through_proxy.ca_path = None;
    through_proxy.max_connections = 1;
    let db = database::open(Config::Postgres(through_proxy), Policy::default())
        .await
        .unwrap();
    let (actor, original) = replacement_fixture(&db).await;
    let mut edit = original.clone();
    edit.name = "Committed without acknowledgement".into();
    proxy.armed.store(true, std::sync::atomic::Ordering::SeqCst);
    let result = db
        .resources()
        .save(
            actor,
            edit.clone().into(),
            Some(original.id),
            Some(original.modified_date.clone()),
        )
        .await;
    assert!(matches!(
        result,
        Err(Error::Storage(
            ruphish_application::storage::Error::UncertainCommit
        ))
    ));
    assert!(!proxy.armed.load(std::sync::atomic::Ordering::SeqCst));
    let mut inspect = postgres_inspect(&config).await;
    let stored: String = sqlx::query_scalar("SELECT name FROM templates WHERE id=$1")
        .bind(original.id)
        .fetch_one(&mut inspect)
        .await
        .unwrap();
    assert_eq!(stored, edit.name);
    assert!(matches!(
        db.resources()
            .save(
                actor,
                edit.into(),
                Some(original.id),
                Some(original.modified_date)
            )
            .await,
        Err(Error::Changed)
    ));
    assert_eq!(
        db.resources()
            .list(actor, Kind::Template)
            .await
            .unwrap()
            .len(),
        1
    );
    inspect.close().await.unwrap();
    db.close().await;
    drop(proxy);
    postgres_cleanup(&mut admin, &config).await;
}

#[tokio::test]
#[ignore = "Requires canonical PostgreSQL fixture and RUPHISH_POSTGRES_TEST_PORT"]
async fn postgres_migration_lock_is_bounded_and_cancellation_releases_connection() {
    use sqlx::migrate::Migrate;
    let (mut admin, mut config) = postgres_fixture().await;
    config.lock_timeout_seconds = 1;
    let mut inspect = postgres_inspect(&config).await;
    inspect.lock().await.unwrap();

    assert!(matches!(
        database::open(Config::Postgres(config.clone()), Policy::default()).await,
        Err(database::Error::Busy)
    ));

    config.lock_timeout_seconds = 30;
    let opening = config.clone();
    let pending =
        tokio::spawn(
            async move { database::open(Config::Postgres(opening), Policy::default()).await },
        );
    loop {
        let waiting: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND application_name='ruphish' AND wait_event='advisory')").fetch_one(&mut inspect).await.unwrap();
        if waiting {
            break;
        }
        tokio::task::yield_now().await;
    }
    pending.abort();
    assert!(matches!(pending.await, Err(error) if error.is_cancelled()));
    inspect.unlock().await.unwrap();
    config.lock_timeout_seconds = 1;
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    assert!(
        db.bootstrap(Some("Startup after cancellation 42!".into()), None)
            .await
            .unwrap()
            .is_some()
    );
    db.close().await;
    inspect.close().await.unwrap();
    postgres_cleanup(&mut admin, &config).await;
}

#[tokio::test]
#[ignore = "Requires canonical PostgreSQL fixture and RUPHISH_POSTGRES_TEST_PORT"]
async fn postgres_deadlock_rolls_back_the_losing_edit_and_recovers() {
    let (mut admin, config) = postgres_fixture().await;
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    let (actor, original) = replacement_fixture(&db).await;
    let options = sqlx::postgres::PgConnectOptions::new_without_pgpass()
        .host(&config.host)
        .port(config.port)
        .database(&config.database)
        .username("postgres")
        .password("ruphish-postgres-fixture-only")
        .ssl_mode(sqlx::postgres::PgSslMode::VerifyFull)
        .ssl_root_cert(config.ca_path.as_ref().unwrap())
        .options([("search_path", "ruphish,pg_catalog")]);
    let mut inspect = sqlx::PgConnection::connect_with(&options).await.unwrap();
    // Make the facade's connection detect the deliberate cycle first.
    sqlx::query("SET deadlock_timeout='10s'")
        .execute(&mut inspect)
        .await
        .unwrap();
    let mut held = inspect.begin().await.unwrap();
    sqlx::query("SELECT id FROM templates WHERE id=$1 FOR UPDATE")
        .bind(original.id)
        .fetch_one(&mut *held)
        .await
        .unwrap();
    let resources = db.resources();
    let mut edit = original.clone();
    edit.name = "Deadlocked edit".into();
    let expected = original.modified_date.clone();
    let id = original.id;
    let writing = tokio::spawn(async move {
        resources
            .save(actor, edit.into(), Some(id), Some(expected))
            .await
    });
    loop {
        sqlx::query("SELECT pg_stat_clear_snapshot()")
            .execute(&mut *held)
            .await
            .unwrap();
        let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND application_name='ruphish' AND wait_event_type='Lock')").fetch_one(&mut *held).await.unwrap();
        if waiting {
            break;
        }
        tokio::task::yield_now().await;
    }
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(actor.id)
        .fetch_one(&mut *held)
        .await
        .unwrap();
    assert!(matches!(
        writing.await.unwrap(),
        Err(Error::Storage(
            ruphish_application::storage::Error::TransactionConflict
        ))
    ));
    held.rollback().await.unwrap();
    assert!(
        db.resources()
            .get(actor, Kind::Template, original.id)
            .await
            .unwrap()
            == original
    );
    let mut edit = original.clone();
    edit.name = "Recovered edit".into();
    assert!(
        db.resources()
            .save(
                actor,
                edit.into(),
                Some(original.id),
                Some(original.modified_date)
            )
            .await
            .is_ok()
    );
    inspect.close().await.unwrap();
    db.close().await;
    postgres_cleanup(&mut admin, &config).await;
}

#[tokio::test]
#[ignore = "Requires canonical MySQL fixture and RUPHISH_MYSQL_TEST_PORT"]
async fn mysql_preferences_preserve_existing_accounts_and_survive_reopen() {
    let (mut inspect, config) = fixture().await;
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    db.bootstrap(
        Some("Preferences fixture password 42!".into()),
        Some("preferences-key".into()),
    )
    .await
    .unwrap();
    let original = db.accounts().by_name("admin").await.unwrap();
    db.close().await;
    sqlx::raw_sql("UPDATE users SET password_change_required=FALSE")
        .execute(&mut inspect)
        .await
        .unwrap();
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    let account = db.accounts().by_name("admin").await.unwrap();
    assert_eq!(account.hash, original.hash);
    assert_eq!(account.identity, original.identity);
    assert_eq!(account.preferences, Default::default());
    let chosen = auth::Preferences {
        language: Some("de".into()),
        format_locale: Some("zh-CN".into()),
        time_zone: Some("Europe/Berlin".into()),
        show_utc_offset: false,
    };
    db.accounts()
        .write(account.identity, auth::Write::Preferences(chosen.clone()))
        .await
        .unwrap();
    db.close().await;
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    assert_eq!(
        db.accounts()
            .by_id(account.identity.id)
            .await
            .unwrap()
            .preferences,
        auth::Preferences {
            show_utc_offset: false,
            ..chosen.clone()
        }
    );
    db.accounts()
        .write(account.identity, auth::Write::Preferences(chosen.clone()))
        .await
        .unwrap();
    db.close().await;
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    assert_eq!(
        db.accounts()
            .by_id(account.identity.id)
            .await
            .unwrap()
            .preferences,
        chosen
    );
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
#[ignore = "Requires canonical PostgreSQL fixture and RUPHISH_POSTGRES_TEST_PORT"]
async fn postgres_preferences_preserve_existing_accounts_and_survive_reopen() {
    let (mut inspect, config) = postgres_fixture().await;
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    db.bootstrap(
        Some("Preferences fixture password 42!".into()),
        Some("preferences-key".into()),
    )
    .await
    .unwrap();
    let original = db.accounts().by_name("admin").await.unwrap();
    db.close().await;
    let mut connection = postgres_inspect(&config).await;
    sqlx::raw_sql("UPDATE users SET password_change_required=FALSE")
        .execute(&mut connection)
        .await
        .unwrap();
    connection.close().await.unwrap();
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    let account = db.accounts().by_name("admin").await.unwrap();
    assert_eq!(account.hash, original.hash);
    assert_eq!(account.identity, original.identity);
    assert_eq!(account.preferences, Default::default());
    let chosen = auth::Preferences {
        language: Some("de".into()),
        format_locale: Some("zh-CN".into()),
        time_zone: Some("Europe/Berlin".into()),
        show_utc_offset: false,
    };
    db.accounts()
        .write(account.identity, auth::Write::Preferences(chosen.clone()))
        .await
        .unwrap();
    db.close().await;
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    assert_eq!(
        db.accounts()
            .by_id(account.identity.id)
            .await
            .unwrap()
            .preferences,
        auth::Preferences {
            show_utc_offset: false,
            ..chosen.clone()
        }
    );
    db.accounts()
        .write(account.identity, auth::Write::Preferences(chosen.clone()))
        .await
        .unwrap();
    db.close().await;
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    assert_eq!(
        db.accounts()
            .by_id(account.identity.id)
            .await
            .unwrap()
            .preferences,
        chosen
    );
    db.close().await;
    postgres_cleanup(&mut inspect, &config).await;
}

#[path = "support/query_work.rs"]
mod query_work;

#[path = "support/client_observations.rs"]
mod client_observations;
