//! Shared real-provider workflows, with independent expected state and competing writers.
use super::*;
use ruphish_application::campaign::drafts::Configuration;
use ruphish_domain::campaign_draft::{Policy as DraftPolicy, Reference, Selections};

pub(super) async fn exercise(
    db: &Database,
    actor: auth::Identity,
    other: auth::Identity,
    mut input: campaign::Create,
    now: i64,
    fault: Option<&std::sync::atomic::AtomicBool>,
) {
    let store = db.campaigns();
    let request = (
        "draft-functional-00000001".to_owned(),
        "empty-step-0".to_owned(),
    );
    let empty = Configuration::default();
    if let Some(fault) = fault {
        fault.store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(matches!(
            store
                .create_draft(actor, request.clone(), empty.clone(), 0, now)
                .await,
            Err(Error::Storage(
                ruphish_application::storage::Error::UncertainCommit
            ))
        ));
        assert!(!fault.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(store.drafts(actor, 0, 51).await.unwrap()[0].revision, 1);
    }
    let first = store
        .create_draft(actor, request.clone(), empty.clone(), 0, now)
        .await
        .unwrap();
    let id = first.id;
    assert!(id > 0);
    assert_eq!(first.revision, 1);
    assert_eq!(
        ruphish_application::datetime::parse(first.attribution.created_at.as_deref().unwrap())
            .unwrap()
            .as_second(),
        now
    );
    assert_eq!(
        store
            .create_draft(actor, request.clone(), empty.clone(), 0, now)
            .await
            .unwrap()
            .revision,
        1,
        "lost create response must not create another draft"
    );
    let page = store
        .directory(
            actor,
            campaign::directory::Filter::All,
            Default::default(),
            50,
        )
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, id.to_string());
    assert_eq!(page.items[0].revision, Some(1));
    assert!(page.items[0].total.is_none());
    assert_eq!(
        store
            .directory(
                other,
                campaign::directory::Filter::All,
                Default::default(),
                50
            )
            .await
            .unwrap()
            .items[0]
            .id,
        id.to_string()
    );
    assert!(store.list(actor).await.unwrap().is_empty());
    assert!(
        store.claim(now).await.unwrap().is_none(),
        "saving a draft schedules nothing"
    );
    assert_eq!(store.draft(other, id).await.unwrap().revision, 1);
    assert!(matches!(
        store
            .save_draft(other, id, None, empty.clone(), 0, now)
            .await,
        Err(Error::RevisionRequired)
    ));
    assert!(matches!(
        store
            .create_draft(
                actor,
                (request.0.clone(), "different".into()),
                empty.clone(),
                0,
                now
            )
            .await,
        Err(Error::Conflict(_))
    ));
    // Keys are scoped to the caller, even though the resulting drafts are shared.
    let another = store
        .create_draft(other, request.clone(), empty.clone(), 0, now)
        .await
        .unwrap();
    assert!(another.id > id);
    store.delete_draft(other, another.id, 1).await.unwrap();
    assert!(matches!(
        store
            .create_draft(other, request.clone(), empty.clone(), 0, now)
            .await,
        Err(Error::NotFound(_))
    ));
    assert!(matches!(
        store.delete_draft(other, id, 999).await,
        Err(Error::Changed)
    ));
    let left = Configuration {
        name: "First tab".into(),
        ..empty.clone()
    };
    let right = Configuration {
        name: "Second tab".into(),
        ..empty.clone()
    };
    assert!(matches!(
        store
            .save_draft(other, id, None, left.clone(), 1, now)
            .await,
        Err(Error::RevisionRequired)
    ));
    let (a, b) = tokio::join!(
        store.save_draft(actor, id, Some(1), left.clone(), 1, now),
        store.save_draft(other, id, Some(1), right.clone(), 1, now)
    );
    assert_eq!(
        usize::from(a.is_ok()) + usize::from(b.is_ok()),
        1,
        "one competing edit wins"
    );
    assert!(matches!(
        a.as_ref().err().or(b.as_ref().err()),
        Some(Error::Changed)
    ));
    let winning_editor = if a.is_ok() { actor.id } else { other.id };
    let winner = a.or(b).unwrap();
    assert_eq!(winner.attribution.created_by, Some(actor.id));
    assert_eq!(winner.attribution.created_at, first.attribution.created_at);
    assert_eq!(winner.attribution.edited_by, Some(winning_editor));
    assert_eq!(
        store.draft(actor, id).await.unwrap().configuration,
        winner.configuration
    );
    assert_eq!(
        store
            .save_draft(actor, id, Some(1), winner.configuration.clone(), 1, now)
            .await
            .unwrap()
            .revision,
        2,
        "lost update response is reconcilable"
    );
    assert_eq!(
        store.draft(other, id).await.unwrap().attribution,
        winner.attribution
    );
    assert!(matches!(
        store.delete_draft(actor, id, 1).await,
        Err(Error::Changed)
    ));
    input.policy.track_opens = false;
    let review = store.prepare(actor, &input, now).await.unwrap();
    assert!(!review.policy.track_opens);
    input.policy = campaign::Overrides {
        track_opens: review.policy.track_opens,
        redirect_url: Some(review.policy.redirect_url.clone()),
        learning_page_id: review.policy.learning_page_id,
    };
    let configuration = Configuration {
        name: input.name.clone(),
        url: input.url.clone(),
        selection: Selections {
            groups: input
                .groups
                .iter()
                .map(|r| Reference { id: r.id })
                .collect(),
            template: Reference {
                id: input.template.id,
            },
            page: Reference { id: input.page.id },
            smtp: Reference {
                id: input.profile.id,
            },
        },
        policy: Some(DraftPolicy {
            track_opens: review.policy.track_opens,
            redirect_url: Some(review.policy.redirect_url),
            learning_page_id: review.policy.learning_page_id,
        }),
        ..Default::default()
    };
    let mut unresolved = configuration.clone();
    unresolved.policy.as_mut().unwrap().redirect_url = None;
    let pending = store
        .create_draft(
            actor,
            (
                "unresolved-tracking-0001".into(),
                "unresolved-step-1".into(),
            ),
            unresolved.clone(),
            1,
            now,
        )
        .await
        .unwrap();
    let pending_id = pending.id;
    assert_eq!(
        store.draft(actor, pending_id).await.unwrap().configuration,
        unresolved
    );
    assert!(matches!(
        store
            .launch_draft(actor, pending_id, pending.revision, input.clone(), now)
            .await,
        Err(Error::Invalid(_))
    ));
    store
        .delete_draft(actor, pending_id, pending.revision)
        .await
        .unwrap();
    let saved = store
        .save_draft(actor, id, Some(2), configuration.clone(), 3, now)
        .await
        .unwrap();
    let mut wrong_tracking = input.clone();
    wrong_tracking.policy.track_opens = true;
    assert!(matches!(
        store
            .launch_draft(actor, id, saved.revision, wrong_tracking, now)
            .await,
        Err(Error::Changed)
    ));
    let mut tampered = input.clone();
    tampered.name = "Unreviewed change".into();
    assert!(matches!(
        store
            .launch_draft(actor, id, saved.revision, tampered, now)
            .await,
        Err(Error::Changed)
    ));
    let mut stale = input.clone();
    stale.template.revision = Some("999999".into());
    assert!(
        store
            .launch_draft(actor, id, saved.revision, stale, now)
            .await
            .is_err()
    );
    assert!(store.list(actor).await.unwrap().is_empty());
    assert!(store.draft(actor, id).await.unwrap().campaign_id.is_none());
    if let Some(fault) = fault {
        fault.store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(matches!(
            store
                .launch_draft(actor, id, saved.revision, input.clone(), now)
                .await,
            Err(Error::Storage(
                ruphish_application::storage::Error::UncertainCommit
            ))
        ));
        assert!(!fault.load(std::sync::atomic::Ordering::SeqCst));
        assert!(store.draft(actor, id).await.unwrap().campaign_id.is_some());
    }
    let (a, b) = tokio::join!(
        store.launch_draft(actor, id, saved.revision, input.clone(), now),
        store.launch_draft(other, id, saved.revision, input.clone(), now)
    );
    let launched = a.unwrap();
    assert!(!launched.policy.as_ref().unwrap().track_opens);
    assert_eq!(launched.id, b.unwrap().id);
    assert_eq!(store.list(actor).await.unwrap().len(), 1);
    assert_eq!(launched.results.len(), 1);
    assert_eq!(
        store.draft(actor, id).await.unwrap().campaign_id,
        Some(launched.id)
    );
    assert!(
        store.drafts(actor, 0, 51).await.unwrap().is_empty(),
        "launched drafts leave editable overview"
    );
    assert!(matches!(
        store
            .save_draft(actor, id, Some(saved.revision), configuration, 3, now)
            .await,
        Err(Error::Changed)
    ));
    assert!(store.delete_draft(actor, id, saved.revision).await.is_err());
    assert_eq!(
        store
            .launch_draft(actor, id, saved.revision, input.clone(), now + 100)
            .await
            .unwrap()
            .id,
        launched.id,
        "replay survives resource and time changes"
    );
    let mixed = store
        .create_draft(
            actor,
            ("mixed-directory-pending".into(), "empty-step-0".into()),
            empty.clone(),
            0,
            now,
        )
        .await
        .unwrap();
    let page = store
        .directory(
            actor,
            campaign::directory::Filter::All,
            Default::default(),
            1,
        )
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, mixed.id.to_string());
    let cursor = campaign::directory::Cursor::parse(page.next_after.as_deref().unwrap()).unwrap();
    let tail = store
        .directory(actor, campaign::directory::Filter::All, cursor, 1)
        .await
        .unwrap();
    assert_eq!(tail.items.len(), 1);
    assert_eq!(tail.items[0].id, launched.id.to_string());
    assert_eq!(tail.items[0].total, Some(1));
    assert!(tail.next_after.is_none());
    assert!(
        store
            .directory(
                actor,
                campaign::directory::Filter::Completed,
                Default::default(),
                50
            )
            .await
            .unwrap()
            .items
            .is_empty()
    );
    store.delete_draft(actor, mixed.id, 1).await.unwrap();
    store.delete(actor, launched.id).await.unwrap();
    assert!(
        store
            .launch_draft(actor, id, saved.revision, input, now)
            .await
            .is_err(),
        "deleted campaign must never relaunch from its old draft"
    );
    assert!(store.list(actor).await.unwrap().is_empty());
    assert!(store.claim(now).await.unwrap().is_none());
    let race = ("concurrent-create".to_owned(), "empty-step-0".to_owned());
    let (a, b) = tokio::join!(
        store.create_draft(actor, race.clone(), empty.clone(), 0, now),
        store.create_draft(actor, race.clone(), empty.clone(), 0, now)
    );
    let raced = a.unwrap();
    assert_eq!(raced.id, b.unwrap().id);
    assert_eq!(store.drafts(actor, 0, 51).await.unwrap().len(), 1);
    store.delete_draft(actor, raced.id, 1).await.unwrap();
    // Independent concurrent creations allocate different IDs even for identical content.
    let (a, b) = tokio::join!(
        store.create_draft(
            actor,
            ("parallel-a".into(), "empty-step-0".into()),
            empty.clone(),
            0,
            now
        ),
        store.create_draft(
            actor,
            ("parallel-b".into(), "empty-step-0".into()),
            empty.clone(),
            0,
            now
        )
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert!(a.id > 0 && b.id > 0);
    assert_ne!(a.id, b.id);
    assert_eq!(store.drafts(actor, 0, 51).await.unwrap().len(), 2);
    assert_eq!(store.draft(actor, a.id).await.unwrap().configuration, empty);
    assert_eq!(store.draft(actor, b.id).await.unwrap().configuration, empty);
    store.delete_draft(actor, a.id, 1).await.unwrap();
    store.delete_draft(actor, b.id, 1).await.unwrap();
    // Competing requests cannot assign different content to the same creation key.
    let (a, b) = tokio::join!(
        store.create_draft(
            actor,
            ("competing-key".into(), "left".into()),
            left.clone(),
            0,
            now
        ),
        store.create_draft(
            actor,
            ("competing-key".into(), "right".into()),
            right.clone(),
            0,
            now
        )
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(matches!(
        a.as_ref().err().or(b.as_ref().err()),
        Some(Error::Conflict(_))
    ));
    let winner = a.or(b).unwrap();
    assert_eq!(store.drafts(actor, 0, 51).await.unwrap().len(), 1);
    assert_eq!(
        store.draft(actor, winner.id).await.unwrap().configuration,
        winner.configuration
    );
    store.delete_draft(actor, winner.id, 1).await.unwrap();
    // Numeric tie-breaks must stay numeric across 9/10, including sorted continuations.
    let mut expected = Vec::new();
    for n in 0..12 {
        expected.push(
            store
                .create_draft(
                    actor,
                    (format!("numeric-order-{n}"), "empty-step-0".into()),
                    empty.clone(),
                    0,
                    now,
                )
                .await
                .unwrap()
                .id,
        );
    }
    let mut seen = Vec::new();
    let mut selection = ruphish_domain::pagination::SortedPage {
        sort: ruphish_domain::pagination::DirectorySort::Name,
        order: ruphish_domain::reporting::SortDirection::Asc,
        scope: "numeric-drafts".into(),
        after: None,
    };
    loop {
        let page = store
            .directory(
                actor,
                campaign::directory::Filter::Drafts,
                campaign::directory::Cursor {
                    sorted: Some(selection.clone()),
                    ..Default::default()
                },
                3,
            )
            .await
            .unwrap();
        seen.extend(page.items.iter().map(|r| r.id.parse::<i64>().unwrap()));
        let Some(next) = page.next_after else {
            break;
        };
        let cursor: ruphish_domain::pagination::Cursor<ruphish_domain::pagination::DirectorySort> =
            serde_json::from_str(&next).unwrap();
        selection.after = Some(cursor.position);
    }
    assert_eq!(seen, expected);
    for id in expected {
        store.delete_draft(actor, id, 1).await.unwrap();
    }
    // Keep one incomplete draft for the surrounding provider's reopen assertions.
    store
        .create_draft(
            actor,
            ("draft-survives-restart-01".into(), "empty-step-2".into()),
            empty.clone(),
            2,
            now,
        )
        .await
        .unwrap();
}
