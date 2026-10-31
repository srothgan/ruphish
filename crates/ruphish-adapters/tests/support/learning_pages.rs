//! The same resource/destination lifecycle on each real database provider.
use super::*;

pub(super) async fn exercise(
    db: &Database,
    actor: auth::Identity,
    other: auth::Identity,
    mut input: campaign::Create,
    now: i64,
) {
    let resources = db.resources();
    let campaigns = db.campaigns();
    let public = db.interactions();
    let authored = resource(
        "Explanation",
        Content::LearningPage(LearningPage {
            html: "<h1>First explanation</h1>".into(),
            public_id: "caller-chosen-identity".into(),
        }),
    );
    let page = resources
        .save(actor, authored.clone(), None, None)
        .await
        .unwrap();
    let other_page = resources.save(other, authored, None, None).await.unwrap();
    let Content::LearningPage(content) = &page.content else {
        panic!("wrong resource kind")
    };
    let public_id = content.public_id.clone();
    assert_eq!(public_id.len(), 32);
    assert_ne!(public_id, "caller-chosen-identity");
    assert!(
        resources
            .get(other, Kind::LearningPage, page.id)
            .await
            .is_err()
    );
    assert!(
        resources
            .delete(other, Kind::LearningPage, page.id, None)
            .await
            .is_err()
    );
    input.name = "Learning destination".into();
    input.idempotency = None;
    input.policy.learning_page_id = Some(other_page.id);
    assert!(campaigns.prepare(actor, &input, now).await.is_err());
    input.policy.learning_page_id = Some(page.id);
    input.policy.redirect_url = Some("https://outside.example.test".into());
    assert!(campaigns.prepare(actor, &input, now).await.is_err());
    input.policy.redirect_url = None;
    let prepared = campaigns.prepare(actor, &input, now).await.unwrap();
    assert!(prepared.policy.redirect_url.is_empty());
    let campaign = campaigns.create(actor, input.clone(), now).await.unwrap();
    assert_eq!(
        campaign.policy.as_ref().unwrap().learning_page_id,
        Some(page.id)
    );
    assert!(matches!(
        resources
            .delete(actor, Kind::LearningPage, page.id, None)
            .await,
        Err(Error::InUse)
    ));
    let deliveries = campaigns
        .deliveries(actor, campaign.id, 0, 50)
        .await
        .unwrap();
    let rid = &deliveries.items[0].rid;
    let before = campaigns
        .get(actor, campaign.id)
        .await
        .unwrap()
        .events
        .len();
    for _ in 0..2 {
        let interactions::Output::Html { html, csp } = public
            .learning_page("https://training.example.test", &public_id)
            .await
            .unwrap()
        else {
            panic!("expected HTML")
        };
        assert!(html.contains("First explanation"));
        assert!(csp.contains("script-src 'none'"));
    }
    assert!(
        public
            .learning_page("https://wrong.example.test", &public_id)
            .await
            .is_err()
    );
    assert_eq!(
        campaigns
            .get(actor, campaign.id)
            .await
            .unwrap()
            .events
            .len(),
        before
    );
    for _ in 0..2 {
        let interactions::Output::Submitted { redirect } = public
            .interact(
                "https://training.example.test",
                rid,
                interactions::Action::Submit {
                    event_id: "d".repeat(32),
                    password_present: true,
                },
                now,
            )
            .await
            .unwrap()
        else {
            panic!("expected submission")
        };
        assert_eq!(
            redirect.as_deref(),
            Some(format!("https://training.example.test/learn/{public_id}").as_str())
        );
    }
    assert_eq!(
        campaigns
            .get(actor, campaign.id)
            .await
            .unwrap()
            .events
            .len(),
        before + 1
    );
    let mut changed = page.clone();
    changed.content = Content::LearningPage(LearningPage {
        html: "<h1>Updated explanation</h1>".into(),
        public_id: "replacement-identity".into(),
    });
    let changed = resources
        .save(
            actor,
            changed.clone().into(),
            Some(page.id),
            Some(page.modified_date.clone()),
        )
        .await
        .unwrap();
    assert!(matches!(
        resources
            .save(
                actor,
                changed.clone().into(),
                Some(page.id),
                Some(page.modified_date.clone())
            )
            .await,
        Err(Error::Changed)
    ));
    let interactions::Output::Html { html, .. } = public
        .learning_page("https://training.example.test", &public_id)
        .await
        .unwrap()
    else {
        panic!("expected HTML")
    };
    assert!(html.contains("Updated explanation"));
    campaigns.complete(actor, campaign.id, now).await.unwrap();
    resources
        .delete(actor, Kind::LearningPage, page.id, None)
        .await
        .unwrap();
    assert!(
        public
            .learning_page("https://training.example.test", &public_id)
            .await
            .is_err()
    );
    campaigns.delete(actor, campaign.id).await.unwrap();

    // Competing launch and delete must never leave an unfinished campaign dangling.
    let racing = resources
        .save(
            actor,
            resource(
                "Racing explanation",
                Content::LearningPage(LearningPage {
                    html: "<p>Training</p>".into(),
                    ..Default::default()
                }),
            ),
            None,
            None,
        )
        .await
        .unwrap();
    input.policy.learning_page_id = Some(racing.id);
    let (launch, deletion) = tokio::join!(
        campaigns.create(actor, input, now),
        resources.delete(actor, Kind::LearningPage, racing.id, None)
    );
    match launch {
        Ok(launched) => {
            assert!(matches!(deletion, Err(Error::InUse)));
            campaigns.delete(actor, launched.id).await.unwrap();
            resources
                .delete(actor, Kind::LearningPage, racing.id, None)
                .await
                .unwrap();
        }
        Err(_) => assert!(deletion.is_ok()),
    }
    resources
        .delete(other, Kind::LearningPage, other_page.id, None)
        .await
        .unwrap();
}
