//! Observable sender history, invalidation and launch policy across real providers.
use super::*;
use ruphish_application::profile_tests::{Kind as TestKind, Outcome, Request, Start};

async fn exercise(db: &Database) {
    let who = company_settings::account(db).await.identity;
    let now = 1_900_000_000;
    db.domains()
        .save(
            who,
            None,
            domains::Edit {
                name: "Test origin".into(),
                origin: "https://profile-tests.example.test".into(),
                enabled: true,
                revision: None,
            },
            now,
        )
        .await
        .unwrap();
    let mut launch = company_settings::input(db, who, "https://profile-tests.example.test").await;
    let id = launch.profile.id;
    let profile = db.resources().get(who, Kind::Profile, id).await.unwrap();
    let tests = db.profile_tests();
    let initial = tests.read(who, id, now).await.unwrap();
    assert!(initial.eligible && !initial.required);
    assert!(initial.history.is_empty());
    let saved = db.settings().get(who).await.unwrap();
    let mut policy = saved.policy;
    policy.sending_profile_test_required = true;
    db.settings()
        .save(who, saved.revision, policy, now)
        .await
        .unwrap();
    assert!(!tests.read(who, id, now).await.unwrap().eligible);
    assert!(matches!(
        db.campaigns().prepare(who, &launch, now).await,
        Err(Error::ProfileTestRequired)
    ));
    let mut scheduled = launch.clone();
    scheduled.name = "Scheduled sender policy".into();
    scheduled.launch = Some(now + 3600);
    assert!(matches!(
        db.campaigns().create(who, scheduled.clone(), now).await,
        Err(Error::ProfileTestRequired)
    ));
    let request = |identity: &str, kind| Request {
        id: identity.to_owned(),
        revision: profile.revision,
        kind,
        email: (kind == TestKind::Email).then(|| "operator@example.test".into()),
    };
    let started = tests
        .begin(
            who,
            id,
            request("connection-profile-test", TestKind::Connection),
            now,
        )
        .await
        .unwrap();
    assert!(matches!(started, Start::Started(_, _)));
    assert!(matches!(
        tests
            .begin(
                who,
                id,
                request("overlapping-profile-test", TestKind::Email),
                now
            )
            .await,
        Err(Error::ProfileTestBusy)
    ));
    tests
        .finish(
            "connection-profile-test",
            Outcome::Passed,
            "",
            None,
            now + 1,
        )
        .await
        .unwrap();
    assert!(
        !tests.read(who, id, now + 1).await.unwrap().eligible,
        "Connection-only success cannot qualify"
    );
    let accepted = request("accepted-profile-test", TestKind::Email);
    tests
        .begin(who, id, accepted.clone(), now + 2)
        .await
        .unwrap();
    tests.submitting(&accepted.id, now + 2).await.unwrap();
    tests
        .finish(&accepted.id, Outcome::Accepted, "", None, now + 3)
        .await
        .unwrap();
    assert!(tests.read(who, id, now + 3).await.unwrap().eligible);
    assert!(
        matches!(tests.begin(who, id, accepted, now+4).await.unwrap(), Start::Existing(a) if a.outcome == Outcome::Accepted)
    );
    assert!(
        db.campaigns().create(who, scheduled, now + 4).await.is_ok(),
        "Qualified profiles can launch scheduled campaigns"
    );
    let mut idempotent = launch.clone();
    idempotent.idempotency = Some(("sender-policy-launch".into(), "f".repeat(64)));
    let active = db
        .campaigns()
        .create(who, idempotent.clone(), now + 4)
        .await
        .unwrap()
        .record;
    tests
        .begin(
            who,
            id,
            request("later-failed-profile-test", TestKind::Connection),
            now + 5,
        )
        .await
        .unwrap();
    tests
        .finish(
            "later-failed-profile-test",
            Outcome::Failed,
            "smtp_authentication_failed",
            Some(535),
            now + 6,
        )
        .await
        .unwrap();
    assert!(!tests.read(who, id, now + 6).await.unwrap().eligible);
    assert!(matches!(
        db.campaigns().create(who, launch.clone(), now + 6).await,
        Err(Error::ProfileTestRequired)
    ));
    assert_eq!(
        db.campaigns().get(who, active.id).await.unwrap().id,
        active.id,
        "Existing campaign remains accessible"
    );
    let replay = db
        .campaigns()
        .create(who, idempotent, now + 6)
        .await
        .unwrap()
        .record;
    assert_eq!(
        replay.id, active.id,
        "A launch replay reconciles the existing campaign despite later failed tests"
    );
    let claim = db
        .campaigns()
        .claim(now + 6)
        .await
        .unwrap()
        .expect("Existing launched deliveries remain claimable");
    assert_eq!(claim.campaign_id, active.id);
    assert!(db.campaigns().release(&claim, now + 6).await.unwrap());
    tests
        .begin(
            who,
            id,
            request("renewed-profile-test", TestKind::Email),
            now + 7,
        )
        .await
        .unwrap();
    tests
        .submitting("renewed-profile-test", now + 7)
        .await
        .unwrap();
    tests
        .finish("renewed-profile-test", Outcome::Accepted, "", None, now + 8)
        .await
        .unwrap();
    let (left, right) = tokio::join!(
        tests.begin(
            who,
            id,
            request("concurrent-profile-test-left", TestKind::Connection),
            now + 9
        ),
        tests.begin(
            who,
            id,
            request("concurrent-profile-test-right", TestKind::Connection),
            now + 9
        )
    );
    let winner = match (left, right) {
        (Ok(Start::Started(attempt, _)), Err(Error::ProfileTestBusy))
        | (Err(Error::ProfileTestBusy), Ok(Start::Started(attempt, _))) => attempt,
        _ => panic!("Exactly one concurrent test must acquire the profile"),
    };
    tests
        .finish(&winner.id, Outcome::Passed, "", None, now + 9)
        .await
        .unwrap();
    for index in 0..12 {
        let request = request(
            &format!("bounded-profile-history-{index}"),
            TestKind::Connection,
        );
        tests
            .begin(who, id, request.clone(), now + 9)
            .await
            .unwrap();
        tests
            .finish(&request.id, Outcome::Passed, "", None, now + 9)
            .await
            .unwrap();
    }
    let bounded = tests.read(who, id, now + 9).await.unwrap();
    assert_eq!(bounded.history.len(), 10);
    assert!(
        bounded.eligible,
        "Accepted email evidence remains authoritative beyond the displayed history"
    );
    let renamed = db
        .resources()
        .save(
            who,
            resource("Renamed sender", profile.content.clone()),
            Some(id),
            Some(profile.revision.to_string()),
        )
        .await
        .unwrap()
        .record;
    let same = tests.read(who, id, now + 9).await.unwrap();
    assert_eq!(same.generation, initial.generation);
    assert!(same.eligible);
    let Content::Profile(mut configuration) = renamed.content else {
        panic!("expected profile");
    };
    configuration.password = "changed synthetic credential".into();
    let changed = db
        .resources()
        .save(
            who,
            resource("Renamed sender", Content::Profile(configuration)),
            Some(id),
            Some(renamed.revision.to_string()),
        )
        .await
        .unwrap()
        .record;
    let summary = tests.read(who, id, now + 10).await.unwrap();
    assert_eq!(summary.generation, initial.generation + 1);
    assert!(!summary.eligible);
    assert_eq!(
        summary.email.unwrap().outcome,
        Outcome::Accepted,
        "Old evidence remains historical"
    );
    launch.profile.revision = Some(changed.revision.to_string());
    let mut stale = request("stale-profile-test", TestKind::Email);
    assert!(matches!(
        tests.begin(who, id, stale.clone(), now + 11).await,
        Err(Error::Changed)
    ));
    stale.revision = changed.revision;
    tests.begin(who, id, stale, now + 12).await.unwrap();
    tests
        .submitting("stale-profile-test", now + 12)
        .await
        .unwrap();
    let recovered = tests.read(who, id, now + 133).await.unwrap();
    assert!(recovered.active.is_none());
    assert_eq!(recovered.email.unwrap().outcome, Outcome::Unknown);
    assert!(!recovered.eligible);
    assert!(
        tests
            .submitting("stale-profile-test", now + 12)
            .await
            .is_err(),
        "Recovered work cannot submit"
    );
    let expired = Request {
        id: "expired-before-submission".into(),
        revision: changed.revision,
        kind: TestKind::Email,
        email: Some("operator@example.test".into()),
    };
    tests.begin(who, id, expired, now + 134).await.unwrap();
    assert!(
        tests
            .submitting("expired-before-submission", now + 255)
            .await
            .is_err(),
        "Expired execution must not cross the submission boundary"
    );
    let recovered = tests.read(who, id, now + 255).await.unwrap();
    assert_eq!(recovered.history[0].outcome, Outcome::Interrupted);
    let settings = db.settings().get(who).await.unwrap();
    let mut policy = settings.policy;
    policy.sending_profile_test_required = false;
    db.settings()
        .save(who, settings.revision, policy, now + 134)
        .await
        .unwrap();
    assert!(tests.read(who, id, now + 134).await.unwrap().eligible);
    assert!(
        db.campaigns()
            .prepare(who, &launch, now + 134)
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn sqlite_profile_test_history_and_launch_policy() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("tests.db");
    let db = database::open(Config::Sqlite { path: path.clone() }, Policy::default())
        .await
        .unwrap();
    exercise(&db).await;
    db.close().await;
    let db = database::open(Config::Sqlite { path }, Policy::default())
        .await
        .unwrap();
    let caller = db.accounts().by_name("admin").await.unwrap().identity;
    let summary = db
        .profile_tests()
        .read(caller, 1, 1_900_000_200)
        .await
        .unwrap();
    assert_eq!(summary.history.len(), 10);
    assert_eq!(summary.history[0].outcome, Outcome::Interrupted);
    db.close().await;
}

#[tokio::test]
#[ignore = "Requires canonical MySQL fixture"]
async fn mysql_profile_test_history_and_launch_policy() {
    let (mut inspect, config) = fixture().await;
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    exercise(&db).await;
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
async fn postgres_profile_test_history_and_launch_policy() {
    let (mut inspect, config) = postgres_fixture().await;
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    exercise(&db).await;
    db.close().await;
    postgres_cleanup(&mut inspect, &config).await;
}
