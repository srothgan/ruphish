//! Real migrated SQLite, loopback delivery and exported artifacts.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Isolated functional fixtures and assertions"
)]
use ruphish_adapters::{
    outbound::Policy,
    sqlite::{StartupDatabase, integrations::Integrations, reporting::Reports},
};
use ruphish_application::{
    auth::Identity,
    integrations::{MailboxEdit, Repository as _, WebhookEdit},
    reporting::{Filter, Repository as _},
};
use sqlx::Row;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn caller() -> Identity {
    Identity { id: 1, epoch: 0 }
}
async fn fixture() -> (tempfile::TempDir, StartupDatabase) {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("reporting.db");
    std::fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/campaigns/queued.sqlite3"),
        &path,
    )
    .unwrap();
    let database = StartupDatabase::open(&path, "").await.unwrap();
    (directory, database)
}
fn filter() -> Filter {
    Filter {
        limit: 50,
        ..Default::default()
    }
}

#[tokio::test]
async fn observations_filters_and_exports_preserve_evidence_without_credentials_or_locations() {
    let (_directory, database) = fixture().await;
    let pool = database.pool();
    sqlx::query("DELETE FROM events")
        .execute(&pool)
        .await
        .unwrap();
    let recipient = sqlx::query("SELECT id,email,campaign_id FROM results ORDER BY id LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let id: i64 = recipient.get("id");
    let campaign: i64 = recipient.get("campaign_id");
    let email: String = recipient.get("email");
    sqlx::query("UPDATE results SET first_name='=1+1',last_name='' WHERE id=?")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    for (kind, time) in [
        ("Clicked Link", "2026-10-19T12:00:00Z"),
        ("Submitted Data", "2026-10-19T12:01:00Z"),
        ("Email Reported", "2026-10-19T12:02:00Z"),
        ("Email Reported", "2026-10-19T12:02:00Z"),
    ] {
        sqlx::query("INSERT INTO events(campaign_id,email,time,message,details) VALUES(?,?,?,?,?)")
            .bind(campaign).bind(&email).bind(time).bind(kind)
            .bind(r#"{"password_present":true,"payload":{"password":"PRIVATE-CANARY"},"ip":"192.0.2.42"}"#)
            .execute(&pool).await.unwrap();
    }
    let reports = Reports::new(pool.clone());
    let result = reports
        .analysis(caller(), campaign, filter())
        .await
        .unwrap();
    assert_eq!(
        result.unique,
        [0, 0, 1, 1, 1],
        "clicks and submissions must not invent opens or SMTP acceptance"
    );
    assert_eq!(result.progression, [0, 1, 1, 1, 1]);
    assert_eq!(result.totals, [0, 0, 1, 1, 2]);
    assert_eq!(result.events.len(), 4);
    assert!(
        result
            .events
            .iter()
            .all(|event| event.recipient_id == Some(id))
    );
    let mut page = filter();
    page.limit = 2;
    let first = reports
        .analysis(caller(), campaign, page.clone())
        .await
        .unwrap();
    page.offset = 2;
    let second = reports.analysis(caller(), campaign, page).await.unwrap();
    assert!(
        first
            .events
            .iter()
            .all(|a| second.events.iter().all(|b| a.id != b.id)),
        "same-time events must paginate deterministically"
    );
    let mut selected = filter();
    selected.event = "Email Reported".into();
    let selected = reports
        .analysis(caller(), campaign, selected)
        .await
        .unwrap();
    assert_eq!(selected.progression, [0, 0, 0, 0, 1]);
    assert_eq!(selected.event_total, 2);
    assert_eq!(selected.recipient_total, 1);
    let mut invalid = filter();
    invalid.event = "' OR 1=1--".into();
    assert!(reports.analysis(caller(), campaign, invalid).await.is_err());
    assert!(
        reports
            .analysis(Identity { id: 2, epoch: 0 }, campaign, filter())
            .await
            .is_err()
    );
    let csv = reports
        .export(caller(), campaign, filter(), "csv", "events")
        .await
        .unwrap();
    let csv_text = String::from_utf8(csv.bytes).unwrap();
    assert!(!csv_text.contains("PRIVATE-CANARY"));
    assert!(!csv_text.contains("192.0.2.42"));
    assert_eq!(
        csv::Reader::from_reader(csv_text.as_bytes())
            .records()
            .count(),
        4
    );
    let recipient_csv = reports
        .export(caller(), campaign, filter(), "csv", "recipients")
        .await
        .unwrap();
    let mut reader = csv::Reader::from_reader(recipient_csv.bytes.as_slice());
    let row = reader
        .records()
        .map(Result::unwrap)
        .find(|row| row.get(1) == Some(email.as_str()))
        .unwrap();
    assert_eq!(
        row.get(2).map(str::trim_end),
        Some("'=1+1"),
        "spreadsheet formulas remain literal CSV text"
    );
    let excel = reports
        .export(caller(), campaign, filter(), "xlsx", "all")
        .await
        .unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(excel.bytes)).unwrap();
    let mut workbook = String::new();
    std::io::Read::read_to_string(
        &mut archive.by_name("xl/workbook.xml").unwrap(),
        &mut workbook,
    )
    .unwrap();
    assert!(
        workbook.contains("Summary")
            && workbook.contains("Recipients")
            && workbook.contains("Events")
    );
    let mut formula_text_found = false;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut bytes).unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("PRIVATE-CANARY"));
        let xml = String::from_utf8_lossy(&bytes);
        if xml.contains("=1+1") {
            formula_text_found = true;
            assert!(
                !xml.contains("<f>"),
                "recipient names must never become Excel formulas"
            );
        }
    }
    assert!(
        formula_text_found,
        "Excel retains the literal recipient name"
    );
    sqlx::query("INSERT INTO results(campaign_id,user_id,r_id,email,reported,status) VALUES(?,1,'duplicate-legacy',?,0,'Email Sent')")
        .bind(campaign).bind(&email).execute(&pool).await.unwrap();
    let ambiguous = reports
        .analysis(caller(), campaign, filter())
        .await
        .unwrap();
    assert_eq!(
        ambiguous.event_total, 4,
        "ambiguous legacy recipient identity must not multiply events"
    );
    assert_eq!(
        ambiguous.unique, [0; 5],
        "do not guess which duplicated legacy result owns an event"
    );
    database.close().await;
}

#[tokio::test]
async fn committed_events_retry_after_receiver_failure_and_rollback_never_sends() {
    let (_directory, database) = fixture().await;
    let pool = database.pool();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/events", listener.local_addr().unwrap());
    let (sender, mut requests) = tokio::sync::mpsc::channel(4);
    let receiver = tokio::spawn(async move {
        for code in [503, 204] {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0; 4096];
            loop {
                let n = stream.read(&mut buffer).await.unwrap();
                assert_ne!(n, 0);
                bytes.extend_from_slice(&buffer[..n]);
                if let Some(end) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]).to_lowercase();
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.strip_prefix("content-length:")
                                .and_then(|v| v.trim().parse::<usize>().ok())
                        })
                        .unwrap();
                    if bytes.len() >= end + 4 + length {
                        break;
                    }
                }
            }
            sender.send(bytes).await.unwrap();
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 {code} Test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        }
    });
    let integrations = Integrations::new(pool.clone(), Policy::default());
    let hook = integrations
        .save_webhook(
            caller(),
            None,
            WebhookEdit {
                name: "Local receiver".into(),
                url,
                secret: Some("test-signing-secret".into()),
                active: true,
                revision: None,
            },
        )
        .await
        .unwrap();
    let mut transaction = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO events(campaign_id,email,time,message,details) VALUES(1,'','2026-10-19T12:00:00Z','Email Reported','')").execute(&mut *transaction).await.unwrap();
    transaction.rollback().await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM ruphish_webhook_jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    sqlx::query("INSERT INTO events(campaign_id,email,time,message,details) VALUES(1,'recipient@example.test','2026-10-19T12:00:00Z','Email Reported','{\"password\":\"CANARY\"}')").execute(&pool).await.unwrap();
    integrations.tick().await.unwrap();
    let first = requests.recv().await.unwrap();
    assert!(!String::from_utf8_lossy(&first).contains("CANARY"));
    let state: String = sqlx::query_scalar("SELECT state FROM ruphish_webhook_jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(state, "retry");
    sqlx::query("UPDATE ruphish_webhook_jobs SET due=0")
        .execute(&pool)
        .await
        .unwrap();
    let restarted = Integrations::new(pool.clone(), Policy::default());
    restarted.tick().await.unwrap();
    let second = requests.recv().await.unwrap();
    assert_eq!(
        first, second,
        "retries retain the event identity, signed bytes and signature"
    );
    let detail = restarted.webhook(caller(), hook.id).await.unwrap();
    assert_eq!(detail.history.len(), 2);
    assert_eq!(detail.history[0].code, "accepted");
    assert_eq!(detail.history[1].status, Some(503));
    receiver.await.unwrap();
    database.close().await;
}

#[tokio::test]
async fn mailbox_edits_are_owner_scoped_revision_checked_and_secret_preserving() {
    let (_directory, database) = fixture().await;
    let integrations = Integrations::new(database.pool(), Policy::default());
    let edit = || MailboxEdit {
        name: "Reports".into(),
        enabled: false,
        host: "imap.example.test".into(),
        port: 993,
        username: "service".into(),
        password: Some("MAILBOX-CANARY".into()),
        tls: true,
        folder: "INBOX".into(),
        restrict_domain: String::new(),
        delete_reported: false,
        frequency: 60,
        revision: None,
    };
    let mailbox = integrations
        .save_mailbox(caller(), None, edit())
        .await
        .unwrap();
    assert!(mailbox.has_password);
    assert!(
        integrations
            .mailbox(Identity { id: 2, epoch: 0 }, mailbox.id)
            .await
            .is_err()
    );
    let mut update = edit();
    update.password = None;
    update.revision = Some(mailbox.revision);
    update.name = "Renamed reports".into();
    let updated = integrations
        .save_mailbox(caller(), Some(mailbox.id), update)
        .await
        .unwrap();
    assert!(updated.has_password);
    let mut stale = edit();
    stale.revision = Some(mailbox.revision);
    assert!(
        integrations
            .save_mailbox(caller(), Some(mailbox.id), stale)
            .await
            .is_err()
    );
    assert_eq!(
        integrations
            .mailbox(caller(), mailbox.id)
            .await
            .unwrap()
            .name,
        "Renamed reports"
    );
    let mut hostile = edit();
    hostile.host = "host\r\nLOGIN other".into();
    assert!(
        integrations
            .save_mailbox(caller(), None, hostile)
            .await
            .is_err()
    );
    assert!(
        integrations
            .check_mailbox(caller(), mailbox.id)
            .await
            .is_err(),
        "disabled mailboxes cannot be queued"
    );
    assert!(
        integrations
            .delete_mailbox(caller(), mailbox.id, mailbox.revision)
            .await
            .is_err()
    );
    integrations
        .delete_mailbox(caller(), mailbox.id, updated.revision)
        .await
        .unwrap();
    assert!(integrations.mailboxes(caller()).await.unwrap().is_empty());
    database.close().await;
}

#[tokio::test]
async fn legacy_mailbox_and_location_migration_is_recoverable_and_idempotent() {
    use sqlx::Connection;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("legacy.db");
    std::fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/campaigns/queued.sqlite3"),
        &path,
    )
    .unwrap();
    let options = sqlx::sqlite::SqliteConnectOptions::new().filename(&path);
    let mut legacy = sqlx::SqliteConnection::connect_with(&options)
        .await
        .unwrap();
    sqlx::raw_sql("UPDATE results SET ip='192.0.2.42',latitude=51.5,longitude=7.2; INSERT INTO imap(user_id,host,port,username,password,tls,enabled,folder,imap_freq,modified_date,last_login) VALUES(1,'imap.example.test',993,'reports','LEGACY-SERVICE-SECRET',1,1,'Reports',60,'2025-01-02T03:04:05.123456789Z','2025-01-03T04:05:06Z'); INSERT INTO imap(user_id,host,port,enabled) VALUES(1,'',99999,1);").execute(&mut legacy).await.unwrap();
    legacy.close().await.unwrap();
    let db = StartupDatabase::open(&path, "").await.unwrap();
    let backup = db.backup_path().unwrap().to_path_buf();
    let store = Integrations::new(db.pool(), Policy::default());
    let rows = store.mailboxes(caller()).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .any(|r| r.enabled && r.has_password && r.folder == "Reports")
    );
    assert!(rows.iter().any(|r| !r.enabled));
    let imported = rows.iter().find(|r| r.enabled).unwrap();
    assert_eq!(imported.modified_date, "2025-01-02T03:04:05.123456789Z");
    assert_eq!(imported.last_login, "2025-01-03T04:05:06Z");
    let locations: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM results WHERE ip!='' OR latitude!=0 OR longitude!=0",
    )
    .fetch_one(&db.pool())
    .await
    .unwrap();
    assert_eq!(locations, 0);
    let old: i64 = sqlx::query_scalar("SELECT count(*) FROM imap")
        .fetch_one(&db.pool())
        .await
        .unwrap();
    assert_eq!(old, 0, "one authoritative credential store after migration");
    let ids: Vec<_> = rows.iter().map(|r| r.id).collect();
    db.close().await;
    let reopened = StartupDatabase::open(&path, "").await.unwrap();
    let again = Integrations::new(reopened.pool(), Policy::default())
        .mailboxes(caller())
        .await
        .unwrap();
    assert_eq!(ids, again.iter().map(|r| r.id).collect::<Vec<_>>());
    reopened.close().await;
    let mut original = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&backup)
            .read_only(true),
    )
    .await
    .unwrap();
    let preserved: i64 = sqlx::query_scalar("SELECT count(*) FROM results WHERE ip='192.0.2.42'")
        .fetch_one(&mut original)
        .await
        .unwrap();
    assert!(preserved > 0);
    original.close().await.unwrap();
}
