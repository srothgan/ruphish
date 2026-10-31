//! Real database migrations, concurrency and private interaction workflows.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Isolated functional fixture setup and assertions"
)]
mod support;
use ruphish_application::{
    auth::Identity,
    domains::{Edit, Observation, eligibility},
    interactions::{Action, Output},
};
use support::DatabaseFixture;
fn caller() -> Identity {
    Identity { id: 1, epoch: 0 }
}

#[tokio::test]
async fn migrated_http_and_ip_campaigns_retain_exact_origins_and_follow_verification_policy() {
    for origin in [
        "http://training.example.test:8080",
        "http://192.0.2.20",
        "https://192.0.2.20:8443",
        "http://[2001:db8::20]:8080",
        "https://[2001:db8::20]",
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("legacy.db");
        std::fs::copy(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/campaigns/queued.sqlite3"),
            &path,
        )
        .unwrap();
        let raw = sqlx::SqlitePool::connect_with(
            sqlx::sqlite::SqliteConnectOptions::new().filename(&path),
        )
        .await
        .unwrap();
        let url = format!("{origin}/training?source=legacy");
        sqlx::query("UPDATE campaigns SET url=?")
            .bind(&url)
            .execute(&raw)
            .await
            .unwrap();
        raw.close().await;
        let db = DatabaseFixture::open(&path).await.unwrap();
        let domains = db.domains();
        let domain = domains.list(caller()).await.unwrap().remove(0);
        assert_eq!(domain.origin, origin);
        assert_eq!(domain.verified_until, 0);
        assert!(domain.eligible_until > 0);
        assert_eq!(db.campaigns().get(caller(), 1).await.unwrap().url, url);
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        assert!(
            db.interactions()
                .interact(origin, "zSDu57f", Action::Open, now)
                .await
                .is_ok()
        );
        enable_verification(&db, true).await;
        assert!(!domains.serves(origin).await.unwrap());
        assert!(domains.claim(now + 600).await.unwrap().is_none());
        assert!(
            db.interactions()
                .interact(origin, "zSDu57f", Action::Open, now)
                .await
                .is_err()
        );
        assert!(db.campaigns().claim(2_100_000_000).await.unwrap().is_none());
        assert!(
            domains
                .get(caller(), domain.id)
                .await
                .unwrap()
                .active_check
                .is_none()
        );
        db.close().await;
        let db = DatabaseFixture::open(&path).await.unwrap();
        assert!(!db.domains().serves(origin).await.unwrap());
        let settings = db.settings().get(caller()).await.unwrap();
        let mut policy = settings.policy;
        policy.verification_enabled = false;
        db.settings()
            .save(caller(), settings.revision, policy, now)
            .await
            .unwrap();
        assert!(db.domains().serves(origin).await.unwrap());
        assert!(db.campaigns().claim(2_100_000_000).await.unwrap().is_some());
        db.close().await;
    }
}

#[tokio::test]
async fn gophish_conversion_preserves_history_and_new_retry_receipts_survive_restart() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("observations.db");
    std::fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/campaigns/queued.sqlite3"),
        &path,
    )
    .unwrap();
    let raw =
        sqlx::SqlitePool::connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&path))
            .await
            .unwrap();
    sqlx::raw_sql(r#"
        INSERT INTO webhooks(id,name,url,secret,is_active) VALUES(7,'Observation fixture','http://127.0.0.1:1/unused','',1);
        INSERT INTO results(id,campaign_id,user_id,email,r_id,status) VALUES(3,1,1,'bob@example.test','ambiguous-bob','Email Sent');
        INSERT INTO events(id,campaign_id,email,time,message,details) VALUES
        (40,1,'alice@example.test','2026-10-20T12:00:00Z','Submitted Data','{"password_present":true,"password":"HISTORICAL-PRIVATE-CANARY"}'),
        (41,1,'missing@example.test','2026-10-20T12:00:00Z','Email Opened',''),
        (42,1,'bob@example.test','2026-10-20T12:00:00Z','Email Opened','');
    "#).execute(&raw).await.unwrap();
    raw.close().await;
    let db = DatabaseFixture::open(&path).await.unwrap();
    enable_verification(&db, true).await;
    assert!(db.backup_path().is_some());
    let pool = db.inspection_pool();
    sqlx::raw_sql("INSERT INTO ruphish_event_receipts(result_id,request_id) VALUES(1,'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'); UPDATE ruphish_domains SET checked_at=1900000000,verified_until=1900086400;").execute(&pool).await.unwrap();
    let historical: (String, String, Option<i64>, String) =
        sqlx::query_as("SELECT kind,metadata,recipient_id,source FROM events WHERE id=40")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        historical,
        ("form_submitted".into(), "".into(), Some(1), "legacy".into())
    );
    assert_eq!(
        sqlx::query_scalar::<_, Option<i64>>("SELECT recipient_id FROM events WHERE id=42")
            .fetch_one(&pool)
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        sqlx::query_scalar::<_, Option<i64>>("SELECT event_id FROM ruphish_event_receipts")
            .fetch_one(&pool)
            .await
            .unwrap(),
        None
    );
    let public = db.interactions();
    let now = 1_900_000_000;
    public
        .interact(
            "https://training.example.test",
            "zSDu57f",
            Action::Submit {
                client: Default::default(),
                event_id: "a".repeat(32),
                password_present: true,
            },
            now,
        )
        .await
        .unwrap();
    for key in ["b", "c"] {
        let action = Action::Submit {
            client: Default::default(),
            event_id: key.repeat(32),
            password_present: false,
        };
        let (first, retry) = tokio::join!(
            public.interact(
                "https://training.example.test",
                "zSDu57f",
                action.clone(),
                now
            ),
            public.interact("https://training.example.test", "zSDu57f", action, now)
        );
        first.unwrap();
        retry.unwrap();
        public
            .interact(
                "https://training.example.test",
                "zSDu57f",
                Action::Open,
                now,
            )
            .await
            .unwrap();
        public
            .interact(
                "https://training.example.test",
                "zSDu57f",
                Action::View {
                    client: Default::default(),
                },
                now,
            )
            .await
            .unwrap();
    }
    let analysis = db
        .reporting()
        .analysis(
            caller(),
            1,
            ruphish_application::reporting::Filter {
                limit: 100,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(analysis.totals, [0, 4, 2, 3, 0]);
    assert_eq!(analysis.unique, [0, 1, 1, 1, 0]);
    assert_eq!(analysis.progression[3], 1);
    let links: i64 = sqlx::query_scalar("SELECT count(*) FROM ruphish_event_receipts r JOIN events e ON e.id=r.event_id AND e.recipient_id=r.result_id WHERE e.kind='form_submitted'").fetch_one(&pool).await.unwrap();
    assert_eq!(links, 2);
    let jobs: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ruphish_webhook_jobs WHERE webhook_id=7")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        jobs, 6,
        "conversion does not replay historical webhooks; new events enqueue once"
    );
    drop(public);
    db.close().await;
    let active = std::fs::read(&path).unwrap();
    assert!(
        !active
            .windows(b"HISTORICAL-PRIVATE-CANARY".len())
            .any(|bytes| bytes == b"HISTORICAL-PRIVATE-CANARY")
    );
    let db = DatabaseFixture::open(&path).await.unwrap();
    assert!(db.backup_path().is_none());
    db.interactions()
        .interact(
            "https://training.example.test",
            "zSDu57f",
            Action::Submit {
                client: Default::default(),
                event_id: "b".repeat(32),
                password_present: false,
            },
            now,
        )
        .await
        .unwrap();
    let recipient = db
        .reporting()
        .analysis(
            caller(),
            1,
            ruphish_application::reporting::Filter {
                recipient: Some(1),
                limit: 100,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(recipient.totals, [0, 2, 2, 3, 0]);
    assert_eq!(
        recipient
            .events
            .iter()
            .filter(|event| event.kind
                == ruphish_domain::events::RecordedKind::Known(
                    ruphish_domain::events::Kind::FormSubmitted
                ))
            .count(),
        3
    );
    db.close().await;
}
fn proof() -> Observation {
    Observation {
        control: true,
        dns: true,
        routing: true,
        tls: true,
        code: "verified",
    }
}
async fn enable_verification(db: &DatabaseFixture, webhooks: bool) {
    let saved = db.settings().get(caller()).await.unwrap();
    let mut policy = saved.policy;
    policy.verification_enabled = true;
    policy.webhooks_enabled = webhooks;
    db.settings()
        .save(caller(), saved.revision, policy, 0)
        .await
        .unwrap();
}
async fn fixture() -> (tempfile::TempDir, DatabaseFixture) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");
    std::fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/campaigns/queued.sqlite3"),
        &path,
    )
    .unwrap();
    let db = DatabaseFixture::open(&path).await.unwrap();
    enable_verification(&db, false).await;
    (dir, db)
}
#[tokio::test]
async fn legacy_domains_start_unverified_and_stale_claims_cannot_revalidate_disabled_resources() {
    let (_dir, db) = fixture().await;
    let store = db.domains();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let domains = store.list(caller()).await.unwrap();
    assert_eq!(domains.len(), 1);
    let old = &domains[0];
    assert_eq!(old.origin, "https://training.example.test");
    assert_eq!(old.campaigns, 1);
    assert!(eligibility(old.enabled, old.verified_until, now).is_err());
    assert!(
        store
            .get(Identity { id: 2, epoch: 0 }, old.id)
            .await
            .is_err()
    );
    let (a, b) = tokio::join!(store.claim(now), store.claim(now));
    let claims: Vec<_> = [a.unwrap(), b.unwrap()].into_iter().flatten().collect();
    assert_eq!(claims.len(), 1);
    assert!(store.delete(caller(), old.id, old.revision).await.is_err());
    store
        .save(
            caller(),
            Some(old.id),
            Edit {
                name: old.name.clone(),
                origin: old.origin.clone(),
                enabled: false,
                revision: Some(old.revision),
            },
            now,
        )
        .await
        .unwrap();
    store.finish(&claims[0], proof(), now + 1).await.unwrap();
    let disabled = store.get(caller(), old.id).await.unwrap();
    assert_eq!(disabled.verified_until, 0);
    assert!(
        store
            .save(
                caller(),
                Some(old.id),
                Edit {
                    name: "stale".into(),
                    origin: old.origin.clone(),
                    enabled: true,
                    revision: Some(old.revision)
                },
                now
            )
            .await
            .is_err()
    );
    store
        .save(
            caller(),
            Some(old.id),
            Edit {
                name: old.name.clone(),
                origin: old.origin.clone(),
                enabled: true,
                revision: Some(disabled.revision),
            },
            now + 2,
        )
        .await
        .unwrap();
    let check = store.claim(now + 2).await.unwrap().unwrap();
    store.finish(&check, proof(), now + 3).await.unwrap();
    let verified = store.get(caller(), old.id).await.unwrap();
    assert!(eligibility(true, verified.verified_until, now + 4).is_ok());
    assert!(eligibility(true, verified.verified_until, verified.verified_until).is_err());
    db.close().await;
}
#[tokio::test]
async fn interaction_workflow_is_host_bound_idempotent_monotonic_and_presence_only() {
    let (_dir, db) = fixture().await;
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let store = db.domains();
    let public = db.interactions();
    assert!(
        public
            .interact(
                "https://training.example.test",
                "zSDu57f",
                Action::View {
                    client: Default::default()
                },
                now
            )
            .await
            .is_err()
    );
    let check = store.claim(now).await.unwrap().unwrap();
    store.finish(&check, proof(), now + 1).await.unwrap();
    sqlx::query(
        "UPDATE ruphish_campaign_policy SET redirect_url='https://training.example.test/finished'",
    )
    .execute(&db.inspection_pool())
    .await
    .unwrap();
    sqlx::query("UPDATE pages SET html=?").bind("<style>.form { color: navy }</style><form id='form'><label for='pw'>Password</label><input id='pw' name='password' type='password' autocomplete='current-password'><button>Continue</button></form><script>fetch('https://outside.invalid')</script>").execute(&db.inspection_pool()).await.unwrap();
    assert!(
        public
            .interact(
                "https://other.example.test",
                "zSDu57f",
                Action::View {
                    client: Default::default()
                },
                now + 2
            )
            .await
            .is_err()
    );
    let Output::Html { html, csp } = public
        .interact(
            "https://training.example.test",
            "zSDu57f",
            Action::View {
                client: Default::default(),
            },
            now + 2,
        )
        .await
        .unwrap()
    else {
        unreachable!()
    };
    assert!(html.contains("name=\"password\""));
    assert!(html.contains("autocomplete=\"current-password\""));
    assert!(html.contains("outside.invalid"));
    assert!(csp.starts_with("sandbox allow-scripts allow-forms;"));
    assert!(csp.contains("form-action 'none'"));
    let id = "a".repeat(32);
    for _ in 0..2 {
        assert!(matches!(
            public
                .interact(
                    "https://training.example.test",
                    "zSDu57f",
                    Action::Submit {
                        client: Default::default(),
                        event_id: id.clone(),
                        password_present: true
                    },
                    now + 3
                )
                .await
                .unwrap(),
            Output::Submitted { redirect: Some(_) }
        ));
    }
    public
        .interact(
            "https://training.example.test",
            "zSDu57f",
            Action::Open,
            now + 4,
        )
        .await
        .unwrap();
    public
        .interact(
            "https://training.example.test",
            "zSDu57f",
            Action::Report,
            now + 5,
        )
        .await
        .unwrap();
    let row: (String, bool) =
        sqlx::query_as("SELECT status,reported FROM results WHERE r_id='zSDu57f'")
            .fetch_one(&db.inspection_pool())
            .await
            .unwrap();
    assert_eq!(row, ("Submitted Data".into(), true));
    let values: Vec<String> =
        sqlx::query_scalar("SELECT details FROM event_history WHERE message='Submitted Data'")
            .fetch_all(&db.inspection_pool())
            .await
            .unwrap();
    assert_eq!(values, vec!["{\"password_present\":true}"]);
    sqlx::query("UPDATE campaigns SET status='Completed'")
        .execute(&db.inspection_pool())
        .await
        .unwrap();
    assert!(
        public
            .interact(
                "https://training.example.test",
                "zSDu57f",
                Action::View {
                    client: Default::default()
                },
                now + 6
            )
            .await
            .is_err()
    );
    db.close().await;
}
#[tokio::test]
async fn failed_event_write_rolls_back_submission_deduplication_and_result_state() {
    let (_dir, db) = fixture().await;
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let store = db.domains();
    let check = store.claim(now).await.unwrap().unwrap();
    store.finish(&check, proof(), now + 1).await.unwrap();
    sqlx::raw_sql("CREATE TRIGGER reject_event BEFORE INSERT ON events BEGIN SELECT RAISE(ABORT,'synthetic private trigger canary'); END;").execute(&db.inspection_pool()).await.unwrap();
    let public = db.interactions();
    let error = public
        .interact(
            "https://training.example.test",
            "zSDu57f",
            Action::Submit {
                client: Default::default(),
                event_id: "b".repeat(32),
                password_present: true,
            },
            now + 2,
        )
        .await
        .err()
        .unwrap();
    assert!(!error.to_string().contains("canary"));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM ruphish_event_receipts")
        .fetch_one(&db.inspection_pool())
        .await
        .unwrap();
    assert_eq!(count, 0);
    db.close().await;
}
#[test]
fn final_html_preserves_authored_execution_and_assets_inside_managed_form_sandbox() {
    let source = r#"<style>body {background: navy}</style><form action="https://outside.invalid"><input name="username" autocomplete="username"><input type="password" name="password"><button formaction="https://outside.invalid">Submit</button></form><iframe srcdoc="<script>alert(1)</script>"></iframe><svg onload="fetch('https://outside.invalid')"></svg><img src="https://outside.invalid/image"><meta http-equiv="refresh" content="0;url=https://outside.invalid"><script>canaryScript()</script>"#;
    let doc = ruphish_adapters::landing::render(
        source,
        ruphish_adapters::landing::Mode::Landing {
            context: &ruphish_adapters::resources::sample(),
            endpoint: Some("https://training.example.test/?rid=abc"),
        },
    )
    .unwrap();
    assert!(doc.html.contains("name=\"username\""));
    assert!(doc.html.contains("name=\"password\""));
    assert!(doc.html.contains("background: navy"));
    assert!(doc.html.contains("canaryScript"));
    assert!(doc.html.contains("https://outside.invalid/image"));
    assert!(!doc.html.contains("http-equiv=\"refresh\""));
    assert!(!doc.html.contains("formaction="));
    assert!(!doc.html.contains("<iframe"));
    assert_eq!(doc.html.matches("<script ").count(), 1);
    assert!(doc.csp.contains("form-action 'none'"));
    assert!(doc.csp.starts_with("sandbox allow-scripts allow-forms;"));
    assert!(!doc.csp.contains("allow-same-origin"));
    assert!(
        doc.html.find("data-ruphish-collector").unwrap() < doc.html.find("canaryScript").unwrap()
    );
}

#[path = "../../../tests/support/connection.rs"]
mod connection;
#[path = "../../../tests/support/domain_network.rs"]
mod domain_network;

#[tokio::test(flavor = "multi_thread")]
async fn actual_dns_and_https_require_exact_control_trust_routing_and_permitted_destinations() {
    use ruphish_adapters::domain_verification::{Config, DnsHttps};
    use ruphish_application::domains::Verifier;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let (dir, db) = fixture().await;
    let store = db.domains();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let mut check = store.claim(now).await.unwrap().unwrap();
    let expected = check.routing_proof.clone();
    let hits = Arc::new(AtomicUsize::new(0));
    let observed = hits.clone();
    let network = domain_network::Network::new(move |host, path| {
        observed.fetch_add(1, Ordering::SeqCst);
        assert_eq!(host.split_once(':').unwrap().0, "training.example.test");
        assert!(path.starts_with("/.well-known/ruphish/"));
        Some(expected.clone())
    });
    check.domain.origin = network.origin("training.example.test");
    let ca = dir.path().join("ca.pem");
    std::fs::write(&ca, &network.certificate).unwrap();
    let config = Config {
        resolvers: vec![network.resolver],
        allowed_networks: vec![format!("{}/32", network.address.ip()).parse().unwrap()],
        trusted_ca_path: Some(ca),
        ..Default::default()
    };
    network.publish("training.example.test", "incorrect-challenge");
    let observation = DnsHttps::new(config.clone()).unwrap().verify(&check).await;
    assert_eq!(observation.code, "domain_txt_mismatch");
    assert_eq!(hits.load(Ordering::SeqCst), 0);
    network.publish("training.example.test", &check.domain.challenge);
    let mut restricted = config.clone();
    restricted.allowed_networks.clear();
    let observation = DnsHttps::new(restricted).unwrap().verify(&check).await;
    assert_eq!(observation.code, "domain_destination_denied");
    assert!(observation.control);
    assert!(!observation.dns);
    assert_eq!(hits.load(Ordering::SeqCst), 0);
    let mut untrusted = config.clone();
    untrusted.trusted_ca_path = None;
    let observation = DnsHttps::new(untrusted).unwrap().verify(&check).await;
    assert_eq!(observation.code, "domain_https_unavailable");
    assert!(!observation.tls);
    assert_eq!(hits.load(Ordering::SeqCst), 0);
    check.domain.origin = network.origin("wrong.example.test");
    network.publish("wrong.example.test", &check.domain.challenge);
    let observation = DnsHttps::new(config.clone()).unwrap().verify(&check).await;
    assert_eq!(observation.code, "domain_https_unavailable");
    assert_eq!(hits.load(Ordering::SeqCst), 0);
    check.domain.origin = network.origin("training.example.test");
    let observation = DnsHttps::new(config.clone()).unwrap().verify(&check).await;
    assert_eq!(observation.code, "verified");
    assert!(observation.control && observation.dns && observation.routing && observation.tls);
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    check.routing_proof = "a-different-instance-proof".into();
    assert_eq!(
        DnsHttps::new(config.clone())
            .unwrap()
            .verify(&check)
            .await
            .code,
        "domain_routing_mismatch"
    );
    check.domain.challenge_expires = now;
    assert_eq!(
        DnsHttps::new(config).unwrap().verify(&check).await.code,
        "domain_challenge_expired"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 2);
    db.close().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn domain_probe_does_not_follow_redirects_or_accept_oversized_proofs() {
    use ruphish_adapters::domain_verification::{Config, DnsHttps};
    use ruphish_application::domains::Verifier;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let (dir, db) = fixture().await;
    let store = db.domains();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let mut check = store.claim(now).await.unwrap().unwrap();
    for (status, body) in [(302, check.routing_proof.clone()), (200, "x".repeat(129))] {
        let hits = Arc::new(AtomicUsize::new(0));
        let observed = hits.clone();
        let network = domain_network::Network::with_status(status, move |_, _| {
            observed.fetch_add(1, Ordering::SeqCst);
            Some(body.clone())
        });
        check.domain.origin = network.origin("training.example.test");
        network.publish("training.example.test", &check.domain.challenge);
        let ca = dir.path().join("ca.pem");
        std::fs::write(&ca, &network.certificate).unwrap();
        let verifier = DnsHttps::new(Config {
            resolvers: vec![network.resolver],
            allowed_networks: vec![format!("{}/32", network.address.ip()).parse().unwrap()],
            trusted_ca_path: Some(ca),
            ..Default::default()
        })
        .unwrap();
        let observation = verifier.verify(&check).await;
        assert_eq!(observation.code, "domain_routing_mismatch");
        assert!(!observation.routing);
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }
    db.close().await;
}

#[tokio::test]
async fn event_limits_preserve_retry_idempotency_and_roll_back_unrecorded_submissions() {
    let (_dir, db) = fixture().await;
    let store = db.domains();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let check = store.claim(now).await.unwrap().unwrap();
    store.finish(&check, proof(), now + 1).await.unwrap();
    let public = db.interactions();
    let id = "a".repeat(32);
    let submit = || Action::Submit {
        client: Default::default(),
        event_id: id.clone(),
        password_present: true,
    };
    public
        .interact(
            "https://training.example.test",
            "zSDu57f",
            submit(),
            now + 2,
        )
        .await
        .unwrap();
    let details: String = sqlx::query_scalar(
        "SELECT details FROM event_history WHERE message='Submitted Data' ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&db.inspection_pool())
    .await
    .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&details).unwrap(),
        serde_json::json!({"password_present":true})
    );
    sqlx::query("WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<10000) INSERT INTO events(campaign_id,email,time,kind,metadata,recipient_id) SELECT r.campaign_id,r.email,'2030-01-01T00:00:00Z','email_opened','',r.id FROM n JOIN results r ON r.r_id='zSDu57f' WHERE n.x <= 10000-(SELECT count(*) FROM events WHERE recipient_id=r.id)").execute(&db.inspection_pool()).await.unwrap();
    public
        .interact(
            "https://training.example.test",
            "zSDu57f",
            submit(),
            now + 3,
        )
        .await
        .unwrap();
    assert!(
        public
            .interact(
                "https://training.example.test",
                "zSDu57f",
                Action::Submit {
                    client: Default::default(),
                    event_id: "b".repeat(32),
                    password_present: false
                },
                now + 3
            )
            .await
            .is_err()
    );
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM event_history WHERE message='Submitted Data'")
            .fetch_one(&db.inspection_pool())
            .await
            .unwrap();
    assert_eq!(count, 1);
    db.close().await;
}

#[test]
fn canonical_domain_origins_reject_ambiguous_authorities_and_preserve_valid_idna() {
    use ruphish_application::domains::origin;
    assert_eq!(
        origin("https://TRAINING.example.test.:443", false)
            .unwrap()
            .value,
        "https://training.example.test"
    );
    assert_eq!(
        origin("https://b\u{fc}cher.example", false).unwrap().host,
        "xn--bcher-kva.example"
    );
    for (input, expected, verifiable) in [
        (
            "http://TRAINING.example.test.:80",
            "http://training.example.test",
            false,
        ),
        ("https://192.0.2.1:443", "https://192.0.2.1", false),
        ("http://192.0.2.1:8080", "http://192.0.2.1:8080", false),
        (
            "https://[2001:0db8::1]:8443",
            "https://[2001:db8::1]:8443",
            false,
        ),
        ("http://[::1]:80", "http://[::1]", false),
        (
            "https://training.example.test:8443",
            "https://training.example.test:8443",
            true,
        ),
    ] {
        let parsed = origin(input, false).unwrap();
        assert_eq!(parsed.value, expected);
        assert_eq!(parsed.verification_supported(), verifiable);
        assert_eq!(
            origin(
                &format!("{input}/{{{{.FirstName}}}}?email={{{{.Email}}}}"),
                true
            )
            .unwrap(),
            parsed
        );
    }
    for invalid in [
        "https://training.example.test..",
        "https:///training.example.test",
        "https://training.example.test:",
        "https://@training.example.test",
        "https://127.1",
        "http://0x7f000001",
        "http://0177.0.0.1",
        "https://[fe80::1%25eth0]",
        "https://[::1]:65536",
        "ftp://training.example.test",
        "http://training.example.test:{{.Position}}",
        "https://training.example.test\\@evil.example.test",
        "https://user@training.example.test",
        "https://training.example.test/path",
        "https://training.example.test?x=1",
        "https://training.example.test#fragment",
        "https://*.example.test",
        "https://{{.Email}}.example.test",
        "https://training.example.test:0",
        " https://training.example.test",
    ] {
        assert!(origin(invalid, false).is_err(), "{invalid}");
    }
    assert!(origin("https://training.example.test/path/{{.FirstName}}", true).is_ok());
}

#[test]
fn whole_document_layout_attributes_and_embedded_images_survive_final_policy() {
    let source = r#"<!doctype html><html lang="fr"><head><style>.welcome { color: navy }</style></head><body class="welcome" style="margin: 2rem" onload="bad()"><form><input name="email"></form><img src="data:image/png;base64,iVBORw0KGgo="><a href="data:text/html,unsafe">Unsafe link</a></body></html>"#;
    let document = ruphish_adapters::landing::render(
        source,
        ruphish_adapters::landing::Mode::Landing {
            context: &ruphish_adapters::resources::sample(),
            endpoint: None,
        },
    )
    .unwrap();
    assert!(document.html.contains("<html lang=\"fr\">"));
    let parsed = scraper::Html::parse_document(&document.html);
    let body = parsed
        .select(&scraper::Selector::parse("body").unwrap())
        .next()
        .unwrap();
    assert_eq!(body.value().attr("class"), Some("welcome"));
    assert_eq!(body.value().attr("style"), Some("margin: 2rem"));
    assert_eq!(body.value().attr("onload"), Some("bad()"));
    assert!(
        document
            .html
            .contains("src=\"data:image/png;base64,iVBORw0KGgo=\"")
    );
    assert!(document.html.contains(".welcome { color: navy }"));
    assert!(document.html.contains("onload"));
    assert!(document.html.contains("data:text/html"));
    assert!(document.html.contains("data-ruphish-collector"));
    assert!(document.html.contains("const endpoint=null"));
}

#[tokio::test]
async fn domain_expiry_and_disabling_fence_queue_claim_and_submission_without_losing_work() {
    let (_dir, db) = fixture().await;
    let store = db.domains();
    let queue = db.campaigns();
    let now = 2_100_000_000; // Both independent 2035 fixture deliveries are due by this instant.
    assert!(queue.claim(now).await.unwrap().is_none());
    let domain = store.list(caller()).await.unwrap().remove(0);
    let check = store.claim(now).await.unwrap().unwrap();
    store.finish(&check, proof(), now + 1).await.unwrap();
    let claim = queue.claim(now + 2).await.unwrap().unwrap();
    assert!(!queue.arm(&claim, now + 86401).await.unwrap());
    queue.release(&claim, now + 86401).await.unwrap();
    assert!(queue.claim(now + 86401).await.unwrap().is_none());
    let recheck = store.claim(now + 86402).await.unwrap().unwrap();
    store.finish(&recheck, proof(), now + 86403).await.unwrap();
    let resumed = queue.claim(now + 86404).await.unwrap().unwrap();
    store
        .save(
            caller(),
            Some(domain.id),
            Edit {
                name: domain.name,
                origin: domain.origin,
                enabled: false,
                revision: Some(domain.revision),
            },
            now + 86405,
        )
        .await
        .unwrap();
    assert!(!queue.arm(&resumed, now + 86406).await.unwrap());
    queue.release(&resumed, now + 86406).await.unwrap();
    let state: String = sqlx::query_scalar("SELECT state FROM ruphish_delivery WHERE result_id=?")
        .bind(resumed.result_id)
        .fetch_one(&db.inspection_pool())
        .await
        .unwrap();
    assert_eq!(state, "ready");
    assert!(queue.claim(now + 86407).await.unwrap().is_none());
    db.close().await;
}

#[tokio::test]
async fn expired_leases_and_rotated_challenges_cannot_publish_stale_evidence_or_cross_owner_claims()
{
    let (_dir, db) = fixture().await;
    let store = db.domains();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let first = store.claim(now).await.unwrap().unwrap();
    let recovered = store.claim(now + 91).await.unwrap().unwrap();
    store.finish(&first, proof(), now + 92).await.unwrap();
    assert_eq!(
        store
            .get(caller(), first.domain.id)
            .await
            .unwrap()
            .verified_until,
        0
    );
    store.finish(&recovered, proof(), now + 92).await.unwrap();
    let verified = store.get(caller(), first.domain.id).await.unwrap();
    assert!(eligibility(true, verified.verified_until, now + 93).is_ok());
    sqlx::raw_sql("INSERT INTO users(id,username,hash,api_key,role_id,account_locked,password_change_required) SELECT 2,'second-domain-owner',hash,'second-domain-owner-key',role_id,0,0 FROM users WHERE id=1; INSERT INTO ruphish_auth_state(user_id,epoch) VALUES(2,0);").execute(&db.inspection_pool()).await.unwrap();
    let other = Identity { id: 2, epoch: 0 };
    assert!(store.get(other, verified.id).await.is_err());
    let second = store
        .save(
            other,
            None,
            Edit {
                name: "Other owner".into(),
                origin: verified.origin.clone(),
                enabled: true,
                revision: None,
            },
            now + 93,
        )
        .await
        .unwrap();
    let conflicting = store.claim(now + 94).await.unwrap().unwrap();
    assert_eq!(conflicting.domain.id, second.id);
    store.finish(&conflicting, proof(), now + 95).await.unwrap();
    let rejected = store.get(other, second.id).await.unwrap();
    assert_eq!(rejected.code, "domain_claim_conflict");
    assert_eq!(rejected.verified_until, 0);
    let rotated = store
        .request_check(caller(), verified.id, verified.challenge_expires + 1)
        .await
        .unwrap();
    assert_ne!(rotated.challenge, verified.challenge);
    assert!(
        store
            .routing_proof(
                "https://training.example.test",
                &verified.challenge,
                verified.challenge_expires + 2
            )
            .await
            .unwrap()
            .is_none()
    );
    store
        .finish(&recovered, proof(), verified.challenge_expires + 2)
        .await
        .unwrap();
    assert_eq!(
        store
            .get(caller(), verified.id)
            .await
            .unwrap()
            .verified_until,
        verified.verified_until
    );
    db.close().await;
}

#[tokio::test]
async fn verification_history_is_coalesced_atomic_bounded_and_latest_failure_revokes_success() {
    use ruphish_application::domains::verification_status;
    let (_dir, db) = fixture().await;
    let store = db.domains();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let id = store.list(caller()).await.unwrap()[0].id;
    let initial = store.request_check(caller(), id, now).await.unwrap();
    assert_eq!(initial.active_check.as_ref().unwrap().state, "queued");
    let active = initial.active_check.unwrap().id;
    let (a, b) = tokio::join!(
        store.request_check(caller(), id, now),
        store.request_check(caller(), id, now)
    );
    assert_eq!(a.unwrap().active_check.unwrap().id, active);
    assert_eq!(b.unwrap().active_check.unwrap().id, active);
    let check = store.claim(now).await.unwrap().unwrap();
    assert_eq!(
        store
            .get(caller(), id)
            .await
            .unwrap()
            .active_check
            .unwrap()
            .state,
        "running"
    );
    store.finish(&check, proof(), now + 1).await.unwrap();
    store.finish(&check, proof(), now + 2).await.unwrap();
    let passed = store.get(caller(), id).await.unwrap();
    assert!(passed.active_check.is_none());
    assert_eq!(passed.history.len(), 1);
    assert_eq!(passed.history[0].id, active);
    assert_eq!(passed.history[0].state, "passed");
    assert_eq!(passed.history[0].started_at, Some(now));
    assert_eq!(passed.history[0].finished_at, Some(now + 1));
    assert_eq!(passed.verified_until, now + 1 + 86_400);
    assert!((21_300..=21_900).contains(&(passed.next_check_at - now - 1)));
    assert_eq!(
        verification_status(passed.verified_until, passed.checked_at, now + 2),
        "verified"
    );
    assert_eq!(
        verification_status(passed.verified_until, passed.checked_at, now + 86_401),
        "expired"
    );
    let queued = store.request_check(caller(), id, now + 2).await.unwrap();
    assert_eq!(queued.active_check.unwrap().scheduled_at, now + 31);
    assert!(store.claim(now + 30).await.unwrap().is_none());
    let failed = store.claim(now + 31).await.unwrap().unwrap();
    // A failed history write must roll back the domain observation too.
    sqlx::raw_sql("CREATE TRIGGER fail_history BEFORE UPDATE OF outcome ON ruphish_domain_checks BEGIN SELECT RAISE(ABORT,'fixture'); END;").execute(&db.inspection_pool()).await.unwrap();
    assert!(
        store
            .finish(
                &failed,
                Observation {
                    code: "domain_txt_mismatch",
                    ..Default::default()
                },
                now + 32
            )
            .await
            .is_err()
    );
    let unchanged = store.get(caller(), id).await.unwrap();
    assert_eq!(unchanged.verified_until, passed.verified_until);
    assert_eq!(unchanged.history.len(), 1);
    assert!(unchanged.active_check.is_some());
    sqlx::raw_sql("DROP TRIGGER fail_history")
        .execute(&db.inspection_pool())
        .await
        .unwrap();
    store
        .finish(
            &failed,
            Observation {
                code: "domain_txt_mismatch",
                ..Default::default()
            },
            now + 32,
        )
        .await
        .unwrap();
    let rejected = store.get(caller(), id).await.unwrap();
    assert_eq!(rejected.verified_until, 0);
    assert!(eligibility(true, rejected.verified_until, now + 33).is_err());
    assert_eq!(
        verification_status(rejected.verified_until, rejected.checked_at, now + 33),
        "failed"
    );
    assert_eq!(rejected.history[0].state, "failed");
    assert!(rejected.active_check.is_none());
    let mut time = now + 63;
    for index in 0..12 {
        store.request_check(caller(), id, time).await.unwrap();
        let run = store.claim(time).await.unwrap().unwrap();
        let observation = if index % 2 == 0 {
            proof()
        } else {
            Observation {
                code: "domain_check_timeout",
                ..Default::default()
            }
        };
        store.finish(&run, observation, time + 1).await.unwrap();
        time += 32;
    }
    let final_state = store.get(caller(), id).await.unwrap();
    assert_eq!(final_state.history.len(), 10);
    assert_eq!(final_state.history[0].state, "timed_out");
    assert_eq!(final_state.verified_until, 0);
    assert!(final_state.history.windows(2).all(|w| w[0].id > w[1].id));
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ruphish_domain_checks WHERE domain_id=?")
            .bind(id)
            .fetch_one(&db.inspection_pool())
            .await
            .unwrap();
    assert_eq!(count, 10);
    db.close().await;
}

#[tokio::test]
async fn queued_and_running_checks_survive_reopen_and_expired_workers_cannot_publish_late_results()
{
    let (dir, db) = fixture().await;
    let store = db.domains();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let id = store.list(caller()).await.unwrap()[0].id;
    let before = store.request_check(caller(), id, now).await.unwrap();
    let queued_id = before.active_check.unwrap().id;
    db.close().await;
    let db = DatabaseFixture::open(&dir.path().join("state.db"))
        .await
        .unwrap();
    let store = db.domains();
    assert_eq!(
        store
            .get(caller(), before.id)
            .await
            .unwrap()
            .active_check
            .unwrap()
            .id,
        queued_id
    );
    let old = store.claim(now).await.unwrap().unwrap();
    db.close().await;
    let db = DatabaseFixture::open(&dir.path().join("state.db"))
        .await
        .unwrap();
    let store = db.domains();
    let restored = store
        .get(caller(), before.id)
        .await
        .unwrap()
        .active_check
        .unwrap();
    assert_eq!(restored.id, queued_id);
    assert_eq!(restored.state, "running");
    let new = store.claim(now + 91).await.unwrap().unwrap();
    let recovered = store.get(caller(), before.id).await.unwrap();
    assert_ne!(recovered.active_check.unwrap().id, queued_id);
    assert_eq!(recovered.history[0].state, "timed_out");
    assert_eq!(recovered.history[0].code, "domain_worker_timeout");
    store.finish(&old, proof(), now + 92).await.unwrap();
    assert_eq!(
        store.get(caller(), before.id).await.unwrap().verified_until,
        0
    );
    store.finish(&new, proof(), now + 92).await.unwrap();
    let complete = store.get(caller(), before.id).await.unwrap();
    assert_eq!(complete.history.len(), 2);
    assert_eq!(complete.history[0].state, "passed");
    assert!(complete.active_check.is_none());
    db.close().await;
}

#[tokio::test]
async fn pre_release_ruphish_history_is_rejected_without_changes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("old.db");
    std::fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/campaigns/queued.sqlite3"),
        &path,
    )
    .unwrap();
    let pool =
        sqlx::SqlitePool::connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&path))
            .await
            .unwrap();
    sqlx::raw_sql("CREATE TABLE ruphish_schema(version INTEGER PRIMARY KEY NOT NULL,name TEXT NOT NULL); INSERT INTO ruphish_schema VALUES(1,'authentication-v1');").execute(&pool).await.unwrap();
    pool.close().await;
    let original = std::fs::read(&path).unwrap();
    assert!(DatabaseFixture::open(&path).await.is_err());
    assert_eq!(std::fs::read(&path).unwrap(), original);
}
