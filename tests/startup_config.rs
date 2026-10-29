//! Independent Go observations, including the original TestLoadConfig scenario.
use super::*;
use serde_json::{Value, json};

#[test]
fn go_configuration_observations_preserve_merges_except_rejected_unknown_fields() {
    let observations: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/startup/config.json")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.json");
    for observation in observations {
        let body = observation["body"].as_str().unwrap();
        std::fs::write(&path, body).unwrap();
        let result = load(&path);
        // Deliberate deviations: reject unknown fields and the removed migration prefix.
        let unknown = body.contains("migrations_prefix")
            || body.contains("unknown")
            || body.contains("csrf_key\":12");
        assert_eq!(
            result.is_ok(),
            observation["accepted"].as_bool().unwrap() && !unknown,
            "{body}"
        );
        if let Ok(config) = result {
            let server = |s: &Server| json!({"listen_url":s.listen_url,"use_tls":s.use_tls,"cert_path":s.cert_path,"key_path":s.key_path});
            let mut admin = server(&config.admin);
            admin["csrf_key"] = json!(config.admin.csrf_key);
            // nil Go slices serialize as null; both have identical policy behavior.
            for (field, values) in [
                (
                    "allowed_internal_hosts",
                    &config.admin.allowed_internal_hosts,
                ),
                ("trusted_origins", &config.admin.trusted_origins),
            ] {
                let expected = &observation["expected"]["admin_server"][field];
                admin[field] = if values.is_empty() && expected.is_null() {
                    Value::Null
                } else {
                    json!(values)
                };
            }
            let mut expected = observation["expected"].clone();
            expected
                .as_object_mut()
                .unwrap()
                .remove("migrations_prefix");
            assert_eq!(
                json!({"admin_server":admin,"phish_server":server(&config.phish),"db_name":config.db_name,"db_path":config.db_path,"db_sslca_path":config.db_sslca_path,"test_flag":false,"contact_address":config.contact_address,"logging":{"filename":config.logging.filename,"level":config.logging.level}}),
                expected,
                "{body}"
            );
        }
    }
    assert!(load(&directory.path().join("missing")).is_err());
}

#[test]
fn selected_listeners_require_explicit_addresses_and_typos_are_rejected() {
    for body in [
        r#"{"admin_sever":{}}"#,
        r#"{"admin_server":{"listen_ur":"127.0.0.1:0"}}"#,
        r#"{"admin_server":{"use_tsl":true}}"#,
        r#"{"logging":{"levle":"info"}}"#,
    ] {
        assert!(merge(&mut Config::default(), body.as_bytes()).is_err());
    }
    let mut config = Config {
        db_name: "sqlite3".into(),
        db_path: "unused.sqlite3".into(),
        ..Default::default()
    };
    assert!(config.validate(Mode::Admin).is_err());
    config.admin.listen_url = "127.0.0.1:0".into();
    assert!(config.validate(Mode::Admin).is_ok());
    assert!(config.validate(Mode::All).is_err());
    config.phish.listen_url = "127.0.0.1:0".into();
    assert!(config.validate(Mode::All).is_ok());
}

#[test]
fn rotation_settings_validate_and_legacy_null_merge_semantics_remain_intact() {
    for body in [
        r#"{"logging":{"max_size_bytes":-1}}"#,
        r#"{"logging":{"max_files":"five"}}"#,
        r#"{"logging":{"max_size_bytes":1.5}}"#,
    ] {
        assert!(decode(body.as_bytes()).is_err());
    }
    let mut config = decode(include_bytes!("../config.example.json")).unwrap();
    assert_eq!(config.logging.rotation().max_size_bytes, 10 * 1024 * 1024);
    assert_eq!(config.logging.rotation().max_files, 5);
    for field in ["max_size_bytes", "max_files"] {
        merge(
            &mut config,
            format!(r#"{{"logging":{{"{field}":0}}}}"#).as_bytes(),
        )
        .unwrap();
        assert!(config.validate(Mode::All).is_err());
        config = decode(include_bytes!("../config.example.json")).unwrap();
    }
    merge(
        &mut config,
        br#"{"logging":{"max_size_bytes":2048,"MAX_FILES":2},"logging":{"max_size_bytes":null}}"#,
    )
    .unwrap();
    assert_eq!(config.logging.rotation().max_size_bytes, 2048);
    assert_eq!(config.logging.rotation().max_files, 2);
    merge(&mut config, br#"{"logging":null}"#).unwrap();
    assert!(config.logging.filename.as_os_str().is_empty());
    assert_eq!(config.logging.rotation().max_files, 5);
}
