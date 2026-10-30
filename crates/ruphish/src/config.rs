use crate::cli::Mode;
use serde::de::{self, DeserializeOwned, DeserializeSeed, MapAccess, Visitor};
use serde_json::value::RawValue;
use std::{
    fmt,
    path::{Path, PathBuf},
};

#[path = "config_environment.rs"]
mod environment;

#[derive(Clone, Default)]
pub(crate) struct Server {
    pub(crate) listen_url: String,
    pub(crate) use_tls: bool,
    pub(crate) cert_path: PathBuf,
    pub(crate) key_path: PathBuf,
    pub(crate) csrf_key: String,
    pub(crate) allowed_internal_hosts: Vec<String>,
    pub(crate) trusted_origins: Vec<String>,
}

#[derive(Clone, Default)]
pub(crate) struct Logging {
    pub(crate) filename: PathBuf,
    pub(crate) level: String,
    pub(crate) max_size_bytes: Option<u64>,
    pub(crate) max_files: Option<usize>,
}

impl Logging {
    pub(crate) fn rotation(&self) -> ruphish_adapters::rotating_log::Policy {
        let defaults = ruphish_adapters::rotating_log::Policy::default();
        ruphish_adapters::rotating_log::Policy {
            max_size_bytes: self.max_size_bytes.unwrap_or(defaults.max_size_bytes),
            max_files: self.max_files.unwrap_or(defaults.max_files),
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct Config {
    pub(crate) domain_verification: ruphish_adapters::domain_verification::Config,
    pub(crate) admin: Server,
    pub(crate) phish: Server,
    pub(crate) db_name: String,
    pub(crate) db_path: PathBuf,
    pub(crate) db_sslca_path: PathBuf,
    pub(crate) postgres: Option<ruphish_adapters::database::PostgresConfig>,
    pub(crate) mysql: Option<ruphish_adapters::database::MySqlConfig>,
    pub(crate) contact_address: String,
    pub(crate) logging: Logging,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ConfigError {
    #[error("Could not read configuration: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not create default configuration")]
    Initialize(#[source] std::io::Error),
    #[error("Invalid configuration JSON at line {line}, column {column}")]
    Json { line: usize, column: usize },
    #[error("{0}")]
    Invalid(String),
}

pub(crate) fn load(path: &Path) -> Result<Config, ConfigError> {
    let source = std::fs::read(path)?;
    environment::decode(&source, std::env::vars_os())
}

pub(crate) fn decode(source: &[u8]) -> Result<Config, ConfigError> {
    let mut config = Config::default();
    merge(&mut config, source).map_err(|error| ConfigError::Json {
        line: error.line(),
        column: error.column(),
    })?;
    Ok(config)
}

impl Config {
    /// OS-discovered configurations use their own directory for relative paths.
    /// Explicit --config inputs retain the legacy process-working-directory contract.
    pub(crate) fn resolve_paths(&mut self, base: &Path) -> Result<(), ConfigError> {
        fn resolve(path: &mut PathBuf, base: &Path) {
            if !path.as_os_str().is_empty() && path.is_relative() {
                *path = base.join(&*path);
            }
        }
        for path in [
            &mut self.admin.cert_path,
            &mut self.admin.key_path,
            &mut self.phish.cert_path,
            &mut self.phish.key_path,
            &mut self.db_sslca_path,
            &mut self.logging.filename,
        ] {
            resolve(path, base);
        }
        if self.db_name == "sqlite3" {
            resolve(&mut self.db_path, base);
        }
        for path in [
            self.mysql
                .as_mut()
                .and_then(|config| config.ca_path.as_mut()),
            self.postgres
                .as_mut()
                .and_then(|config| config.ca_path.as_mut()),
            self.domain_verification.trusted_ca_path.as_mut(),
        ]
        .into_iter()
        .flatten()
        {
            resolve(path, base);
        }
        Ok(())
    }

    pub(crate) fn validate(&self, mode: Mode) -> Result<(), ConfigError> {
        self.logging
            .rotation()
            .validate()
            .map_err(|error| ConfigError::Invalid(error.to_string()))?;
        self.database()?;
        for (selected, name, server) in [
            (mode.admin(), "admin_server", &self.admin),
            (mode.phish(), "phish_server", &self.phish),
        ] {
            if selected && server.listen_url.trim().is_empty() {
                return Err(ConfigError::Invalid(format!(
                    "{name} requires an explicit listen_url"
                )));
            }
            if selected
                && server.use_tls
                && (server.cert_path.as_os_str().is_empty()
                    || server.key_path.as_os_str().is_empty())
            {
                return Err(ConfigError::Invalid(format!(
                    "{name} TLS requires cert_path and key_path"
                )));
            }
        }
        Ok(())
    }

    pub(crate) fn database(&self) -> Result<ruphish_adapters::database::Config, ConfigError> {
        use ruphish_adapters::database::{Config as Database, MySqlConfig};
        match self.db_name.as_str() {
            "sqlite3" => {
                if self.mysql.is_some()
                    || self.postgres.is_some()
                    || !self.db_sslca_path.as_os_str().is_empty()
                {
                    return Err(ConfigError::Invalid(
                        "Server database settings cannot be used with SQLite".into(),
                    ));
                }
                if self.db_path.as_os_str().is_empty() {
                    return Err(ConfigError::Invalid(
                        "db_path must name a SQLite file".into(),
                    ));
                }
                Ok(Database::Sqlite {
                    path: self.db_path.clone(),
                })
            }
            "postgres" => {
                if self.mysql.is_some()
                    || !self.db_path.as_os_str().is_empty()
                    || !self.db_sslca_path.as_os_str().is_empty()
                {
                    return Err(ConfigError::Invalid("PostgreSQL requires its typed settings and embedded migrations; remove other database settings".into()));
                }
                let settings = self.postgres.clone().ok_or_else(|| {
                    ConfigError::Invalid("PostgreSQL settings are required".into())
                })?;
                settings
                    .validate()
                    .map_err(|e| ConfigError::Invalid(e.to_string()))?;
                Ok(Database::Postgres(settings))
            }
            "mysql" => {
                if self.postgres.is_some() {
                    return Err(ConfigError::Invalid(
                        "PostgreSQL settings cannot be used with MySQL".into(),
                    ));
                }

                let settings = if let Some(settings) = &self.mysql {
                    if !self.db_path.as_os_str().is_empty()
                        || !self.db_sslca_path.as_os_str().is_empty()
                    {
                        return Err(ConfigError::Invalid(
                            "Use either mysql settings or legacy db_path/db_sslca_path, not both"
                                .into(),
                        ));
                    }
                    settings.clone()
                } else {
                    MySqlConfig::from_legacy(
                        self.db_path.to_str().ok_or_else(|| {
                            ConfigError::Invalid("MySQL DSN must be Unicode".into())
                        })?,
                        (!self.db_sslca_path.as_os_str().is_empty())
                            .then(|| self.db_sslca_path.clone()),
                    )
                    .map_err(|e| ConfigError::Invalid(e.to_string()))?
                };
                settings
                    .validate()
                    .map_err(|e| ConfigError::Invalid(e.to_string()))?;
                Ok(Database::MySql(settings))
            }
            _ => Err(ConfigError::Invalid(
                "Unsupported database backend; choose db_name=sqlite3, mysql or postgres".into(),
            )),
        }
    }
}

// Preserve Go's case-insensitive fields, null scalars and ordered duplicate merges.
// RawValue keeps nested duplicate keys intact instead of collapsing them in Value.
trait Apply {
    fn apply(&mut self, name: &str, value: &RawValue) -> Result<(), serde_json::Error>;
}
struct Seed<'a, T>(&'a mut T);
impl<'de, T: Apply> DeserializeSeed<'de> for Seed<'_, T> {
    type Value = ();
    fn deserialize<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        struct Object<'a, T>(&'a mut T);
        impl<'de, T: Apply> Visitor<'de> for Object<'_, T> {
            type Value = ();
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a configuration object")
            }
            fn visit_unit<E: de::Error>(self) -> Result<(), E> {
                Ok(())
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<(), M::Error> {
                while let Some((name, value)) = map.next_entry::<String, Box<RawValue>>()? {
                    self.0
                        .apply(&fold(&name), &value)
                        .map_err(de::Error::custom)?;
                }
                Ok(())
            }
        }
        deserializer.deserialize_any(Object(self.0))
    }
}
fn fold(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'ſ' => 's',
            'K' => 'k',
            _ => c.to_ascii_lowercase(),
        })
        .collect()
}
fn merge<T: Apply>(target: &mut T, source: &[u8]) -> Result<(), serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_slice(source);
    Seed(target).deserialize(&mut deserializer)?;
    deserializer.end()
}
fn scalar<T: DeserializeOwned>(target: &mut T, value: &RawValue) -> Result<(), serde_json::Error> {
    if value.get() != "null" {
        *target = serde_json::from_str(value.get())?;
    }
    Ok(())
}
impl Apply for Config {
    fn apply(&mut self, name: &str, value: &RawValue) -> Result<(), serde_json::Error> {
        match name {
            "domain_verification" => scalar(&mut self.domain_verification, value),
            "admin_server" => merge(&mut self.admin, value.get().as_bytes()),
            "phish_server" => merge(&mut Public(&mut self.phish), value.get().as_bytes()),
            "db_name" => scalar(&mut self.db_name, value),
            "db_path" => scalar(&mut self.db_path, value),
            "db_sslca_path" => scalar(&mut self.db_sslca_path, value),
            "postgres" => scalar(&mut self.postgres, value),
            "mysql" => scalar(&mut self.mysql, value),
            "contact_address" => scalar(&mut self.contact_address, value),
            "logging" if value.get() == "null" => {
                self.logging = Logging::default();
                Ok(())
            }
            "logging" => merge(&mut self.logging, value.get().as_bytes()),
            "test_flag" => {
                let mut flag = false;
                scalar(&mut flag, value)
            }
            _ => Err(de::Error::custom("Unknown configuration field")),
        }
    }
}
impl Apply for Server {
    fn apply(&mut self, name: &str, value: &RawValue) -> Result<(), serde_json::Error> {
        match name {
            "listen_url" => scalar(&mut self.listen_url, value),
            "use_tls" => scalar(&mut self.use_tls, value),
            "cert_path" => scalar(&mut self.cert_path, value),
            "key_path" => scalar(&mut self.key_path, value),
            "csrf_key" => scalar(&mut self.csrf_key, value),
            "allowed_internal_hosts" => {
                self.allowed_internal_hosts =
                    serde_json::from_str::<Option<Vec<String>>>(value.get())?.unwrap_or_default();
                Ok(())
            }
            "trusted_origins" => {
                self.trusted_origins =
                    serde_json::from_str::<Option<Vec<String>>>(value.get())?.unwrap_or_default();
                Ok(())
            }
            _ => Err(de::Error::custom("Unknown listener configuration field")),
        }
    }
}
impl Apply for Logging {
    fn apply(&mut self, name: &str, value: &RawValue) -> Result<(), serde_json::Error> {
        match name {
            "filename" => scalar(&mut self.filename, value),
            "level" => scalar(&mut self.level, value),
            "max_size_bytes" => scalar(&mut self.max_size_bytes, value),
            "max_files" => scalar(&mut self.max_files, value),
            _ => Err(de::Error::custom("Unknown logging configuration field")),
        }
    }
}

struct Public<'a>(&'a mut Server);
impl Apply for Public<'_> {
    fn apply(&mut self, name: &str, value: &RawValue) -> Result<(), serde_json::Error> {
        match name {
            "listen_url" | "use_tls" | "cert_path" | "key_path" => self.0.apply(name, value),
            _ => Err(de::Error::custom(
                "Unknown public listener configuration field",
            )),
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/startup_config.rs"]
mod tests;
