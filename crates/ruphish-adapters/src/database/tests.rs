//! Functional SQLite workflows with a controlled blocking worker, not timing guesses.
#![allow(
    clippy::unwrap_used,
    reason = "Isolated fixtures and functional assertions"
)]

use super::{Config, open};
use crate::outbound::Policy;
use ruphish_application::{
    domains::Observation,
    interactions::{Action, Output},
};
use time::OffsetDateTime;

// Release the worker even if an assertion fails, so runtime shutdown cannot hang.
struct Release(Option<std::sync::mpsc::Sender<()>>);
impl Drop for Release {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}

#[test]
fn administrative_content_uses_shared_rendering_admission_and_owned_inputs() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let dir = tempfile::tempdir().unwrap();
        let db = open(
            Config::Sqlite {
                path: dir.path().join("content.db"),
            },
            Policy::default(),
        )
        .await
        .unwrap();
        let resources = ruphish_application::resources::Service::new(
            db.resources(),
            std::sync::Arc::new(crate::resources::Validate),
            std::sync::Arc::new(crate::imports::Tools {
                policy: Policy::default(),
            }),
            db.content_work(),
        );
        for cancelled in [false, true] {
            let (release, blocked) = std::sync::mpsc::channel();
            let mut release = Release(Some(release));
            let (started, start) = tokio::sync::oneshot::channel();
            let blocker = tokio::task::spawn_blocking(move || {
                let _ = started.send(());
                blocked.recv().unwrap();
            });
            start.await.unwrap();
            let service = resources.clone();
            let request = tokio::spawn(async move {
                service
                    .preview(
                        "<p>Hello {{.FirstName}}</p>".into(),
                        ruphish_application::resources::Recipient {
                            first_name: "Ada".into(),
                            last_name: String::new(),
                            email: "ada@example.test".into(),
                            position: String::new(),
                        },
                    )
                    .await
            });
            while db.rendering.available_permits() == 8 && !request.is_finished() {
                tokio::task::yield_now().await;
            }
            assert!(
                !request.is_finished(),
                "the actual template must wait for a blocking worker"
            );
            assert_eq!(db.rendering.available_permits(), 7);
            if cancelled {
                request.abort();
                assert!(request.await.unwrap_err().is_cancelled());
                assert_eq!(db.rendering.available_permits(), 7);
                release.0.take().unwrap().send(()).unwrap();
            } else {
                release.0.take().unwrap().send(()).unwrap();
                assert!(request.await.unwrap().unwrap().contains("Hello Ada"));
            }
            blocker.await.unwrap();
            let permits = db.rendering.acquire_many(8).await.unwrap();
            drop(permits);
        }
        assert!(resources.page_bundle("invalid ZIP".into()).await.is_err());
        let (html, _) = resources
            .page_preview(
                "<p>Explanation</p><input value=secret>",
                ruphish_application::resources::PagePreview::Learning,
            )
            .await
            .unwrap();
        assert!(html.contains("Explanation"));
        assert!(!html.contains("secret"));
        db.close().await;
    });
}

#[test]
fn rendering_releases_database_preserves_snapshot_and_fences_late_events() {
    for scenario in [
        "snapshot",
        "complete",
        "disable",
        "cancel",
        "invalid_page",
        "invalid_url",
        "invalid_redirect",
        "submit_complete",
        "revoke",
        "submit_revoke",
        "expired",
    ] {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        runtime.block_on(async {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("state.db");
            std::fs::copy(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../tests/fixtures/campaigns/queued.sqlite3"),
                &path,
            ).unwrap();
            let db = open(Config::Sqlite {path,}, Policy::default()).await.unwrap();
            let inspection_pool = match &db.backend { super::Backend::Sqlite(db) => db.pool(), _ => unreachable!() };
            let now = OffsetDateTime::now_utc().unix_timestamp();
            sqlx::query("UPDATE ruphish_settings SET verification_enabled=1").execute(&inspection_pool).await.unwrap();
            let domains = db.domains();
            let check = domains.claim(now).await.unwrap().unwrap();
            domains.finish(&check, Observation {
                control: true, dns: true, routing: true, tls: true, code: "verified",
            }, now).await.unwrap();
            sqlx::raw_sql("UPDATE results SET first_name='Alice<script>injectedCanary()</script>'; UPDATE pages SET html='<h1>Hello {{.FirstName}}</h1><form><input name=\"password\" type=\"password\"></form>'; UPDATE ruphish_campaign_policy SET redirect_url='https://training.example.test/finished';")
                .execute(&inspection_pool).await.unwrap();
            match scenario {
                "invalid_page" => { sqlx::query("UPDATE pages SET html='{{if}}'").execute(&inspection_pool).await.unwrap(); }
                "invalid_url" => { sqlx::query("UPDATE campaigns SET url='https://training.example.test/{{if}}'").execute(&inspection_pool).await.unwrap(); }
                "invalid_redirect" => { sqlx::query("UPDATE ruphish_campaign_policy SET redirect_url='{{if}}'").execute(&inspection_pool).await.unwrap(); }
                _ => {}
            }
            let before: i64 = sqlx::query_scalar("SELECT count(*) FROM event_history").fetch_one(&inspection_pool).await.unwrap();
            let public = db.interactions();

            // Occupy the sole blocking worker. The production operation can finish
            // reading, but no template or HTML work can execute until we release it.
            let (release, wait) = std::sync::mpsc::channel();
            let release = Release(Some(release));
            let (started, ready) = tokio::sync::oneshot::channel();
            let worker = tokio::task::spawn_blocking(move || {
                let _ = started.send(());
                let _ = wait.recv();
            });
            ready.await.unwrap();
            let operation = public.clone();
            let request = tokio::spawn(async move {
                let action = if scenario.starts_with("submit_") {
                    Action::Submit { event_id: "a".repeat(32), password_present: true }
                } else { Action::View };
                operation.interact("training.example.test", "zSDu57f", action, now).await
            });
            while db.rendering.available_permits() == 8 && !request.is_finished() {
                    tokio::task::yield_now().await;
                };
            assert!(!request.is_finished(), "{scenario}: rendering must wait for the blocking worker, including invalid templates");
            assert_eq!(db.rendering.available_permits(), 7);

            // This is the same single-connection pool the request uses. A read AND
            // write must finish while its blocking job remains queued.
            {
                assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM event_history").fetch_one(&inspection_pool).await.unwrap(), before);
                let change = match scenario {
                    "complete" | "submit_complete" => "UPDATE campaigns SET status='Completed'",
                    "disable" => "UPDATE ruphish_domains SET enabled=0",
                    "revoke" | "submit_revoke" => "UPDATE ruphish_domains SET verified_until=0",
                    "expired" => "UPDATE ruphish_domains SET verified_until=1,checked_at=0",
                    _ => "UPDATE results SET first_name='Bob'",
                };
                sqlx::query(change).execute(&inspection_pool).await.unwrap();
            };
            if scenario == "cancel" {
                request.abort();
                assert!(matches!(request.await, Err(error) if error.is_cancelled()));
                assert_eq!(db.rendering.available_permits(), 7, "cancellation must retain capacity until the blocking job ends");
                drop(release);
                worker.await.unwrap();
            } else {
                drop(release);
                worker.await.unwrap();
                let result = request.await.unwrap();
                if matches!(scenario, "snapshot" | "expired") {
                    let Output::Html { html, csp } = result.unwrap() else { unreachable!() };
                    assert!(html.contains("Hello Alice"));
                    assert!(!html.contains("Bob"));
                    assert!(html.contains("injectedCanary"), "text-template output remains authored content under DEC-053");
                    assert!(csp.starts_with("sandbox allow-scripts allow-forms;"));
                    assert!(!csp.contains("allow-same-origin"));
                    assert!(html.contains("name=\"password\""));
                    assert!(csp.contains("form-action 'none'"));
                } else {
                    assert!(result.is_err(), "{scenario}: rejected work must not be acknowledged");
                }
            }
            let permits = db.rendering.acquire_many(8).await.unwrap();
            drop(permits);
            let after: i64 = sqlx::query_scalar("SELECT count(*) FROM event_history").fetch_one(&inspection_pool).await.unwrap();
            assert_eq!(after, before + i64::from(matches!(scenario, "snapshot" | "expired")), "{scenario}");
            assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM ruphish_event_receipts").fetch_one(&inspection_pool).await.unwrap(), 0);
            db.close().await;
        });
    }
}

#[test]
fn preview_rendering_releases_connection_and_preserves_owned_snapshot() {
    for scenario in ["snapshot", "cancel", "invalid"] {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        runtime.block_on(async {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("preview.db");
            std::fs::copy(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../tests/fixtures/campaigns/queued.sqlite3"),
                &path,
            ).unwrap();
            let db = open(Config::Sqlite { path, }, Policy::default()).await.unwrap();
            let inspection_pool = match &db.backend { super::Backend::Sqlite(db) => db.pool(), _ => unreachable!() };
            sqlx::query("UPDATE pages SET html=?")
                .bind(if scenario == "invalid" { "{{if}}" } else { "<h1>Hello {{.FirstName}}</h1>" })
                .execute(&inspection_pool).await.unwrap();
            sqlx::query("INSERT INTO email_requests(user_id,page_id,first_name,last_name,email,position,url,r_id,from_address) SELECT user_id,id,'Alice','','alice@example.test','','https://training.example.test','preview-isolation','sender@example.test' FROM pages LIMIT 1")
                .execute(&inspection_pool).await.unwrap();
            let before: i64 = sqlx::query_scalar("SELECT count(*) FROM event_history").fetch_one(&inspection_pool).await.unwrap();
            let previews = db.previews();
            assert!(previews.contains("preview-isolation").await.unwrap());
            assert!(!previews.contains("not-a-preview").await.unwrap());
            assert!(previews.render("preview-missing").await.is_err());
            let (release, wait) = std::sync::mpsc::channel();
            let release = Release(Some(release));
            let (started, ready) = tokio::sync::oneshot::channel();
            let worker = tokio::task::spawn_blocking(move || {
                let _ = started.send(());
                let _ = wait.recv();
            });
            ready.await.unwrap();
            let request = tokio::spawn(async move { previews.render("preview-isolation").await });
            while db.rendering.available_permits() == 8 && !request.is_finished() {
                    tokio::task::yield_now().await;
                };
            assert!(!request.is_finished());
            assert_eq!(db.rendering.available_permits(), 7);
            // Use the actual single connection, not an independent inspection pool.
            {
                sqlx::query("UPDATE pages SET html='<h1>Changed after snapshot</h1>'")
                    .execute(&inspection_pool).await.unwrap();
                sqlx::query("UPDATE email_requests SET first_name='Bob'")
                    .execute(&inspection_pool).await.unwrap();
            };
            if scenario == "cancel" {
                request.abort();
                assert!(request.await.unwrap_err().is_cancelled());
                assert_eq!(db.rendering.available_permits(), 7, "cancelled callers do not release running work capacity");
                drop(release);
                worker.await.unwrap();
            } else {
                drop(release);
                worker.await.unwrap();
                let result = request.await.unwrap();
                if scenario == "snapshot" {
                    let html = result.unwrap();
                    assert!(html.contains("Hello Alice"));
                    assert!(!html.contains("Bob"));
                    assert!(!html.contains("Changed after snapshot"));
                } else {
                    assert!(result.is_err(), "invalid expressions must fail without partial output");
                }
            }
            let permits = db.rendering.acquire_many(8).await.unwrap();
            drop(permits);
            assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM event_history").fetch_one(&inspection_pool).await.unwrap(), before, "previews must not record campaign events");
            db.close().await;
        });
    }
}
