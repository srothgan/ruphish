//! Conditional management writes share one strict numeric validator parser.
use crate::auth::{Rejection, failure};
use axum::http::{HeaderMap, StatusCode, header};
use ruphish_contracts::errors::ErrorCode;

pub(crate) fn optional(headers: &HeaderMap) -> Result<Option<i64>, Rejection> {
    let Some(value) = headers.get(header::IF_MATCH) else {
        return Ok(None);
    };
    (headers.get_all(header::IF_MATCH).iter().count() == 1)
        .then_some(value)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            value
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
        })
        .filter(|value| !value.starts_with('0') && value.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| *value > 0)
        .map(Some)
        .ok_or_else(|| {
            failure(
                StatusCode::BAD_REQUEST,
                ErrorCode::RevisionRequired,
                "Use the quoted numeric revision returned with the resource",
            )
        })
}

pub(crate) fn required(headers: &HeaderMap) -> Result<i64, Rejection> {
    optional(headers)?.ok_or_else(|| {
        failure(
            StatusCode::PRECONDITION_REQUIRED,
            ErrorCode::RevisionRequired,
            "Load the resource and send its revision in If-Match",
        )
    })
}
