//! Typed MySQL connection policy. Credentials deliberately have no Debug/Serialize.
use super::Error;
use serde::Deserialize;
use std::path::PathBuf;

/// MySQL transport policy. There is no opportunistic plaintext fallback.
#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MySqlTls {
    /// Verify the chain and server hostname, using an optional private CA.
    #[default]
    VerifyIdentity,
    /// Explicit plaintext transport for a trusted local test connection.
    Disabled,
}

/// Validated at the facade boundary before any connection is attempted.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MySqlConfig {
    /// DNS hostname or IP literal, without a scheme or port.
    pub host: String,
    /// TCP port.
    #[serde(default = "default_port")]
    pub port: u16,
    /// Existing database; Ruphish never creates the database or server account.
    pub database: String,
    /// Database service account, not an administrator login to Ruphish.
    pub username: String,
    /// Explicit password. Mutually exclusive with password_env.
    #[serde(default)]
    pub password: Option<String>,
    /// Environment variable containing the password, resolved once on connection.
    #[serde(default)]
    pub password_env: Option<String>,
    /// Verified TLS by default.
    #[serde(default)]
    pub tls: MySqlTls,
    /// Optional PEM certificate authority for verified TLS.
    #[serde(default)]
    pub ca_path: Option<PathBuf>,
    /// Maximum runtime pool size.
    #[serde(default = "default_pool")]
    pub max_connections: u32,
    /// Connection establishment deadline, in seconds.
    #[serde(default = "default_timeout")]
    pub connect_timeout_seconds: u32,
    /// Pool acquisition deadline, in seconds.
    #[serde(default = "default_timeout")]
    pub acquire_timeout_seconds: u32,
    /// Row/metadata/migration ownership lock deadline, in seconds.
    #[serde(default = "default_timeout")]
    pub lock_timeout_seconds: u32,
    /// Explicit source offset for Gophish schemas preceding its UTC migration.
    /// Ignored for already-UTC history; never inferred from the server timezone.
    #[serde(default)]
    pub legacy_timezone: Option<String>,
}

const fn default_port() -> u16 {
    3306
}
const fn default_pool() -> u32 {
    10
}
const fn default_timeout() -> u32 {
    10
}

impl MySqlConfig {
    /// Construct settings with verified TLS and bounded pool/lock defaults.
    pub fn new(host: String, database: String, username: String, password: String) -> Self {
        Self {
            host,
            database,
            username,
            password: Some(password),
            password_env: None,
            port: default_port(),
            tls: MySqlTls::default(),
            ca_path: None,
            max_connections: default_pool(),
            connect_timeout_seconds: default_timeout(),
            acquire_timeout_seconds: default_timeout(),
            lock_timeout_seconds: default_timeout(),
            legacy_timezone: None,
        }
    }

    /// Validate shape and policy without reading secrets or contacting the server.
    pub fn validate(&self) -> Result<(), Error> {
        if let Some(zone) = &self.legacy_timezone {
            let bytes = zone.as_bytes();
            let valid = bytes.len() == 6
                && matches!(bytes[0], b'+' | b'-')
                && bytes[3] == b':'
                && bytes[1..3]
                    .iter()
                    .chain(&bytes[4..6])
                    .all(u8::is_ascii_digit)
                && zone[1..3].parse::<u8>().is_ok_and(|h| h <= 13)
                && zone[4..6].parse::<u8>().is_ok_and(|m| m < 60);
            if !valid {
                return Err(Error::Invalid(
                    "MySQL legacy_timezone must be an explicit offset from -13:59 to +13:59".into(),
                ));
            }
        }
        if self.host.is_empty()
            || self
                .host
                .chars()
                .any(|c| c.is_whitespace() || matches!(c, '/' | '@' | '?' | '#' | '\\'))
            || self.port == 0
            || self.database.is_empty()
            || self.database.len() > 64
            || self
                .database
                .chars()
                .any(|c| c.is_control() || matches!(c, '/' | '\\' | '.'))
            || self.username.is_empty()
            || self.username.chars().any(char::is_control)
        {
            return Err(Error::Invalid(
                "Invalid MySQL host, port, database or service account".into(),
            ));
        }
        if self.password.is_some() == self.password_env.is_some() {
            return Err(Error::Invalid(
                "Specify exactly one MySQL password source".into(),
            ));
        }
        if self
            .password_env
            .as_ref()
            .is_some_and(|v| v.is_empty() || v.contains(['=', '\0']))
        {
            return Err(Error::Invalid(
                "Invalid MySQL password environment variable".into(),
            ));
        }
        if !(1..=128).contains(&self.max_connections)
            || [
                self.connect_timeout_seconds,
                self.acquire_timeout_seconds,
                self.lock_timeout_seconds,
            ]
            .iter()
            .any(|v| !(1..=300).contains(v))
        {
            return Err(Error::Invalid(
                "MySQL pool size must be 1..128 and deadlines 1..300 seconds".into(),
            ));
        }
        if matches!(self.tls, MySqlTls::Disabled) && self.ca_path.is_some() {
            return Err(Error::Invalid("A MySQL CA requires verified TLS".into()));
        }
        Ok(())
    }

    /// Translate the supported Go TCP DSN subset; reject ignored or weakened options.
    pub fn from_legacy(dsn: &str, ca_path: Option<PathBuf>) -> Result<Self, Error> {
        let invalid = || {
            Error::Invalid("Unsupported MySQL DSN; use user:password@tcp(host:port)/database with supported options".into())
        };
        let (credentials, address) = dsn.rsplit_once("@tcp(").ok_or_else(invalid)?;
        let (username, password) = credentials.split_once(':').ok_or_else(invalid)?;
        let (endpoint, rest) = address.split_once(")/").ok_or_else(invalid)?;
        let endpoint = url::Url::parse(&format!("mysql://{endpoint}")).map_err(|_| invalid())?;
        if !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.query().is_some()
            || endpoint.fragment().is_some()
            || !matches!(endpoint.path(), "" | "/")
        {
            return Err(invalid());
        }
        let (name, query) = rest.split_once('?').unwrap_or((rest, ""));
        let database = percent_encoding::percent_decode_str(name)
            .decode_utf8()
            .map_err(|_| invalid())?
            .into_owned();
        let host = endpoint
            .host_str()
            .ok_or_else(invalid)?
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_owned();
        let mut config = Self::new(host, database, username.into(), password.into());
        config.port = endpoint.port().unwrap_or(default_port());
        config.ca_path = ca_path;
        let mut keys = std::collections::BTreeSet::new();
        for (name, value) in url::form_urlencoded::parse(query.as_bytes()) {
            if !keys.insert(name.to_string()) {
                return Err(invalid());
            }
            match (name.as_ref(), value.as_ref()) {
                ("parseTime", "true")
                | ("loc", "UTC")
                | ("charset", "utf8mb4")
                | ("tls", "true") => {}
                ("tls", "ssl_ca") if config.ca_path.is_some() => {}
                ("tls", "false") => config.tls = MySqlTls::Disabled,
                ("timeout", value) => {
                    config.connect_timeout_seconds = value
                        .strip_suffix('s')
                        .and_then(|v| v.parse().ok())
                        .ok_or_else(invalid)?;
                }
                _ => return Err(invalid()),
            }
        }
        config.validate()?;
        Ok(config)
    }

    pub(crate) fn password_value(&self) -> Result<String, Error> {
        match (&self.password, &self.password_env) {
            (Some(value), None) => Ok(value.clone()),
            (None, Some(name)) => std::env::var(name).map_err(|_| {
                Error::Invalid(
                    "MySQL password environment variable is missing or not Unicode".into(),
                )
            }),
            _ => Err(Error::Invalid(
                "Specify exactly one MySQL password source".into(),
            )),
        }
    }
}
