//! Functional SQLite workflows with a controlled blocking worker, not timing guesses.
#![allow(
    clippy::unwrap_used,
    reason = "Isolated fixtures and functional assertions"
)]

use super::*;
use crate::sqlite::{StartupDatabase, domains::Domains};
use ruphish_application::domains::{Observation, Repository};
use std::{sync::Arc, time::Duration};

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
            let db = StartupDatabase::open(&path, "").await.unwrap();
            let now = OffsetDateTime::now_utc().unix_timestamp();
            let domains = Domains::new(db.pool());
            let check = domains.claim(now).await.unwrap().unwrap();
            domains.finish(&check, Observation {
                control: true, dns: true, routing: true, tls: true, code: "verified",
            }, now).await.unwrap();
            sqlx::raw_sql("UPDATE results SET first_name='Alice<script>injectedCanary()</script>'; UPDATE pages SET html='<h1>Hello {{.FirstName}}</h1><form><input name=\"password\" type=\"password\"></form>'; UPDATE ruphish_campaign_policy SET capture_credentials=1,redirect_url='https://training.example.test/finished';")
                .execute(&db.pool()).await.unwrap();
            match scenario {
                "invalid_page" => { sqlx::query("UPDATE pages SET html='{{if}}'").execute(&db.pool()).await.unwrap(); }
                "invalid_url" => { sqlx::query("UPDATE campaigns SET url='https://training.example.test/{{if}}'").execute(&db.pool()).await.unwrap(); }
                "invalid_redirect" => { sqlx::query("UPDATE ruphish_campaign_policy SET redirect_url='{{if}}'").execute(&db.pool()).await.unwrap(); }
                _ => {}
            }
            let before: i64 = sqlx::query_scalar("SELECT count(*) FROM events").fetch_one(&db.pool()).await.unwrap();
            let public = Arc::new(Interactions::new(db.pool()));

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
                let action = if scenario == "submit_complete" {
                    Action::Submit { event_id: "a".repeat(32), password_present: true }
                } else { Action::View };
                operation.interact("training.example.test", "zSDu57f", action, now).await
            });
            tokio::time::timeout(Duration::from_secs(20), async {
                while public.rendering.available_permits() == 8 && !request.is_finished() {
                    tokio::task::yield_now().await;
                }
            }).await.unwrap();
            assert!(!request.is_finished(), "{scenario}: rendering must wait for the blocking worker, including invalid templates");
            assert_eq!(public.rendering.available_permits(), 7);

            // This is the same single-connection pool the request uses. A read AND
            // write must finish while its blocking job remains queued.
            tokio::time::timeout(Duration::from_secs(20), async {
                assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM events").fetch_one(&db.pool()).await.unwrap(), before);
                let change = match scenario {
                    "complete" | "submit_complete" => "UPDATE campaigns SET status='Completed'",
                    "disable" => "UPDATE ruphish_domains SET enabled=0",
                    _ => "UPDATE results SET first_name='Bob'",
                };
                sqlx::query(change).execute(&db.pool()).await.unwrap();
            }).await.unwrap();
            if scenario == "cancel" {
                request.abort();
                assert!(matches!(request.await, Err(error) if error.is_cancelled()));
                assert_eq!(public.rendering.available_permits(), 7, "cancellation must retain capacity until the blocking job ends");
                drop(release);
                worker.await.unwrap();
            } else {
                drop(release);
                worker.await.unwrap();
                let result = request.await.unwrap();
                if scenario == "snapshot" {
                    let Output::Html { html, csp } = result.unwrap() else { unreachable!() };
                    assert!(html.contains("Hello Alice"));
                    assert!(!html.contains("Bob"));
                    assert!(!html.contains("injectedCanary"), "sanitize after substituting recipient values");
                    assert!(html.contains("name=\"password\""));
                    assert!(csp.contains("form-action 'none'"));
                } else {
                    assert!(result.is_err(), "{scenario}: rejected work must not be acknowledged");
                }
            }
            let permits = tokio::time::timeout(Duration::from_secs(20), public.rendering.acquire_many(8)).await.unwrap().unwrap();
            drop(permits);
            let after: i64 = sqlx::query_scalar("SELECT count(*) FROM events").fetch_one(&db.pool()).await.unwrap();
            assert_eq!(after, before + i64::from(scenario == "snapshot"), "{scenario}");
            assert_eq!(sqlx::query_scalar::<_, i64>("SELECT count(*) FROM ruphish_submission_ids").fetch_one(&db.pool()).await.unwrap(), 0);
            db.close().await;
        });
    }
}
