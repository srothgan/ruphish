//! Provider checks use public capabilities; independent SQL injects schema faults.
use super::*;
use ruphish_application::health::Service;

#[tokio::test]
async fn sqlite_health_checks_application_storage_and_closed_pool() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");
    let db = database::open(Config::Sqlite { path: path.clone() }, Policy::default())
        .await
        .unwrap();
    let health = Service::new(db.health());
    health.started();
    assert!(health.ready().await);
    let options = sqlx::sqlite::SqliteConnectOptions::new().filename(&path);
    let mut inspect = sqlx::SqliteConnection::connect_with(&options)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE ruphish_settings RENAME TO unavailable_settings")
        .execute(&mut inspect)
        .await
        .unwrap();
    assert!(!health.ready().await);
    sqlx::query("ALTER TABLE unavailable_settings RENAME TO ruphish_settings")
        .execute(&mut inspect)
        .await
        .unwrap();
    assert!(health.ready().await);
    db.close().await;
    assert!(!health.ready().await);
}

#[tokio::test]
#[ignore = "Requires canonical MySQL fixture and RUPHISH_MYSQL_TEST_PORT"]
async fn mysql_health_checks_application_storage_and_closed_pool() {
    let (mut inspect, config) = fixture().await;
    let db = database::open(Config::MySql(config.clone()), Policy::default())
        .await
        .unwrap();
    let health = Service::new(db.health());
    health.started();
    assert!(health.ready().await);
    sqlx::query("RENAME TABLE ruphish_settings TO unavailable_settings")
        .execute(&mut inspect)
        .await
        .unwrap();
    assert!(!health.ready().await);
    sqlx::query("RENAME TABLE unavailable_settings TO ruphish_settings")
        .execute(&mut inspect)
        .await
        .unwrap();
    assert!(health.ready().await);
    db.close().await;
    assert!(!health.ready().await);
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "DROP DATABASE `{}`",
        config.database
    )))
    .execute(&mut inspect)
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "Requires canonical PostgreSQL fixture and RUPHISH_POSTGRES_TEST_PORT"]
async fn postgres_health_checks_application_storage_and_closed_pool() {
    let (mut admin, config) = postgres_fixture().await;
    let db = database::open(Config::Postgres(config.clone()), Policy::default())
        .await
        .unwrap();
    let health = Service::new(db.health());
    health.started();
    assert!(health.ready().await);
    let mut inspect = postgres_inspect(&config).await;
    sqlx::query("ALTER TABLE ruphish_settings RENAME TO unavailable_settings")
        .execute(&mut inspect)
        .await
        .unwrap();
    assert!(!health.ready().await);
    sqlx::query("ALTER TABLE unavailable_settings RENAME TO ruphish_settings")
        .execute(&mut inspect)
        .await
        .unwrap();
    assert!(health.ready().await);
    db.close().await;
    assert!(!health.ready().await);
    inspect.close().await.unwrap();
    postgres_cleanup(&mut admin, &config).await;
}
