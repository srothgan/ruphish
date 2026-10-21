//! Classify commit acknowledgement separately from ordinary statement errors.
use ruphish_application::storage::Error;
pub(crate) fn commit_error(error: sqlx::Error, classify: fn(sqlx::Error) -> Error) -> Error {
    // Only explicit server rejections prove rollback. Transport loss, shutdown,
    // cancellation and an unrecognized acknowledgement require reconciliation.
    let rejected = error
        .as_database_error()
        .and_then(|e| e.code())
        .is_some_and(|code| code.starts_with("23") || code.starts_with("40") || code == "25P02");
    if rejected {
        classify(error)
    } else {
        Error::UncertainCommit
    }
}
