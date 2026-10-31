//! Regression workflows for audit findings, with synthetic state and loopback only.
#![allow(clippy::unwrap_used, reason = "Isolated fixture setup and assertions")]
use crate::{
    database::{self, Config},
    outbound::Policy,
    sqlite::integrations::Integrations,
};
use ruphish_application::{
    auth::{Edit, Identity, Service},
    integrations::{MailboxEdit, Repository, WebhookEdit, work::*},
    resources::Error,
};
use std::sync::Arc;

fn admin() -> Identity {
    Identity { id: 1, epoch: 0 }
}
async fn fixture() -> (tempfile::TempDir, database::Database, sqlx::SqlitePool) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("security.db");
    std::fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/campaigns/queued.sqlite3"),
        &path,
    )
    .unwrap();
    let db = database::open(Config::Sqlite { path: path.clone() }, Policy::default())
        .await
        .unwrap();
    let pool =
        sqlx::SqlitePool::connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(path))
            .await
            .unwrap();
    let settings = db.settings().get(admin()).await.unwrap();
    let mut policy = settings.policy;
    policy.webhooks_enabled = true;
    policy.reporting_mailboxes_enabled = true;
    db.settings()
        .save(admin(), settings.revision, policy, 0)
        .await
        .unwrap();
    (dir, db, pool)
}
fn mailbox(name: &str) -> MailboxEdit {
    MailboxEdit {
        name: name.into(),
        enabled: true,
        host: "mail.example.test".into(),
        port: 993,
        username: "synthetic".into(),
        password: Some("synthetic-password".into()),
        tls: true,
        ignore_cert_errors: false,
        folder: "INBOX".into(),
        restrict_domain: String::new(),
        delete_reported: false,
        frequency: 60,
        revision: None,
    }
}

#[tokio::test]
async fn mailbox_certificate_edits_fence_claims_without_resetting_receipts() {
    let (_dir, db, pool) = fixture().await;
    let repo = db.integrations();
    let worker = Integrations::new(pool.clone(), Policy::default());
    let mut initial = mailbox("Certificate policy");
    initial.ignore_cert_errors = true;
    let saved = repo.save_mailbox(admin(), None, initial).await.unwrap();
    sqlx::query("UPDATE ruphish_mailboxes SET initialized=1,uidvalidity=41,last_uid=12 WHERE id=?")
        .bind(saved.id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO ruphish_mailbox_messages VALUES(?,41,12,'matched','passed',0)")
        .bind(saved.id)
        .execute(&pool)
        .await
        .unwrap();
    let claim = worker.claim_mailbox(1_800_000_000).await.unwrap().unwrap();
    assert!(claim.config.ignore_cert_errors);
    let mut edit = mailbox("Certificate policy");
    edit.revision = Some(saved.revision);
    edit.password = None;
    let updated = repo
        .save_mailbox(admin(), Some(saved.id), edit)
        .await
        .unwrap();
    assert!(!updated.ignore_cert_errors);
    assert!(worker.mailbox_current(&claim.config).await.is_err());
    worker
        .finish_mailbox(&claim, "passed", 1_800_000_100, 1_800_000_001)
        .await
        .unwrap();
    let next = worker.claim_mailbox(1_800_000_002).await.unwrap().unwrap();
    assert!(!next.config.ignore_cert_errors);
    assert!(next.config.initialized);
    assert_eq!((next.config.validity, next.config.last), (41, 12));
    assert_eq!(
        worker
            .pending_acknowledgements(&next.config, 41)
            .await
            .unwrap(),
        vec![12]
    );
    assert!(
        repo.save_mailbox(admin(), Some(saved.id), {
            let mut stale = mailbox("Certificate policy");
            stale.revision = Some(saved.revision);
            stale.ignore_cert_errors = true;
            stale
        })
        .await
        .is_err()
    );
    assert!(
        !repo
            .mailbox(admin(), saved.id)
            .await
            .unwrap()
            .ignore_cert_errors
    );
    pool.close().await;
    db.close().await;
}

#[tokio::test]
async fn stale_mailbox_claims_cannot_initialize_replacements_even_for_the_same_owner() {
    let (_dir, db, pool) = fixture().await;
    let auth = Service::new(
        db.accounts(),
        Arc::new(crate::password_worker::PasswordWorker::default()),
    );
    let other = auth
        .save(
            admin(),
            None,
            Edit {
                username: "other-owner".into(),
                password: "synthetic-password".into(),
                role: "user".into(),
                password_change_required: false,
                account_locked: false,
            },
        )
        .await
        .unwrap();
    let repo = db.integrations();
    let worker = Integrations::new(pool.clone(), Policy::default());
    for owner in [admin(), other.identity] {
        let original = repo
            .save_mailbox(admin(), None, mailbox("original"))
            .await
            .unwrap();
        let claim = worker.claim_mailbox(1_800_000_000).await.unwrap().unwrap();
        assert_eq!(claim.config.id, original.id);
        repo.delete_mailbox(admin(), original.id, Some(original.revision))
            .await
            .unwrap();
        let replacement = repo
            .save_mailbox(owner, None, mailbox("replacement"))
            .await
            .unwrap();
        assert_ne!(replacement.id, original.id);
        assert_eq!(
            worker.initialize_mailbox(&claim.config, 123, 500).await,
            Err("configuration_changed")
        );
        worker.mailbox_login(&claim.config).await.unwrap();
        worker
            .finish_mailbox(&claim, "passed", 1_800_000_100, 1_800_000_001)
            .await
            .unwrap();
        let state: (i64, i64, i64, i64) = sqlx::query_as(
            "SELECT owner_id,initialized,uidvalidity,last_uid FROM ruphish_mailboxes WHERE id=?",
        )
        .bind(replacement.id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(state, (owner.id, 0, 0, 0));
        repo.delete_mailbox(owner, replacement.id, Some(replacement.revision))
            .await
            .unwrap();
    }
    pool.close().await;
    db.close().await;
}

struct CancelBeforeSend {
    repo: Integrations,
    id: i64,
    revision: i64,
}
#[async_trait::async_trait]
impl WebhookQueue for CancelBeforeSend {
    async fn claim_webhook(&self, now: i64) -> Result<Option<WebhookClaim>, Error> {
        let claim = self.repo.claim_webhook(now).await?;
        if claim.is_some() {
            self.repo
                .delete_webhook(admin(), self.id, Some(self.revision))
                .await?;
        }
        Ok(claim)
    }
    async fn webhook_current(&self, claim: &WebhookClaim, now: i64) -> Result<bool, Error> {
        self.repo.webhook_current(claim, now).await
    }
    async fn finish_webhook(
        &self,
        claim: &WebhookClaim,
        outcome: WebhookCompletion,
        now: i64,
    ) -> Result<(), Error> {
        self.repo.finish_webhook(claim, outcome, now).await
    }
}

#[tokio::test]
async fn deleted_webhooks_do_not_connect_after_claiming() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let (_dir, db, pool) = fixture().await;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let hook = db
        .integrations()
        .save_webhook(
            admin(),
            None,
            WebhookEdit {
                name: "cancelled".into(),
                url: format!("http://{}/", listener.local_addr().unwrap()),
                secret: Some("synthetic-secret".into()),
                active: true,
                revision: None,
            },
        )
        .await
        .unwrap();
    sqlx::query("INSERT INTO events(campaign_id,email,time,kind,metadata,recipient_id) SELECT campaign_id,email,'2026-10-22T00:00:00Z','link_clicked','{}',id FROM results LIMIT 1").execute(&pool).await.unwrap();
    let policy = Policy::new(&["127.0.0.1".into()]).unwrap();
    let queue = CancelBeforeSend {
        repo: Integrations::new(pool.clone(), policy.clone()),
        id: hook.id,
        revision: hook.revision,
    };
    crate::integration_work::webhooks(&queue, &policy)
        .await
        .unwrap();
    assert!(
        matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM webhooks")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    pool.close().await;
    db.close().await;
}

#[tokio::test]
async fn legacy_campaign_list_rejects_large_graphs_but_directory_remains_available() {
    let (_dir, db, pool) = fixture().await;
    let campaigns = db.campaigns();
    assert!(!campaigns.list(admin()).await.unwrap().is_empty());
    // One oversized persisted value is enough to exceed the aggregate budget.
    sqlx::query("UPDATE templates SET html=?")
        .bind("x".repeat(8 * 1024 * 1024))
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        campaigns.list(admin()).await,
        Err(Error::Invalid(_))
    ));
    assert!(
        !campaigns
            .overview(admin(), 0, 10)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    pool.close().await;
    db.close().await;
}

#[tokio::test]
async fn integration_identity_preserves_populated_history_across_restarts() {
    let (dir, db, pool) = fixture().await;
    let original = db
        .integrations()
        .save_mailbox(admin(), None, mailbox("preserved"))
        .await
        .unwrap();
    let worker = Integrations::new(pool.clone(), Policy::default());
    let claim = worker.claim_mailbox(1_800_000_000).await.unwrap().unwrap();
    worker
        .initialize_mailbox(&claim.config, 123, 500)
        .await
        .unwrap();
    let hook = db
        .integrations()
        .save_webhook(
            admin(),
            None,
            WebhookEdit {
                name: "preserved".into(),
                url: "https://example.test/hook".into(),
                secret: Some("synthetic-secret".into()),
                active: true,
                revision: None,
            },
        )
        .await
        .unwrap();
    sqlx::query("INSERT INTO events(campaign_id,email,time,kind,metadata,recipient_id) SELECT campaign_id,email,'2026-10-22T00:00:00Z','link_clicked','{}',id FROM results LIMIT 1").execute(&pool).await.unwrap();
    let delivery = worker.claim_webhook(1_800_000_000).await.unwrap().unwrap();
    let tables = [
        "ruphish_mailboxes",
        "ruphish_mailbox_runs",
        "ruphish_webhook_jobs",
        "ruphish_webhook_attempts",
    ];
    let mut snapshots = Vec::new();
    let mut tx = pool.begin().await.unwrap();
    for table in tables {
        let columns: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_table_info(?) ORDER BY cid")
                .bind(table)
                .fetch_all(&mut *tx)
                .await
                .unwrap();
        let pairs = columns
            .iter()
            .map(|name| format!("'{name}',{name}"))
            .collect::<Vec<_>>()
            .join(",");
        let snapshot = format!(
            "SELECT json_group_array(json_object({pairs})) FROM (SELECT * FROM {table} ORDER BY id)"
        );
        let before: String = sqlx::query_scalar(sqlx::AssertSqlSafe(snapshot.clone()))
            .fetch_one(&mut *tx)
            .await
            .unwrap();
        snapshots.push((snapshot, before));
    }
    tx.commit().await.unwrap();
    db.close().await;
    pool.close().await;

    let path = dir.path().join("security.db");
    let upgraded = database::open(Config::Sqlite { path: path.clone() }, Policy::default())
        .await
        .unwrap();
    assert!(upgraded.backup_path().is_none());
    let inspect =
        sqlx::SqlitePool::connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&path))
            .await
            .unwrap();
    for (query, before) in snapshots {
        let after: String = sqlx::query_scalar(sqlx::AssertSqlSafe(query))
            .fetch_one(&inspect)
            .await
            .unwrap();
        assert_eq!(
            before, after,
            "reopening preserves every persisted integration field"
        );
    }
    let worker = Integrations::new(inspect.clone(), Policy::default());
    assert!(
        worker
            .webhook_current(&delivery, 1_800_000_001)
            .await
            .unwrap()
    );
    upgraded
        .integrations()
        .delete_webhook(admin(), hook.id, Some(hook.revision))
        .await
        .unwrap();
    upgraded
        .integrations()
        .delete_mailbox(admin(), original.id, Some(original.revision))
        .await
        .unwrap();
    upgraded.close().await;
    inspect.close().await;
    let reopened = database::open(Config::Sqlite { path }, Policy::default())
        .await
        .unwrap();
    assert!(reopened.backup_path().is_none());
    let replacement = reopened
        .integrations()
        .save_mailbox(admin(), None, mailbox("after-restart"))
        .await
        .unwrap();
    assert!(replacement.id > original.id);
    reopened.close().await;
}

#[tokio::test]
async fn webhook_claims_keep_only_closed_client_categories_and_preserve_them_on_retry() {
    let (_dir, db, pool) = fixture().await;
    db.integrations()
        .save_webhook(
            admin(),
            None,
            WebhookEdit {
                name: "client observation".into(),
                url: "https://receiver.example.test/hook".into(),
                secret: Some("synthetic-secret".into()),
                active: true,
                revision: None,
            },
        )
        .await
        .unwrap();
    let client = r#"{"browser":"firefox","os":"linux","device":"desktop"}"#;
    sqlx::query("INSERT INTO events(campaign_id,email,time,kind,metadata,recipient_id,client) SELECT campaign_id,email,'2026-10-22T00:00:00Z','link_clicked',?,id,? FROM results LIMIT 1")
        .bind(r#"{"user_agent":"SECRET-CLIENT-CANARY","ip":"192.0.2.19","password":"SECRET-CLIENT-CANARY"}"#).bind(client).execute(&pool).await.unwrap();
    let queue = Integrations::new(pool.clone(), Policy::default());
    let now = ruphish_application::datetime::now().as_second();
    let claim = queue.claim_webhook(now).await.unwrap().unwrap();
    let body: serde_json::Value = serde_json::from_slice(&claim.body).unwrap();
    assert_eq!(
        body["client"],
        serde_json::json!({"browser":"firefox","os":"linux","device":"desktop"})
    );
    assert_eq!(body["details"], "");
    assert!(!String::from_utf8_lossy(&claim.body).contains("SECRET-CLIENT-CANARY"));
    queue
        .finish_webhook(
            &claim,
            WebhookCompletion {
                state: "retry",
                code: "http_transient",
                status: Some(503),
                due: now + 30,
            },
            now,
        )
        .await
        .unwrap();
    let retry = queue.claim_webhook(now + 31).await.unwrap().unwrap();
    assert_eq!(retry.event, claim.event);
    assert_eq!(retry.body, claim.body);
    pool.close().await;
    db.close().await;
}
