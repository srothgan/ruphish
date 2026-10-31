//! Upgrade actual Goose-created databases and test preservation and failure boundaries.

use ruphish_adapters::{database::migrations, password_worker as password};
use sqlx::{Connection, Row, SqliteConnection, sqlite::SqliteConnectOptions};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}
async fn open(path: &Path) -> Result<SqliteConnection, sqlx::Error> {
    SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(path)).await
}

#[tokio::test]
async fn legacy_webhooks_choose_an_administrator_and_reject_missing_ownership() {
    let directory = tempfile::tempdir().unwrap();
    for missing_admin in [false, true] {
        let source = directory.path().join(format!("source-{missing_admin}.db"));
        let destination = directory.path().join(format!("output-{missing_admin}.db"));
        fs::copy(
            fixtures().join("compatibility/20220321133237.sqlite3"),
            &source,
        )
        .unwrap();
        let mut connection = open(&source).await.unwrap();
        sqlx::raw_sql("INSERT INTO users(id,username,hash,api_key,role_id) VALUES(5,'lower-normal','hash','normal-key',2),(31,'second-admin','hash','admin-key',1); DELETE FROM webhooks; INSERT INTO webhooks(id,name,url,secret,is_active) VALUES(71,'Imported','https://receiver.example.test/hook','preserved-secret',1);").execute(&mut connection).await.unwrap();
        if missing_admin {
            sqlx::raw_sql("DELETE FROM role_permissions WHERE permission_id=(SELECT id FROM permissions WHERE slug='modify_system')").execute(&mut connection).await.unwrap();
        }
        connection.close().await.unwrap();
        let original = fs::read(&source).unwrap();
        let outcome = migrations::upgrade_copy(&source, &destination).await;
        assert_eq!(fs::read(&source).unwrap(), original);
        if missing_admin {
            assert!(outcome.is_err());
            assert!(!destination.exists());
        } else {
            outcome.unwrap();
            let mut connection = open(&destination).await.unwrap();
            assert_eq!(
                sqlx::query_scalar::<_, i64>(
                    "SELECT owner_id FROM ruphish_webhook_state WHERE webhook_id=71"
                )
                .fetch_one(&mut connection)
                .await
                .unwrap(),
                17
            );
            assert_eq!(
                sqlx::query_scalar::<_, String>(
                    "SELECT secret FROM webhooks WHERE id=71 AND is_active=1"
                )
                .fetch_one(&mut connection)
                .await
                .unwrap(),
                "preserved-secret"
            );
            connection.close().await.unwrap();
        }
    }
}

#[tokio::test]
async fn all_populated_goose_versions_upgrade_without_losing_records_or_the_source() {
    let versions: Vec<i64> = serde_json::from_str(
        &fs::read_to_string(fixtures().join("compatibility/versions.json")).unwrap(),
    )
    .unwrap();
    let directory = tempfile::tempdir().unwrap();
    let fresh = directory.path().join("fresh.db");
    migrations::create_baseline(&fresh).await.unwrap();
    let mut fresh_connection = open(&fresh).await.unwrap();
    let expected_schema = schema(&mut fresh_connection).await.unwrap();
    fresh_connection.close().await.unwrap();
    for version in versions {
        let source = fixtures().join(format!("compatibility/{version}.sqlite3"));
        let original_bytes = fs::read(&source).unwrap();
        let destination = directory.path().join(format!("{version}.db"));
        migrations::upgrade_copy(&source, &destination)
            .await
            .unwrap_or_else(|error| panic!("{version}: {error}"));
        assert_eq!(
            fs::read(&source).unwrap(),
            original_bytes,
            "original {version} changed"
        );
        let mut connection = open(&destination).await.unwrap();
        assert_eq!(
            schema(&mut connection).await.unwrap(),
            expected_schema,
            "schema at {version}"
        );
        let track_opens: bool = sqlx::query_scalar(
            "SELECT track_opens FROM ruphish_campaign_policy WHERE campaign_id=53",
        )
        .fetch_one(&mut connection)
        .await
        .unwrap();
        assert!(
            track_opens,
            "migrated {version} defaults to enabled even without a tracker in the stored HTML"
        );
        let collected: i64 =
            sqlx::query_scalar("SELECT count(*) FROM events WHERE client IS NOT NULL")
                .fetch_one(&mut connection)
                .await
                .unwrap();
        assert_eq!(
            collected, 0,
            "historical {version} must not invent client observations"
        );
        let user = sqlx::query("SELECT username,hash,api_key,role_id FROM users WHERE id=17")
            .fetch_one(&mut connection)
            .await
            .unwrap();
        assert_eq!(user.get::<String, _>("username"), "historical-admin");
        assert_eq!(user.get::<String, _>("api_key"), "historical-key");
        assert!(password::verify(
            "FixtureOnlyPassword42!",
            &user.get::<String, _>("hash")
        ));
        let role: String = sqlx::query_scalar("SELECT slug FROM roles WHERE id=?")
            .bind(user.get::<i64, _>("role_id"))
            .fetch_one(&mut connection)
            .await
            .unwrap();
        assert_eq!(role, "admin");
        for (query, expected) in [
            (
                "SELECT email FROM targets WHERE id=31",
                "history@example.test",
            ),
            (
                "SELECT name FROM groups WHERE id=23 AND user_id=17",
                "Preserved group",
            ),
            (
                "SELECT content FROM attachments WHERE id=43 AND template_id=41",
                "AAEC/w==",
            ),
            (
                "SELECT html FROM templates WHERE id=41 AND user_id=17",
                "<table><tr><td>{{.FirstName}}</td></tr></table>",
            ),
            (
                "SELECT name FROM campaigns WHERE id=53 AND template_id=41 AND page_id=47 AND user_id=17",
                "Preserved campaign",
            ),
            (
                "SELECT r_id FROM results WHERE id=59 AND campaign_id=53 AND user_id=17",
                "preserved-rid",
            ),
            (
                "SELECT message FROM event_history WHERE id=61 AND campaign_id=53",
                "Email Sent",
            ),
        ] {
            assert_eq!(
                sqlx::query_scalar::<_, String>(query)
                    .fetch_one(&mut connection)
                    .await
                    .unwrap(),
                expected,
                "version {version}"
            );
        }
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT target_id FROM group_targets WHERE group_id=23")
                .fetch_one(&mut connection)
                .await
                .unwrap(),
            31
        );
        if version >= 20160227180335 {
            assert_eq!(
                sqlx::query_scalar::<_, String>(
                    "SELECT password FROM smtp WHERE id=67 AND user_id=17"
                )
                .fetch_one(&mut connection)
                .await
                .unwrap(),
                "synthetic-smtp-secret"
            );
        }
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT max(version) FROM _sqlx_migrations WHERE success=1"
            )
            .fetch_one(&mut connection)
            .await
            .unwrap(),
            1
        );
        connection.close().await.unwrap();
    }
}

#[tokio::test]
async fn latest_real_gophish_database_is_preserved_and_repeated_upgrades_are_idempotent() {
    let directory = tempfile::tempdir().unwrap();
    let original = fixtures().join("gophish-v0.12.1/legacy-latest.sqlite3");
    let first = directory.path().join("first.db");
    let second = directory.path().join("second.db");
    migrations::upgrade_copy(&original, &first).await.unwrap();
    migrations::upgrade_copy(&first, &second).await.unwrap();
    for path in [&original, &first, &second] {
        let mut connection = open(path).await.unwrap();
        let hash: String = sqlx::query_scalar("SELECT hash FROM users WHERE id=1")
            .fetch_one(&mut connection)
            .await
            .unwrap();
        assert!(password::verify("FixtureOnlyPassword42!", &hash));
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT email FROM targets")
                .fetch_one(&mut connection)
                .await
                .unwrap(),
            "ada@example.test"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(if path == &original {
                "SELECT count(*) FROM goose_db_version"
            } else {
                "SELECT count(*) FROM _sqlx_migrations WHERE version=1 AND success=1"
            })
            .fetch_one(&mut connection)
            .await
            .unwrap(),
            if path == &original { 26 } else { 1 }
        );
        connection.close().await.unwrap();
    }
}

#[tokio::test]
async fn fresh_schema_has_no_implicit_accounts_and_never_overwrites_existing_files() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("fresh.db");
    migrations::create_baseline(&path).await.unwrap();
    let before = fs::read(&path).unwrap();
    assert!(migrations::create_baseline(&path).await.is_err());
    assert!(migrations::upgrade_copy(&path, &path).await.is_err());
    assert_eq!(fs::read(&path).unwrap(), before);
    let mut connection = open(&path).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM users")
            .fetch_one(&mut connection)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
            .fetch_one(&mut connection)
            .await
            .unwrap(),
        "ok"
    );
    connection.close().await.unwrap();
}

#[tokio::test]
async fn bad_history_schema_and_migration_failures_never_publish_or_modify_the_source() {
    let directory = tempfile::tempdir().unwrap();
    let input = fixtures().join("compatibility/20160227180335.sqlite3");
    for (index,mutation) in [
        "INSERT INTO goose_db_version(version_id,is_applied) VALUES(99999999999999,1)",
        "DELETE FROM goose_db_version WHERE version_id=20160131153104",
        "ALTER TABLE templates DROP COLUMN subject",
        "CREATE TRIGGER fail_upgrade BEFORE INSERT ON goose_db_version BEGIN SELECT RAISE(ABORT,'injected migration failure'); END",
        "UPDATE goose_db_version SET is_applied=2 WHERE version_id=20160118194630",
    ].iter().enumerate() {
        let source=directory.path().join(format!("bad-{index}.db")); let destination=directory.path().join(format!("out-{index}.db"));
        fs::copy(&input,&source).unwrap();
        let mut connection=open(&source).await.unwrap();
        sqlx::raw_sql(*mutation).execute(&mut connection).await.unwrap(); connection.close().await.unwrap();
        let before=fs::read(&source).unwrap();
        let files_before: std::collections::BTreeSet<_> = fs::read_dir(directory.path()).unwrap().map(|entry| entry.unwrap().file_name()).collect();
        assert!(migrations::upgrade_copy(&source,&destination).await.is_err(),"mutation {mutation}");
        assert!(!destination.exists()); assert_eq!(fs::read(&source).unwrap(),before);
        let files_after: std::collections::BTreeSet<_> = fs::read_dir(directory.path()).unwrap().map(|entry| entry.unwrap().file_name()).collect();
        assert_eq!(files_after, files_before, "failed upgrade leaked a temporary database");
    }
    let corrupt = directory.path().join("corrupt.db");
    fs::write(&corrupt, b"not sqlite").unwrap();
    let output = directory.path().join("corrupt-output.db");
    assert!(migrations::upgrade_copy(&corrupt, &output).await.is_err());
    assert!(!output.exists());
    assert!(
        migrations::upgrade_copy(&directory.path().join("missing.db"), &output)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn committed_wal_records_are_included_in_the_upgrade_snapshot() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("wal.db");
    let destination = directory.path().join("snapshot.db");
    fs::copy(
        fixtures().join("gophish-v0.12.1/legacy-latest.sqlite3"),
        &source,
    )
    .unwrap();
    let mut writer = open(&source).await.unwrap();
    sqlx::raw_sql("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; UPDATE groups SET name='Committed in WAL' WHERE id=1;").execute(&mut writer).await.unwrap();
    migrations::upgrade_copy(&source, &destination)
        .await
        .unwrap();
    let mut snapshot = open(&destination).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT name FROM groups WHERE id=1")
            .fetch_one(&mut snapshot)
            .await
            .unwrap(),
        "Committed in WAL"
    );
    snapshot.close().await.unwrap();
    writer.close().await.unwrap();
}

#[tokio::test]
async fn old_smtp_profiles_are_converted_with_their_campaign_and_owner() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("old.db");
    let destination = directory.path().join("new.db");
    fs::copy(
        fixtures().join("compatibility/20160118194630.sqlite3"),
        &source,
    )
    .unwrap();
    let mut connection = open(&source).await.unwrap();
    sqlx::query("INSERT INTO smtp(campaign_id,host,username,from_address) VALUES(53,'localhost:2525','preserved-user','sender@example.test')").execute(&mut connection).await.unwrap();
    connection.close().await.unwrap();
    let before = fs::read(&source).unwrap();
    migrations::upgrade_copy(&source, &destination)
        .await
        .unwrap();
    assert_eq!(fs::read(&source).unwrap(), before);
    let mut upgraded = open(&destination).await.unwrap();
    let profile=sqlx::query("SELECT s.host,s.username,s.from_address,s.user_id FROM smtp AS s JOIN campaigns AS c ON c.smtp_id=s.id WHERE c.id=53").fetch_one(&mut upgraded).await.unwrap();
    assert_eq!(profile.get::<String, _>("host"), "localhost:2525");
    assert_eq!(profile.get::<String, _>("username"), "preserved-user");
    assert_eq!(
        profile.get::<String, _>("from_address"),
        "sender@example.test"
    );
    assert_eq!(profile.get::<i64, _>("user_id"), 17);
    upgraded.close().await.unwrap();
    let ambiguous = directory.path().join("ambiguous.db");
    fs::copy(&source, &ambiguous).unwrap();
    let mut duplicate = open(&ambiguous).await.unwrap();
    sqlx::query("INSERT INTO smtp(campaign_id,host) VALUES(53,'second-profile:25')")
        .execute(&mut duplicate)
        .await
        .unwrap();
    duplicate.close().await.unwrap();
    let bytes = fs::read(&ambiguous).unwrap();
    let ambiguous_output = directory.path().join("ambiguous-output.db");
    assert!(
        migrations::upgrade_copy(&ambiguous, &ambiguous_output)
            .await
            .is_err()
    );
    assert!(!ambiguous_output.exists());
    assert_eq!(fs::read(&ambiguous).unwrap(), bytes);
    let mut original = open(&source).await.unwrap();
    sqlx::raw_sql("INSERT INTO smtp(campaign_id,host) VALUES(999,'orphan:25')")
        .execute(&mut original)
        .await
        .unwrap();
    original.close().await.unwrap();
    assert!(
        migrations::upgrade_copy(&source, &directory.path().join("orphan-output.db"))
            .await
            .is_err()
    );
}

async fn schema(
    connection: &mut SqliteConnection,
) -> Result<Vec<(String, String, String)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT type,name,sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY type,name",
    )
    .fetch_all(connection)
    .await
}

#[tokio::test]
async fn conversion_preserves_deleted_identity_high_water_marks() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.db");
    let destination = directory.path().join("converted.db");
    fs::copy(
        fixtures().join("gophish-v0.12.1/legacy-latest.sqlite3"),
        &source,
    )
    .unwrap();
    let mut connection = open(&source).await.unwrap();
    sqlx::raw_sql("INSERT INTO groups(id,name) VALUES(9000,'deleted'); DELETE FROM groups WHERE id=9000; DELETE FROM attachments; INSERT INTO attachments(id,name) VALUES(8000,'deleted'); DELETE FROM attachments;").execute(&mut connection).await.unwrap();
    connection.close().await.unwrap();
    migrations::upgrade_copy(&source, &destination)
        .await
        .unwrap();
    let mut converted = open(&destination).await.unwrap();
    for (query, retired) in [
        ("INSERT INTO groups(name) VALUES('new') RETURNING id", 9000),
        (
            "INSERT INTO attachments(name) VALUES('new') RETURNING id",
            8000,
        ),
    ] {
        let id: i64 = sqlx::query_scalar(query)
            .fetch_one(&mut converted)
            .await
            .unwrap();
        assert!(id > retired, "reused retired identity {id}");
    }
    converted.close().await.unwrap();
}

#[tokio::test]
async fn in_place_conversion_failure_rolls_back_and_retains_a_verified_backup() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("source.db");
    fs::copy(
        fixtures().join("gophish-v0.12.1/legacy-latest.sqlite3"),
        &source,
    )
    .unwrap();
    let mut connection = open(&source).await.unwrap();
    sqlx::raw_sql("INSERT INTO imap(user_id,host,port,username,password) VALUES(NULL,'mail.example.test',993,'synthetic','fixture-only');")
        .execute(&mut connection)
        .await
        .unwrap();
    let original_schema = schema(&mut connection).await.unwrap();
    connection.close().await.unwrap();
    let result = ruphish_adapters::database::open(
        ruphish_adapters::database::Config::Sqlite {
            path: source.clone(),
        },
        ruphish_adapters::outbound::Policy::new(&[]).unwrap(),
    )
    .await;
    assert!(result.is_err());
    let backups: Vec<_> = fs::read_dir(directory.path())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("ruphish-pre-upgrade-")
        })
        .collect();
    assert_eq!(backups.len(), 1);
    for path in [&source, &backups[0]] {
        let mut connection = open(path).await.unwrap();
        assert_eq!(schema(&mut connection).await.unwrap(), original_schema);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM imap WHERE user_id IS NULL")
                .fetch_one(&mut connection)
                .await
                .unwrap(),
            1
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
                .fetch_one(&mut connection)
                .await
                .unwrap(),
            "ok"
        );
        connection.close().await.unwrap();
    }
}

#[tokio::test]
async fn application_history_and_schema_drift_are_rejected_without_blessing_them() {
    let directory = tempfile::tempdir().unwrap();
    for (index, mutation) in [
        "UPDATE _sqlx_migrations SET checksum=x'00'",
        "UPDATE _sqlx_migrations SET success=0",
        "UPDATE _sqlx_migrations SET version=99",
        "DELETE FROM _sqlx_migrations",
        "DROP INDEX ruphish_campaign_domain",
        "DROP TRIGGER ruphish_event_insert",
        "ALTER TABLE users ADD COLUMN unexpected TEXT",
    ]
    .iter()
    .enumerate()
    {
        let source = directory.path().join(format!("current-{index}.db"));
        let output = directory.path().join(format!("output-{index}.db"));
        migrations::create_baseline(&source).await.unwrap();
        let mut connection = open(&source).await.unwrap();
        sqlx::raw_sql(*mutation)
            .execute(&mut connection)
            .await
            .unwrap();
        connection.close().await.unwrap();
        let before = fs::read(&source).unwrap();
        assert!(
            migrations::upgrade_copy(&source, &output).await.is_err(),
            "{mutation}"
        );
        assert!(!output.exists());
        assert_eq!(fs::read(&source).unwrap(), before);
    }
}
