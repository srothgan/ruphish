//! MySQL executable fixture; workflows are shared across server providers.
use super::*;
use sqlx::Connection;
#[test]
#[ignore = "Requires the canonical MySQL acceptance service and RUPHISH_MYSQL_TEST_PORT"]
fn mysql_process_full_production_workflow() {
    with_database(false);
}

#[test]
#[ignore = "Requires the canonical MySQL acceptance service and RUPHISH_MYSQL_TEST_PORT"]
fn mysql_health_process_recovers_after_database_outage() {
    with_database(true);
}

fn with_database(health_only: bool) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let port: u16 = std::env::var("RUPHISH_MYSQL_TEST_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let ca = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/acceptance/data/mysql/ca.pem")
        .canonicalize()
        .unwrap();
    let options = sqlx::mysql::MySqlConnectOptions::new()
        .host("127.0.0.1")
        .port(port)
        .username("root")
        .password("ruphish-mysql-fixture-only")
        .ssl_mode(sqlx::mysql::MySqlSslMode::VerifyIdentity)
        .ssl_ca(&ca);
    let mut inspector = runtime
        .block_on(sqlx::MySqlConnection::connect_with(&options))
        .unwrap();
    let name = format!(
        "ruphish_process_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    runtime.block_on(sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE DATABASE `{name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_0900_bin; USE `{name}`"))).execute(&mut inspector)).unwrap();
    let config = json!({"db_name":"mysql","db_path":"","mysql":{"host":"127.0.0.1","port":port,"database":name,"username":"root","password":"ruphish-mysql-fixture-only","ca_path":ca}});
    if health_only {
        super::health_process::server_outage(config);
    } else {
        super::server_database_process::exercise(config, |_, fault| {
            for statement in super::server_database_process::fault_statements("mysql", fault) {
                if matches!(
                    fault,
                    super::server_database_process::Fault::AssertEmptyLaunch
                ) {
                    assert_eq!(
                        runtime
                            .block_on(
                                sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(statement))
                                    .fetch_one(&mut inspector)
                            )
                            .unwrap(),
                        0
                    );
                } else {
                    runtime
                        .block_on(
                            sqlx::raw_sql(sqlx::AssertSqlSafe(statement)).execute(&mut inspector),
                        )
                        .unwrap();
                }
            }
        });
    }
    runtime
        .block_on(
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DROP DATABASE `{name}`")))
                .execute(&mut inspector),
        )
        .unwrap();
}
