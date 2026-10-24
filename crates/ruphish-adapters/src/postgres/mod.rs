//! Private PostgreSQL provider. Driver types never escape the database facade.
use ruphish_application::storage::Error;
use sqlx::Row;
pub(super) mod accounts;
pub(super) mod campaigns;
pub(super) mod domains;
mod engagement;
mod events;
pub(super) mod integrations;
pub(super) mod interactions;
mod locks;
mod mailbox_worker;
mod migrations;
pub(super) mod overview;
pub(super) mod previews;
pub(super) mod reporting;
pub(super) mod resources;
pub(super) mod sessions;
mod startup;
mod webhook_worker;
pub(super) use startup::StartupDatabase;

pub(super) fn storage_error(error: sqlx::Error) -> Error {
    match error {
        sqlx::Error::PoolTimedOut => Error::Busy,
        sqlx::Error::Database(e) => match e.code().as_deref() {
            Some("40001" | "40P01") => Error::TransactionConflict,
            Some("55P03" | "57014") => Error::Busy,
            Some("23502" | "23503" | "23505" | "23514") => Error::Integrity,
            Some("42P01" | "42703") => Error::UnsupportedSchema,
            _ => Error::Unavailable,
        },
        _ => Error::Unavailable,
    }
}
pub(super) fn commit_error(error: sqlx::Error) -> ruphish_application::storage::Error {
    crate::database::commit_error(error, storage_error)
}
trait InsertId {
    fn insert_id(&self) -> Result<i64, sqlx::Error>;
}
impl InsertId for sqlx::postgres::PgRow {
    fn insert_id(&self) -> Result<i64, sqlx::Error> {
        self.try_get("id")
    }
}

pub(super) fn resource_error(error: sqlx::Error) -> ruphish_application::resources::Error {
    use ruphish_application::resources::Error;
    if error
        .as_database_error()
        .and_then(|e| e.code())
        .is_some_and(|c| matches!(c.as_ref(), "22021" | "22001"))
    {
        Error::Invalid(
            "storage_text_invalid: text contains NUL or exceeds the supported field length".into(),
        )
    } else {
        Error::Storage(storage_error(error))
    }
}

pub(super) mod settings;
