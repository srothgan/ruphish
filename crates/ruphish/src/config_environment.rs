//! Environment overrides reuse the JSON decoder and its final validation.
use super::{Apply, Config, ConfigError, RawValue, merge};
use serde_json::{Map, Value};
use std::{collections::BTreeSet, ffi::OsString};

enum Kind {
    String,
    Json,
}

fn field(name: &str) -> Result<Option<(&str, String, Kind)>, ConfigError> {
    let invalid = || {
        ConfigError::Invalid(format!(
            "Unknown configuration environment variable: {name}"
        ))
    };
    for (prefix, group, strings, json) in [
        (
            "ADMIN_SERVER_",
            "admin_server",
            &["listen_url", "cert_path", "key_path", "csrf_key"][..],
            &["use_tls", "trusted_origins", "allowed_internal_hosts"][..],
        ),
        (
            "PHISH_SERVER_",
            "phish_server",
            &["listen_url", "cert_path", "key_path"][..],
            &["use_tls"][..],
        ),
        (
            "LOGGING_",
            "logging",
            &["filename", "level"][..],
            &["max_size_bytes", "max_files"][..],
        ),
        (
            "DOMAIN_VERIFICATION_",
            "domain_verification",
            &["trusted_ca_path"][..],
            &["resolvers", "allowed_networks", "trusted_proxies"][..],
        ),
        (
            "MYSQL_",
            "mysql",
            &[
                "host",
                "database",
                "username",
                "password",
                "password_env",
                "tls",
                "ca_path",
                "legacy_timezone",
            ][..],
            &[
                "port",
                "max_connections",
                "connect_timeout_seconds",
                "acquire_timeout_seconds",
                "lock_timeout_seconds",
            ][..],
        ),
        (
            "POSTGRES_",
            "postgres",
            &[
                "host",
                "database",
                "username",
                "password",
                "password_env",
                "tls",
                "ca_path",
            ][..],
            &[
                "port",
                "max_connections",
                "connect_timeout_seconds",
                "acquire_timeout_seconds",
                "lock_timeout_seconds",
                "statement_timeout_seconds",
            ][..],
        ),
    ] {
        if let Some(suffix) = name.strip_prefix(prefix) {
            let key = suffix.to_ascii_lowercase();
            let kind = if strings.contains(&key.as_str()) {
                Kind::String
            } else if json.contains(&key.as_str()) {
                Kind::Json
            } else {
                return Err(invalid());
            };
            return Ok(Some((group, key, kind)));
        }
    }
    if matches!(
        name,
        "DB_NAME" | "DB_PATH" | "DB_SSLCA_PATH" | "CONTACT_ADDRESS"
    ) {
        return Ok(Some(("", name.to_ascii_lowercase(), Kind::String)));
    }
    if name.starts_with("DB_") {
        return Err(invalid());
    }
    Ok(None)
}

// Keep the last non-null typed object, matching Config::apply. Do not serialize
// credential-bearing structs or collapse the legacy decoder's duplicate merges.
#[derive(Default)]
struct Groups(Map<String, Value>);
impl Apply for Groups {
    fn apply(&mut self, name: &str, value: &RawValue) -> Result<(), serde_json::Error> {
        if matches!(name, "mysql" | "postgres" | "domain_verification") && value.get() != "null" {
            self.0
                .insert(name.into(), serde_json::from_str(value.get())?);
        }
        Ok(())
    }
}

pub(super) fn decode(
    source: &[u8],
    variables: impl IntoIterator<Item = (OsString, OsString)>,
) -> Result<Config, ConfigError> {
    let mut config = super::decode(source)?;
    let mut overrides = Map::new();
    let mut names = BTreeSet::new();
    for (name, value) in variables {
        let Some(name) = name.to_str() else { continue };
        let name = name.to_ascii_uppercase();
        let Some((group, key, kind)) = field(&name)? else {
            continue;
        };
        if !names.insert(name.clone()) {
            return Err(ConfigError::Invalid(format!(
                "Duplicate configuration environment variable: {name}"
            )));
        }
        let invalid =
            || ConfigError::Invalid(format!("Invalid value for environment variable {name}"));
        let text = value.to_str().ok_or_else(invalid)?;
        let value = match kind {
            Kind::String => Value::String(text.into()),
            Kind::Json => {
                let value: Value = serde_json::from_str(text).map_err(|_| invalid())?;
                if value.is_null() {
                    return Err(invalid());
                }
                value
            }
        };
        if group.is_empty() {
            overrides.insert(key, value);
        } else {
            let object = overrides
                .entry(group)
                .or_insert_with(|| Value::Object(Map::new()));
            let object = object.as_object_mut().ok_or_else(invalid)?;
            object.insert(key, value);
        }
    }
    if overrides.is_empty() {
        return Ok(config);
    }
    let invalid = || {
        ConfigError::Invalid(
            "Invalid environment configuration; check field types and required database settings"
                .into(),
        )
    };
    let mut groups = Groups::default();
    merge(&mut groups, source).map_err(|_| invalid())?;
    // Selecting another backend replaces that backend's connection configuration.
    // It does not migrate data or carry SQLite paths/legacy DSNs into a server DB.
    if overrides
        .get("db_name")
        .and_then(Value::as_str)
        .is_some_and(|name| name != config.db_name)
    {
        config.db_path.clear();
        config.db_sslca_path.clear();
        config.mysql = None;
        config.postgres = None;
        groups.0.remove("mysql");
        groups.0.remove("postgres");
    }
    for (group, fields) in &mut overrides {
        if let Some(mut original) = groups.0.remove(group) {
            let target = original.as_object_mut().ok_or_else(invalid)?;
            let supplied = fields.as_object().ok_or_else(invalid)?;
            // Password source changes replace the previous mutually exclusive source.
            if matches!(group.as_str(), "mysql" | "postgres") {
                if supplied.contains_key("password") && !supplied.contains_key("password_env") {
                    target.remove("password_env");
                } else if supplied.contains_key("password_env")
                    && !supplied.contains_key("password")
                {
                    target.remove("password");
                }
            }
            target.extend(supplied.clone());
            *fields = original;
        }
    }
    let bytes = serde_json::to_vec(&overrides).map_err(|_| invalid())?;
    merge(&mut config, &bytes).map_err(|_| invalid())?;
    Ok(config)
}

#[cfg(test)]
#[path = "../../../tests/startup_environment.rs"]
mod tests;
