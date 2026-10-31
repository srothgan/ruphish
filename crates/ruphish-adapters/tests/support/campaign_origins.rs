//! Exact origins and the verification switch across real persistence and delivery boundaries.
use super::*;
use ruphish_application::delivery::Sender;

pub(super) async fn exercise(
    db: &Database,
    actor: auth::Identity,
    mut input: campaign::Create,
    now: i64,
) {
    let settings = db.settings();
    let original = settings.get(actor).await.unwrap().policy;
    assert!(original.verification_enabled);
    input.idempotency = None;
    let domains = db.domains();
    let campaigns = db.campaigns();
    let learning = db
        .resources()
        .save(
            actor,
            resource(
                "Origin explanation",
                Content::LearningPage(LearningPage {
                    html: "<h1>Origin explanation</h1>".into(),
                    ..Default::default()
                }),
            ),
            None,
            None,
        )
        .await
        .unwrap();
    let Content::LearningPage(page) = &learning.content else {
        panic!("learning page")
    };
    input.policy.learning_page_id = Some(learning.id);
    input.policy.redirect_url = None;
    for origin in [
        "http://plain.example.test",
        "http://192.0.2.20:8080",
        "https://192.0.2.20:8443",
        "http://[2001:db8::20]:8080",
        "https://[2001:db8::20]:8443",
    ] {
        let domain = domains
            .save(
                actor,
                None,
                domains::Edit {
                    name: origin.into(),
                    origin: origin.into(),
                    enabled: true,
                    revision: None,
                },
                now,
            )
            .await
            .unwrap();
        assert_eq!(domain.origin, origin);
        assert_eq!(domain.verified_until, 0);
        assert_eq!(domain.eligible_until, 0);
        assert!(domain.active_check.is_none());
        assert!(
            matches!(domains.request_check(actor, domain.id, now).await, Err(Error::Invalid(code)) if code == "domain_verification_unsupported")
        );
        assert!(domains.claim(now).await.unwrap().is_none());
        input.url = format!("{origin}/training/{{{{.FirstName}}}}?email={{{{.Email}}}}");
        assert!(campaigns.prepare(actor, &input, now).await.is_err());
        let current = settings.get(actor).await.unwrap();
        let mut off = current.policy;
        off.verification_enabled = false;
        settings
            .save(actor, current.revision, off, now)
            .await
            .unwrap();
        let prepared = campaigns.prepare(actor, &input, now).await.unwrap();
        assert_eq!(prepared.recipients.len(), 1);
        let campaign = campaigns.create(actor, input.clone(), now).await.unwrap();
        let claim = campaigns.claim(now).await.unwrap().unwrap();
        assert_eq!(claim.campaign_id, campaign.id);
        let context = ruphish_adapters::smtp::context(&claim.message).unwrap();
        let rendered = url::Url::parse(&context.url).unwrap();
        assert_eq!(rendered.origin().ascii_serialization(), origin);
        assert_eq!(
            percent_encoding::percent_decode_str(rendered.path())
                .decode_utf8()
                .unwrap(),
            format!("/training/{}", claim.message.recipient.first_name)
        );
        assert!(
            rendered
                .query_pairs()
                .any(|(k, v)| k == "email" && v == claim.message.recipient.email)
        );
        let mail = ruphish_adapters::smtp::Smtp::new(Policy::default(), String::new())
            .compose(&claim.message)
            .unwrap();
        let parsed = mail_parser::MessageParser::default()
            .parse(&mail.bytes)
            .unwrap();
        assert!(parsed.body_html(0).unwrap().contains(origin));
        let rid = &campaign.results[0].rid;
        let public = db.interactions();
        assert!(domains.serves(origin).await.unwrap());
        let wrong_scheme = if origin.starts_with("https:") {
            origin.replacen("https:", "http:", 1)
        } else {
            origin.replacen("http:", "https:", 1)
        };
        let mut wrong_port = rendered.clone();
        wrong_port.set_port(Some(9999)).unwrap();
        for wrong in [
            wrong_scheme,
            wrong_port.origin().ascii_serialization(),
            "http://other.example.test".into(),
        ] {
            assert!(
                public
                    .interact(&wrong, rid, interactions::Action::Open, now)
                    .await
                    .is_err()
            );
        }
        assert_eq!(
            campaigns
                .get(actor, campaign.id)
                .await
                .unwrap()
                .events
                .len(),
            campaign.events.len()
        );
        public
            .interact(origin, rid, interactions::Action::Open, now)
            .await
            .unwrap();
        let view = public
            .interact(
                origin,
                rid,
                interactions::Action::View {
                    client: Default::default(),
                },
                now,
            )
            .await
            .unwrap();
        assert!(
            matches!(view, interactions::Output::Html { html, .. } if html.contains(&format!("{origin}/?rid={rid}")))
        );
        let learning_view = public.learning_page(origin, &page.public_id).await.unwrap();
        assert!(
            matches!(learning_view, interactions::Output::Html { html, .. } if html.contains("Origin explanation"))
        );
        let submitted = public
            .interact(
                origin,
                rid,
                interactions::Action::Submit {
                    client: Default::default(),
                    event_id: "b".repeat(32),
                    password_present: false,
                },
                now,
            )
            .await
            .unwrap();
        assert!(
            matches!(submitted, interactions::Output::Submitted { redirect: Some(destination) } if destination == format!("{origin}/learn/{}", page.public_id))
        );
        let current = settings.get(actor).await.unwrap();
        settings
            .save(actor, current.revision, original.clone(), now)
            .await
            .unwrap();
        assert!(!domains.serves(origin).await.unwrap());
        assert!(!campaigns.arm(&claim, now).await.unwrap());
        assert!(
            public
                .interact(origin, rid, interactions::Action::Open, now)
                .await
                .is_err()
        );
        assert!(
            domains
                .get(actor, domain.id)
                .await
                .unwrap()
                .active_check
                .is_none()
        );
        assert!(domains.claim(now).await.unwrap().is_none());
        campaigns.complete(actor, campaign.id, now).await.unwrap();
        campaigns.delete(actor, campaign.id).await.unwrap();
        domains
            .delete(actor, domain.id, domain.revision)
            .await
            .unwrap();
    }
    db.resources()
        .delete(actor, Kind::LearningPage, learning.id, None)
        .await
        .unwrap();
}
