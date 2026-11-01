//! Observable launch, queue, rendering and historical guarantees on every provider.
use super::*;
use ruphish_application::delivery::Sender;

pub(super) async fn exercise(
    db: &Database,
    actor: auth::Identity,
    mut input: campaign::Create,
    now: i64,
) {
    let resources = db.resources();
    let campaigns = db.campaigns();
    let mut selected = Vec::new();
    for (kind, id) in [
        (Kind::Template, input.template.id),
        (Kind::Page, input.page.id),
        (Kind::Profile, input.profile.id),
    ] {
        let mut copy = resources.get(actor, kind, id).await.unwrap();
        copy.name = format!("Frozen {}", kind.label());
        selected.push(
            resources
                .save(actor, copy.into(), None, None)
                .await
                .unwrap(),
        );
    }
    let template = selected[0].clone();
    let page = selected[1].clone();
    let profile = selected[2].clone();
    let select = |r: &Resource| campaign::Selection {
        id: r.id,
        revision: Some(r.modified_date.clone()),
    };
    input.template = select(&template);
    input.page = select(&page);
    input.profile = select(&profile);
    input.idempotency = None;
    input.launch = Some(now + 60);
    let launched = campaigns.create(actor, input.clone(), now).await.unwrap();
    assert!(campaigns.claim(now).await.unwrap().is_none());
    let before = campaigns.header(actor, launched.id).await.unwrap();
    // Future sends must retain content even after all source resources have changed.
    for original in [&template, &page, &profile] {
        let mut edit = original.clone();
        edit.name.push_str(" edited");
        match &mut edit.content {
            Content::Template(t) => {
                t.subject = "Replacement subject".into();
                t.html = "<p>Replacement mail</p>".into();
                t.attachments.clear();
            }
            Content::Page(p) => {
                p.html = "<h1>Replacement landing</h1>".into();
                p.redirect_url = "https://changed.example.test".into();
            }
            Content::Profile(p) => {
                p.from_address = "changed@example.test".into();
                p.headers.clear();
                p.password = "rotated-secret".into();
                p.host = "127.0.0.1:2526".into();
            }
            _ => panic!("unexpected kind"),
        }
        resources
            .save(
                actor,
                edit.into(),
                Some(original.id),
                Some(original.modified_date.clone()),
            )
            .await
            .unwrap();
    }
    assert!(
        matches!(
            campaigns.create(actor, input.clone(), now).await,
            Err(Error::Changed)
        ),
        "reviewed resources changed"
    );
    let after = campaigns.header(actor, launched.id).await.unwrap();
    assert_eq!(after.subject, before.subject);
    assert_eq!(after.template.name, before.template.name);
    assert_eq!(after.page.name, before.page.name);
    assert_eq!(after.profile_sender, before.profile_sender);
    let stored = campaigns.get(actor, launched.id).await.unwrap();
    assert!(stored.resources[0] == template);
    assert!(stored.resources[1] == page);
    let claim = campaigns.claim(now + 60).await.unwrap().unwrap();
    assert_eq!(claim.campaign_id, launched.id);
    let Content::Template(expected_template) = &template.content else {
        panic!("template")
    };
    let Content::Profile(expected_profile) = &profile.content else {
        panic!("profile")
    };
    assert_eq!(&claim.message.template, expected_template);
    assert_eq!(
        claim.message.profile.from_address,
        expected_profile.from_address
    );
    assert_eq!(claim.message.profile.headers, expected_profile.headers);
    assert_eq!(claim.message.profile.password, "rotated-secret");
    assert_eq!(claim.message.profile.host, "127.0.0.1:2526");
    let mime = ruphish_adapters::smtp::Smtp::new(Policy::default(), String::new())
        .compose(&claim.message)
        .unwrap();
    let parsed = mail_parser::MessageParser::default()
        .parse(&mime.bytes)
        .unwrap();
    assert!(!parsed.body_html(0).unwrap().contains("Replacement mail"));
    assert_eq!(
        parsed.attachment_count(),
        expected_template.attachments.len()
    );
    let interactions::Output::Html { html, .. } = db
        .interactions()
        .interact(
            "https://training.example.test",
            &launched.results[0].rid,
            interactions::Action::View {
                client: Default::default(),
            },
            now + 60,
        )
        .await
        .unwrap()
    else {
        panic!("landing")
    };
    assert!(html.contains("Hello"));
    assert!(!html.contains("Replacement landing"));
    // Rotation after connection preparation fences submission and safely reclaims it.
    let mut rotated = resources
        .get(actor, Kind::Profile, profile.id)
        .await
        .unwrap();
    let revision = rotated.modified_date.clone();
    if let Content::Profile(p) = &mut rotated.content {
        p.password = "second-secret".into();
    }
    resources
        .save(actor, rotated.into(), Some(profile.id), Some(revision))
        .await
        .unwrap();
    assert!(!campaigns.arm(&claim, now + 60).await.unwrap());
    let retry = campaigns.claim(now + 61).await.unwrap().unwrap();
    assert_eq!(retry.message.profile.password, "second-secret");
    assert_eq!(&retry.message.template, expected_template);
    // A template edit cannot invalidate immutable campaign content mid-preparation.
    let mut edited = resources
        .get(actor, Kind::Template, template.id)
        .await
        .unwrap();
    let revision = edited.modified_date.clone();
    if let Content::Template(t) = &mut edited.content {
        t.subject = "Another replacement".into();
    }
    resources
        .save(actor, edited.into(), Some(template.id), Some(revision))
        .await
        .unwrap();
    assert!(campaigns.arm(&retry, now + 61).await.unwrap());
    campaigns
        .finish(
            &retry,
            Err(campaign::DeliveryFailure::new(
                campaign::FailureKind::Retryable,
                campaign::Stage::Acceptance,
                Some(451),
            )),
            now + 61,
        )
        .await
        .unwrap();
    let due = campaigns
        .deliveries(actor, launched.id, 0, 50)
        .await
        .unwrap()
        .items[0]
        .due;
    let retry = campaigns.claim(due).await.unwrap().unwrap();
    assert_eq!(&retry.message.template, expected_template);
    assert_eq!(
        retry.message.profile.from_address,
        expected_profile.from_address
    );
    assert_eq!(retry.message.profile.password, "second-secret");
    assert!(campaigns.arm(&retry, due).await.unwrap());
    campaigns.finish(&retry, Ok(()), due).await.unwrap();
    // A concurrent edit either follows the snapshot or invalidates the reviewed launch.
    let mut contender = resources
        .get(actor, Kind::Template, template.id)
        .await
        .unwrap();
    input.template = select(&contender);
    input.page = select(&resources.get(actor, Kind::Page, page.id).await.unwrap());
    input.profile = select(
        &resources
            .get(actor, Kind::Profile, profile.id)
            .await
            .unwrap(),
    );
    input.launch = Some(now + 3600);
    let reviewed = contender.clone();
    let revision = contender.modified_date.clone();
    if let Content::Template(t) = &mut contender.content {
        t.subject = "Concurrent edit".into();
    }
    let (launch, edit) = tokio::join!(
        campaigns.create(actor, input, now),
        resources.save(actor, contender.into(), Some(template.id), Some(revision))
    );
    edit.unwrap();
    match launch {
        Ok(c) => {
            assert!(c.resources[0] == reviewed);
            campaigns.delete(actor, c.id).await.unwrap();
        }
        Err(e) => assert!(matches!(e, Error::Changed)),
    }
    campaigns
        .complete(actor, launched.id, due + 1)
        .await
        .unwrap();
    for original in [&template, &page, &profile] {
        resources
            .delete(actor, original.kind(), original.id, None)
            .await
            .unwrap();
    }
    let historical = campaigns.get(actor, launched.id).await.unwrap();
    assert!(historical.resources[0] == template);
    assert!(historical.resources[1] == page);
    assert_eq!(
        campaigns.header(actor, launched.id).await.unwrap().subject,
        before.subject
    );
    assert!(
        db.interactions()
            .interact(
                "https://training.example.test",
                &launched.results[0].rid,
                interactions::Action::View {
                    client: Default::default()
                },
                due + 2
            )
            .await
            .is_err()
    );
    campaigns.delete(actor, launched.id).await.unwrap();
}
