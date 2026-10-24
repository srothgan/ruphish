//! The only public database entry point. Select a provider once at startup.
//!
//! Repositories expose application operations, never pools or SQL executors.
//! Driver modules and constructors are deliberately inaccessible to consumers.
//!
//! ```compile_fail
//! use ruphish_adapters::sqlite::StartupDatabase;
//! ```
//! ```compile_fail
//! fn bypass(db: &ruphish_adapters::database::Database) { db.pool(); }
//! ```
//! ```compile_fail
//! use ruphish_adapters::postgres::StartupDatabase;
//! ```
//! ```compile_fail
//! use ruphish_adapters::mysql::StartupDatabase;
//! ```
use crate::{mysql, outbound::Policy, postgres, sqlite};
pub use app::storage::Error;
use ruphish_application as app;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
mod mysql_config;
mod postgres_config;
#[cfg(test)]
mod security_tests;
pub use postgres_config::{PostgresConfig, PostgresTls};
mod errors;
mod sessions;
pub(crate) mod text;
pub(crate) use errors::commit_error;
pub use mysql_config::{MySqlConfig, MySqlTls};
pub use sessions::SessionStore;
pub mod migrations;

/// Supported connection settings. Unsupported engines cannot be silently selected.
pub enum Config {
    /// Existing MySQL database with automatic application schema upgrades.
    MySql(MySqlConfig),
    /// PostgreSQL database with transactional application migrations.
    Postgres(PostgresConfig),
    /// Local database file, including version-aware startup migration.
    Sqlite {
        /// File to open or initialize.
        path: PathBuf,
        /// Optional legacy migration-directory compatibility check.
        migrations_prefix: String,
    },
}

/// Owned database lifecycle and provider-neutral feature capabilities.
pub struct Database {
    backend: Backend,
    settings: Arc<dyn app::settings::Repository>,
    rendering: Arc<tokio::sync::Semaphore>,
    sessions: Arc<dyn app::storage::Sessions>,
    accounts: Arc<dyn app::auth::Repository>,
    resources: Arc<dyn app::resources::Repository>,
    campaigns: Arc<dyn app::campaign::Store>,
    domains: Arc<dyn app::domains::Repository>,
    interactions: Arc<dyn app::interactions::Public>,
    previews: Arc<dyn app::delivery::Previews>,
    reporting: Arc<dyn app::reporting::Repository>,
    overview: Arc<dyn app::overview::Repository>,
    integrations: Arc<dyn app::integrations::Store>,
}

enum Backend {
    Sqlite(sqlite::StartupDatabase),
    MySql(mysql::StartupDatabase),
    Postgres(postgres::StartupDatabase),
}

/// Open, recognize and migrate storage before exposing any feature operations.
/// Outbound policy applies to integration operations, never to database routing.
pub async fn open(config: Config, policy: Policy) -> Result<Database, Error> {
    match config {
        Config::Sqlite {
            path,
            migrations_prefix,
        } => {
            let backend = sqlite::StartupDatabase::open(&path, &migrations_prefix)
                .await
                .map_err(startup_error)?;
            let pool = backend.pool();
            let rendering = Arc::new(tokio::sync::Semaphore::new(8));
            Ok(Database {
                settings: Arc::new(sqlite::settings::SettingsStore::new(pool.clone())),
                sessions: Arc::new(sqlite::sessions::Sessions::new(pool.clone())),
                accounts: Arc::new(sqlite::accounts::Accounts::new(pool.clone())),
                resources: Arc::new(sqlite::resources::Resources::new(pool.clone())),
                campaigns: Arc::new(sqlite::campaigns::Campaigns::new(pool.clone())),
                domains: Arc::new(sqlite::domains::Domains::new(pool.clone())),
                interactions: Arc::new(crate::public_interactions::Interactions::new(
                    Arc::new(sqlite::interactions::Interactions::new(pool.clone())),
                    rendering.clone(),
                )),
                previews: Arc::new(crate::previews::PreviewService::new(
                    Arc::new(sqlite::previews::PreviewStore::new(pool.clone())),
                    rendering.clone(),
                )),
                reporting: Arc::new(crate::reporting::Reporting::new(Arc::new(
                    sqlite::reporting::Reports::new(pool.clone()),
                ))),
                overview: Arc::new(sqlite::overview::Overview::new(pool.clone())),
                integrations: Arc::new(sqlite::integrations::Integrations::new(
                    pool.clone(),
                    policy,
                )),
                backend: Backend::Sqlite(backend),
                rendering,
            })
        }
        Config::MySql(config) => {
            let backend = mysql::StartupDatabase::open(&config).await?;
            let pool = backend.pool();
            let rendering = Arc::new(tokio::sync::Semaphore::new(8));
            Ok(Database {
                settings: Arc::new(mysql::settings::SettingsStore::new(pool.clone())),
                sessions: Arc::new(mysql::sessions::Sessions::new(pool.clone())),
                accounts: Arc::new(mysql::accounts::Accounts::new(pool.clone())),
                resources: Arc::new(mysql::resources::Resources::new(pool.clone())),
                campaigns: Arc::new(mysql::campaigns::Campaigns::new(pool.clone())),
                domains: Arc::new(mysql::domains::Domains::new(pool.clone())),
                interactions: Arc::new(crate::public_interactions::Interactions::new(
                    Arc::new(mysql::interactions::Interactions::new(pool.clone())),
                    rendering.clone(),
                )),
                previews: Arc::new(crate::previews::PreviewService::new(
                    Arc::new(mysql::previews::PreviewStore::new(pool.clone())),
                    rendering.clone(),
                )),
                reporting: Arc::new(crate::reporting::Reporting::new(Arc::new(
                    mysql::reporting::Reports::new(pool.clone()),
                ))),
                overview: Arc::new(mysql::overview::Overview::new(pool.clone())),
                integrations: Arc::new(mysql::integrations::Integrations::new(
                    pool.clone(),
                    policy,
                )),
                backend: Backend::MySql(backend),
                rendering,
            })
        }
        Config::Postgres(config) => {
            let backend = postgres::StartupDatabase::open(&config).await?;
            let pool = backend.pool();
            let rendering = Arc::new(tokio::sync::Semaphore::new(8));
            Ok(Database {
                settings: Arc::new(postgres::settings::SettingsStore::new(pool.clone())),
                sessions: Arc::new(postgres::sessions::Sessions::new(pool.clone())),
                accounts: Arc::new(postgres::accounts::Accounts::new(pool.clone())),
                resources: Arc::new(postgres::resources::Resources::new(pool.clone())),
                campaigns: Arc::new(postgres::campaigns::Campaigns::new(pool.clone())),
                domains: Arc::new(postgres::domains::Domains::new(pool.clone())),
                interactions: Arc::new(crate::public_interactions::Interactions::new(
                    Arc::new(postgres::interactions::Interactions::new(pool.clone())),
                    rendering.clone(),
                )),
                previews: Arc::new(crate::previews::PreviewService::new(
                    Arc::new(postgres::previews::PreviewStore::new(pool.clone())),
                    rendering.clone(),
                )),
                reporting: Arc::new(crate::reporting::Reporting::new(Arc::new(
                    postgres::reporting::Reports::new(pool.clone()),
                ))),
                overview: Arc::new(postgres::overview::Overview::new(pool.clone())),
                integrations: Arc::new(postgres::integrations::Integrations::new(
                    pool.clone(),
                    policy,
                )),
                backend: Backend::Postgres(backend),
                rendering,
            })
        }
    }
}

impl Database {
    /// Installation policy and administrator campaign closure.
    pub fn settings(&self) -> Arc<dyn app::settings::Repository> {
        self.settings.clone()
    }
    /// Recoverable file snapshot for file-based providers; server backups are retained table sets.
    pub fn backup_path(&self) -> Option<&Path> {
        match &self.backend {
            Backend::Sqlite(db) => db.backup_path(),
            Backend::MySql(_) | Backend::Postgres(_) => None,
        }
    }
    /// Whether a recoverable legacy file snapshot or retained table set exists.
    pub fn has_recovery_backup(&self) -> bool {
        match &self.backend {
            Backend::Sqlite(db) => db.backup_path().is_some(),
            Backend::MySql(db) => db.has_recovery_backup(),
            Backend::Postgres(_) => false,
        }
    }
    /// Create the initial administrator or refresh its temporary password atomically.
    pub async fn bootstrap(
        &self,
        password: Option<String>,
        token: Option<String>,
    ) -> Result<Option<String>, Error> {
        match &self.backend {
            Backend::Sqlite(db) => db.bootstrap(password, token).await.map_err(startup_error),
            Backend::MySql(db) => db.bootstrap(password, token).await,
            Backend::Postgres(db) => db.bootstrap(password, token).await,
        }
    }
    /// Shared framework bridge; its persistence remains provider-independent.
    pub fn sessions(&self) -> SessionStore {
        SessionStore::new(self.sessions.clone())
    }
    /// Drain connections. Outstanding capability handles cannot reopen a closed provider.
    pub async fn close(self) {
        self.rendering.close();
        match self.backend {
            Backend::Sqlite(db) => db.close().await,
            Backend::MySql(db) => db.close().await,
            Backend::Postgres(db) => db.close().await,
        }
    }
    /// Obtain the provider-independent accounts operations.
    pub fn accounts(&self) -> Arc<dyn app::auth::Repository> {
        self.accounts.clone()
    }
    /// Obtain the provider-independent resources operations.
    pub fn resources(&self) -> Arc<dyn app::resources::Repository> {
        self.resources.clone()
    }
    /// Obtain the provider-independent campaigns operations.
    pub fn campaigns(&self) -> Arc<dyn app::campaign::Store> {
        self.campaigns.clone()
    }
    /// Obtain the provider-independent domains operations.
    pub fn domains(&self) -> Arc<dyn app::domains::Repository> {
        self.domains.clone()
    }
    /// Obtain the provider-independent interactions operations.
    pub fn interactions(&self) -> Arc<dyn app::interactions::Public> {
        self.interactions.clone()
    }
    /// Obtain the provider-independent previews operations.
    pub fn previews(&self) -> Arc<dyn app::delivery::Previews> {
        self.previews.clone()
    }
    /// Obtain the provider-independent reporting operations.
    pub fn reporting(&self) -> Arc<dyn app::reporting::Repository> {
        self.reporting.clone()
    }
    /// Obtain the provider-independent overview operations.
    pub fn overview(&self) -> Arc<dyn app::overview::Repository> {
        self.overview.clone()
    }
    /// Obtain the provider-independent integrations operations.
    pub fn integrations(&self) -> Arc<dyn app::integrations::Store> {
        self.integrations.clone()
    }
}

fn startup_error(error: sqlite::StartupError) -> Error {
    match error {
        sqlite::StartupError::Upgrade(error) => upgrade_error(error),
        sqlite::StartupError::Io(error) => Error::Io(error.kind()),
        sqlite::StartupError::Database(error) => sqlite::storage_error(error),
        sqlite::StartupError::Invalid(message) => Error::Invalid(message),
    }
}
fn upgrade_error(error: sqlite::UpgradeError) -> Error {
    match error {
        sqlite::UpgradeError::Io(error) => Error::Io(error.kind()),
        sqlite::UpgradeError::Database(error) => sqlite::storage_error(error),
        sqlite::UpgradeError::Unrecognized(message) => Error::Invalid(format!(
            "Unrecognized or inconsistent Gophish database: {message}"
        )),
    }
}

#[cfg(test)]
mod tests;
