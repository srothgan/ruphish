//! Real MySQL metadata/transport tests. Independent connections only provision,
//! corrupt or inspect fixtures; the operation under test always uses the facade.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "Integration setup and assertions"
)]
use ruphish_adapters::database::{
    Error, MySqlConfig, MySqlTls,
    migrations::{MySqlSchema, inspect_mysql},
};
use sha2::{Digest, Sha256};
use sqlx::{
    Connection, MySqlConnection,
    mysql::{MySqlConnectOptions, MySqlSslMode},
};
use std::{path::PathBuf, time::Duration};

#[test]
fn mysql_configuration_and_legacy_dsn_reject_ambiguous_or_unsafe_inputs() {
    let config = MySqlConfig::from_legacy(
        "alice:se:cr@et@tcp([::1]:3307)/training?parseTime=true&loc=UTC&tls=true&timeout=3s",
        None,
    )
    .unwrap();
    assert_eq!(config.host, "::1");
    assert_eq!(config.port, 3307);
    assert_eq!(config.password.as_deref(), Some("se:cr@et"));
    assert_eq!(config.connect_timeout_seconds, 3);
    assert!(matches!(config.tls, MySqlTls::VerifyIdentity));
    for suffix in [
        "tls=skip-verify",
        "tls=preferred",
        "parseTime=false",
        "loc=Europe%2FBerlin",
        "timeout=0s",
        "timeout=999s",
        "tls=true&tls=false",
        "allowAllFiles=true",
        "readTimeout=5s",
    ] {
        let input = format!("alice:credential-canary@tcp(localhost:3306)/training?{suffix}");
        let error = MySqlConfig::from_legacy(&input, None).err().unwrap();
        assert!(!format!("{error:?} {error}").contains("credential-canary"));
    }
    let mut config = config;
    config.password_env = Some("RUPHISH_MYSQL_PASSWORD".into());
    assert!(config.validate().is_err());
    let source = r#"{"host":"localhost","database":"training","username":"alice","password":"canary","unknown":true}"#;
    assert!(serde_json::from_str::<MySqlConfig>(source).is_err());
}

struct PreflightFixture {
    inspector: MySqlConnection,
    options: MySqlConnectOptions,
    config: MySqlConfig,
    root: PathBuf,
    name: String,
    lock: String,
}

async fn fixture() -> PreflightFixture {
    let port: u16 = std::env::var("RUPHISH_MYSQL_TEST_PORT")
        .expect("Set RUPHISH_MYSQL_TEST_PORT from docker compose port mysql 3306")
        .parse()
        .unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ca = root.join("tests/acceptance/data/mysql/ca.pem");
    let options = MySqlConnectOptions::new()
        .host("127.0.0.1")
        .port(port)
        .username("root")
        .password("ruphish-mysql-fixture-only")
        .ssl_mode(MySqlSslMode::VerifyIdentity)
        .ssl_ca(&ca);
    let mut inspector = MySqlConnection::connect_with(&options).await.unwrap();
    let version: String = sqlx::query_scalar("SELECT VERSION()")
        .fetch_one(&mut inspector)
        .await
        .unwrap();
    println!("MySQL preflight server: {version}");
    let mut random = [0_u8; 12];
    getrandom::fill(&mut random).unwrap();
    let name = format!("ruphish_test_{}", hex::encode(random));
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "CREATE DATABASE `{name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_0900_bin"
    )))
    .execute(&mut inspector)
    .await
    .unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("USE `{name}`")))
        .execute(&mut inspector)
        .await
        .unwrap();
    let mut config = MySqlConfig::new(
        "127.0.0.1".into(),
        name.clone(),
        "root".into(),
        "ruphish-mysql-fixture-only".into(),
    );
    config.port = port;
    config.ca_path = Some(ca);
    config.lock_timeout_seconds = 1;
    let lock = format!(
        "ruphish:migrate:{}",
        &hex::encode(Sha256::digest(name.as_bytes()))[..40]
    );
    PreflightFixture {
        inspector,
        options,
        config,
        root,
        name,
        lock,
    }
}

async fn cleanup(mut inspector: MySqlConnection, name: &str) {
    // Only the random database created by this test is removed.
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DROP DATABASE `{name}`")))
        .execute(&mut inspector)
        .await
        .unwrap();
    inspector.close().await.unwrap();
}

#[tokio::test]
#[ignore = "Requires the canonical acceptance mysql service; run explicitly with RUPHISH_MYSQL_TEST_PORT"]
async fn mysql_preflight_transport_credentials_and_bounded_ownership() {
    let PreflightFixture {
        mut inspector,
        config,
        name,
        lock,
        ..
    } = fixture().await;
    assert_eq!(
        inspect_mysql(&config).await.unwrap().schema,
        MySqlSchema::Empty
    );
    let mut untrusted = config.clone();
    untrusted.ca_path = None;
    assert!(
        inspect_mysql(&untrusted).await.is_err(),
        "default verification must reject an untrusted CA"
    );
    let mut incorrect = config.clone();
    incorrect.password = Some("credential-canary-wrong".into());
    let error = inspect_mysql(&incorrect).await.unwrap_err();
    assert!(!format!("{error:?} {error}").contains("credential-canary"));
    let mut missing = config.clone();
    missing.database.push_str("_missing");
    assert!(
        inspect_mysql(&missing).await.is_err(),
        "must not provision a missing database"
    );
    let owned: i64 = sqlx::query_scalar("SELECT GET_LOCK(?,1)")
        .bind(&lock)
        .fetch_one(&mut inspector)
        .await
        .unwrap();
    assert_eq!(owned, 1);
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(5), inspect_mysql(&config))
            .await
            .unwrap(),
        Err(Error::Busy)
    ));
    sqlx::query("SELECT RELEASE_LOCK(?)")
        .bind(&lock)
        .execute(&mut inspector)
        .await
        .unwrap();
    assert_eq!(
        inspect_mysql(&config).await.unwrap().schema,
        MySqlSchema::Empty
    );
    cleanup(inspector, &name).await;
}

#[tokio::test]
#[ignore = "Requires the canonical acceptance mysql service; run explicitly with RUPHISH_MYSQL_TEST_PORT"]
async fn mysql_preflight_all_legacy_versions_cancellation_and_mutated_histories() {
    let PreflightFixture {
        mut inspector,
        options,
        config,
        root,
        name,
        lock,
    } = fixture().await;
    sqlx::raw_sql("CREATE TABLE goose_db_version(id BIGINT AUTO_INCREMENT PRIMARY KEY,version_id BIGINT NOT NULL,is_applied BOOLEAN NOT NULL,tstamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP); INSERT INTO goose_db_version(version_id,is_applied) VALUES(0,1);").execute(&mut inspector).await.unwrap();
    let fixtures = root.join("tests/fixtures/mysql-legacy");
    let mut scripts: Vec<_> = std::fs::read_dir(&fixtures)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "sql"))
        .collect();
    scripts.sort();
    for script in &scripts {
        let file = script.file_name().unwrap().to_str().unwrap();
        let version: i64 = file.split('_').next().unwrap().parse().unwrap();
        let source = std::fs::read_to_string(script).unwrap();
        let up = source
            .split("-- +goose Up")
            .nth(1)
            .unwrap()
            .split("-- +goose Down")
            .next()
            .unwrap();
        sqlx::raw_sql(sqlx::AssertSqlSafe(up))
            .execute(&mut inspector)
            .await
            .unwrap_or_else(|e| panic!("fixture {file}: {e}"));
        sqlx::query("INSERT INTO goose_db_version(version_id,is_applied) VALUES(?,1)")
            .bind(version)
            .execute(&mut inspector)
            .await
            .unwrap();
        let result = inspect_mysql(&config)
            .await
            .unwrap_or_else(|e| panic!("recognize {file}: {e}"));
        assert_eq!(
            result.schema,
            MySqlSchema::Gophish {
                version,
                requires_legacy_timezone: version < 20170828220440
            }
        );
    }
    // Data canaries verify that assessment never performs the destructive/privacy
    // transformations or rewrites passwords. Migration will be a separate operation.
    sqlx::raw_sql("INSERT INTO users(id,username,hash,api_key,role_id) VALUES(17,'fixture-admin','synthetic-bcrypt-preservation-canary','synthetic-api-key',1); INSERT INTO `groups`(id,user_id,name) VALUES(23,17,'shared group'); INSERT INTO targets(id,email) VALUES(31,'fixture@example.invalid'); INSERT INTO group_targets VALUES(23,31);").execute(&mut inspector).await.unwrap();
    assert!(inspect_mysql(&config).await.is_ok());
    // Stop a real assessment while it owns the migration lock but waits on a
    // table lock. Dropping the task must not strand ownership on a pooled session.
    sqlx::raw_sql("LOCK TABLES goose_db_version WRITE")
        .execute(&mut inspector)
        .await
        .unwrap();
    let mut observer = MySqlConnection::connect_with(&options).await.unwrap();
    let cancelled_config = config.clone();
    let cancelled = tokio::spawn(async move { inspect_mysql(&cancelled_config).await });
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let owner: Option<u64> = sqlx::query_scalar("SELECT IS_USED_LOCK(?)")
                .bind(&lock)
                .fetch_one(&mut observer)
                .await
                .unwrap();
            if owner.is_some() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    cancelled.abort();
    assert!(cancelled.await.unwrap_err().is_cancelled());
    sqlx::raw_sql("UNLOCK TABLES")
        .execute(&mut inspector)
        .await
        .unwrap();
    observer.close().await.unwrap();
    assert!(inspect_mysql(&config).await.is_ok());
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT hash FROM users WHERE id=17")
            .fetch_one(&mut inspector)
            .await
            .unwrap(),
        "synthetic-bcrypt-preservation-canary"
    );
    sqlx::raw_sql("ALTER TABLE users ADD unexpected INT")
        .execute(&mut inspector)
        .await
        .unwrap();
    assert!(matches!(
        inspect_mysql(&config).await,
        Err(Error::UnsupportedSchema)
    ));
    sqlx::raw_sql("ALTER TABLE users DROP unexpected; INSERT INTO goose_db_version(version_id,is_applied) VALUES(99999999999999,1)").execute(&mut inspector).await.unwrap();
    assert!(matches!(
        inspect_mysql(&config).await,
        Err(Error::UnsupportedSchema)
    ));
    sqlx::raw_sql("DELETE FROM goose_db_version WHERE version_id=99999999999999; UPDATE goose_db_version SET is_applied=0 WHERE version_id=20220321133237").execute(&mut inspector).await.unwrap();
    assert!(matches!(
        inspect_mysql(&config).await,
        Err(Error::UnsupportedSchema)
    ));
    sqlx::raw_sql("UPDATE goose_db_version SET is_applied=1 WHERE version_id=20220321133237; DELETE FROM goose_db_version WHERE version_id=20161202153627").execute(&mut inspector).await.unwrap();
    assert!(matches!(
        inspect_mysql(&config).await,
        Err(Error::UnsupportedSchema)
    ));
    cleanup(inspector, &name).await;
}
