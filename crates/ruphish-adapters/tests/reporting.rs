//! Real migrated SQLite, loopback delivery and exported artifacts.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Isolated functional fixtures and assertions"
)]
mod support;
use ruphish_application::{
    auth::Identity,
    integrations::{MailboxEdit, WebhookEdit},
    reporting::Filter,
};
use sqlx::Row;
use support::DatabaseFixture;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn caller() -> Identity {
    Identity { id: 1, epoch: 0 }
}
async fn fixture() -> (tempfile::TempDir, DatabaseFixture) {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("reporting.db");
    std::fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/campaigns/queued.sqlite3"),
        &path,
    )
    .unwrap();
    let database = DatabaseFixture::open(&path).await.unwrap();
    let settings = database.settings().get(caller()).await.unwrap();
    let mut policy = settings.policy;
    policy.webhooks_enabled = true;
    policy.reporting_mailboxes_enabled = true;
    database
        .settings()
        .save(caller(), settings.revision, policy, 0)
        .await
        .unwrap();
    (directory, database)
}
fn filter() -> Filter {
    Filter {
        limit: 50,
        ..Default::default()
    }
}

#[tokio::test]
async fn recipient_exports_preserve_audience_fields_and_stored_dates_across_pages() {
    let (_directory, database) = fixture().await;
    let pool = database.inspection_pool();
    let campaign: i64 = sqlx::query_scalar("SELECT id FROM campaigns ORDER BY id LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    for (id, email, first, last, position, sent, modified) in [
        (
            9001,
            "export-a@example.test",
            "Élise, \"E\"",
            "Müller",
            "=1+1",
            Some("2026-10-19 14:00:00.123456789+02:00"),
            Some("2026-10-20T01:00:00-03:00"),
        ),
        (
            9002,
            "export-b@example.test",
            "",
            "",
            "",
            Some("0001-01-01T00:00:00Z"),
            None,
        ),
        (
            9003,
            "export-c@example.test",
            "",
            "",
            "Analyst",
            None,
            Some("invalid legacy date"),
        ),
    ] {
        sqlx::query("INSERT INTO results(id,campaign_id,user_id,r_id,email,first_name,last_name,position,send_date,modified_date,status,reported) VALUES(?,?,1,?,?,?,?,?,?,?,'Scheduled',0)")
            .bind(id).bind(campaign).bind(format!("export-{id}")).bind(email)
            .bind(first).bind(last).bind(position).bind(sent).bind(modified)
            .execute(&pool).await.unwrap();
    }
    let selected = || Filter {
        recipients: vec![9001, 9002, 9003],
        limit: 1,
        offset: 1,
        ..Default::default()
    };
    let reports = database.reporting();
    let page = reports
        .analysis(caller(), campaign, selected())
        .await
        .unwrap();
    assert_eq!(page.recipients.len(), 1);
    assert_eq!(page.recipients[0].id, 9002);
    assert_eq!(
        page.recipients[0].delivery, None,
        "no native queue record is not an ambiguous SMTP outcome"
    );
    let download = reports
        .export(caller(), campaign, selected(), "csv", "recipients")
        .await
        .unwrap();
    let mut reader = csv::Reader::from_reader(download.bytes.as_slice());
    let headers = reader.headers().unwrap().clone();
    let rows = reader.records().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(
        rows.len(),
        3,
        "exports include all selected pages and no other recipients"
    );
    let cell = |row: usize, column: &str| {
        &rows[row][headers.iter().position(|name| name == column).unwrap()]
    };
    assert_eq!(cell(0, "Recipient ID"), "9001");
    assert_eq!(cell(0, "Delivery"), "historical");
    assert_eq!(cell(0, "First Name"), "Élise, \"E\"");
    assert_eq!(cell(0, "Last Name"), "Müller");
    assert_eq!(cell(0, "Position"), "'=1+1");
    assert_eq!(cell(0, "Send Date (UTC)"), "2026-10-19T12:00:00.123456789Z");
    assert_eq!(cell(0, "Modified Date (UTC)"), "2026-10-20T04:00:00Z");
    assert_eq!(
        cell(0, "Accepted events"),
        "0",
        "a stored send date is not delivery evidence"
    );
    for column in [
        "First Name",
        "Last Name",
        "Position",
        "Send Date (UTC)",
        "Modified Date (UTC)",
    ] {
        assert_eq!(cell(1, column), "");
    }
    assert_eq!(cell(2, "Send Date (UTC)"), "");
    assert_eq!(
        cell(2, "Modified Date (UTC)"),
        "",
        "invalid historical dates are not fabricated"
    );
    let excel = reports
        .export(caller(), campaign, selected(), "xlsx", "recipients")
        .await
        .unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(excel.bytes)).unwrap();
    let mut sheet = String::new();
    std::io::Read::read_to_string(
        &mut archive.by_name("xl/worksheets/sheet2.xml").unwrap(),
        &mut sheet,
    )
    .unwrap();
    for (address, expected) in [
        ("K2", "Élise, \"E\""),
        ("L2", "Müller"),
        ("M2", "=1+1"),
        ("N2", "2026-10-19T12:00:00.123456789Z"),
        ("O2", "2026-10-20T04:00:00Z"),
        ("M4", "Analyst"),
    ] {
        let cell = sheet
            .split(&format!("r=\"{address}\" "))
            .nth(1)
            .unwrap()
            .split("</c>")
            .next()
            .unwrap();
        assert!(cell.contains("t=\"inlineStr\""), "{address} must be text");
        assert!(
            cell.contains(&format!("<t>{expected}</t>")),
            "{address}: {cell}"
        );
    }
    assert!(
        !sheet.contains("<f>"),
        "position text must not become a formula"
    );
    assert!(
        reports
            .export(
                Identity { id: 2, epoch: 0 },
                campaign,
                selected(),
                "csv",
                "recipients"
            )
            .await
            .is_err()
    );
    let mut empty = selected();
    empty.search = "not-an-export-recipient".into();
    let empty = reports
        .export(caller(), campaign, empty, "csv", "recipients")
        .await
        .unwrap();
    assert_eq!(
        csv::Reader::from_reader(empty.bytes.as_slice())
            .records()
            .count(),
        0
    );
    sqlx::query("UPDATE results SET position=? WHERE id IN (9001,9002)")
        .bind("x".repeat(17 * 1024 * 1024))
        .execute(&pool)
        .await
        .unwrap();
    let ordinary = reports
        .analysis(caller(), campaign, selected())
        .await
        .unwrap();
    assert_eq!(ordinary.recipient_total, 3);
    assert_eq!(
        ordinary.recipients.len(),
        1,
        "ordinary pagination is independent of export admission"
    );
    for format in ["csv", "xlsx"] {
        assert!(
            matches!(
                reports.export(caller(), campaign, selected(), format, "recipients").await,
                Err(ruphish_application::resources::Error::Invalid(message)) if message.contains("32 MiB")
            ),
            "legacy position fields must count toward the export input bound"
        );
    }
}

#[tokio::test]
async fn observations_filters_and_exports_preserve_evidence_without_credentials_or_locations() {
    let (_directory, database) = fixture().await;
    let pool = database.inspection_pool();
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
        ("link_clicked", "2026-10-19T12:00:00Z"),
        ("form_submitted", "2026-10-19T12:01:00Z"),
        ("email_reported", "2026-10-19T12:02:00Z"),
        ("email_reported", "2026-10-19T12:02:00Z"),
    ] {
        sqlx::query("INSERT INTO events(campaign_id,email,time,kind,metadata,recipient_id) VALUES(?,?,?,?,?,?)")
            .bind(campaign).bind(&email).bind(time).bind(kind)
            .bind(r#"{"password_present":true,"payload":{"password":"PRIVATE-CANARY"},"ip":"192.0.2.42"}"#).bind(id)
            .execute(&pool).await.unwrap();
    }
    let reports = database.reporting();
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
    selected.event = vec![ruphish_domain::events::Kind::EmailReported];
    let selected = reports
        .analysis(caller(), campaign, selected)
        .await
        .unwrap();
    assert_eq!(selected.progression, [0, 1, 1, 1, 1]);
    assert_eq!(selected.unique, [0, 0, 1, 1, 1]);
    assert_eq!(selected.totals, [0, 0, 1, 1, 2]);
    assert_eq!(selected.event_total, 4);
    assert_eq!(selected.recipient_total, 1);
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
        ambiguous.unique,
        [0, 0, 1, 1, 1],
        "a later duplicate address must not detach events from their recorded recipient"
    );
    database.close().await;
}

#[tokio::test]
async fn committed_events_retry_after_receiver_failure_and_rollback_never_sends() {
    let (directory, database) = fixture().await;
    let pool = database.inspection_pool();
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
    let integrations = database.integrations();
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
    sqlx::query("INSERT INTO events(campaign_id,email,time,kind,metadata) VALUES(1,'','2026-10-19T12:00:00Z','email_reported','')").execute(&mut *transaction).await.unwrap();
    transaction.rollback().await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM ruphish_webhook_jobs")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    sqlx::query("INSERT INTO events(campaign_id,email,time,kind,metadata) VALUES(1,'recipient@example.test','2026-10-19T12:00:00Z','email_reported','{\"password\":\"CANARY\"}')").execute(&pool).await.unwrap();
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
    database.close().await;
    let reopened = DatabaseFixture::open(&directory.path().join("reporting.db"))
        .await
        .unwrap();
    let restarted = reopened.integrations();
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
    reopened.close().await;
}

#[tokio::test]
async fn mailbox_edits_are_owner_scoped_revision_checked_and_secret_preserving() {
    let (_directory, database) = fixture().await;
    let integrations = database.integrations();
    let edit = || MailboxEdit {
        name: "Reports".into(),
        enabled: false,
        host: "imap.example.test".into(),
        port: 993,
        username: "service".into(),
        password: Some("MAILBOX-CANARY".into()),
        tls: true,
        ignore_cert_errors: false,
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
    assert!(!mailbox.ignore_cert_errors);
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
    update.ignore_cert_errors = true;
    let updated = integrations
        .save_mailbox(caller(), Some(mailbox.id), update)
        .await
        .unwrap();
    assert!(updated.has_password);
    assert!(updated.ignore_cert_errors);
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
            .delete_mailbox(caller(), mailbox.id, Some(mailbox.revision))
            .await
            .is_err()
    );
    integrations
        .delete_mailbox(caller(), mailbox.id, Some(updated.revision))
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
    sqlx::raw_sql("UPDATE results SET ip='192.0.2.42',latitude=51.5,longitude=7.2; INSERT INTO imap(user_id,host,port,username,password,tls,enabled,folder,imap_freq,modified_date,last_login,ignore_cert_errors) VALUES(1,'imap.example.test',993,'reports','LEGACY-SERVICE-SECRET',1,1,'Reports',60,'2025-01-02T03:04:05.123456789Z','2025-01-03T04:05:06Z',1); INSERT INTO imap(user_id,host,port,enabled) VALUES(1,'',99999,1);").execute(&mut legacy).await.unwrap();
    legacy.close().await.unwrap();
    let db = DatabaseFixture::open(&path).await.unwrap();
    let backup = db.backup_path().unwrap().to_path_buf();
    let store = db.integrations();
    let rows = store.mailboxes(caller()).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .any(|r| r.enabled && r.has_password && r.folder == "Reports")
    );
    assert!(rows.iter().any(|r| !r.enabled));
    let imported = rows.iter().find(|r| r.enabled).unwrap();
    assert!(imported.ignore_cert_errors);
    assert!(
        rows.iter()
            .filter(|r| !r.enabled)
            .all(|r| !r.ignore_cert_errors)
    );
    assert_eq!(imported.modified_date, "2025-01-02T03:04:05.123456789Z");
    assert_eq!(imported.last_login, "2025-01-03T04:05:06Z");
    let locations: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM results WHERE ip!='' OR latitude!=0 OR longitude!=0",
    )
    .fetch_one(&db.inspection_pool())
    .await
    .unwrap();
    assert_eq!(locations, 0);
    let old: i64 = sqlx::query_scalar("SELECT count(*) FROM imap")
        .fetch_one(&db.inspection_pool())
        .await
        .unwrap();
    assert_eq!(old, 0, "one authoritative credential store after migration");
    let ids: Vec<_> = rows.iter().map(|r| r.id).collect();
    db.close().await;
    let reopened = DatabaseFixture::open(&path).await.unwrap();
    let again = reopened.integrations().mailboxes(caller()).await.unwrap();
    assert!(again.iter().find(|r| r.enabled).unwrap().ignore_cert_errors);
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

#[tokio::test]
async fn changing_webhook_during_delivery_preserves_cancellation_and_releases_database() {
    webhook_cancellation(false).await;
}

#[tokio::test]
async fn disabling_webhooks_fences_a_late_network_success_without_changing_configuration() {
    webhook_cancellation(true).await;
}

async fn webhook_cancellation(global: bool) {
    let (_directory, database) = fixture().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/events", listener.local_addr().unwrap());
    let (received, ready) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    let receiver = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut buffer = [0; 4096];
        loop {
            let n = stream.read(&mut buffer).await.unwrap();
            assert_ne!(n, 0);
            request.extend_from_slice(&buffer[..n]);
            if let Some(end) = request.windows(4).position(|s| s == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&request[..end]).to_lowercase();
                let length: usize = headers
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("content-length:")
                            .map(|v| v.trim().parse().unwrap())
                    })
                    .unwrap();
                if request.len() >= end + 4 + length {
                    break;
                }
            }
        }
        received.send(request).unwrap();
        released.await.unwrap();
        stream
            .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
    });
    let integrations = database.integrations();
    let hook = integrations
        .save_webhook(
            caller(),
            None,
            WebhookEdit {
                name: "Slow receiver".into(),
                url: url.clone(),
                secret: Some("signing-test".into()),
                active: true,
                revision: None,
            },
        )
        .await
        .unwrap();
    sqlx::query("INSERT INTO events(campaign_id,email,time,kind,metadata) VALUES(1,'recipient@example.test','2026-10-19T12:00:00Z','email_reported','{\"password\":\"MUST-NOT-SEND\"}')")
        .execute(&database.inspection_pool()).await.unwrap();
    let worker = integrations.clone();
    let operation = tokio::spawn(async move { worker.deliver_webhooks().await });
    let request = ready.await.unwrap();
    assert!(!String::from_utf8_lossy(&request).contains("MUST-NOT-SEND"));
    // These public facade operations use the same single-connection pool as the worker.
    {
        assert!(
            integrations
                .webhook(caller(), hook.id)
                .await
                .unwrap()
                .history[0]
                .finished
                .is_none()
        );
        if global {
            let settings = database.settings();
            let saved = settings.get(caller()).await.unwrap();
            let mut policy = saved.policy;
            policy.webhooks_enabled = false;
            settings
                .save(caller(), saved.revision, policy, 100)
                .await
                .unwrap();
        } else {
            integrations
                .save_webhook(
                    caller(),
                    Some(hook.id),
                    WebhookEdit {
                        name: "Disabled while sending".into(),
                        url,
                        secret: None,
                        active: false,
                        revision: Some(hook.revision),
                    },
                )
                .await
                .unwrap();
        }
    };
    release.send(()).unwrap();
    operation.await.unwrap().unwrap();
    receiver.await.unwrap();
    let detail = integrations.webhook(caller(), hook.id).await.unwrap();
    assert_eq!(
        detail.active, global,
        "global disablement retains the individual configuration"
    );
    assert_eq!(detail.history.len(), 1);
    assert_eq!(detail.history[0].code, "configuration_changed");
    let state: String =
        sqlx::query_scalar("SELECT state FROM ruphish_webhook_jobs WHERE webhook_id=?")
            .bind(hook.id)
            .fetch_one(&database.inspection_pool())
            .await
            .unwrap();
    assert_eq!(
        state, "cancelled",
        "late network success must not resurrect a cancelled job"
    );
    integrations.deliver_webhooks().await.unwrap();
    assert_eq!(
        integrations
            .webhook(caller(), hook.id)
            .await
            .unwrap()
            .history
            .len(),
        1
    );
    database.close().await;
}

#[tokio::test]
async fn mailbox_disconnects_back_off_across_restart_and_invalid_claims_do_not_commit() {
    let (directory, mut database) = fixture().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let mailbox = database
        .integrations()
        .save_mailbox(
            caller(),
            None,
            MailboxEdit {
                name: "Disconnecting receiver".into(),
                enabled: true,
                host: "127.0.0.1".into(),
                port,
                username: "service".into(),
                password: Some("SERVICE-CANARY".into()),
                tls: false,
                ignore_cert_errors: false,
                folder: "INBOX".into(),
                restrict_domain: String::new(),
                delete_reported: false,
                frequency: 300,
                revision: None,
            },
        )
        .await
        .unwrap();
    sqlx::query("UPDATE ruphish_mailboxes SET failures=-1 WHERE id=?")
        .bind(mailbox.id)
        .execute(&database.inspection_pool())
        .await
        .unwrap();
    assert!(database.integrations().poll_mailboxes().await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT lease FROM ruphish_mailboxes WHERE id=?")
            .bind(mailbox.id)
            .fetch_one(&database.inspection_pool())
            .await
            .unwrap(),
        0
    );
    assert!(
        database
            .integrations()
            .mailbox(caller(), mailbox.id)
            .await
            .unwrap()
            .history
            .is_empty()
    );
    sqlx::query("UPDATE ruphish_mailboxes SET failures=0 WHERE id=?")
        .bind(mailbox.id)
        .execute(&database.inspection_pool())
        .await
        .unwrap();
    let receiver = tokio::spawn(async move {
        for _ in 0..7 {
            let (stream, _) = listener.accept().await.unwrap();
            drop(stream); // Disconnect before sending an IMAP greeting or receiving a credential.
        }
    });
    for (index, expected_delay) in [60, 120, 240, 480, 960, 1920, 3600].into_iter().enumerate() {
        if index == 3 {
            database.close().await;
            database = DatabaseFixture::open(&directory.path().join("reporting.db"))
                .await
                .unwrap();
        }
        database
            .integrations()
            .check_mailbox(caller(), mailbox.id)
            .await
            .unwrap();
        database.integrations().poll_mailboxes().await.unwrap();
        let result = database
            .integrations()
            .mailbox(caller(), mailbox.id)
            .await
            .unwrap();
        assert_eq!(result.code, "mailbox_protocol_error");
        assert_eq!(result.next_check - result.last_check, expected_delay);
        assert_eq!(result.history.len(), index + 1);
        assert!(result.history.iter().all(|run| run.finished.is_some()));
        assert!(!result.running);
    }
    receiver.await.unwrap();
    database.close().await;
}

#[tokio::test]
async fn disabled_webhooks_cancel_backlog_and_reenable_only_delivers_new_events() {
    let (_directory, database) = fixture().await;
    let pool = database.inspection_pool();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let hook = database
        .integrations()
        .save_webhook(
            caller(),
            None,
            WebhookEdit {
                name: "Policy receiver".into(),
                url: format!("http://{}/events", listener.local_addr().unwrap()),
                secret: Some("policy-secret".into()),
                active: true,
                revision: None,
            },
        )
        .await
        .unwrap();
    sqlx::query("INSERT INTO events(campaign_id,email,time,kind,metadata) VALUES(1,'','2026-10-24T12:00:00Z','email_reported','')").execute(&pool).await.unwrap();
    let settings = database.settings();
    let before = settings.get(caller()).await.unwrap();
    let mut policy = before.policy;
    policy.webhooks_enabled = false;
    let off = settings
        .save(caller(), before.revision, policy.clone(), 100)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM ruphish_webhook_jobs WHERE webhook_id=?"
        )
        .bind(hook.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "cancelled"
    );
    sqlx::query("INSERT INTO events(campaign_id,email,time,kind,metadata) VALUES(1,'','2026-10-24T12:01:00Z','email_reported','')").execute(&pool).await.unwrap();
    policy.webhooks_enabled = true;
    settings
        .save(caller(), off.revision, policy, 101)
        .await
        .unwrap();
    database.integrations().deliver_webhooks().await.unwrap();
    assert!(
        database
            .integrations()
            .webhook(caller(), hook.id)
            .await
            .unwrap()
            .history
            .is_empty()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM ruphish_webhook_jobs WHERE webhook_id=?"
        )
        .bind(hook.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    let receiver = tokio::spawn(async move {
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
        stream
            .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
    });
    sqlx::query("INSERT INTO events(campaign_id,email,time,kind,metadata) VALUES(1,'','2026-10-24T12:02:00Z','email_reported','')").execute(&pool).await.unwrap();
    database.integrations().deliver_webhooks().await.unwrap();
    receiver.await.unwrap();
    let history = database
        .integrations()
        .webhook(caller(), hook.id)
        .await
        .unwrap()
        .history;
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].code, "accepted");
    database.close().await;
}
