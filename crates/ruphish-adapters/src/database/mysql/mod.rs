//! Private MySQL driver boundary. No pool or driver types escape the facade.

use crate::database::mysql::startup::preflight;
pub(crate) mod accounts;
mod attribution;
pub(crate) mod campaigns;
pub(crate) mod domains;
mod engagement;
mod event_edit;
mod events;
pub(crate) mod integrations;
pub(crate) mod interactions;
mod inventory_sql;
mod locks;
pub(crate) mod overview;
pub(crate) mod previews;
pub mod profile_diagnostics;
pub(crate) mod reporting;
pub(crate) mod resources;
pub(crate) mod sessions;
pub(crate) mod settings;
mod startup;
mod worker_sql;

pub(crate) use preflight::inspect;

pub(crate) fn storage_error(error: sqlx::Error) -> ruphish_application::storage::Error {
    use ruphish_application::storage::Error;
    match error {
        sqlx::Error::PoolTimedOut => Error::Busy,
        sqlx::Error::Database(error) => {
            let code = error
                .try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>()
                .map(|e| e.number());
            match code {
                Some(1205 | 3572) => Error::Busy,
                Some(1213) => Error::TransactionConflict,
                Some(1062 | 1451 | 1452 | 3819) => Error::Integrity,
                Some(1054 | 1146) => Error::UnsupportedSchema,
                _ => Error::Unavailable,
            }
        }
        _ => Error::Unavailable,
    }
}

pub(crate) use startup::StartupDatabase;
trait InsertId {
    fn insert_id(&self) -> Result<i64, sqlx::Error>;
}
impl InsertId for sqlx::mysql::MySqlQueryResult {
    fn insert_id(&self) -> Result<i64, sqlx::Error> {
        i64::try_from(self.last_insert_id())
            .map_err(|_| sqlx::Error::Protocol("Identifier outside supported range".into()))
    }
}

/// A server rejection is known; losing the acknowledgement is not a rollback.
pub(crate) fn commit_error(error: sqlx::Error) -> ruphish_application::storage::Error {
    crate::database::commit_error(error, storage_error)
}

/// Classify statement failures without exposing driver diagnostics.
pub(crate) fn resource_error(error: sqlx::Error) -> ruphish_application::resources::Error {
    ruphish_application::resources::Error::Storage(storage_error(error))
}
