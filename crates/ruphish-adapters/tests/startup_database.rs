//! SQLite startup using committed WAL data, real transactions and migration configuration.
use ruphish_adapters::sqlite::{LATEST_LEGACY_VERSION, StartupDatabase};
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use std::path::Path;

#[tokio::test]
async fn startup_snapshot_includes_committed_wal_records_and_never_resets_queue_ownership() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("database.sqlite3");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/compatibility/20201201000000.sqlite3"),
        &path,
    )
    .unwrap();
    let mut writer = SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&path))
        .await
        .unwrap();
    sqlx::raw_sql("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; UPDATE groups SET name='Committed WAL preservation' WHERE id=23; INSERT INTO mail_logs(id,campaign_id,user_id,r_id,processing,send_attempt) VALUES(9001,53,17,'preserved-rid',1,3);").execute(&mut writer).await.unwrap();
    let database = StartupDatabase::open(&path, "").await.unwrap();
    let snapshot = database.backup_path().unwrap().to_owned();
    database.close().await;
    for file in [&path, &snapshot] {
        let mut connection =
            SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(file))
                .await
                .unwrap();
        assert_eq!(
            sqlx::query_scalar::<_, String>("SELECT name FROM groups WHERE id=23")
                .fetch_one(&mut connection)
                .await
                .unwrap(),
            "Committed WAL preservation"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM mail_logs WHERE processing != 1")
                .fetch_one(&mut connection)
                .await
                .unwrap(),
            0
        );
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT send_attempt FROM mail_logs WHERE id=9001 AND processing=1 AND r_id='preserved-rid'").fetch_one(&mut connection).await.unwrap(),3);
        let version: i64 = sqlx::query_scalar("SELECT max(version_id) FROM goose_db_version")
            .fetch_one(&mut connection)
            .await
            .unwrap();
        assert_eq!(
            version,
            if file == &path {
                LATEST_LEGACY_VERSION
            } else {
                20201201000000
            }
        );
        connection.close().await.unwrap();
    }
    writer.close().await.unwrap();
}

#[tokio::test]
async fn custom_migrations_must_match_the_verified_set_before_any_database_is_created() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("database.sqlite3");
    let prefix = directory.path().join("custom_");
    let migrations = directory.path().join("custom_sqlite3");
    std::fs::create_dir(&migrations).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations/gophish");
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), migrations.join(entry.file_name())).unwrap();
    }
    std::fs::write(
        migrations.join("99999999999999_custom.sql"),
        "-- unexpected custom migration",
    )
    .unwrap();
    assert!(
        StartupDatabase::open(&path, prefix.to_str().unwrap())
            .await
            .is_err()
    );
    assert!(!path.exists());
    std::fs::remove_file(migrations.join("99999999999999_custom.sql")).unwrap();
    let database = StartupDatabase::open(&path, prefix.to_str().unwrap())
        .await
        .unwrap();
    database.close().await;
    let entry = std::fs::read_dir(migrations)
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    std::fs::write(entry.path(), "changed SQL").unwrap();
    let original = std::fs::read(&path).unwrap();
    assert!(
        StartupDatabase::open(&path, prefix.to_str().unwrap())
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);
}
