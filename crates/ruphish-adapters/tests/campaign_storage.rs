//! Real SQLite workflows, imported Go state, contention and failed transactions.
#![allow(
    clippy::unwrap_used,
    reason = "Isolated integration-test fixture setup must fail on errors"
)]
mod support;
use ruphish_application::{
    auth::Identity,
    campaign::{Create, DeliveryFailure, FailureKind, Selection, Stage},
};
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use support::DatabaseFixture;
#[path = "../../../tests/support/domain_network.rs"]
mod domain_network;

struct Fixture {
    network: domain_network::Network,
    routing: Arc<Mutex<Option<Routing>>>,
    directory: tempfile::TempDir,
}

#[derive(Clone)]
struct Routing {
    repository: Arc<dyn ruphish_application::domains::Repository>,
    runtime: tokio::runtime::Handle,
    now: i64,
}

// Delivery scenarios retain their independent logical clock. Actual DNS and HTTPS
// verification runs before using their origin; no verified database flag is seeded.
async fn verify_domain(db: &DatabaseFixture, fixture: &Fixture, now: i64) {
    use ruphish_application::domains::Verifier as _;
    let saved = db.settings().get(caller()).await.unwrap();
    if !saved.policy.verification_enabled {
        let mut policy = saved.policy;
        policy.verification_enabled = true;
        db.settings()
            .save(caller(), saved.revision, policy, now)
            .await
            .unwrap();
    }
    let domains = db.domains();
    let existing = domains.list(caller()).await.unwrap().remove(0);
    if ruphish_application::domains::eligibility(existing.enabled, existing.verified_until, now)
        .is_ok()
    {
        return;
    }
    *fixture.routing.lock().unwrap() = Some(Routing {
        repository: db.domains(),
        runtime: tokio::runtime::Handle::current(),
        now,
    });
    let network = &fixture.network;
    let domain = domains
        .request_check(caller(), existing.id, now)
        .await
        .unwrap();
    network.publish(&domain.hostname, &domain.challenge);
    let ca = fixture.directory.path().join("ca.pem");
    std::fs::write(&ca, &network.certificate).unwrap();
    let verifier = ruphish_adapters::domain_verification::DnsHttps::new(
        ruphish_adapters::domain_verification::Config {
            resolvers: vec![network.resolver],
            allowed_networks: vec![network.address.ip().into()],
            trusted_ca_path: Some(ca),
            ..Default::default()
        },
    )
    .unwrap();
    let check = domains.claim(now).await.unwrap().unwrap();
    let observation = verifier.verify(&check).await;
    assert_eq!(observation.code, "verified");
    domains.finish(&check, observation, now).await.unwrap();
    *fixture.routing.lock().unwrap() = None;
}

async fn prepared_database(path: &Path, fixture: &Fixture) -> DatabaseFixture {
    let db = DatabaseFixture::open(path, "").await.unwrap();
    verify_domain(&db, fixture, 1_900_000_000).await;
    db
}
fn caller() -> Identity {
    Identity { id: 1, epoch: 0 }
}
fn input(fixture: &Fixture) -> Create {
    let select = |id| Selection {
        id,
        ..Default::default()
    };
    Create {
        name: "Campaign workflow".into(),
        groups: vec![select(1), select(2)],
        template: select(1),
        page: select(1),
        profile: select(1),
        url: fixture.network.origin("training.example.test"),
        ..Default::default()
    }
}
async fn fixture() -> (Fixture, std::path::PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("state.db");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/campaigns/queued.sqlite3"),
        &path,
    )
    .unwrap();
    let routing = Arc::new(Mutex::new(None::<Routing>));
    let destination = routing.clone();
    let network = domain_network::Network::new(move |host, path| {
        let challenge = path.strip_prefix("/.well-known/ruphish/")?;
        let routing = destination.lock().unwrap().clone()?;
        routing
            .runtime
            .block_on(
                routing
                    .repository
                    .routing_proof(host, challenge, routing.now),
            )
            .unwrap()
    });
    // Adapt only the copied fixture's transport endpoint before migration. The original
    // corpus remains unchanged, and the listener stays alive across database reopens.
    let mut raw = SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&path))
        .await
        .unwrap();
    sqlx::query("UPDATE campaigns SET url=? WHERE url='https://training.example.test'")
        .bind(network.origin("training.example.test"))
        .execute(&mut raw)
        .await
        .unwrap();
    raw.close().await.unwrap();
    (
        Fixture {
            network,
            routing,
            directory,
        },
        path,
    )
}
#[tokio::test]
async fn participant_privacy_removes_legacy_payloads_and_never_exposes_injected_values() {
    let (fixture, path) = fixture().await;
    let secret = "synthetic-participant-login-canary-731";
    let mut raw = SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&path))
        .await
        .unwrap();
    sqlx::query("UPDATE events SET details=?")
        .bind(format!(
            r#"{{"payload":{{"username":"{secret}","password":"{secret}"}},"error":"{secret}"}}"#
        ))
        .execute(&mut raw)
        .await
        .unwrap();
    raw.close().await.unwrap();
    let db = prepared_database(&path, &fixture).await;
    let backup = db.backup_path().unwrap().to_path_buf();
    let mut original = SqliteConnection::connect_with(
        &SqliteConnectOptions::new().filename(backup).read_only(true),
    )
    .await
    .unwrap();
    let preserved: String = sqlx::query_scalar("SELECT details FROM events LIMIT 1")
        .fetch_one(&mut original)
        .await
        .unwrap();
    assert!(
        preserved.contains(secret),
        "recovery original is preserved, not falsely described as sanitized"
    );
    original.close().await.unwrap();
    let details: Vec<String> = sqlx::query_scalar("SELECT details FROM event_history")
        .fetch_all(&db.inspection_pool())
        .await
        .unwrap();
    assert!(details.iter().all(String::is_empty));
    // Defensive read boundary covers crafted rows, nested payloads and type confusion.
    for payload in [
        format!(
            r#"{{"password_present":true,"username_present":false,"payload":{{"password":"{secret}"}},"error":"{secret}","code":"{secret}"}}"#
        ),
        format!(r#"{{"password_present":"{secret}","username_present":{{"value":"{secret}"}}}}"#),
        secret.into(),
    ] {
        sqlx::query("UPDATE events SET metadata=?")
            .bind(payload)
            .execute(&db.inspection_pool())
            .await
            .unwrap();
        let events = db.campaigns().get(caller(), 1).await.unwrap().events;
        for event in events {
            assert!(!event.details.contains(secret));
            assert!(!event.details.contains("payload"));
            if !event.details.is_empty() {
                assert_eq!(
                    serde_json::from_str::<serde_json::Value>(&event.details).unwrap(),
                    serde_json::json!({"password_present":true,"username_present":false})
                );
            }
        }
    }
    db.close().await;
}
#[tokio::test]
async fn named_zone_schedule_uses_calendar_rules_for_folds_gaps_and_non_hour_changes() {
    let (fixture, path) = fixture().await;
    let db = prepared_database(&path, &fixture).await;
    let store = db.campaigns();
    // Independent expected UTC instants; do not derive them with the resolver.
    for (local, zone, expected) in [
        (
            "2026-03-29T01:59:59",
            "Europe/Berlin",
            "2026-03-29T00:59:59Z",
        ),
        (
            "2026-03-29T02:00:00",
            "Europe/Berlin",
            "2026-03-29T01:00:00Z",
        ),
        (
            "2026-03-29T02:30:00",
            "Europe/Berlin",
            "2026-03-29T01:30:00Z",
        ),
        (
            "2026-03-29T03:00:00",
            "Europe/Berlin",
            "2026-03-29T01:00:00Z",
        ),
        (
            "2026-10-25T02:30:00",
            "Europe/Berlin",
            "2026-10-25T00:30:00Z",
        ),
        (
            "2026-10-25T03:00:00",
            "Europe/Berlin",
            "2026-10-25T02:00:00Z",
        ),
        (
            "2026-03-08T02:30:00",
            "America/New_York",
            "2026-03-08T07:30:00Z",
        ),
        (
            "2026-11-01T01:30:00",
            "America/New_York",
            "2026-11-01T05:30:00Z",
        ),
        (
            "2026-10-04T02:15:00",
            "Australia/Lord_Howe",
            "2026-10-03T15:45:00Z",
        ),
        (
            "2026-04-05T01:45:00",
            "Australia/Lord_Howe",
            "2026-04-04T14:45:00Z",
        ),
        (
            "2011-12-30T12:00:00",
            "Pacific/Apia",
            "2011-12-30T22:00:00Z",
        ),
        (
            "2026-01-01T12:00:00",
            "Asia/Kathmandu",
            "2026-01-01T06:15:00Z",
        ),
        ("2026-01-01T12:00:00", "UTC", "2026-01-01T12:00:00Z"),
    ] {
        let resolved = store.resolve_time(local, zone).unwrap();
        assert_eq!(
            jiff::Timestamp::from_second(resolved).unwrap().to_string(),
            expected,
            "{local} {zone}"
        );
    }
    for (local, zone) in [
        ("2026-01-01T12:00:00", "Unknown/Place"),
        ("2026-02-30T12:00:00", "Europe/Berlin"),
        ("not a date", "UTC"),
        ("2026-10-25T02:30:00+01:00", "Europe/Berlin"),
    ] {
        assert!(store.resolve_time(local, zone).is_err(), "{local} {zone}");
    }
    db.close().await;
}
#[tokio::test]
async fn legacy_upgrade_preserves_unknown_work_and_effective_capture_policy() {
    let (fixture, path) = fixture().await;
    let mut raw = SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&path))
        .await
        .unwrap();
    sqlx::query("UPDATE mail_logs SET processing=1,send_attempt=3")
        .execute(&mut raw)
        .await
        .unwrap();
    raw.close().await.unwrap();
    let db = prepared_database(&path, &fixture).await;
    assert!(db.backup_path().is_some());
    let store = db.campaigns();
    verify_domain(&db, &fixture, 2_100_000_000).await;
    assert!(store.claim(2_100_000_000).await.unwrap().is_none());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM ruphish_delivery WHERE state='unknown' AND retries=3"
        )
        .fetch_one(&db.inspection_pool())
        .await
        .unwrap(),
        2
    );
    let before = store.get(caller(), 1).await.unwrap();
    let redirect = before.policy.unwrap().redirect_url;
    sqlx::query("UPDATE pages SET redirect_url='https://changed.example.test/'")
        .execute(&db.inspection_pool())
        .await
        .unwrap();
    assert_eq!(
        store
            .get(caller(), 1)
            .await
            .unwrap()
            .policy
            .unwrap()
            .redirect_url,
        redirect
    );
    db.close().await;
    let db = prepared_database(&path, &fixture).await;
    assert!(db.backup_path().is_none());
    assert!(db.campaigns().claim(2_100_000_000).await.unwrap().is_none());
    db.close().await;
}
#[tokio::test]
async fn creation_is_atomic_deduplicated_and_idempotent_with_stable_policy() {
    let (fixture, path) = fixture().await;
    let db = prepared_database(&path, &fixture).await;
    let store = db.campaigns();
    let mut request = input(&fixture);
    request.send_by = Some(1_900_000_600);
    request.idempotency = Some(("review-123".into(), "independent-body-fingerprint".into()));
    request.policy.redirect_url = Some(String::new());
    let campaign = store
        .create(caller(), request.clone(), 1_900_000_000)
        .await
        .unwrap();
    assert_eq!(campaign.results.len(), 2);
    assert_eq!(campaign.results[0].recipient.first_name, "Alice");
    assert_eq!(campaign.policy.unwrap().redirect_url, "");
    let due: Vec<i64> = sqlx::query_scalar(
        "SELECT due FROM ruphish_delivery WHERE campaign_id=? ORDER BY result_id",
    )
    .bind(campaign.id)
    .fetch_all(&db.inspection_pool())
    .await
    .unwrap();
    assert_eq!(due, vec![1_900_000_000, 1_900_000_300]);
    assert_eq!(
        store
            .create(caller(), request.clone(), 1_900_000_001)
            .await
            .unwrap()
            .id,
        campaign.id
    );
    request.idempotency.as_mut().unwrap().1 = "changed".into();
    assert!(
        store
            .create(caller(), request, 1_900_000_001)
            .await
            .is_err()
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM campaigns")
        .fetch_one(&db.inspection_pool())
        .await
        .unwrap();
    sqlx::raw_sql("CREATE TRIGGER reject_queue BEFORE INSERT ON ruphish_delivery BEGIN SELECT RAISE(ABORT,'synthetic failure'); END;").execute(&db.inspection_pool()).await.unwrap();
    assert!(
        store
            .create(caller(), input(&fixture), 1_900_000_000)
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM campaigns")
            .fetch_one(&db.inspection_pool())
            .await
            .unwrap(),
        count
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM results WHERE campaign_id NOT IN (SELECT id FROM campaigns)"
        )
        .fetch_one(&db.inspection_pool())
        .await
        .unwrap(),
        0
    );
    sqlx::raw_sql("DROP TRIGGER reject_queue;")
        .execute(&db.inspection_pool())
        .await
        .unwrap();
    store.delete(caller(), campaign.id).await.unwrap();
    let mut replay = input(&fixture);
    replay.idempotency = Some(("review-123".into(), "independent-body-fingerprint".into()));
    assert!(store.create(caller(), replay, 1_900_000_000).await.is_err());
    db.close().await;
}
#[tokio::test]
async fn competing_claims_recover_preparation_but_never_resend_expired_submission() {
    let (fixture, path) = fixture().await;
    let db = prepared_database(&path, &fixture).await;
    let second = prepared_database(&path, &fixture).await;
    let a = db.campaigns();
    let b = second.campaigns();
    let campaign = a
        .create(caller(), input(&fixture), 1_900_000_000)
        .await
        .unwrap();
    let (x, y) = tokio::join!(a.claim(1_900_000_000), b.claim(1_900_000_000));
    let claims: Vec<_> = [x.unwrap(), y.unwrap()].into_iter().flatten().collect();
    assert_eq!(
        claims.len(),
        1,
        "one live connection per profile across processes"
    );
    let old = &claims[0];
    let recovered = b.claim(1_900_000_901).await.unwrap().unwrap();
    assert_eq!(old.result_id, recovered.result_id);
    assert_ne!(old.generation, recovered.generation);
    assert!(!a.arm(old, 1_900_000_902).await.unwrap());
    assert!(b.arm(&recovered, 1_900_000_902).await.unwrap());
    // Recovery may pick another recipient, but must not replay the armed one.
    if let Some(other) = a.claim(1_900_002_103).await.unwrap() {
        assert_ne!(other.result_id, recovered.result_id);
    }
    let state: String = sqlx::query_scalar("SELECT state FROM ruphish_delivery WHERE result_id=?")
        .bind(recovered.result_id)
        .fetch_one(&db.inspection_pool())
        .await
        .unwrap();
    assert_eq!(state, "unknown");
    a.complete(caller(), campaign.id, 1_900_002_104)
        .await
        .unwrap();
    // A late confirmed outcome is truth, but cannot reopen the completed campaign.
    b.finish(&recovered, Ok(()), 1_900_002_105).await.unwrap();
    assert_eq!(
        a.get(caller(), campaign.id).await.unwrap().status,
        "Completed"
    );
    assert!(a.claim(1_900_003_000).await.unwrap().is_none());
    second.close().await;
    db.close().await;
}
#[tokio::test]
async fn retries_use_failure_time_and_failed_outcome_commit_never_changes_queue_partially() {
    let (fixture, path) = fixture().await;
    let db = prepared_database(&path, &fixture).await;
    let store = db.campaigns();
    store
        .create(caller(), input(&fixture), 1_900_000_000)
        .await
        .unwrap();
    verify_domain(&db, &fixture, 1_900_010_000).await;
    let claim = store.claim(1_900_010_000).await.unwrap().unwrap();
    assert!(store.arm(&claim, 1_900_010_000).await.unwrap());
    let failure = DeliveryFailure::new(FailureKind::Retryable, Stage::Acceptance, Some(451));
    sqlx::raw_sql("CREATE TRIGGER reject_event BEFORE INSERT ON events BEGIN SELECT RAISE(ABORT,'synthetic commit failure'); END;").execute(&db.inspection_pool()).await.unwrap();
    assert!(
        store
            .finish(&claim, Err(failure.clone()), 1_900_010_001)
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM ruphish_delivery WHERE result_id=?")
            .bind(claim.result_id)
            .fetch_one(&db.inspection_pool())
            .await
            .unwrap(),
        "submitting"
    );
    sqlx::raw_sql("DROP TRIGGER reject_event;")
        .execute(&db.inspection_pool())
        .await
        .unwrap();
    store
        .finish(&claim, Err(failure), 1_900_010_001)
        .await
        .unwrap();
    let due: i64 = sqlx::query_scalar("SELECT due FROM ruphish_delivery WHERE result_id=?")
        .bind(claim.result_id)
        .fetch_one(&db.inspection_pool())
        .await
        .unwrap();
    assert!((1_900_010_121..=1_900_010_133).contains(&due));
    let events: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM event_history WHERE message='Error Sending Email'",
    )
    .fetch_one(&db.inspection_pool())
    .await
    .unwrap();
    store.finish(&claim, Ok(()), 1_900_010_005).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM event_history WHERE message='Error Sending Email'"
        )
        .fetch_one(&db.inspection_pool())
        .await
        .unwrap(),
        events
    );
    db.close().await;
}

#[tokio::test]
async fn schedule_and_result_snapshots_cover_immediate_future_mixed_and_overlap_cases() {
    let (fixture, path) = fixture().await;
    let db = prepared_database(&path, &fixture).await;
    let store = db.campaigns();
    let now = 1_900_000_000;
    for (launch, end, expected) in [
        (None, None, vec!["Sending", "Sending"]),
        (Some(now + 600), None, vec!["Scheduled", "Scheduled"]),
        (None, Some(now + 600), vec!["Sending", "Scheduled"]),
        (Some(now), Some(now), vec!["Sending", "Sending"]),
    ] {
        let mut request = input(&fixture);
        request.launch = launch;
        request.send_by = end;
        let campaign = store.create(caller(), request, now).await.unwrap();
        assert_eq!(
            campaign
                .results
                .iter()
                .map(|r| r.status.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(campaign.results.len(), 2);
        if launch.is_none() {
            assert_eq!(campaign.launch, campaign.created);
        }
        for row in &campaign.results {
            assert_eq!(row.modified_date, campaign.created);
        }
        if end.is_none() {
            assert!(
                campaign
                    .results
                    .iter()
                    .all(|r| r.send_date == campaign.launch)
            );
        }
        assert_eq!(campaign.results[0].recipient.first_name, "Alice");
        for row in &campaign.results {
            assert_eq!(row.rid.len(), 32);
            assert!(row.rid.bytes().all(|b| b.is_ascii_hexdigit()));
        }
        if end == Some(now + 600) {
            assert_eq!(campaign.results[1].send_date, "2030-03-17T17:51:40Z");
        }
        store
            .complete(caller(), campaign.id, now + 1)
            .await
            .unwrap();
    }
    for (launch, end) in [(None, Some(now - 1)), (Some(now + 1), Some(now))] {
        let mut request = input(&fixture);
        request.launch = launch;
        request.send_by = end;
        assert!(store.create(caller(), request, now).await.is_err());
    }
    db.close().await;
}
#[tokio::test]
async fn every_creation_write_boundary_rolls_back_the_whole_launch() {
    let (fixture, path) = fixture().await;
    let db = prepared_database(&path, &fixture).await;
    let store = db.campaigns();
    for table in [
        "campaigns",
        "ruphish_campaign_policy",
        "ruphish_campaign_groups",
        "events",
        "results",
        "ruphish_delivery",
        "ruphish_campaign_launch",
    ] {
        let before: i64 = sqlx::query_scalar("SELECT count(*) FROM campaigns")
            .fetch_one(&db.inspection_pool())
            .await
            .unwrap();
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE TRIGGER reject_write BEFORE INSERT ON {table} BEGIN SELECT RAISE(ABORT,'injected'); END;"))).execute(&db.inspection_pool()).await.unwrap();
        let mut request = input(&fixture);
        request.idempotency = Some(("atomic".into(), "digest".into()));
        assert!(
            store
                .create(caller(), request, 1_900_000_000)
                .await
                .is_err(),
            "{table}"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM campaigns")
                .fetch_one(&db.inspection_pool())
                .await
                .unwrap(),
            before
        );
        for child in [
            "ruphish_campaign_policy",
            "ruphish_campaign_groups",
            "events",
            "results",
            "ruphish_delivery",
            "ruphish_campaign_launch",
        ] {
            let query = format!(
                "SELECT count(*) FROM {child} WHERE campaign_id NOT IN (SELECT id FROM campaigns)"
            );
            assert_eq!(
                sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(query))
                    .fetch_one(&db.inspection_pool())
                    .await
                    .unwrap(),
                0,
                "{table}: {child}"
            );
        }
        sqlx::raw_sql("DROP TRIGGER reject_write")
            .execute(&db.inspection_pool())
            .await
            .unwrap();
    }
    db.close().await;
}
#[tokio::test]
async fn persisted_retry_budget_exhausts_and_cannot_resurrect_after_restart() {
    let (fixture, path) = fixture().await;
    let db = prepared_database(&path, &fixture).await;
    let store = db.campaigns();
    let created = store
        .create(caller(), input(&fixture), 1_900_000_000)
        .await
        .unwrap();
    sqlx::query("UPDATE ruphish_delivery SET state='cancelled' WHERE campaign_id=? AND result_id!=(SELECT min(result_id) FROM ruphish_delivery WHERE campaign_id=?)").bind(created.id).bind(created.id).execute(&db.inspection_pool()).await.unwrap();
    let mut now = 1_900_000_000;
    for attempt in 0..=8u32 {
        verify_domain(&db, &fixture, now).await;
        let claim = store.claim(now).await.unwrap().unwrap();
        assert_eq!(claim.campaign_id, created.id);
        store
            .finish(
                &claim,
                Err(DeliveryFailure::new(
                    FailureKind::Retryable,
                    Stage::Envelope,
                    Some(451),
                )),
                now,
            )
            .await
            .unwrap();
        let row = store
            .deliveries(caller(), created.id, 0, 10)
            .await
            .unwrap()
            .items
            .remove(0);
        if attempt < 8 {
            assert_eq!(row.state, "retry");
            let minimum = 120 * (1i64 << attempt);
            assert!((now + minimum..=now + minimum + minimum / 10).contains(&row.due));
            assert!(store.claim(row.due - 1).await.unwrap().is_none());
            now = row.due;
        } else {
            assert_eq!(row.state, "failed");
            assert_eq!(row.code, "delivery_retry_exhausted");
            assert_eq!(row.retries, 8);
            assert_eq!(row.attempts.len(), 9);
        }
    }
    db.close().await;
    let db = prepared_database(&path, &fixture).await;
    assert!(db.campaigns().claim(now + 10_000).await.unwrap().is_none());
    db.close().await;
}
#[tokio::test]
async fn resource_changes_owner_lock_and_completion_fence_unsubmitted_work() {
    let (fixture, path) = fixture().await;
    let db = prepared_database(&path, &fixture).await;
    let store = db.campaigns();
    let campaign = store
        .create(caller(), input(&fixture), 1_900_000_000)
        .await
        .unwrap();
    let claim = store.claim(1_900_000_000).await.unwrap().unwrap();
    sqlx::query(
        "UPDATE templates SET subject='Changed',modified_date='2031-01-01T00:00:00Z' WHERE id=?",
    )
    .bind(claim.message.template_id)
    .execute(&db.inspection_pool())
    .await
    .unwrap();
    assert!(!store.arm(&claim, 1_900_000_001).await.unwrap());
    let next = store.claim(1_900_000_002).await.unwrap().unwrap();
    assert_eq!(next.message.template.subject, "Changed");
    assert_ne!(claim.generation, next.generation);
    store
        .complete(caller(), campaign.id, 1_900_000_003)
        .await
        .unwrap();
    assert!(!store.arm(&next, 1_900_000_004).await.unwrap());
    let view = store
        .deliveries(caller(), campaign.id, 0, 10)
        .await
        .unwrap();
    assert!(view.items.iter().all(|r| r.state == "cancelled"));
    assert!(
        view.items
            .iter()
            .flat_map(|r| &r.attempts)
            .all(|a| a.state == "cancelled")
    );
    let second = store
        .create(caller(), input(&fixture), 1_900_000_010)
        .await
        .unwrap();
    sqlx::query("UPDATE users SET account_locked=1 WHERE id=1")
        .execute(&db.inspection_pool())
        .await
        .unwrap();
    assert!(store.claim(1_900_000_011).await.unwrap().is_none());
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT code FROM ruphish_delivery WHERE campaign_id=? ORDER BY result_id LIMIT 1"
        )
        .bind(second.id)
        .fetch_one(&db.inspection_pool())
        .await
        .unwrap(),
        "campaign_owner_unavailable"
    );
    db.close().await;
}
#[tokio::test]
async fn migration_quarantines_global_rid_collisions_instead_of_choosing_an_owner() {
    let (fixture, path) = fixture().await;
    let mut raw = SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&path))
        .await
        .unwrap();
    sqlx::raw_sql("INSERT INTO results(campaign_id,user_id,r_id,email,status) SELECT campaign_id,999,r_id,'other@example.test','Scheduled' FROM results LIMIT 1").execute(&mut raw).await.unwrap();
    raw.close().await.unwrap();
    let db = prepared_database(&path, &fixture).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM ruphish_legacy_queue_issues")
            .fetch_one(&db.inspection_pool())
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM mail_logs")
            .fetch_one(&db.inspection_pool())
            .await
            .unwrap(),
        1
    );
    db.close().await;
}

#[tokio::test]
async fn ten_campaigns_keep_resources_and_credentials_isolated_under_bounded_fair_claims() {
    use ruphish_application::delivery::Sender;
    let (fixture, path) = fixture().await;
    let db = prepared_database(&path, &fixture).await;
    let store = db.campaigns();
    let sender =
        ruphish_adapters::smtp::Smtp::new(Default::default(), "contact@example.test".into());
    let mut campaigns = std::collections::HashMap::new();
    for index in 0..10i64 {
        let template=sqlx::query("INSERT INTO templates(user_id,name,subject,text,html,modified_date,envelope_sender) VALUES(1,?,'{{.RId}} - Subject',?,'{{.RId}} - HTML','2030-01-01T00:00:00Z','visible@example.test')").bind(format!("Template {index}")).bind(format!("Content {index}: {{{{.RId}}}}" )).execute(&db.inspection_pool()).await.unwrap().last_insert_rowid();
        let profile=sqlx::query("INSERT INTO smtp(user_id,name,host,username,password,from_address,modified_date,ignore_cert_errors) VALUES(1,?,'127.0.0.1:25',?,?,?,'2030-01-01T00:00:00Z',0)").bind(format!("Profile {index}")).bind(format!("account-{index}")).bind(format!("service-secret-{index}")).bind(format!("transport-{index}@example.test")).execute(&db.inspection_pool()).await.unwrap().last_insert_rowid();
        let mut request = input(&fixture);
        request.template = Selection {
            id: template,
            ..Default::default()
        };
        request.profile = Selection {
            id: profile,
            ..Default::default()
        };
        let campaign = store
            .create(caller(), request, 1_900_000_000)
            .await
            .unwrap();
        campaigns.insert(campaign.id, index);
    }
    let mut seen = std::collections::HashSet::new();
    for batch in 0..5 {
        let mut claims = Vec::new();
        for _ in 0..4 {
            claims.push(store.claim(1_900_000_000 + batch).await.unwrap().unwrap());
        }
        assert!(store.claim(1_900_000_000 + batch).await.unwrap().is_none());
        for claim in claims {
            assert!(seen.insert(claim.result_id));
            let index = campaigns[&claim.campaign_id];
            assert_eq!(
                claim.message.profile.password,
                format!("service-secret-{index}")
            );
            let mail = sender.compose(&claim.message).unwrap();
            assert_eq!(mail.from, format!("transport-{index}@example.test"));
            let parsed = mail_parser::MessageParser::default()
                .parse(&mail.bytes)
                .unwrap();
            assert_eq!(
                parsed.subject().unwrap(),
                format!("{} - Subject", claim.message.rid)
            );
            assert_eq!(
                parsed.body_text(0).unwrap().trim(),
                format!("Content {index}: {}", claim.message.rid)
            );
            assert!(
                parsed
                    .header_raw("From")
                    .unwrap()
                    .contains("visible@example.test")
            );
            assert!(store.arm(&claim, 1_900_000_010 + batch).await.unwrap());
            store
                .finish(&claim, Ok(()), 1_900_000_020 + batch)
                .await
                .unwrap();
        }
    }
    assert_eq!(seen.len(), 20);
    for id in campaigns.keys() {
        let c = store.get(caller(), *id).await.unwrap();
        assert!(c.results.iter().all(|r| r.status == "Email Sent"));
        assert_eq!(
            c.events
                .iter()
                .filter(|e| e.message == "Email Sent")
                .count(),
            2
        );
        for result in &c.results {
            assert!(c.events.iter().any(|e| e.message == "Email Sent"
                && e.email == result.recipient.email
                && e.time == result.send_date));
        }
    }
    assert!(store.claim(1_900_000_100).await.unwrap().is_none());
    db.close().await;
}
#[tokio::test]
async fn outcome_projection_preserves_engagement_and_deleted_campaigns_cannot_be_recreated() {
    let (fixture, path) = fixture().await;
    let db = prepared_database(&path, &fixture).await;
    let store = db.campaigns();
    let c = store
        .create(caller(), input(&fixture), 1_900_000_000)
        .await
        .unwrap();
    let claim = store.claim(1_900_000_000).await.unwrap().unwrap();
    assert!(store.arm(&claim, 1_900_000_000).await.unwrap());
    sqlx::query("UPDATE results SET status='Clicked Link' WHERE id=?")
        .bind(claim.result_id)
        .execute(&db.inspection_pool())
        .await
        .unwrap();
    store.finish(&claim, Ok(()), 1_900_000_001).await.unwrap();
    assert_eq!(
        store.get(caller(), c.id).await.unwrap().results[0].status,
        "Clicked Link"
    );
    let second = store.claim(1_900_000_002).await.unwrap().unwrap();
    assert!(store.arm(&second, 1_900_000_002).await.unwrap());
    store.delete(caller(), c.id).await.unwrap();
    store.finish(&second, Ok(()), 1_900_000_003).await.unwrap();
    assert!(store.get(caller(), c.id).await.is_err());
    for table in [
        "results",
        "events",
        "ruphish_delivery",
        "ruphish_campaign_policy",
        "ruphish_campaign_groups",
    ] {
        let query = format!("SELECT count(*) FROM {table} WHERE campaign_id=?");
        assert_eq!(
            sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(query))
                .bind(c.id)
                .fetch_one(&db.inspection_pool())
                .await
                .unwrap(),
            0
        );
    }
    db.close().await;
}

#[tokio::test]
#[ignore = "Explicit measured acceptance workload, not a wall-clock assertion"]
async fn measure_launch_read_claim_and_compose_at_legacy_audience_sizes() {
    use ruphish_application::delivery::Sender;
    for size in [100, 1_000, 5_000, 10_000] {
        let (fixture, path) = fixture().await;
        let db = prepared_database(&path, &fixture).await;
        let store = db.campaigns();
        sqlx::raw_sql("DELETE FROM group_targets WHERE group_id=1; WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<10000) INSERT INTO targets(first_name,last_name,email,position) SELECT 'Sample',printf('Recipient %d',x),printf('performance-%d@example.test',x),'' FROM n;").execute(&db.inspection_pool()).await.unwrap();
        sqlx::query("INSERT INTO group_targets(group_id,target_id) SELECT 1,id FROM targets WHERE email LIKE 'performance-%' ORDER BY id LIMIT ?").bind(size).execute(&db.inspection_pool()).await.unwrap();
        let mut request = input(&fixture);
        request.groups.truncate(1);
        let clock = std::time::Instant::now();
        let campaign = store
            .create(caller(), request, 1_900_000_000)
            .await
            .unwrap();
        let launch = clock.elapsed();
        assert_eq!(campaign.results.len(), size as usize);
        let clock = std::time::Instant::now();
        let full = store.get(caller(), campaign.id).await.unwrap();
        let read = clock.elapsed();
        assert_eq!(full.results.len(), size as usize);
        let clock = std::time::Instant::now();
        let metadata = store.overview(caller(), 0, 50).await.unwrap();
        let overview = clock.elapsed();
        assert_eq!(
            metadata
                .items
                .iter()
                .find(|r| r.id == campaign.id)
                .unwrap()
                .total,
            size
        );
        let clock = std::time::Instant::now();
        let claim = store.claim(1_900_000_000).await.unwrap().unwrap();
        let claim_time = clock.elapsed();
        let sender = ruphish_adapters::smtp::Smtp::new(Default::default(), String::new());
        let clock = std::time::Instant::now();
        for _ in 0..100 {
            sender.compose(&claim.message).unwrap();
        }
        let compose = clock.elapsed();
        println!(
            "audience={size} launch_ms={} full_read_ms={} overview_us={} claim_us={} compose_100_ms={}",
            launch.as_millis(),
            read.as_millis(),
            overview.as_micros(),
            claim_time.as_micros(),
            compose.as_millis()
        );
        db.close().await;
    }
}

#[tokio::test]
async fn duplicate_attributes_are_reviewable_and_first_recipient_wins_within_a_group() {
    let (fixture, path) = fixture().await;
    let db = prepared_database(&path, &fixture).await;
    let store = db.campaigns();
    sqlx::raw_sql("INSERT INTO targets(id,email,first_name,last_name,position) VALUES(999,'alice@example.test','Duplicate','Discarded','Different'); INSERT INTO group_targets VALUES(1,999);").execute(&db.inspection_pool()).await.unwrap();
    let request = input(&fixture);
    let review = store
        .prepare(caller(), &request, 1_900_000_000)
        .await
        .unwrap();
    assert_eq!(review.conflict_count, 1);
    assert_eq!(review.conflicts[0].0.first_name, "Alice");
    assert_eq!(review.conflicts[0].1.first_name, "Duplicate");
    let campaign = store
        .create(caller(), request, 1_900_000_000)
        .await
        .unwrap();
    assert_eq!(campaign.results.len(), 2);
    assert_eq!(campaign.results[0].recipient.first_name, "Alice");
    db.close().await;
}
