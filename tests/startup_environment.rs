//! Configuration overlays without mutating the test process environment.
use super::*;
use crate::cli::Mode;

fn load(source: &[u8], variables: &[(&str, &str)]) -> Result<Config, ConfigError> {
    decode(
        source,
        variables
            .iter()
            .map(|(key, value)| (OsString::from(key), OsString::from(value))),
    )
}

const DEFAULT: &[u8] = include_bytes!("../docker/config.json");

#[test]
fn listeners_logging_and_policy_override_files_without_losing_other_fields() {
    let config = load(
        DEFAULT,
        &[
            ("ADMIN_SERVER_LISTEN_URL", "127.0.0.1:9443"),
            ("ADMIN_SERVER_USE_TLS", "true"),
            (
                "ADMIN_SERVER_TRUSTED_ORIGINS",
                "[\"https://admin.example.test\"]",
            ),
            ("LOGGING_LEVEL", "warn"),
            ("LOGGING_MAX_FILES", "7"),
            ("DOMAIN_VERIFICATION_TRUSTED_PROXIES", "[\"10.0.0.0/24\"]"),
            ("CONTACT_ADDRESS", "contact@example.test"),
            ("PATH", "ignored"),
        ],
    )
    .unwrap();
    config.validate(Mode::All).unwrap();
    assert!(config.admin.use_tls);
    assert_eq!(config.admin.listen_url, "127.0.0.1:9443");
    assert_eq!(
        config.admin.key_path.to_str(),
        Some("/var/lib/ruphish/admin.key")
    );
    assert_eq!(config.admin.trusted_origins, ["https://admin.example.test"]);
    assert_eq!(config.phish.listen_url, "0.0.0.0:8081");
    assert_eq!(config.logging.level, "warn");
    assert_eq!(config.logging.max_files, Some(7));
    assert_eq!(config.domain_verification.trusted_proxies.len(), 1);
    assert_eq!(config.contact_address, "contact@example.test");
}

#[test]
fn compose_selects_each_server_backend_without_an_inherited_sqlite_path() {
    for (backend, prefix) in [("postgres", "POSTGRES"), ("mysql", "MYSQL")] {
        let config = load(
            DEFAULT,
            &[
                ("DB_NAME", backend),
                (&format!("{prefix}_HOST"), "database"),
                (&format!("{prefix}_DATABASE"), "ruphish"),
                (&format!("{prefix}_USERNAME"), "123"),
                (&format!("{prefix}_PASSWORD"), "true"),
                (&format!("{prefix}_MAX_CONNECTIONS"), "12"),
            ],
        )
        .unwrap();
        config.validate(Mode::All).unwrap();
        assert!(config.db_path.as_os_str().is_empty());
        if let Some(settings) = config.mysql {
            assert_eq!(settings.host, "database");
            assert_eq!(settings.username, "123");
            assert_eq!(settings.password.as_deref(), Some("true"));
            assert_eq!(settings.max_connections, 12);
            assert!(matches!(
                settings.tls,
                ruphish_adapters::database::MySqlTls::VerifyIdentity
            ));
        } else {
            let settings = config.postgres.unwrap();
            assert_eq!(settings.host, "database");
            assert_eq!(settings.username, "123");
            assert_eq!(settings.password.as_deref(), Some("true"));
            assert_eq!(settings.max_connections, 12);
            assert!(matches!(
                settings.tls,
                ruphish_adapters::database::PostgresTls::VerifyIdentity
            ));
        }
    }
}

#[test]
fn typed_partial_overrides_preserve_file_settings_and_replace_the_password_source() {
    let source = br#"{"db_name":"postgres","postgres":{"host":"db.example.test","database":"existing","username":"operator","password_env":"OLD_SECRET","port":5433},"POSTGRES":null}"#;
    let config = load(
        source,
        &[
            ("POSTGRES_PASSWORD", "new-secret"),
            ("POSTGRES_PORT", "5434"),
        ],
    )
    .unwrap();
    config.database().unwrap();
    let pg = config.postgres.unwrap();
    assert_eq!(pg.host, "db.example.test");
    assert_eq!(pg.database, "existing");
    assert_eq!(pg.port, 5434);
    assert_eq!(pg.password.as_deref(), Some("new-secret"));
    assert!(pg.password_env.is_none());
    let config = load(source, &[("DB_NAME", "sqlite3"), ("DB_PATH", "fresh.db")]).unwrap();
    config.database().unwrap();
    assert!(config.postgres.is_none());
    assert_eq!(config.db_path.to_str(), Some("fresh.db"));
}

#[test]
fn invalid_and_ambiguous_environment_settings_fail_without_echoing_values() {
    for variables in [
        vec![("ADMIN_SERVER_USE_TLS", "private-invalid-value")],
        vec![("ADMIN_SERVER_USE_TLS", "null")],
        vec![("ADMIN_SERVER_USE_TLS", "\"private-invalid-value\"")],
        vec![("ADMIN_SERVER_TYPO", "private-invalid-value")],
        vec![("LOGGING_MAX_FILES", "true")],
        vec![("MYSQL_PORT", "-1")],
        vec![("DB_NAME", "postgres")],
        vec![("LOGGING_LEVEL", "info"), ("logging_level", "warn")],
    ] {
        let result = load(DEFAULT, &variables).and_then(|config| config.validate(Mode::All));
        let error = result.err().unwrap().to_string();
        assert!(!error.contains("private-invalid-value"));
    }
    let source = br#"{"db_name":"postgres","postgres":{"host":"db","database":"app","username":"user","password":"original"}}"#;
    let config = load(
        source,
        &[
            ("POSTGRES_PASSWORD", "one"),
            ("POSTGRES_PASSWORD_ENV", "OTHER"),
        ],
    )
    .unwrap();
    assert!(config.database().is_err());
}

#[test]
fn multiple_provider_environment_settings_are_rejected() {
    let variables = [
        ("DB_NAME", "postgres"),
        ("POSTGRES_HOST", "postgres"),
        ("POSTGRES_DATABASE", "app"),
        ("POSTGRES_USERNAME", "app"),
        ("POSTGRES_PASSWORD", "postgres-secret"),
        ("MYSQL_HOST", "mysql"),
        ("MYSQL_DATABASE", "app"),
        ("MYSQL_USERNAME", "app"),
        ("MYSQL_PASSWORD", "mysql-secret"),
    ];
    let error = load(DEFAULT, &variables)
        .unwrap()
        .database()
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("remove other database settings"));
    assert!(!error.contains("postgres-secret"));
    assert!(!error.contains("mysql-secret"));
    let config = load(DEFAULT, &variables[1..]).unwrap();
    assert!(
        config.database().is_err(),
        "SQLite cannot accept server database settings"
    );
}
