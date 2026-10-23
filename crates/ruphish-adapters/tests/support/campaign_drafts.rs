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
    let id = "draft-functional-00000001";
    let empty = Configuration::default();
    if let Some(fault) = fault {
        fault.store(true, std::sync::atomic::Ordering::SeqCst);
        assert!(matches!(
            store
                .save_draft(actor, id, None, empty.clone(), 0, now)
                .await,
            Err(Error::Storage(
                ruphish_application::storage::Error::UncertainCommit
            ))
        ));
        assert!(!fault.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(store.draft(actor, id).await.unwrap().revision, 1);
    }
    let first = store
        .save_draft(actor, id, None, empty.clone(), 0, now)
        .await
        .unwrap();
    assert_eq!(first.revision, 1);
    assert_eq!(
        store
            .save_draft(actor, id, None, empty.clone(), 0, now)
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
    assert_eq!(page.items[0].id, id);
    assert_eq!(page.items[0].revision, Some(1));
    assert!(page.items[0].total.is_none());
    assert!(
        store
            .directory(
                other,
                campaign::directory::Filter::All,
                Default::default(),
                50
            )
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert!(store.list(actor).await.unwrap().is_empty());
    assert!(
        store.claim(now).await.unwrap().is_none(),
        "saving a draft schedules nothing"
    );
    assert!(matches!(
        store.draft(other, id).await,
        Err(Error::NotFound(_))
    ));
    assert!(matches!(
        store
            .save_draft(other, id, Some(1), empty.clone(), 0, now)
            .await,
        Err(Error::NotFound(_))
    ));
    assert!(matches!(
        store.delete_draft(other, id, 1).await,
        Err(Error::NotFound(_))
    ));
    assert!(matches!(
        store.launch_draft(other, id, 1, input.clone(), now).await,
        Err(Error::NotFound(_))
    ));
    let left = Configuration {
        name: "First tab".into(),
        ..empty.clone()
    };
    let right = Configuration {
        name: "Second tab".into(),
        ..empty.clone()
    };
    let (a, b) = tokio::join!(
        store.save_draft(actor, id, Some(1), left.clone(), 1, now),
        store.save_draft(actor, id, Some(1), right.clone(), 1, now)
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
    let winner = a.or(b).unwrap();
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
    assert!(matches!(
        store.delete_draft(actor, id, 1).await,
        Err(Error::Changed)
    ));
    let review = store.prepare(actor, &input, now).await.unwrap();
    input.policy = campaign::Overrides {
        capture_credentials: Some(true),
        capture_passwords: Some(true),
        redirect_url: Some(review.policy.redirect_url.clone()),
    };
    let configuration = Configuration {
        name: input.name.clone(),
        url: input.url.clone(),
        selection: Selections {
            groups: input
                .groups
                .iter()
                .map(|r| Reference { id: r.id.unwrap() })
                .collect(),
            template: Reference {
                id: input.template.id.unwrap(),
            },
            page: Reference {
                id: input.page.id.unwrap(),
            },
            smtp: Reference {
                id: input.profile.id.unwrap(),
            },
        },
        policy: Some(DraftPolicy {
            capture_credentials: true,
            capture_passwords: true,
            redirect_url: review.policy.redirect_url,
        }),
        ..Default::default()
    };
    let saved = store
        .save_draft(actor, id, Some(2), configuration.clone(), 3, now)
        .await
        .unwrap();
    let mut tampered = input.clone();
    tampered.name = "Unreviewed change".into();
    assert!(matches!(
        store
            .launch_draft(actor, id, saved.revision, tampered, now)
            .await,
        Err(Error::Changed)
    ));
    let mut stale = input.clone();
    stale.template.revision = Some("stale".into());
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
        store.launch_draft(actor, id, saved.revision, input.clone(), now)
    );
    let launched = a.unwrap();
    assert_eq!(launched.id, b.unwrap().id);
    assert_eq!(store.list(actor).await.unwrap().len(), 1);
    assert_eq!(launched.results.len(), 1);
    assert_eq!(
        store.draft(actor, id).await.unwrap().campaign_id,
        Some(launched.id)
    );
    assert!(
        store.drafts(actor, "", 51).await.unwrap().is_empty(),
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
    store
        .save_draft(
            actor,
            "mixed-directory-pending",
            None,
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
    assert_eq!(page.items[0].id, "mixed-directory-pending");
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
    store
        .delete_draft(actor, "mixed-directory-pending", 1)
        .await
        .unwrap();
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
    // Keep one incomplete draft for the surrounding provider's reopen assertions.
    store
        .save_draft(actor, "draft-survives-restart-01", None, empty, 2, now)
        .await
        .unwrap();
}
