//! Typed PostgreSQL connection policy. Credentials deliberately have no Debug/Serialize.
use super::Error;
use serde::Deserialize;
use std::path::PathBuf;

/// PostgreSQL transport policy. There is no opportunistic plaintext fallback.
#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostgresTls {
    /// Verify the chain and server hostname, using an optional private CA.
    #[default]
    VerifyIdentity,
    /// Explicit plaintext transport for a trusted local test connection.
    Disabled,
}

/// Validated at the facade boundary before any connection is attempted.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostgresConfig {
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
    pub tls: PostgresTls,
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
    /// Statement deadline for runtime connections (migrations have a separate bound).
    #[serde(default = "default_statement_timeout")]
    pub statement_timeout_seconds: u32,
}

const fn default_statement_timeout() -> u32 {
    30
}
const fn default_port() -> u16 {
    5432
}
const fn default_pool() -> u32 {
    10
}
const fn default_timeout() -> u32 {
    10
}

impl PostgresConfig {
    /// Construct settings with verified TLS and bounded pool/lock defaults.
    pub fn new(host: String, database: String, username: String, password: String) -> Self {
        Self {
            host,
            database,
            username,
            password: Some(password),
            password_env: None,
            port: default_port(),
            tls: PostgresTls::default(),
            ca_path: None,
            max_connections: default_pool(),
            connect_timeout_seconds: default_timeout(),
            acquire_timeout_seconds: default_timeout(),
            lock_timeout_seconds: default_timeout(),
            statement_timeout_seconds: default_statement_timeout(),
        }
    }

    /// Validate shape and policy without reading secrets or contacting the server.
    pub fn validate(&self) -> Result<(), Error> {
        if self.host.is_empty()
            || self
                .host
                .chars()
                .any(|c| c.is_whitespace() || matches!(c, '/' | '@' | '?' | '#' | '\\'))
            || self.port == 0
            || self.database.is_empty()
            || self.database.len() > 63
            || self
                .database
                .chars()
                .any(|c| c.is_control() || matches!(c, '/' | '\\' | '.'))
            || self.username.is_empty()
            || self.username.chars().any(char::is_control)
        {
            return Err(Error::Invalid(
                "Invalid PostgreSQL host, port, database or service account".into(),
            ));
        }
        if self.password.is_some() == self.password_env.is_some() {
            return Err(Error::Invalid(
                "Specify exactly one PostgreSQL password source".into(),
            ));
        }
        if self
            .password_env
            .as_ref()
            .is_some_and(|v| v.is_empty() || v.contains(['=', '\0']))
        {
            return Err(Error::Invalid(
                "Invalid PostgreSQL password environment variable".into(),
            ));
        }
        if !(1..=128).contains(&self.max_connections)
            || [
                self.connect_timeout_seconds,
                self.acquire_timeout_seconds,
                self.lock_timeout_seconds,
                self.statement_timeout_seconds,
            ]
            .iter()
            .any(|v| !(1..=300).contains(v))
        {
            return Err(Error::Invalid(
                "PostgreSQL pool size must be 1..128 and deadlines 1..300 seconds".into(),
            ));
        }
        if matches!(self.tls, PostgresTls::Disabled) && self.ca_path.is_some() {
            return Err(Error::Invalid(
                "A PostgreSQL CA requires verified TLS".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn password_value(&self) -> Result<String, Error> {
        match (&self.password, &self.password_env) {
            (Some(value), None) => Ok(value.clone()),
            (None, Some(name)) => std::env::var(name).map_err(|_| {
                Error::Invalid(
                    "PostgreSQL password environment variable is missing or not Unicode".into(),
                )
            }),
            _ => Err(Error::Invalid(
                "Specify exactly one PostgreSQL password source".into(),
            )),
        }
    }
}
