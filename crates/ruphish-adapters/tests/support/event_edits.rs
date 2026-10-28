//! Independent expectations shared by real provider fixtures.
use ruphish_adapters::database::Database;
use ruphish_application::{auth::Identity, event_edit::*, reporting::Filter, resources::Error};
use ruphish_domain::events::Kind;
pub const OWNER: Identity = Identity { id: 1, epoch: 0 };
pub const NOW: i64 = 1_790_000_000;
pub const SEED: &str = r#"
INSERT INTO users(id,username,api_key,role_id,password_change_required,account_locked) VALUES(1,'event-owner','event-owner-key',1,FALSE,FALSE),(2,'event-other','event-other-key',2,FALSE,FALSE);
INSERT INTO ruphish_auth_state VALUES(1,0),(2,0);
INSERT INTO campaigns(id,user_id,name,status,launch_date,completed_date) VALUES(10,1,'Event edits','Completed','2026-01-01T00:00:00Z','2026-09-01T00:00:00Z'),(20,2,'Other','Completed','2026-01-01T00:00:00Z','2026-09-01T00:00:00Z');
INSERT INTO results(id,campaign_id,user_id,r_id,email,status,reported) VALUES(11,10,1,'event-recipient','alice@example.test','Email Sent',FALSE),(12,10,1,'event-second','second@example.test','Email Sent',FALSE),(21,20,2,'other-recipient','bob@example.test','Email Sent',FALSE);
INSERT INTO ruphish_delivery(result_id,campaign_id,state,due) VALUES(11,10,'accepted',1767225600);
INSERT INTO events(id,campaign_id,recipient_id,email,time,kind,metadata,source) VALUES
(101,10,11,'alice@example.test','2026-06-01T00:00:00Z','link_clicked','{}','public'),
(102,10,11,'alice@example.test','2026-06-01T00:00:01Z','link_clicked','{}','public'),
(103,10,11,'alice@example.test','2026-06-01T00:00:02Z','link_clicked','{}','public'),
(104,10,11,'alice@example.test','2026-06-01T00:01:00Z','link_clicked','{}','public'),
(105,10,11,'alice@example.test','2026-06-01T00:03:00Z','link_clicked','{}','public');
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
async fn results(db: &Database) -> ruphish_application::reporting::Analysis {
    db.reporting()
        .analysis(
            OWNER,
            10,
            Filter {
                limit: 100,
                ..Default::default()
            },
        )
        .await
        .unwrap()
}
pub async fn contract(db: &Database) {
    let repo = db.campaigns();
    let before = results(db).await;
    assert_eq!(before.totals, [0, 0, 5, 0, 0]);
    assert_eq!(before.unique, [0, 0, 1, 0, 0]);
    assert_eq!(before.participant_buckets.len(), 4);
    assert!(
        before
            .participant_buckets
            .iter()
            .all(|b| b.counts == [0, 0, 1, 0, 0]),
        "repeats across buckets and quiet intervals cannot inflate participant totals"
    );
    let paged = db
        .reporting()
        .analysis(
            OWNER,
            10,
            Filter {
                limit: 1,
                offset: 4,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(paged.events.len(), 1);
    assert_eq!(
        paged.participant_buckets.last().unwrap().counts,
        [0, 0, 1, 0, 0]
    );
    let start = ruphish_domain::datetime::parse("2026-06-01T00:02:00Z")
        .unwrap()
        .as_second();
    let window = db
        .reporting()
        .analysis(
            OWNER,
            10,
            Filter {
                from: Some(start),
                until: Some(start + 120),
                limit: 1,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(window.totals, [0, 0, 1, 0, 0]);
    assert_eq!(window.participant_buckets.len(), 1);
    assert_eq!(window.participant_buckets[0].time, start + 60);
    assert_eq!(window.participant_buckets[0].counts, [0, 0, 1, 0, 0]);
    let empty = db
        .reporting()
        .analysis(
            OWNER,
            10,
            Filter {
                event: vec![Kind::FormSubmitted],
                limit: 1,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(empty.participant_buckets.is_empty());
    assert!(before.events.iter().all(|e| !e.edited));
    let mut noop = change(
        "unchanged",
        Some(101),
        0,
        Action::Edit,
        Some(Kind::LinkClicked),
    );
    noop.value.as_mut().unwrap().time = "2026-06-01T02:00:00+02:00".into();
    let unchanged = repo
        .change_event(OWNER, 10, 11, noop, NOW)
        .await
        .unwrap()
        .event
        .unwrap();
    assert!(!unchanged.edited);
    assert_eq!(unchanged.revision, 0);
    assert_eq!(unchanged.current.time, "2026-06-01T00:00:00Z");
    let add = change("add", None, 0, Action::Add, Some(Kind::EmailReported));
    let added = repo
        .change_event(OWNER, 10, 11, add.clone(), NOW)
        .await
        .unwrap();
    let row = added.event.as_ref().unwrap();
    assert_eq!(row.source, "operator");
    assert!(!row.edited);
    assert_eq!(row.revision, 0);
    assert_eq!(
        repo.change_event(OWNER, 10, 11, add.clone(), NOW)
            .await
            .unwrap(),
        added
    );
    assert!(
        !results(db)
            .await
            .events
            .iter()
            .find(|e| e.id == added.id)
            .unwrap()
            .edited
    );
    let mut reused = add.clone();
    reused.value.as_mut().unwrap().kind = Kind::EmailOpened;
    assert!(matches!(
        repo.change_event(OWNER, 10, 11, reused, NOW).await,
        Err(Error::Changed)
    ));
    let mut edit = change(
        "edit-added",
        Some(added.id),
        0,
        Action::Edit,
        Some(Kind::FormSubmitted),
    );
    edit.value.as_mut().unwrap().presence.username_present = Some(true);
    edit.value.as_mut().unwrap().presence.password_present = Some(false);
    let changed = repo
        .change_event(OWNER, 10, 11, edit.clone(), NOW)
        .await
        .unwrap();
    assert!(changed.event.as_ref().unwrap().edited);
    let snapshot = results(db).await;
    let submission = snapshot.events.iter().find(|e| e.id == added.id).unwrap();
    assert_eq!(submission.username_present, Some(true));
    assert_eq!(submission.password_present, Some(false));
    assert_eq!(
        snapshot.participant_buckets.last().unwrap().counts,
        [0, 0, 1, 1, 0]
    );
    let manual = db
        .reporting()
        .analysis(
            OWNER,
            10,
            Filter {
                source: Some(ruphish_domain::reporting::Source::Operator),
                limit: 1,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        manual.participant_buckets.last().unwrap().counts,
        [0, 0, 0, 1, 0]
    );
    assert_eq!(changed.event.as_ref().unwrap().source, "operator");
    assert_eq!(changed.event.as_ref().unwrap().revision, 1);
    assert_eq!(
        repo.change_event(OWNER, 10, 11, edit.clone(), NOW)
            .await
            .unwrap(),
        changed
    );
    edit.request_id = "event-change-noop-again".into();
    edit.revision = 1;
    assert_eq!(
        repo.change_event(OWNER, 10, 11, edit, NOW).await.unwrap(),
        changed
    );
    let reverted = repo
        .change_event(
            OWNER,
            10,
            11,
            change(
                "change-back",
                Some(added.id),
                1,
                Action::Edit,
                Some(Kind::EmailReported),
            ),
            NOW,
        )
        .await
        .unwrap();
    assert!(
        reverted.event.as_ref().unwrap().edited,
        "edited records a manual edit, not a comparison against retained history"
    );
    let system = repo
        .change_event(
            OWNER,
            10,
            11,
            change(
                "system-edit",
                Some(102),
                0,
                Action::Edit,
                Some(Kind::FormSubmitted),
            ),
            NOW,
        )
        .await
        .unwrap();
    assert_eq!(system.event.as_ref().unwrap().source, "public");
    assert!(system.event.as_ref().unwrap().edited);
    assert!(matches!(
        repo.change_event(
            OWNER,
            10,
            11,
            change("stale", Some(102), 0, Action::Edit, Some(Kind::EmailOpened)),
            NOW
        )
        .await,
        Err(Error::Changed)
    ));
    assert!(matches!(
        repo.change_event(
            Identity { id: 2, epoch: 0 },
            10,
            11,
            change("owner", Some(102), 1, Action::Delete, None),
            NOW
        )
        .await,
        Err(Error::NotFound(_))
    ));
    assert!(matches!(
        repo.change_event(
            OWNER,
            10,
            21,
            change("recipient", Some(102), 1, Action::Delete, None),
            NOW
        )
        .await,
        Err(Error::NotFound(_))
    ));
    let deleted = repo
        .change_event(
            OWNER,
            10,
            11,
            change("delete", Some(101), 0, Action::Delete, None),
            NOW,
        )
        .await
        .unwrap();
    assert_eq!(deleted.id, 101);
    assert_eq!(
        results(db).await.participant_buckets.last().unwrap().counts,
        [0, 0, 1, 1, 1]
    );
    assert!(deleted.event.is_none());
    assert!(matches!(
        repo.change_event(
            OWNER,
            10,
            11,
            change(
                "missing",
                Some(101),
                0,
                Action::Edit,
                Some(Kind::EmailOpened)
            ),
            NOW
        )
        .await,
        Err(Error::NotFound(_))
    ));
    let (a, b) = tokio::join!(
        repo.change_event(
            OWNER,
            10,
            11,
            change(
                "race-a",
                Some(103),
                0,
                Action::Edit,
                Some(Kind::EmailOpened)
            ),
            NOW
        ),
        repo.change_event(
            OWNER,
            10,
            11,
            change(
                "race-b",
                Some(103),
                0,
                Action::Edit,
                Some(Kind::EmailReported)
            ),
            NOW
        )
    );
    assert!(matches!(
        (&a, &b),
        (Ok(_), Err(Error::Changed)) | (Err(Error::Changed), Ok(_))
    ));
    for time in [
        "2025-01-01T00:00:00Z",
        "2026-10-01T00:00:00Z",
        "2026-06-02T00:00:00",
    ] {
        let mut invalid = change("invalid", None, 0, Action::Add, Some(Kind::EmailOpened));
        invalid.value.as_mut().unwrap().time = time.into();
        assert!(
            repo.change_event(OWNER, 10, 11, invalid, NOW)
                .await
                .is_err()
        );
    }
    let after = results(db).await;
    assert_eq!(after.event_total, 5);
    assert!(!after.events.iter().any(|e| e.id == 101));
    assert!(after.events.iter().find(|e| e.id == 102).unwrap().edited);
    assert!(
        after
            .events
            .iter()
            .find(|e| e.id == added.id)
            .unwrap()
            .edited
    );
    assert!(!after.events.iter().find(|e| e.id == 104).unwrap().edited);
    let history = repo
        .recipient_history(OWNER, 10, 11, None, None)
        .await
        .unwrap();
    assert_eq!(history.events.len(), 5);
    assert!(!history.events.iter().any(|e| e.id == 101));
    assert!(
        history
            .events
            .iter()
            .find(|e| e.id == 102)
            .unwrap()
            .record
            .as_ref()
            .unwrap()
            .edited
    );
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
    assert!(csv.contains("Edited"));
    assert!(!csv.contains("Correction revision"));
    assert!(csv.contains("operator"));
    // A second participant adds one to the campaign, even with repeat actions.
    let mut added_ids = Vec::new();
    for key in ["second-first-click", "second-repeat-click"] {
        let event = repo
            .change_event(
                OWNER,
                10,
                12,
                change(key, None, 0, Action::Add, Some(Kind::LinkClicked)),
                NOW,
            )
            .await
            .unwrap();
        added_ids.push(event.id);
    }
    let clicked = db
        .reporting()
        .analysis(
            OWNER,
            10,
            Filter {
                event: vec![Kind::LinkClicked],
                limit: 1,
                offset: 1,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let opened = i64::from(a.is_ok());
    assert_eq!(clicked.unique, [0, opened, 2, 1, 1]);
    assert_eq!(clicked.totals, [0, opened, 4, 1, 2 - opened]);
    assert_eq!(
        clicked.participant_buckets.first().unwrap().counts,
        [0, 0, 1, 0, 0]
    );
    assert_eq!(
        clicked.participant_buckets.last().unwrap().counts,
        [0, opened, 2, 1, 1]
    );
    for id in added_ids {
        repo.change_event(
            OWNER,
            10,
            12,
            change(
                &format!("second-delete-{id}"),
                Some(id),
                0,
                Action::Delete,
                None,
            ),
            NOW,
        )
        .await
        .unwrap();
    }
    assert_eq!(
        results(db).await.participant_buckets.last().unwrap().counts[2],
        1,
        "removing the last matching event removes the participant from this metric"
    );
}
