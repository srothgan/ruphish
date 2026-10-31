//! Independent expected outcomes for the same original-Gophish dataset on both engines.
use super::*;

pub const SEED: &str = include_str!("../../../../tests/fixtures/migration/rich.sql");
pub const OWNER: auth::Identity = auth::Identity { id: 17, epoch: 0 };
pub const OTHER: auth::Identity = auth::Identity { id: 18, epoch: 0 };

pub fn hash() -> String {
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/gophish-v0.12.1/password.json"
    ))
    .unwrap();
    value["bcrypt_hash"].as_str().unwrap().to_owned()
}

// Raw checks complement the public facade, which intentionally hides legacy private values.
pub const INSPECTIONS: &[(&str, i64)] = &[
    ("SELECT count(*) FROM users", 4),
    ("SELECT count(*) FROM group_targets", 4),
    (
        "SELECT count(*) FROM attachments WHERE id IN (43,44) AND template_id=41",
        2,
    ),
    (
        "SELECT count(*) FROM email_requests WHERE id=91 AND user_id=17 AND template_id=41 AND page_id=47 AND r_id='test-rid'",
        1,
    ),
    (
        "SELECT count(*) FROM results WHERE ip<>'' OR latitude<>0 OR longitude<>0",
        0,
    ),
    (
        "SELECT count(*) FROM events WHERE metadata<>'' OR client IS NOT NULL OR source<>'legacy'",
        0,
    ),
    ("SELECT count(*) FROM ruphish_legacy_queue_issues", 0),
    ("SELECT count(*) FROM mail_logs", 0),
    ("SELECT count(*) FROM imap", 0),
    ("SELECT count(*) FROM ruphish_webhook_jobs", 0),
    (
        "SELECT count(*) FROM ruphish_webhook_state WHERE webhook_id IN (81,82) AND owner_id=17 AND revision=1",
        2,
    ),
    (
        "SELECT count(*) FROM ruphish_mailboxes WHERE owner_id=17 AND password='imap-secret'",
        1,
    ),
    (
        "SELECT count(*) FROM ruphish_campaign_domains WHERE campaign_id IN (53,54,55,56)",
        4,
    ),
];

// One independently invalid queue condition per input. In particular, invalid
// retries must not be masked by a missing recipient, as in the older fixture.
pub const QUEUE_CASES: &[(&str, &str, &[i64])] = &[
    (
        "negative retry",
        "UPDATE mail_logs SET send_attempt=-1 WHERE id=301",
        &[301],
    ),
    (
        "retry overflow",
        "UPDATE mail_logs SET send_attempt=9 WHERE id=301",
        &[301],
    ),
    (
        "absent due time",
        "UPDATE mail_logs SET send_date=NULL WHERE id=301",
        &[301],
    ),
    (
        "missing recipient",
        "UPDATE mail_logs SET r_id='missing' WHERE id=301",
        &[301],
    ),
    (
        "wrong owner",
        "UPDATE mail_logs SET user_id=18 WHERE id=301",
        &[301],
    ),
    (
        "wrong campaign",
        "UPDATE mail_logs SET campaign_id=54 WHERE id=301",
        &[301],
    ),
    (
        "duplicate queue",
        "INSERT INTO mail_logs(id,campaign_id,user_id,r_id,send_date,send_attempt,processing) SELECT 306,campaign_id,user_id,r_id,send_date,send_attempt,processing FROM mail_logs WHERE id=301",
        &[301, 306],
    ),
    (
        "global recipient collision",
        "INSERT INTO results(id,campaign_id,user_id,r_id,email,status) VALUES(111,55,18,'pending','collision@example.test','Scheduled')",
        &[301],
    ),
];

pub async fn contract(db: &Database) {
    let accounts = db.accounts();
    let admin = accounts.by_key("admin-key").await.unwrap();
    assert_eq!(admin.identity, OWNER);
    assert_eq!(admin.role.slug, "admin");
    assert!(admin.role.permissions.iter().any(|p| p == "modify_system"));
    assert_eq!(admin.hash, hash());
    assert!(ruphish_adapters::password_worker::verify(
        "FixtureOnlyPassword42!",
        &admin.hash
    ));
    assert_eq!(accounts.by_key("user-key").await.unwrap().identity, OTHER);
    assert!(accounts.by_id(19).await.unwrap().account_locked);
    assert!(accounts.by_id(20).await.unwrap().password_change_required);
    let service = auth::Service::new(
        accounts,
        std::sync::Arc::new(ruphish_adapters::password_worker::PasswordWorker::default()),
    );
    assert!(
        service
            .session(auth::Identity { id: 19, epoch: 0 })
            .await
            .is_err()
    );

    let resources = db.resources();
    assert_eq!(
        resources
            .list(OWNER, Kind::Group)
            .await
            .unwrap()
            .iter()
            .map(|g| g.id)
            .collect::<Vec<_>>(),
        [21, 22, 24]
    );
    assert_eq!(
        resources
            .list(OTHER, Kind::Group)
            .await
            .unwrap()
            .iter()
            .map(|g| g.id)
            .collect::<Vec<_>>(),
        [23]
    );
    assert!(matches!(
        resources.get(OTHER, Kind::Group, 21).await,
        Err(Error::NotFound(_))
    ));
    let group = resources.get(OWNER, Kind::Group, 21).await.unwrap();
    assert_eq!(group.name, "Équipe 東京");
    let Content::Group(recipients) = group.content else {
        panic!("group")
    };
    assert_eq!(
        recipients,
        [
            Recipient {
                email: "Shared@Example.test".into(),
                first_name: "Zoë".into(),
                last_name: "李".into(),
                position: "Current role".into()
            },
            Recipient {
                email: "second@example.test".into(),
                ..Default::default()
            },
        ]
    );
    let Content::Group(overlap) = resources.get(OWNER, Kind::Group, 22).await.unwrap().content
    else {
        panic!("group")
    };
    assert_eq!(overlap, recipients[..1]);
    let Content::Group(empty) = resources.get(OWNER, Kind::Group, 24).await.unwrap().content else {
        panic!("group")
    };
    assert!(empty.is_empty());
    let Content::Template(template) = resources
        .get(OWNER, Kind::Template, 41)
        .await
        .unwrap()
        .content
    else {
        panic!("template")
    };
    assert_eq!(
        template,
        Template {
            subject: "Hello {{.FirstName}}".into(),
            text: "Plain {{.URL}}".into(),
            html: "<p>Grüße {{.FirstName}} — 東京</p>".into(),
            envelope_sender: "Visible <visible@example.test>".into(),
            attachments: vec![
                Attachment {
                    name: "資料.bin".into(),
                    mime_type: "application/octet-stream".into(),
                    content: "AAEC/w==".into()
                },
                Attachment {
                    name: "empty.txt".into(),
                    mime_type: "text/plain".into(),
                    content: String::new()
                }
            ],
        }
    );
    let Content::Profile(profile) = resources
        .get(OWNER, Kind::Profile, 67)
        .await
        .unwrap()
        .content
    else {
        panic!("profile")
    };
    assert_eq!(profile.host, "smtp.example.test:587");
    assert_eq!(profile.interface_type, "SMTP");
    assert_eq!(profile.username, "service");
    assert_eq!(profile.from_address, "Sender <sender@example.test>");
    assert_eq!(profile.password, "smtp-secret");
    assert!(profile.ignore_cert_errors);
    assert_eq!(
        profile.headers,
        [
            Header {
                key: "X-Training".into(),
                value: "{{.RId}}".into()
            },
            Header {
                key: "Reply-To".into(),
                value: "reply@example.test".into()
            }
        ]
    );
    let campaign = db.campaigns().get(OWNER, 53).await.unwrap();
    assert_eq!(campaign.status.as_str(), "In progress");
    assert_eq!(
        db.campaigns().get(OWNER, 54).await.unwrap().status.as_str(),
        "Completed"
    );
    assert_eq!(
        db.campaigns().get(OTHER, 55).await.unwrap().status.as_str(),
        "Queued"
    );
    assert_eq!(
        campaign.url,
        "https://training.example.test:8443/path?source=legacy"
    );
    assert_eq!(
        campaign
            .results
            .iter()
            .map(|r| (r.id, r.status.as_str()))
            .collect::<Vec<_>>(),
        [
            (101, "Submitted Data"),
            (102, "Email Opened"),
            (103, "Clicked Link"),
            (104, "Email Sent"),
            (105, "Error"),
            (106, "Sending"),
            (107, "Retrying"),
        ]
    );
    assert_eq!(campaign.results[0].recipient.position, "Historical role");
    assert!(campaign.results[0].reported);
    assert_eq!(
        campaign
            .events
            .iter()
            .map(|e| (e.id, e.recipient_id, e.message.as_str()))
            .collect::<Vec<_>>(),
        [
            (201, None, "Campaign Created"),
            (202, Some(101), "Email Sent"),
            (203, Some(101), "Email Opened"),
            (204, Some(101), "Email Opened"),
            (205, Some(101), "Clicked Link"),
            (206, Some(101), "Submitted Data"),
            (207, Some(101), "Email Reported"),
            (208, Some(105), "Error Sending Email"),
            (209, None, "Email Opened"),
        ]
    );
    assert!(
        campaign
            .events
            .iter()
            .all(|e| e.details.is_empty() && e.client.is_none())
    );
    assert_eq!(campaign.progression(), [1, 1, 1, 1, 1]);
    assert!(
        campaign.groups.is_empty(),
        "Gophish does not store campaign/group provenance"
    );
    assert_eq!(
        campaign.policy.unwrap().redirect_url,
        "https://learn.example.test/finish?q=1"
    );
    assert!(matches!(
        db.campaigns().get(OTHER, 53).await,
        Err(Error::NotFound(_))
    ));
    assert!(
        db.campaigns()
            .get(OWNER, 54)
            .await
            .unwrap()
            .policy
            .is_none()
    );
    for (owner, campaign_id, expected) in [
        (
            OWNER,
            53,
            vec![
                (106, "ready", 0, ""),
                (107, "unknown", 8, "legacy_delivery_outcome_unknown"),
            ],
        ),
        // Cancellation is the state; existing completion also retains the last
        // diagnostic rather than promising that a cancelled row has no code.
        (
            OWNER,
            54,
            vec![(108, "cancelled", 0, "campaign_page_missing")],
        ),
        (OTHER, 55, vec![(109, "ready", 8, "")]),
        (
            OWNER,
            56,
            vec![(110, "blocked", 0, "campaign_page_missing")],
        ),
    ] {
        let deliveries = db
            .campaigns()
            .deliveries(owner, campaign_id, 0, 50)
            .await
            .unwrap();
        assert_eq!(
            deliveries
                .items
                .iter()
                .map(|d| (d.id, d.state.as_str(), d.retries, d.code.as_str()))
                .collect::<Vec<_>>(),
            expected
        );
        assert!(deliveries.items.iter().all(|d| d.due == 1_893_456_000));
    }
    let mailboxes = db.integrations().mailboxes(OWNER).await.unwrap();
    assert_eq!(mailboxes.len(), 1);
    assert!(
        mailboxes[0].enabled
            && mailboxes[0].has_password
            && mailboxes[0].ignore_cert_errors
            && mailboxes[0].delete_reported
    );
    assert_eq!(
        (&*mailboxes[0].folder, mailboxes[0].frequency),
        ("Reports", 120)
    );
    let disabled = db.integrations().mailboxes(OTHER).await.unwrap();
    assert_eq!(disabled.len(), 1);
    assert!(!disabled[0].enabled);
    assert_eq!(
        (
            disabled[0].port,
            disabled[0].folder.as_str(),
            disabled[0].frequency
        ),
        (993, "INBOX", 60)
    );
    let hooks = db.integrations().webhooks(OWNER).await.unwrap();
    assert_eq!(
        hooks
            .iter()
            .map(|h| (h.id, h.active, h.has_secret))
            .collect::<Vec<_>>(),
        [(81, true, true), (82, false, false)]
    );
    assert!(db.integrations().webhooks(OTHER).await.unwrap().is_empty());
}
