//! Real executable PostgreSQL fixture using a restricted application role.
use super::*;
use sqlx::Connection;
#[test]
#[ignore = "Requires canonical PostgreSQL fixture and RUPHISH_POSTGRES_TEST_PORT"]
fn postgres_process_full_production_workflow() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let port = std::env::var("RUPHISH_POSTGRES_TEST_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let ca = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/acceptance/data/postgres/ca.pem")
        .canonicalize()
        .unwrap();
    let options = sqlx::postgres::PgConnectOptions::new_without_pgpass()
        .host("127.0.0.1")
        .port(port)
        .database("postgres")
        .username("postgres")
        .password("ruphish-postgres-fixture-only")
        .ssl_mode(sqlx::postgres::PgSslMode::VerifyFull)
        .ssl_root_cert(&ca);
    let mut admin = runtime
        .block_on(sqlx::PgConnection::connect_with(&options))
        .unwrap();
    let name = format!(
        "rp_process_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    runtime
        .block_on(
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                "CREATE ROLE {name} LOGIN PASSWORD 'synthetic-process-password'"
            )))
            .execute(&mut admin),
        )
        .unwrap();
    runtime
        .block_on(
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                "CREATE DATABASE {name} OWNER {name}"
            )))
            .execute(&mut admin),
        )
        .unwrap();
    let mut inspector = runtime
        .block_on(sqlx::PgConnection::connect_with(
            &options
                .database(&name)
                .options([("search_path", "ruphish,pg_catalog")]),
        ))
        .unwrap();
    super::server_database_process::exercise(
        json!({"db_name":"postgres","db_path":"","postgres":{"host":"127.0.0.1","port":port,"database":name,"username":name,"password":"synthetic-process-password","ca_path":ca}}),
        |_, fault| {
            for statement in super::server_database_process::fault_statements("postgres", fault) {
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
        },
    );
    runtime.block_on(inspector.close()).unwrap();
    runtime
        .block_on(
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
                "DROP DATABASE {name} WITH (FORCE)"
            )))
            .execute(&mut admin),
        )
        .unwrap();
    runtime
        .block_on(
            sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DROP ROLE {name}"))).execute(&mut admin),
        )
        .unwrap();
}
