//! Independent expectations shared by real provider fixtures.
use ruphish_adapters::database::Database;
use ruphish_application::{auth::Identity, event_edit::*, reporting::Filter, resources::Error};
use ruphish_domain::events::Kind;
pub const OWNER: Identity = Identity { id: 1, epoch: 0 };
pub const NOW: i64 = 1_790_000_000;
pub const SEED: &str = r#"
INSERT INTO users(id,username,api_key,role_id,password_change_required,account_locked) VALUES(1,'event-owner','event-owner-key',1,FALSE,FALSE),(2,'event-other','event-other-key',2,FALSE,FALSE);
INSERT INTO ruphish_auth_state VALUES(1,0),(2,0);
INSERT INTO campaigns(id,user_id,name,status,launch_date,completed_date) VALUES(10,1,'Corrections','Completed','2026-01-01T00:00:00Z','2026-09-01T00:00:00Z'),(20,2,'Other','Completed','2026-01-01T00:00:00Z','2026-09-01T00:00:00Z');
INSERT INTO results(id,campaign_id,user_id,r_id,email,status,reported) VALUES(11,10,1,'event-recipient','alice@example.test','Email Sent',FALSE),(21,20,2,'other-recipient','bob@example.test','Email Sent',FALSE);
INSERT INTO ruphish_delivery(result_id,campaign_id,state,due) VALUES(11,10,'accepted',1767225600);
INSERT INTO events(id,campaign_id,recipient_id,email,time,kind,metadata,source) VALUES
(101,10,11,'alice@example.test','2026-06-01T00:00:00Z','link_clicked','{}','public'),
(102,10,11,'alice@example.test','2026-06-01T00:00:01Z','link_clicked','{}','public'),
(103,10,11,'alice@example.test','2026-06-01T00:00:02Z','link_clicked','{}','public'),
(104,10,11,'alice@example.test','2026-06-01T00:00:03Z','link_clicked','{}','public'),
(105,10,11,'alice@example.test','2026-06-01T00:00:04Z','link_clicked','{}','public');
"#;
pub fn change(
    key: &str,
    id: Option<i64>,
    revision: i64,
    action: Action,
    kind: Option<Kind>,
) -> Change {
    Change {
        request_id: format!("event-test-request-{key}"),
        event_id: id,
        revision,
        action,
        value: kind.map(|kind| Value {
            time: "2026-06-02T00:00:00Z".into(),
            kind,
            presence: Presence::default(),
        }),
    }
}
pub async fn contract(db: &Database) {
    let repo = db.campaigns();
    let results = db
        .reporting()
        .analysis(
            OWNER,
            10,
            Filter {
                limit: 100,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(results.totals, [0, 0, 5, 0, 0]);
    let exclude = change("exclude", Some(101), 0, Action::Exclude, None);
    let first = repo
        .change_event(OWNER, OWNER, 10, 11, exclude.clone(), NOW)
        .await
        .unwrap();
    assert!(first.excluded);
    assert_eq!(first.source, "public");
    assert_eq!(
        repo.change_event(OWNER, OWNER, 10, 11, exclude.clone(), NOW + 1)
            .await
            .unwrap(),
        first
    );
    let mut conflicting_retry = exclude;
    conflicting_retry.action = Action::Restore;
    assert!(matches!(
        repo.change_event(OWNER, OWNER, 10, 11, conflicting_retry, NOW)
            .await,
        Err(Error::Changed)
    ));
    let results = db
        .reporting()
        .analysis(
            OWNER,
            10,
            Filter {
                limit: 100,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(results.totals[2], 4);
    assert_eq!(results.unique[2], 1);
    assert!(matches!(
        repo.change_event(
            OWNER,
            OWNER,
            10,
            11,
            change("stale", Some(101), 0, Action::Edit, Some(Kind::EmailOpened)),
            NOW
        )
        .await,
        Err(Error::Changed)
    ));
    assert!(matches!(
        repo.change_event(
            Identity { id: 2, epoch: 0 },
            Identity { id: 2, epoch: 0 },
            10,
            11,
            change("other", Some(101), 1, Action::Restore, None),
            NOW
        )
        .await,
        Err(Error::NotFound(_))
    ));
    assert!(matches!(
        repo.change_event(
            OWNER,
            OWNER,
            10,
            21,
            change("recipient", Some(101), 1, Action::Restore, None),
            NOW
        )
        .await,
        Err(Error::NotFound(_))
    ));
    let mut edit = change(
        "submission",
        Some(102),
        0,
        Action::Edit,
        Some(Kind::FormSubmitted),
    );
    edit.value.as_mut().unwrap().presence.password_present = Some(true);
    let edited = repo
        .change_event(OWNER, OWNER, 10, 11, edit, NOW)
        .await
        .unwrap();
    assert!(edited.edited());
    assert_eq!(edited.original.time, "2026-06-01T00:00:01Z");
    assert_eq!(edited.current.time, "2026-06-02T00:00:00Z");
    assert!(
        repo.change_event(
            OWNER,
            OWNER,
            10,
            11,
            change("smtp", Some(103), 0, Action::Edit, Some(Kind::EmailSent)),
            NOW
        )
        .await
        .is_err()
    );
    for time in [
        "2025-01-01T00:00:00Z",
        "2026-10-01T00:00:00Z",
        "2099-01-01T00:00:00Z",
        "2026-06-02T00:00:00",
    ] {
        let mut invalid = change(
            "invalid-time",
            None,
            0,
            Action::Add,
            Some(Kind::LinkClicked),
        );
        invalid.value.as_mut().unwrap().time = time.into();
        assert!(
            repo.change_event(OWNER, OWNER, 10, 11, invalid, NOW)
                .await
                .is_err()
        );
    }
    let add = change("add", None, 0, Action::Add, Some(Kind::EmailReported));
    let added = repo
        .change_event(OWNER, OWNER, 10, 11, add.clone(), NOW)
        .await
        .unwrap();
    assert_eq!(added.source, "operator");
    assert_eq!(
        repo.change_event(OWNER, OWNER, 10, 11, add, NOW)
            .await
            .unwrap(),
        added
    );
    let restored = repo
        .change_event(
            OWNER,
            OWNER,
            10,
            11,
            change("restore", Some(101), 1, Action::Restore, None),
            NOW,
        )
        .await
        .unwrap();
    assert!(!restored.excluded);
    for id in [101, 103, 104, 105] {
        repo.change_event(
            OWNER,
            OWNER,
            10,
            11,
            change(
                &format!("exclude-{id}"),
                Some(id),
                if id == 101 { 2 } else { 0 },
                Action::Exclude,
                None,
            ),
            NOW,
        )
        .await
        .unwrap();
    }
    let results = db
        .reporting()
        .analysis(
            OWNER,
            10,
            Filter {
                limit: 100,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(results.totals, [0, 0, 0, 1, 1]);
    assert_eq!(results.unique[2], 0);
    assert_eq!(results.progression, [0, 1, 1, 1, 1]);
    let csv = db
        .reporting()
        .export(
            OWNER,
            10,
            Filter {
                limit: 100,
                ..Default::default()
            },
            "csv",
            "events",
        )
        .await
        .unwrap();
    let csv = String::from_utf8(csv.bytes).unwrap();
    assert!(csv.contains("Correction revision"));
    assert!(csv.contains("operator"));
    assert!(!csv.contains("Clicked Link"));
    let history = repo
        .recipient_history(OWNER, 10, 11, None, None)
        .await
        .unwrap();
    assert_eq!(history.events.len(), 6);
    assert!(
        history
            .events
            .iter()
            .find(|e| e.id == 101)
            .unwrap()
            .correction
            .as_ref()
            .unwrap()
            .excluded
    );
    assert_eq!(history.attempts.len(), 0);
    let audit = repo.inspect_event(OWNER, 10, 11, 101, None).await.unwrap();
    assert_eq!(audit.audit.len(), 3);
    assert_eq!(audit.record.revision, 3);
    assert!(
        audit
            .audit
            .iter()
            .all(|a| a.actor == 1 && a.performed_at == NOW)
    );
    assert_eq!(
        repo.header(OWNER, 10).await.unwrap().status,
        ruphish_domain::campaign::CampaignStatus::Completed
    );
    let a = change("provider-race-a", Some(added.id), 1, Action::Exclude, None);
    let b = change("provider-race-b", Some(added.id), 1, Action::Restore, None);
    let (a, b) = tokio::join!(
        repo.change_event(OWNER, OWNER, 10, 11, a, NOW),
        repo.change_event(OWNER, OWNER, 10, 11, b, NOW)
    );
    assert!(matches!(
        (&a, &b),
        (Ok(_), Err(Error::Changed)) | (Err(Error::Changed), Ok(_))
    ));
    assert_eq!(
        repo.inspect_event(OWNER, 10, 11, added.id, None)
            .await
            .unwrap()
            .audit
            .len(),
        2
    );
}
