//! Shared access, attribution and deletion through real repositories and workers.
use super::{query_work::Inspector, *};

const ADMIN: auth::Identity = auth::Identity { id: 1, epoch: 0 };
const ALICE: auth::Identity = auth::Identity { id: 2, epoch: 0 };
const BOB: auth::Identity = auth::Identity { id: 3, epoch: 0 };

async fn contract(db: &Database, inspect: &mut Inspector) {
    inspect.execute("INSERT INTO users(id,username,hash,api_key,role_id,password_change_required,account_locked) VALUES(1,'admin','hash','admin-key',1,FALSE,FALSE),(2,'alice','hash','alice-key',2,FALSE,FALSE),(3,'bob','hash','bob-key',2,FALSE,FALSE); INSERT INTO ruphish_auth_state VALUES(1,0),(2,0),(3,0); UPDATE ruphish_settings SET verification_enabled=FALSE,webhooks_enabled=TRUE,reporting_mailboxes_enabled=TRUE;").await;
    // Explicit fixture IDs advance MySQL/SQLite allocation, but not PostgreSQL identities.
    if matches!(inspect, Inspector::Postgres(_)) {
        inspect
            .execute("SELECT setval(pg_get_serial_sequence('users','id'),3)")
            .await;
    }
    revision_contract(db).await;
    contended_replacements(db).await;
    let resources = db.resources();
    let group = resources
        .save(
            ALICE,
            resource(
                "Shared group",
                Content::Group(vec![Recipient {
                    email: "person@example.test".into(),
                    ..Default::default()
                }]),
            ),
            None,
            None,
        )
        .await
        .unwrap();
    let template = resources
        .save(
            ALICE,
            resource(
                "Shared email",
                Content::Template(Template {
                    subject: "Original".into(),
                    html: "<p>Original</p>".into(),
                    ..Default::default()
                }),
            ),
            None,
            None,
        )
        .await
        .unwrap();
    let page = resources
        .save(
            BOB,
            resource(
                "Shared landing",
                Content::Page(Page {
                    html: "<h1>Original landing</h1>".into(),
                    ..Default::default()
                }),
            ),
            None,
            None,
        )
        .await
        .unwrap();
    let profile = resources
        .save(
            ALICE,
            resource(
                "Shared SMTP",
                Content::Profile(Profile {
                    host: "127.0.0.1:2525".into(),
                    from_address: "training@example.test".into(),
                    interface_type: "SMTP".into(),
                    password: "kept-secret".into(),
                    ..Default::default()
                }),
            ),
            None,
            None,
        )
        .await
        .unwrap();
    for (kind, saved) in [
        (Kind::Group, &group),
        (Kind::Template, &template),
        (Kind::Page, &page),
        (Kind::Profile, &profile),
    ] {
        let read = resources.get(BOB, kind, saved.id).await.unwrap();
        assert_eq!(read.attribution, saved.attribution);
        assert_eq!(
            saved.attribution.created_at.as_deref(),
            Some(saved.modified_date.as_str())
        );
        assert_eq!(
            resources.summaries(BOB, kind, None).await.unwrap()[0].attribution,
            saved.attribution
        );
        assert!(
            resources
                .get(auth::Identity { id: 999, epoch: 0 }, kind, saved.id)
                .await
                .is_err()
        );
    }
    assert!(db.settings().get(BOB).await.is_err());
    assert!(db.accounts().list(BOB).await.is_err());
    let mut left = template.clone();
    left.name = "Shared email A".into();
    left.attribution.created_by = Some(999);
    left.attribution.edited_by = Some(999);
    let mut right = template.clone();
    right.name = "Shared email B".into();
    let (a, b) = tokio::join!(
        resources.save(
            BOB,
            left.into(),
            Some(template.id),
            Some(template.revision.to_string())
        ),
        resources.save(
            ALICE,
            right.into(),
            Some(template.id),
            Some(template.revision.to_string())
        )
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(matches!(
        a.as_ref().err().or(b.as_ref().err()),
        Some(Error::Changed)
    ));
    let editor = if a.is_ok() { BOB.id } else { ALICE.id };
    let edited = a.or(b).unwrap();
    assert_eq!(edited.attribution.created_by, Some(ALICE.id));
    assert_eq!(
        edited.attribution.created_at,
        template.attribution.created_at
    );
    assert_eq!(edited.attribution.edited_by, Some(editor));
    assert_eq!(
        edited.attribution.edited_at.as_ref(),
        Some(&edited.modified_date)
    );
    assert_eq!(
        resources
            .get(ADMIN, Kind::Template, template.id)
            .await
            .unwrap()
            .attribution,
        edited.attribution
    );
    let persisted = resources
        .get(ADMIN, Kind::Template, template.id)
        .await
        .unwrap();
    assert_eq!(
        persisted.name,
        if editor == BOB.id {
            "Shared email A"
        } else {
            "Shared email B"
        }
    );
    assert!(persisted.content == edited.content);
    assert!(matches!(
        resources
            .save(
                BOB,
                template.clone().into(),
                Some(template.id),
                Some(template.revision.to_string())
            )
            .await,
        Err(Error::Changed)
    ));
    let unchanged = resources
        .get(ADMIN, Kind::Template, template.id)
        .await
        .unwrap();
    assert!(
        unchanged == persisted,
        "a delayed stale save must not alter content or attribution"
    );
    assert!(matches!(
        resources
            .save(
                BOB,
                resource(&edited.name, Content::Template(Template::default())),
                None,
                None
            )
            .await,
        Err(Error::Conflict(_))
    ));
    let now = 1_800_000_000;
    let domain = db
        .domains()
        .save(
            ALICE,
            None,
            domains::Edit {
                name: "Shared domain".into(),
                origin: "http://127.0.0.1:8081".into(),
                enabled: true,
                revision: None,
            },
            now,
        )
        .await
        .unwrap();
    assert_eq!(
        db.domains()
            .get(BOB, domain.id)
            .await
            .unwrap()
            .attribution
            .created_by,
        Some(ALICE.id)
    );
    let select = |r: &Resource| campaign::Selection {
        id: r.id,
        revision: Some(r.revision.to_string()),
    };
    let input = campaign::Create {
        name: "Shared campaign".into(),
        groups: vec![select(&group)],
        template: select(&edited),
        page: select(&page),
        profile: select(&profile),
        url: "http://127.0.0.1:8081".into(),
        launch: Some(now + 60),
        idempotency: Some(("same-key".into(), "fingerprint".into())),
        ..Default::default()
    };
    let campaigns = db.campaigns();
    let launched = campaigns.create(BOB, input.clone(), now).await.unwrap();
    let retry = campaigns.create(BOB, input.clone(), now).await.unwrap();
    assert_eq!(launched.id, retry.id);
    let separate = campaigns.create(ALICE, input.clone(), now).await.unwrap();
    assert_ne!(
        launched.id, separate.id,
        "launch retry identities remain caller-specific"
    );
    campaigns.complete(BOB, separate.id, now).await.unwrap();
    assert_eq!(
        campaigns
            .header(ALICE, launched.id)
            .await
            .unwrap()
            .attribution
            .created_by,
        Some(BOB.id)
    );
    let mut changed = edited.clone();
    if let Content::Template(t) = &mut changed.content {
        t.subject = "Replacement".into();
    }
    resources
        .save(
            ALICE,
            changed.into(),
            Some(edited.id),
            Some(edited.revision.to_string()),
        )
        .await
        .unwrap();
    let before = campaigns
        .header(ADMIN, launched.id)
        .await
        .unwrap()
        .attribution;
    // Delete both creator and editor while the shared campaign is scheduled.
    db.accounts()
        .write(ADMIN, auth::Write::Delete(ALICE.id))
        .await
        .unwrap();
    db.accounts()
        .write(ADMIN, auth::Write::Delete(BOB.id))
        .await
        .unwrap();
    assert!(db.accounts().by_key("alice-key").await.is_err());
    assert!(
        resources
            .get(ALICE, Kind::Template, template.id)
            .await
            .is_err()
    );
    assert_eq!(
        inspect
            .scalar("SELECT count(*) FROM ruphish_deleted_accounts")
            .await,
        2
    );
    assert_eq!(
        inspect
            .scalar("SELECT count(*) FROM ruphish_campaign_creation")
            .await,
        2
    );
    assert!(
        db.accounts()
            .display_identities(&[])
            .await
            .unwrap()
            .is_empty()
    );
    let selected = db
        .accounts()
        .display_identities(&[ALICE.id, ALICE.id, 999999])
        .await
        .unwrap();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].id, ALICE.id);
    assert!(selected[0].deleted);
    let authors = db
        .accounts()
        .display_identities(&[1, 2, 3, 4])
        .await
        .unwrap();
    assert!(
        authors
            .iter()
            .any(|a| a.id == ALICE.id && a.username == "alice" && a.deleted)
    );
    assert!(
        authors
            .iter()
            .any(|a| a.id == BOB.id && a.username == "bob" && a.deleted)
    );
    let replacement = db
        .accounts()
        .write(
            ADMIN,
            auth::Write::Save {
                target: None,
                hash: Some("fixture-hash".into()),
                edit: auth::Edit {
                    username: "alice".into(),
                    password: String::new(),
                    role: "user".into(),
                    password_change_required: false,
                    account_locked: false,
                },
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_ne!(
        replacement.identity.id, ALICE.id,
        "username reuse must not reuse the deleted attribution identity"
    );
    let authors = db
        .accounts()
        .display_identities(&[1, 2, 3, 4])
        .await
        .unwrap();
    assert_eq!(authors.iter().filter(|a| a.username == "alice").count(), 2);
    let retained = resources
        .get(ADMIN, Kind::Profile, profile.id)
        .await
        .unwrap();
    assert!(matches!(retained.content,Content::Profile(p) if p.password=="kept-secret"));
    let claim = campaigns.claim(now + 60).await.unwrap().unwrap();
    assert_eq!(claim.campaign_id, launched.id);
    assert_eq!(claim.message.template.subject, "Original");
    assert!(campaigns.arm(&claim, now + 61).await.unwrap());
    assert_eq!(
        campaigns
            .header(ADMIN, launched.id)
            .await
            .unwrap()
            .attribution,
        before,
        "worker activity is not a human edit"
    );
    assert!(matches!(
        db.interactions()
            .interact(
                "http://127.0.0.1:8081",
                &launched.results[0].rid,
                interactions::Action::View {
                    client: Default::default()
                },
                now + 62
            )
            .await
            .unwrap(),
        interactions::Output::Html { .. }
    ));
    // Historical duplicate names retain separate IDs, and cannot be resolved arbitrarily.
    inspect.execute("INSERT INTO pages(id,user_id,name,html,modified_date) VALUES(900,2,'Imported duplicate','First','2020-01-01T00:00:00Z'),(901,3,'Imported duplicate','Second','2020-01-01T00:00:00Z');").await;
    let historical = resources.get(ADMIN, Kind::Page, 900).await.unwrap();
    assert_eq!(historical.attribution.created_by, Some(ALICE.id));
    assert_eq!(historical.attribution.edited_by, None);
    assert_eq!(historical.attribution.edited_at, None);
    assert_eq!(historical.attribution.created_at, None);
    let mut historical_edit = historical.clone();
    historical_edit.content = Content::Page(Page {
        html: "<p>Updated</p>".into(),
        ..Default::default()
    });
    let historical_edit = resources
        .save(
            ADMIN,
            historical_edit.into(),
            Some(historical.id),
            Some(historical.revision.to_string()),
        )
        .await
        .unwrap();
    assert_eq!(historical_edit.attribution.created_at, None);
    assert!(historical_edit.attribution.edited_at.is_some());
    assert_eq!(resources.get(ADMIN, Kind::Page, 901).await.unwrap().id, 901);
    assert!(matches!(
        resources
            .get_by_name(ADMIN, Kind::Page, "Imported duplicate")
            .await,
        Err(Error::Invalid(_))
    ));
    // Existing campaign references stay exact, but a new origin-only selection is ambiguous.
    inspect.execute("INSERT INTO ruphish_domains(id,owner_id,name,origin,hostname,enabled,challenge,challenge_expires) VALUES(900,3,'Historical duplicate origin','http://127.0.0.1:8081','127.0.0.1',TRUE,'old',0);").await;
    let mut input = input;
    input.template.revision = None;
    assert!(matches!(
        db.campaigns().prepare(ADMIN, &input, now + 63).await,
        Err(Error::Invalid(_))
    ));

    let duplicate = db.domains().get(ADMIN, 900).await.unwrap();
    db.domains()
        .save(
            ADMIN,
            Some(900),
            domains::Edit {
                name: duplicate.name,
                origin: duplicate.origin,
                enabled: false,
                revision: Some(duplicate.revision),
            },
            now + 64,
        )
        .await
        .unwrap();
    assert!(
        db.campaigns()
            .prepare(ADMIN, &input, now + 65)
            .await
            .is_ok(),
        "disabling a duplicate resolves selection without deleting history"
    );
}

#[tokio::test]
async fn sqlite_shared_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("shared.db");
    let db = database::open(Config::Sqlite { path: path.clone() }, Policy::default())
        .await
        .unwrap();
    let connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(&path),
    )
    .await
    .unwrap();
    contract(&db, &mut Inspector::Sqlite(connection)).await;
    db.close().await;
    let reopened = database::open(Config::Sqlite { path }, Policy::default())
        .await
        .unwrap();
    assert_eq!(
        reopened
            .resources()
            .list(ADMIN, Kind::Template)
            .await
            .unwrap()
            .len(),
        1
    );
    reopened.close().await;
}

#[tokio::test]
#[ignore = "Requires canonical MySQL fixture"]
async fn mysql_shared_workspace() {
    let (connection, config) = fixture().await;
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    let mut inspect = Inspector::Mysql(connection);
    contract(&db, &mut inspect).await;
    db.close().await;
    inspect
        .execute(&format!("DROP DATABASE `{}`", config.database))
        .await;
}

#[tokio::test]
#[ignore = "Requires canonical PostgreSQL fixture"]
async fn postgres_shared_workspace() {
    let (mut admin, config) = postgres_fixture().await;
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    let mut inspect = Inspector::Postgres(postgres_inspect(&config).await);
    contract(&db, &mut inspect).await;
    drop(inspect);
    db.close().await;
    postgres_cleanup(&mut admin, &config).await;
}

// Exercise actual transactions and complete resource replacements, not a revision helper.
async fn revision_contract(db: &Database) {
    let domains = db.domains();
    let edit = |name: &str, revision| domains::Edit {
        name: name.into(),
        origin: "http://127.0.0.1:18081".into(),
        enabled: true,
        revision,
    };
    let domain = domains
        .save(ALICE, None, edit("Revision origin", None), 1_800_000_000)
        .await
        .unwrap();
    assert_eq!(domain.revision, 1);
    assert!(domain.attribution.created_at.is_some());
    assert!(matches!(
        domains
            .save(
                BOB,
                Some(domain.id),
                edit("Missing version", None),
                1_800_000_001
            )
            .await,
        Err(Error::RevisionRequired)
    ));
    let (a, b) = tokio::join!(
        domains.save(
            ALICE,
            Some(domain.id),
            edit("Origin A", Some(1)),
            1_800_000_001
        ),
        domains.save(
            BOB,
            Some(domain.id),
            edit("Origin B", Some(1)),
            1_800_000_001
        )
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(matches!(
        a.as_ref().err().or(b.as_ref().err()),
        Some(Error::Changed)
    ));
    let saved = a.or(b).unwrap();
    assert_eq!(saved.revision, 2);
    assert_eq!(saved.attribution.created_at, domain.attribution.created_at);
    assert_eq!(domains.get(ADMIN, saved.id).await.unwrap().name, saved.name);
    assert!(matches!(
        domains.delete(ADMIN, saved.id, 1).await,
        Err(Error::Changed)
    ));
    let (changed, deleted) = tokio::join!(
        domains.save(
            ALICE,
            Some(saved.id),
            edit("Origin C", Some(2)),
            1_800_000_002
        ),
        domains.delete(BOB, saved.id, 2)
    );
    match (changed, deleted) {
        (Ok(value), Err(Error::Changed)) => {
            assert_eq!(value.revision, 3);
            domains.delete(ADMIN, value.id, 3).await.unwrap();
        }
        (Err(Error::NotFound(_)), Ok(())) => {}
        _ => panic!("one domain edit/delete must commit"),
    }
    assert!(matches!(
        domains
            .save(
                ALICE,
                Some(saved.id),
                edit("Cannot resurrect", Some(2)),
                1_800_000_003
            )
            .await,
        Err(Error::NotFound(_))
    ));
    let resources = db.resources();
    for (kind, content) in [
        (
            Kind::Group,
            Content::Group(vec![Recipient {
                email: "original@example.test".into(),
                ..Default::default()
            }]),
        ),
        (
            Kind::Template,
            Content::Template(Template {
                subject: "Original".into(),
                html: "<p>Original</p>".into(),
                ..Default::default()
            }),
        ),
        (
            Kind::Page,
            Content::Page(Page {
                html: "<p>Original</p>".into(),
                ..Default::default()
            }),
        ),
        (
            Kind::LearningPage,
            Content::LearningPage(LearningPage {
                html: "<p>Original</p>".into(),
                public_id: String::new(),
            }),
        ),
        (
            Kind::Profile,
            Content::Profile(Profile {
                host: "127.0.0.1:2525".into(),
                from_address: "training@example.test".into(),
                interface_type: "SMTP".into(),
                password: "unchanged-secret".into(),
                ..Default::default()
            }),
        ),
    ] {
        let original = resources
            .save(ALICE, resource("Revision contract", content), None, None)
            .await
            .unwrap();
        assert_eq!(original.revision, 1);
        let mut left = original.clone();
        left.name = "Revision winner A".into();
        let mut right = original.clone();
        right.name = "Revision winner B".into();
        assert!(matches!(
            resources
                .save(BOB, left.clone().into(), Some(original.id), None)
                .await,
            Err(Error::RevisionRequired)
        ));
        assert!(matches!(
            resources.delete(BOB, kind, original.id, None).await,
            Err(Error::RevisionRequired)
        ));
        assert!(resources.get(BOB, kind, original.id).await.unwrap() == original.record);
        let (a, b) = tokio::join!(
            resources.save(ALICE, left.into(), Some(original.id), Some("1".into())),
            resources.save(BOB, right.into(), Some(original.id), Some("1".into()))
        );
        assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
        assert!(matches!(
            a.as_ref().err().or(b.as_ref().err()),
            Some(Error::Changed)
        ));
        let editor = if a.is_ok() { ALICE.id } else { BOB.id };
        let winner = a.or(b).unwrap();
        assert_eq!(winner.revision, 2);
        assert_eq!(winner.attribution.edited_by, Some(editor));
        assert!(resources.get(ADMIN, kind, original.id).await.unwrap() == winner.record);
        let summary = resources
            .summaries(ADMIN, kind, None)
            .await
            .unwrap()
            .into_iter()
            .find(|r| r.id == original.id)
            .unwrap();
        assert_eq!(summary.revision, 2);
        assert!(matches!(
            resources
                .delete(BOB, kind, original.id, Some("1".into()))
                .await,
            Err(Error::Changed)
        ));
        // A delete of the reviewed version cannot erase a newer edit.
        let mut last = winner.clone();
        last.name = "Revision final edit".into();
        let (edit, delete) = tokio::join!(
            resources.save(ALICE, last.into(), Some(original.id), Some("2".into())),
            resources.delete(BOB, kind, original.id, Some("2".into()))
        );
        match (edit, delete) {
            (Ok(saved), Err(Error::Changed)) => {
                assert_eq!(saved.revision, 3);
                assert_eq!(saved.name, "Revision final edit");
                assert!(resources.get(BOB, kind, original.id).await.unwrap() == saved.record);
                resources
                    .delete(BOB, kind, original.id, Some("3".into()))
                    .await
                    .unwrap();
            }
            (Err(Error::NotFound(_)), Ok(())) => {}
            _ => panic!("exactly one edit/delete must commit"),
        }
        assert!(matches!(
            resources
                .save(
                    ALICE,
                    original.clone().into(),
                    Some(original.id),
                    Some("1".into())
                )
                .await,
            Err(Error::NotFound(_))
        ));
        assert!(matches!(
            resources
                .delete(BOB, kind, original.id, Some("2".into()))
                .await,
            Err(Error::NotFound(_))
        ));
        assert!(matches!(
            resources.get(ADMIN, kind, original.id).await,
            Err(Error::NotFound(_))
        ));
    }
}

// Eight independent callers start from the same review. Each replacement has distinct
// nested data, so a mixed parent/child result cannot pass by checking only its revision.
async fn contended_replacements(db: &Database) {
    let repo = db.resources();
    for kind in [Kind::Group, Kind::Template] {
        let content = |writer: usize| match kind {
            Kind::Group => Content::Group(vec![Recipient {
                email: format!("writer-{writer}@example.test"),
                first_name: format!("Writer {writer}"),
                ..Default::default()
            }]),
            _ => Content::Template(Template {
                subject: format!("Subject {writer}"),
                html: format!("<p>Writer {writer}</p>"),
                attachments: vec![ruphish_application::resources::Attachment {
                    name: format!("writer-{writer}.txt"),
                    mime_type: "text/plain".into(),
                    content: "Zml4dHVyZQ==".into(),
                }],
                ..Default::default()
            }),
        };
        let original = repo
            .save(
                ALICE,
                resource("Contended replacement", content(99)),
                None,
                None,
            )
            .await
            .unwrap();
        let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(8));
        let mut tasks = tokio::task::JoinSet::new();
        for writer in 0..8 {
            let repo = repo.clone();
            let barrier = barrier.clone();
            let input = resource(&format!("Winner {writer}"), content(writer));
            let id = original.id;
            tasks.spawn(async move {
                barrier.wait().await;
                (
                    writer,
                    repo.save(
                        if writer % 2 == 0 { ALICE } else { BOB },
                        input,
                        Some(id),
                        Some("1".into()),
                    )
                    .await,
                )
            });
        }
        let mut winners = vec![];
        let mut conflicts = 0;
        while let Some(result) = tasks.join_next().await {
            let (writer, result) = result.unwrap();
            match result {
                Ok(saved) => winners.push((writer, saved)),
                Err(Error::Changed) => conflicts += 1,
                Err(other) => panic!("unexpected replacement failure: {other:?}"),
            }
        }
        assert_eq!(conflicts, 7);
        assert_eq!(winners.len(), 1);
        let (writer, saved) = winners.pop().unwrap();
        let stored = repo.get(ADMIN, kind, original.id).await.unwrap();
        assert_eq!(stored.revision, 2);
        assert_eq!(stored.name, format!("Winner {writer}"));
        assert!(stored.content == content(writer));
        assert!(stored == saved.record);
        assert!(
            saved
                .identities
                .iter()
                .any(|i| i.id == ALICE.id && i.username == "alice" && !i.deleted)
        );
        let editor = if writer % 2 == 0 { ALICE.id } else { BOB.id };
        assert!(
            saved
                .identities
                .iter()
                .any(|i| i.id == editor && !i.deleted)
        );
        assert_eq!(
            saved.identities.len(),
            if editor == ALICE.id { 1 } else { 2 }
        );
        // A failed missing/stale revision leaves every child row and the revision untouched.
        for revision in [None, Some("1".into()), Some("9223372036854775807".into())] {
            assert!(
                repo.save(
                    BOB,
                    resource("Rejected", content(98)),
                    Some(original.id),
                    revision
                )
                .await
                .is_err()
            );
            assert!(repo.get(ADMIN, kind, original.id).await.unwrap() == stored);
        }
        repo.delete(ADMIN, kind, original.id, Some("2".into()))
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn sqlite_attribution_failure_rolls_back_content_children_and_revision() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("attribution-rollback.db");
    let db = database::open(Config::Sqlite { path: path.clone() }, Policy::default())
        .await
        .unwrap();
    let mut connection = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(&path),
    )
    .await
    .unwrap();
    let display_schema: String =
        sqlx::query_scalar("SELECT sql FROM sqlite_schema WHERE name='ruphish_deleted_accounts'")
            .fetch_one(&mut connection)
            .await
            .unwrap();
    let mut inspect = Inspector::Sqlite(connection);
    inspect.execute("INSERT INTO users(id,username,hash,api_key,role_id,password_change_required,account_locked) VALUES(1,'admin','hash','key',1,FALSE,FALSE); INSERT INTO ruphish_auth_state VALUES(1,0);").await;
    let repo = db.resources();
    let group = |name: &str, email: &str| {
        resource(
            name,
            Content::Group(vec![Recipient {
                email: email.into(),
                ..Default::default()
            }]),
        )
    };
    let original = repo
        .save(
            ADMIN,
            group("Original", "original@example.test"),
            None,
            None,
        )
        .await
        .unwrap();
    // The display-only dependency fails after parent and child writes. It must fail
    // the transaction, not turn a committed edit into a misleading failed response.
    inspect.execute("DROP TABLE ruphish_deleted_accounts").await;
    assert!(matches!(
        repo.save(
            ADMIN,
            group("Replacement", "replacement@example.test"),
            Some(original.id),
            Some("1".into())
        )
        .await,
        Err(Error::Storage(
            ruphish_application::storage::Error::Unavailable
        ))
    ));
    assert!(repo.get(ADMIN, Kind::Group, original.id).await.unwrap() == original.record);
    assert_eq!(inspect.scalar("SELECT count(*) FROM targets").await, 1);
    assert_eq!(
        inspect.scalar("SELECT count(*) FROM group_targets").await,
        1
    );
    assert!(
        repo.save(
            ADMIN,
            group("Failed creation", "new@example.test"),
            None,
            None
        )
        .await
        .is_err()
    );
    assert_eq!(inspect.scalar("SELECT count(*) FROM groups").await, 1);
    inspect.execute(&display_schema).await;
    let retry = repo
        .save(
            ADMIN,
            group("Replacement", "replacement@example.test"),
            Some(original.id),
            Some("1".into()),
        )
        .await
        .unwrap();
    assert_eq!(retry.revision, 2);
    assert_eq!(retry.identities.len(), 1);
    assert_eq!(retry.identities[0].username, "admin");
    drop(inspect);
    db.close().await;
    let reopened = database::open(Config::Sqlite { path }, Policy::default())
        .await
        .unwrap();
    assert!(
        reopened
            .resources()
            .get(ADMIN, Kind::Group, retry.id)
            .await
            .unwrap()
            == retry.record
    );
    reopened.close().await;
}
