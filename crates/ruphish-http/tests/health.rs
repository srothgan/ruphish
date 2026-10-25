//! Probe behavior under startup, storage failure, contention, cancellation and drain.
#![allow(clippy::unwrap_used, reason = "Test setup and assertions")]

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use ruphish_application::{
    health::{Service, Storage},
    storage::Error,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::{Notify, Semaphore};
use tower::ServiceExt;

struct Database {
    mode: AtomicUsize,
    calls: AtomicUsize,
    entered: Semaphore,
    release: Notify,
}

#[async_trait::async_trait]
impl Storage for Database {
    async fn check(&self) -> Result<(), Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.mode.load(Ordering::SeqCst) {
            1 => Err(Error::Unavailable),
            2 => {
                self.entered.add_permits(1);
                self.release.notified().await;
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

fn fixture() -> (Arc<Database>, Service, Router) {
    let database = Arc::new(Database {
        mode: AtomicUsize::new(0),
        calls: AtomicUsize::new(0),
        entered: Semaphore::new(0),
        release: Notify::new(),
    });
    let service = Service::new(database.clone());
    let router = ruphish_http::health::router(service.clone());
    (database, service, router)
}

async fn probe(router: &Router, path: &str, expected: StatusCode, body: &str) {
    let response = router
        .clone()
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), expected);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["content-type"], "application/json");
    assert!(!response.headers().contains_key("set-cookie"));
    assert_eq!(
        to_bytes(response.into_body(), 128).await.unwrap().as_ref(),
        body.as_bytes()
    );
}

#[tokio::test]
async fn health_separates_liveness_from_startup_storage_failure_and_shutdown() {
    let (db, service, router) = fixture();
    probe(
        &router,
        "/health/live",
        StatusCode::OK,
        r#"{"status":"live"}"#,
    )
    .await;
    probe(
        &router,
        "/health/ready",
        StatusCode::SERVICE_UNAVAILABLE,
        r#"{"status":"not_ready"}"#,
    )
    .await;
    assert_eq!(db.calls.load(Ordering::SeqCst), 0);
    service.started();
    probe(
        &router,
        "/health/ready",
        StatusCode::OK,
        r#"{"status":"ready"}"#,
    )
    .await;
    db.mode.store(1, Ordering::SeqCst);
    probe(
        &router,
        "/health/ready",
        StatusCode::SERVICE_UNAVAILABLE,
        r#"{"status":"not_ready"}"#,
    )
    .await;
    probe(
        &router,
        "/health/live",
        StatusCode::OK,
        r#"{"status":"live"}"#,
    )
    .await;
    db.mode.store(0, Ordering::SeqCst);
    probe(
        &router,
        "/health/ready",
        StatusCode::OK,
        r#"{"status":"ready"}"#,
    )
    .await;
    service.draining();
    service.started();
    let calls = db.calls.load(Ordering::SeqCst);
    probe(
        &router,
        "/health/ready",
        StatusCode::SERVICE_UNAVAILABLE,
        r#"{"status":"not_ready"}"#,
    )
    .await;
    probe(
        &router,
        "/health/live",
        StatusCode::OK,
        r#"{"status":"live"}"#,
    )
    .await;
    assert_eq!(db.calls.load(Ordering::SeqCst), calls);
}

#[tokio::test]
async fn health_bounds_concurrent_probes_and_rechecks_shutdown_after_io() {
    let (db, service, router) = fixture();
    service.started();
    db.mode.store(2, Ordering::SeqCst);
    let first_router = router.clone();
    let first = tokio::spawn(async move {
        probe(
            &first_router,
            "/health/ready",
            StatusCode::SERVICE_UNAVAILABLE,
            r#"{"status":"not_ready"}"#,
        )
        .await;
    });
    db.entered.acquire().await.unwrap().forget();
    probe(
        &router,
        "/health/ready",
        StatusCode::SERVICE_UNAVAILABLE,
        r#"{"status":"not_ready"}"#,
    )
    .await;
    probe(
        &router,
        "/health/live",
        StatusCode::OK,
        r#"{"status":"live"}"#,
    )
    .await;
    assert_eq!(db.calls.load(Ordering::SeqCst), 1);
    service.draining();
    db.release.notify_one();
    first.await.unwrap();
}

#[tokio::test]
async fn health_deadline_and_request_cancellation_release_probe_capacity() {
    let (db, service, router) = fixture();
    service.started();
    db.mode.store(2, Ordering::SeqCst);
    tokio::time::timeout(
        std::time::Duration::from_secs(3),
        probe(
            &router,
            "/health/ready",
            StatusCode::SERVICE_UNAVAILABLE,
            r#"{"status":"not_ready"}"#,
        ),
    )
    .await
    .unwrap();
    db.entered.acquire().await.unwrap().forget();
    let request = tokio::spawn({
        let service = service.clone();
        async move { service.ready().await }
    });
    db.entered.acquire().await.unwrap().forget();
    request.abort();
    assert!(request.await.unwrap_err().is_cancelled());
    db.mode.store(0, Ordering::SeqCst);
    probe(
        &router,
        "/health/ready",
        StatusCode::OK,
        r#"{"status":"ready"}"#,
    )
    .await;
}

#[tokio::test]
async fn health_supports_head_and_rejects_mutation_methods() {
    let (_, service, router) = fixture();
    service.started();
    for path in ["/health/live", "/health/ready"] {
        for (method, status) in [
            ("HEAD", StatusCode::OK),
            ("POST", StatusCode::METHOD_NOT_ALLOWED),
        ] {
            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), status);
            assert_eq!(response.headers()["cache-control"], "no-store");
            assert!(
                to_bytes(response.into_body(), 128)
                    .await
                    .unwrap()
                    .is_empty()
            );
        }
    }
}
