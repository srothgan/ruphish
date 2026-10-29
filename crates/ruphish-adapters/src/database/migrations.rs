//! Explicit source assessment and complete SQLite initialization/conversion.
use super::{Error, sqlite, upgrade_error};
use std::path::Path;

/// Read-only MySQL schema assessment, never a claim that an upgrade has run.
#[derive(Debug, PartialEq, Eq)]
pub struct MySqlInspection {
    /// Server version returned by MySQL, without connection information.
    pub server_version: String,
    /// Recognized empty or legacy schema.
    pub schema: MySqlSchema,
}

/// Recognized source of a prospective MySQL initialization or upgrade.
#[derive(Debug, PartialEq, Eq)]
pub enum MySqlSchema {
    /// Existing database with no application tables.
    Empty,
    /// Matching Goose history and table/column shapes.
    Gophish {
        /// Last applied upstream MySQL migration.
        version: i64,
        /// Historical dates precede Gophish's UTC migration and need interpretation.
        requires_legacy_timezone: bool,
    },
}

/// Inspect through the provider boundary using verified transport and a bounded lock.
/// This reads metadata/history only: it does not initialize, migrate or bootstrap.
/// Use this for source assessment; ordinary startup initializes/upgrades through `database::open`.
pub async fn inspect_mysql(config: &super::MySqlConfig) -> Result<MySqlInspection, Error> {
    crate::mysql::inspect(config).await
}
/// Latest supported Goose schema version, not a Ruphish schema version.
pub const LATEST_LEGACY_VERSION: i64 = sqlite::LATEST_LEGACY_VERSION;
/// Create the complete release baseline without bootstrap accounts.
pub async fn create_baseline(path: &Path) -> Result<(), Error> {
    sqlite::create(path).await.map_err(upgrade_error)
}
/// Upgrade a coherent file snapshot without changing the source or clobbering output.
/// The destination is a complete current Ruphish database.
pub async fn upgrade_copy(source: &Path, destination: &Path) -> Result<(), Error> {
    sqlite::upgrade_copy(source, destination)
        .await
        .map_err(upgrade_error)
}
