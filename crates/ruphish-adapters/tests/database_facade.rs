//! Real providers: isolation, restart, shared capabilities and closed-handle behavior.
#![allow(
    clippy::unwrap_used,
    reason = "Isolated functional fixtures and assertions"
)]
use ruphish_adapters::{
    database::{self, Config, Database, Error},
    outbound::Policy,
};
use ruphish_application::resources::{Content, Kind, Recipient, Resource};
use std::path::Path;

async fn open(path: &Path) -> Database {
    database::open(
        Config::Sqlite {
            path: path.to_owned(),
            migrations_prefix: String::new(),
        },
        Policy::default(),
    )
    .await
    .unwrap()
}
#[tokio::test]
async fn configured_databases_stay_isolated_and_closed_capabilities_cannot_reconnect() {
    let directory = tempfile::tempdir().unwrap();
    let first_path = directory.path().join("first.db");
    let first = open(&first_path).await;
    let second = open(&directory.path().join("second.db")).await;
    first
        .bootstrap(
            Some("first-test-password".into()),
            Some("first-test-token".into()),
        )
        .await
        .unwrap();
    second
        .bootstrap(
            Some("second-test-password".into()),
            Some("second-test-token".into()),
        )
        .await
        .unwrap();
    assert!(second.accounts().by_key("first-test-token").await.is_err());
    for (provider, key) in [(&first, "first-test-token"), (&second, "second-test-token")] {
        let actor = provider.accounts().by_key(key).await.unwrap();
        let accounts = ruphish_application::auth::Service::new(
            provider.accounts(),
            std::sync::Arc::new(ruphish_adapters::password_worker::PasswordWorker::default()),
        );
        accounts
            .change_password(
                actor.identity,
                None,
                "completed-test-password".into(),
                "completed-test-password",
            )
            .await
            .unwrap();
    }
    let actor = first.accounts().by_key("first-test-token").await.unwrap();
    let resources = first.resources();
    let saved = resources
        .save(
            actor.identity,
            Resource {
                id: 0,
                name: "Independent audience".into(),
                modified_date: String::new(),
                content: Content::Group(vec![Recipient {
                    email: "fixture@example.test".into(),
                    ..Default::default()
                }]),
            },
            None,
            None,
        )
        .await
        .unwrap();
    let second_actor = second.accounts().by_key("second-test-token").await.unwrap();
    assert!(matches!(
        second
            .resources()
            .get(second_actor.identity, Kind::Group, saved.id)
            .await,
        Err(ruphish_application::resources::Error::NotFound(_))
    ));
    assert!(
        second
            .resources()
            .list(second_actor.identity, Kind::Group)
            .await
            .unwrap()
            .is_empty()
    );
    let sessions = first.sessions();
    let key = sessions.cookie_key().await.unwrap();
    first.close().await;
    assert!(
        resources
            .get(actor.identity, Kind::Group, saved.id)
            .await
            .is_err()
    );
    assert!(sessions.cookie_key().await.is_err());
    let reopened = open(&first_path).await;
    assert_eq!(reopened.sessions().cookie_key().await.unwrap(), key);
    let restored = reopened
        .resources()
        .get(actor.identity, Kind::Group, saved.id)
        .await
        .unwrap();
    assert_eq!(restored.name, saved.name);
    assert!(restored.content == saved.content);
    // A new factory does not revive old handles.
    assert!(resources.list(actor.identity, Kind::Group).await.is_err());
    reopened.close().await;
    second.close().await;
}
#[tokio::test]
async fn failed_configuration_never_falls_back_or_leaks_driver_details() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("connection-secret-canary.db");
    std::fs::write(&path, b"not a database; secret fixture bytes").unwrap();
    let before = std::fs::read(&path).unwrap();
    let result = database::open(
        Config::Sqlite {
            path: path.clone(),
            migrations_prefix: String::new(),
        },
        Policy::default(),
    )
    .await;
    assert!(matches!(result, Err(Error::Unavailable)));
    if let Err(error) = result {
        for rendered in [error.to_string(), format!("{error:?}")] {
            assert!(!rendered.contains("secret"));
            assert!(!rendered.contains("SELECT"));
        }
    }
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let absent = directory.path().join("must-not-be-created.db");
    let result = database::open(
        Config::Sqlite {
            path: absent.clone(),
            migrations_prefix: directory
                .path()
                .join("missing_")
                .to_string_lossy()
                .into_owned(),
        },
        Policy::default(),
    )
    .await;
    assert!(result.is_err());
    assert!(!absent.exists());
}
