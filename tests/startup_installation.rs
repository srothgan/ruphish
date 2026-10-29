//! Persisted first-run settings, no-clobber publication and path semantics.
use super::*;
use serde_json::json;

fn locations(root: &Path) -> Locations {
    Locations {
        config: root.join("config/config.json"),
        data: root.join("data"),
        logs: root.join("logs"),
    }
}

#[test]
fn generated_defaults_are_private_sqlite_settings_and_existing_edits_are_preserved() {
    let directory = tempfile::tempdir().unwrap();
    let paths = locations(directory.path());
    let config = load_default(&paths).unwrap();
    assert_eq!(config.db_name, "sqlite3");
    assert_eq!(config.db_path.parent(), Some(paths.data.as_path()));
    assert!(!config.db_path.file_name().unwrap().is_empty());
    assert_eq!(config.logging.filename, paths.logs.join("ruphish.log"));
    assert_eq!(config.admin.key_path, paths.data.join("ruphish_admin.key"));
    assert!(
        !config.db_path.exists(),
        "configuration generation does not initialize storage"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&paths.config)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(&paths.data).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&paths.config).unwrap()).unwrap();
    value["db_path"] = json!("operator.sqlite3");
    value["logging"]["filename"] = json!("operator.log");
    value["admin_server"]["key_path"] = json!("operator.key");
    let edited = serde_json::to_vec(&value).unwrap();
    std::fs::write(&paths.config, &edited).unwrap();
    let config = load_default(&paths).unwrap();
    let base = paths.config.parent().unwrap();
    assert_eq!(config.db_path, base.join("operator.sqlite3"));
    assert_eq!(config.logging.filename, base.join("operator.log"));
    assert_eq!(config.admin.key_path, base.join("operator.key"));
    assert_eq!(std::fs::read(&paths.config).unwrap(), edited);
    let explicit = load(Some(&paths.config)).unwrap();
    assert_eq!(explicit.db_path, Path::new("operator.sqlite3"));
    assert_eq!(explicit.logging.filename, Path::new("operator.log"));
}

#[test]
fn malformed_existing_files_and_explicit_missing_paths_never_generate_replacements() {
    let directory = tempfile::tempdir().unwrap();
    let paths = locations(directory.path());
    std::fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
    std::fs::write(&paths.config, b"{partial operator content").unwrap();
    assert!(matches!(
        load_default(&paths),
        Err(ConfigError::Json { .. })
    ));
    assert_eq!(
        std::fs::read(&paths.config).unwrap(),
        b"{partial operator content"
    );
    assert!(!paths.data.exists());
    let explicit = directory.path().join("missing/config.json");
    assert!(load(Some(&explicit)).is_err());
    assert!(!explicit.parent().unwrap().exists());
}

#[test]
fn simultaneous_initialization_publishes_one_complete_configuration_and_ignores_orphan_temps() {
    let directory = tempfile::tempdir().unwrap();
    let paths = locations(directory.path());
    std::fs::create_dir_all(paths.config.parent().unwrap()).unwrap();
    // Model an interrupted write before atomic publication: only a temp file exists.
    std::fs::write(paths.config.with_file_name(".tmp-interrupted"), b"{partial").unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
    let workers: Vec<_> = (0..4)
        .map(|_| {
            let paths = paths.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                load_default(&paths).unwrap().db_path
            })
        })
        .collect();
    for worker in workers {
        let database = worker.join().unwrap();
        assert_eq!(database.parent(), Some(paths.data.as_path()));
    }
    config::load(&paths.config)
        .unwrap()
        .validate(Mode::All)
        .unwrap();
    let original = std::fs::read(&paths.config).unwrap();
    paths.initialize().unwrap();
    assert_eq!(std::fs::read(&paths.config).unwrap(), original);
    assert_eq!(
        std::fs::read(paths.config.with_file_name(".tmp-interrupted")).unwrap(),
        b"{partial"
    );
}

#[test]
fn creation_failure_does_not_publish_config_or_replace_obstructing_files() {
    let directory = tempfile::tempdir().unwrap();
    let paths = locations(directory.path());
    std::fs::write(&paths.data, b"operator file").unwrap();
    assert!(matches!(
        load_default(&paths),
        Err(ConfigError::Initialize(_))
    ));
    assert!(!paths.config.exists());
    assert_eq!(std::fs::read(&paths.data).unwrap(), b"operator file");
}

#[test]
fn server_database_configuration_remains_explicit_and_dsn_values_are_not_rebased() {
    let directory = tempfile::tempdir().unwrap();
    let paths = locations(directory.path());
    paths.initialize().unwrap();
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&paths.config).unwrap()).unwrap();
    value["db_name"] = json!("postgres");
    value["db_path"] = json!("");
    value["postgres"] = json!({"host":"localhost","database":"operator","username":"operator","password":"synthetic-only","ca_path":"database-ca.pem"});
    let bytes = serde_json::to_vec(&value).unwrap();
    std::fs::write(&paths.config, &bytes).unwrap();
    let config = load_default(&paths).unwrap();
    config.validate(Mode::All).unwrap();
    assert_eq!(
        config.postgres.as_ref().unwrap().ca_path.as_deref(),
        Some(
            paths
                .config
                .parent()
                .unwrap()
                .join("database-ca.pem")
                .as_path()
        )
    );
    assert!(matches!(
        config.database().unwrap(),
        ruphish_adapters::database::Config::Postgres(_)
    ));
    value["db_name"] = json!("mysql");
    value.as_object_mut().unwrap().remove("postgres");
    value["db_path"] = json!("operator:synthetic-only@tcp(localhost:3306)/operator");
    std::fs::write(&paths.config, serde_json::to_vec(&value).unwrap()).unwrap();
    let config = load_default(&paths).unwrap();
    assert_eq!(
        config.db_path,
        Path::new("operator:synthetic-only@tcp(localhost:3306)/operator")
    );
    value["db_name"] = json!("unsupported");
    std::fs::write(&paths.config, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(load_default(&paths).unwrap().validate(Mode::All).is_err());
    assert_eq!(std::fs::read_dir(&paths.data).unwrap().count(), 0);
}
