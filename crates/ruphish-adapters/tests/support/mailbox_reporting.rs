//! Real TLS IMAP, durable checkpoints and campaign transactions on each provider.
use super::{query_work::Inspector, *};
use ruphish_application::integrations::MailboxEdit;

const ADMIN: auth::Identity = auth::Identity { id: 1, epoch: 0 };
const COLLEAGUE: auth::Identity = auth::Identity { id: 2, epoch: 0 };
const SEED: &str = "
INSERT INTO users(id,username,api_key,role_id,password_change_required,account_locked) VALUES(1,'mail-owner','mail-key',1,FALSE,FALSE),(2,'campaign-owner','campaign-key',2,FALSE,FALSE);
INSERT INTO ruphish_auth_state VALUES(1,0),(2,0);
UPDATE ruphish_settings SET reporting_mailboxes_enabled=TRUE,webhooks_enabled=TRUE;
INSERT INTO campaigns(id,user_id,name,url,status,launch_date) VALUES
(101,2,'Earlier send','https://training.test/','In progress','2026-10-01T00:00:00Z'),
(102,2,'Later send','https://training.test/','Emails Sent','2026-09-01T00:00:00Z'),
(103,2,'Closed','https://training.test/','Completed','2026-08-01T00:00:00Z'),
(104,1,'Unsent','https://training.test/','Queued','2026-07-01T00:00:00Z');
INSERT INTO results(id,campaign_id,user_id,r_id,email,status,reported,modified_date) VALUES
(101,101,2,'earlier','one@example.test','Email Sent',FALSE,'2026-10-10T07:00:00Z'),
(102,102,2,'later','two@example.test','Email Sent',FALSE,'2026-10-10T08:00:00Z'),
(103,103,2,'closed','three@example.test','Email Sent',FALSE,'2026-08-01T00:00:00Z');
INSERT INTO events(campaign_id,recipient_id,email,time,kind,source) VALUES
(101,101,'one@example.test','2026-10-10T09:00:00+02:00','email_sent','application'),
(102,102,'two@example.test','2026-10-10T08:00:00Z','email_sent','application'),
(103,103,'three@example.test','2026-08-01T00:00:00Z','email_sent','application'),
(101,101,'one@example.test','2026-01-01T00:00:00Z','email_sent','operator');
";

fn edit(port: u16, revision: Option<i64>) -> MailboxEdit {
    MailboxEdit {
        name: "Reports".into(),
        enabled: true,
        host: "127.0.0.1".into(),
        port,
        username: "service".into(),
        password: Some("MAILBOX-SECRET-CANARY".into()),
        tls: true,
        ignore_cert_errors: true,
        folder: "INBOX".into(),
        restrict_domain: String::new(),
        delete_reported: false,
        frequency: 60,
        revision,
    }
}
fn message(rid: &str) -> Vec<u8> {
    format!("Subject: synthetic report\r\n\r\nhttps://training.test/?rid={rid}").into_bytes()
}
async fn poll(db: &Database, id: i64) {
    let repo = db.integrations();
    repo.check_mailbox(ADMIN, id).await.unwrap();
    repo.poll_mailboxes().await.unwrap();
}
fn copy_config(config: &Config) -> Config {
    match config {
        Config::Sqlite { path } => Config::Sqlite { path: path.clone() },
        Config::MySql(config) => Config::MySql(config.clone()),
        Config::Postgres(config) => Config::Postgres(config.clone()),
    }
}
async fn contract(config: Config, inspect: &mut Inspector) {
    let policy = Policy::new(&["127.0.0.1".into()]).unwrap();
    let db = database::open(copy_config(&config), policy.clone())
        .await
        .unwrap();
    inspect.execute(SEED).await;
    let peer = mailbox::Mailbox::new();
    {
        let mut state = peer.state.lock().unwrap();
        state.messages.insert(1, message("earlier"));
        state.internal_dates.insert(1, "2026-10-08".into());
        state.messages.insert(2, message("closed"));
        state.messages.insert(3, message("later"));
        state.messages.insert(
            4,
            b"Subject: wrong origin\r\n\r\nhttps://other.test/?rid=earlier".to_vec(),
        );
        state.messages.insert(5, b"Subject: ambiguous\r\n\r\nhttps://training.test/?rid=earlier https://training.test/?rid=later".to_vec());
        state.messages.insert(6, message("earlier"));
        for uid in 7..=35 {
            state.messages.insert(uid, message("missing"));
        }
        state.arrive_on_search.insert(36, message("earlier"));
        state.reject_ack = true;
    }
    let repo = db.integrations();
    repo.save_webhook(
        ADMIN,
        None,
        ruphish_application::integrations::WebhookEdit {
            name: "Report events".into(),
            url: "https://receiver.example.test/hook".into(),
            secret: Some("synthetic-secret".into()),
            active: true,
            revision: None,
        },
    )
    .await
    .unwrap();
    let saved = repo
        .save_mailbox(ADMIN, None, edit(peer.address.port(), None))
        .await
        .unwrap();
    assert_eq!(inspect.scalar("SELECT count(*) FROM ruphish_mailboxes WHERE initial_since='2026-10-10T07:00:00.000000000Z'").await, 1);
    // An ordinary edit before first login cannot change the stored creation boundary.
    let saved = repo
        .save_mailbox(
            ADMIN,
            Some(saved.id),
            edit(peer.address.port(), Some(saved.revision)),
        )
        .await
        .unwrap();
    repo.poll_mailboxes().await.unwrap();
    assert_eq!(
        peer.state.lock().unwrap().searches,
        ["UID SEARCH UID 1:35 SINCE 09-Oct-2026"]
    );
    assert_eq!(
        inspect
            .scalar("SELECT last_uid FROM ruphish_mailboxes")
            .await,
        1
    );
    assert_eq!(
        inspect
            .scalar("SELECT count(*) FROM ruphish_mailboxes WHERE initial_since IS NULL")
            .await,
        1
    );
    poll(&db, saved.id).await;
    assert_eq!(
        repo.mailbox(ADMIN, saved.id).await.unwrap().code,
        "mailbox_acknowledgement_failed"
    );
    assert_eq!(
        inspect
            .scalar("SELECT last_uid FROM ruphish_mailboxes")
            .await,
        3
    );
    assert_eq!(
        inspect
            .scalar("SELECT count(*) FROM events WHERE campaign_id=103 AND kind='email_reported'")
            .await,
        0
    );
    assert_eq!(inspect.scalar("SELECT count(*) FROM results WHERE id=103 AND reported=FALSE AND modified_date='2026-08-01T00:00:00Z'").await, 1);
    assert_eq!(inspect.scalar("SELECT count(*) FROM ruphish_mailbox_messages WHERE uid=2 AND code='mailbox_campaign_closed' AND acknowledged=FALSE").await, 1);
    assert_eq!(
        inspect
            .scalar("SELECT count(*) FROM events WHERE campaign_id=102 AND kind='email_reported'")
            .await,
        1
    );
    assert!(
        db.campaigns()
            .complete(ADMIN, 102, 1_800_000_000)
            .await
            .is_err()
    );
    db.campaigns()
        .complete(COLLEAGUE, 102, 1_800_000_000)
        .await
        .unwrap();
    db.close().await;
    drop(repo);
    peer.state.lock().unwrap().reject_ack = false;
    let db = database::open(config, policy).await.unwrap();
    poll(&db, saved.id).await;
    assert_eq!(
        inspect
            .scalar("SELECT last_uid FROM ruphish_mailboxes")
            .await,
        28,
        "bounded processing after the saved UID"
    );
    assert_eq!(
        peer.state.lock().unwrap().seen,
        [3, 6],
        "committed report is acknowledged despite subsequent closure"
    );
    poll(&db, saved.id).await;
    assert_eq!(
        inspect
            .scalar("SELECT last_uid FROM ruphish_mailboxes")
            .await,
        36
    );
    assert_eq!(
        peer.state.lock().unwrap().seen,
        [3, 6, 36],
        "arrival during initialization is not skipped"
    );
    assert_eq!(
        inspect
            .scalar("SELECT count(*) FROM events WHERE campaign_id=101 AND kind='email_reported'")
            .await,
        2
    );
    // Completion and report persistence may win in either order, but no report can commit afterward.
    peer.state
        .lock()
        .unwrap()
        .messages
        .insert(37, message("earlier"));
    let repo = db.integrations();
    repo.check_mailbox(ADMIN, saved.id).await.unwrap();
    let campaigns = db.campaigns();
    let (reported, completed) = tokio::join!(
        repo.poll_mailboxes(),
        campaigns.complete(COLLEAGUE, 101, 1_800_000_001)
    );
    reported.unwrap();
    completed.unwrap();
    let committed = inspect
        .scalar("SELECT count(*) FROM events WHERE kind='email_reported'")
        .await;
    assert!((3..=4).contains(&committed));
    peer.state
        .lock()
        .unwrap()
        .messages
        .insert(38, message("earlier"));
    poll(&db, saved.id).await;
    assert_eq!(
        inspect
            .scalar("SELECT count(*) FROM events WHERE kind='email_reported'")
            .await,
        committed
    );
    assert_eq!(
        inspect
            .scalar("SELECT last_uid FROM ruphish_mailboxes")
            .await,
        38
    );
    assert_eq!(inspect.scalar("SELECT count(*) FROM ruphish_mailbox_messages WHERE uid=38 AND code='mailbox_campaign_closed'").await, 1);
    assert!(!peer.state.lock().unwrap().seen.contains(&38));
    assert_eq!(
        inspect
            .scalar("SELECT count(*) FROM ruphish_webhook_jobs")
            .await,
        committed
    );
    assert_eq!(inspect.scalar("SELECT count(*) FROM ruphish_webhook_jobs j JOIN events e ON e.id=j.event_id WHERE e.campaign_id=103").await, 0);

    // No eligible actual send: new mailbox skips existing messages. Polling continues with all campaigns closed.
    let mut next = edit(peer.address.port(), None);
    next.name = "No active sends".into();
    let fallback = repo.save_mailbox(ADMIN, None, next).await.unwrap();
    repo.poll_mailboxes().await.unwrap();
    assert_eq!(
        inspect
            .scalar("SELECT last_uid FROM ruphish_mailboxes WHERE name='No active sends'")
            .await,
        38
    );
    let searches = peer.state.lock().unwrap().searches.len();
    let logins = peer.state.lock().unwrap().logins;
    poll(&db, saved.id).await;
    assert!(peer.state.lock().unwrap().logins > logins);
    assert_eq!(peer.state.lock().unwrap().searches.len(), searches);
    // Explicit UIDVALIDITY recovery starts with new arrivals, not the old campaign boundary.
    peer.state.lock().unwrap().validity = 42;
    poll(&db, saved.id).await;
    let changed = repo.mailbox(ADMIN, saved.id).await.unwrap();
    assert_eq!(changed.code, "mailbox_uidvalidity_changed");
    repo.resume_mailbox(ADMIN, saved.id, changed.revision)
        .await
        .unwrap();
    repo.poll_mailboxes().await.unwrap();
    assert_eq!(
        inspect
            .scalar("SELECT last_uid FROM ruphish_mailboxes WHERE name='Reports'")
            .await,
        38
    );
    assert_eq!(
        inspect
            .scalar("SELECT count(*) FROM events WHERE kind='email_reported'")
            .await,
        committed
    );
    for enabled in [false, true] {
        let current = repo.mailbox(ADMIN, saved.id).await.unwrap();
        let mut change = edit(peer.address.port(), Some(current.revision));
        change.enabled = enabled;
        repo.save_mailbox(ADMIN, Some(saved.id), change)
            .await
            .unwrap();
        repo.poll_mailboxes().await.unwrap();
        assert_eq!(
            inspect
                .scalar("SELECT last_uid FROM ruphish_mailboxes WHERE name='Reports'")
                .await,
            38
        );
    }
    let current = repo.mailbox(ADMIN, saved.id).await.unwrap();
    let mut change = edit(peer.address.port(), Some(current.revision));
    change.folder = "Other reports".into();
    repo.save_mailbox(ADMIN, Some(saved.id), change)
        .await
        .unwrap();
    assert_eq!(inspect.scalar("SELECT count(*) FROM ruphish_mailboxes WHERE name='Reports' AND initialized=FALSE AND initial_since IS NULL").await, 1);
    repo.poll_mailboxes().await.unwrap();
    assert_eq!(
        inspect
            .scalar("SELECT last_uid FROM ruphish_mailboxes WHERE name='Reports'")
            .await,
        38
    );
    repo.delete_mailbox(ADMIN, fallback.id, Some(fallback.revision))
        .await
        .unwrap();
    db.close().await;
}

#[tokio::test]
async fn sqlite_mailbox_reporting() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mailbox.db");
    let config = Config::Sqlite { path: path.clone() };
    let db = database::open(copy_config(&config), Policy::default())
        .await
        .unwrap();
    db.close().await;
    let connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(path),
    )
    .await
    .unwrap();
    contract(config, &mut Inspector::Sqlite(connection)).await;
}
#[tokio::test]
#[ignore = "Requires canonical MySQL fixture"]
async fn mysql_mailbox_reporting() {
    let (connection, config) = fixture().await;
    let mut inspect = Inspector::Mysql(connection);
    contract(Config::MySql(config.clone()), &mut inspect).await;
    inspect
        .execute(&format!("DROP DATABASE `{}`", config.database))
        .await;
}
#[tokio::test]
#[ignore = "Requires canonical PostgreSQL fixture"]
async fn postgres_mailbox_reporting() {
    let (mut admin, config) = postgres_fixture().await;
    let mut inspect = Inspector::Postgres(postgres_inspect(&config).await);
    contract(Config::Postgres(config.clone()), &mut inspect).await;
    drop(inspect);
    postgres_cleanup(&mut admin, &config).await;
}
