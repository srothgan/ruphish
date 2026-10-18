//! Independent Go observations, including the original TestLoadConfig scenario.
use super::*;
use serde_json::{Value, json};

#[test]
fn unchanged_go_configuration_observations_preserve_merges_nulls_types_and_defaults() {
    let observations: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/startup/config.json")).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.json");
    for observation in observations {
        let body = observation["body"].as_str().unwrap();
        std::fs::write(&path, body).unwrap();
        let result = load(&path);
        assert_eq!(
            result.is_ok(),
            observation["accepted"].as_bool().unwrap(),
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
            assert_eq!(
                json!({"admin_server":admin,"phish_server":server(&config.phish),"db_name":config.db_name,"db_path":config.db_path,"db_sslca_path":config.db_sslca_path,"migrations_prefix":format!("{}{}", config.migrations_prefix, config.db_name),"test_flag":false,"contact_address":config.contact_address,"logging":{"filename":config.logging.filename,"level":config.logging.level}}),
                observation["expected"],
                "{body}"
            );
        }
    }
    assert!(load(&directory.path().join("missing")).is_err());
}
