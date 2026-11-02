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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn validators_cannot_bypass_conditional_writes() {
        for invalid in [
            "*",
            "1",
            "\"\"",
            "\"0\"",
            "\"01\"",
            "\"-1\"",
            "\"+1\"",
            "\"1.0\"",
            "W/\"1\"",
            "\"1\", \"2\"",
            "\"9223372036854775808\"",
            "\" 1\"",
            "\"1 \"",
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(header::IF_MATCH, HeaderValue::from_str(invalid).unwrap());
            assert!(optional(&headers).is_err(), "accepted {invalid}");
        }
        let mut headers = HeaderMap::new();
        assert!(matches!(optional(&headers), Ok(None)));
        assert!(required(&headers).is_err());
        headers.insert(header::IF_MATCH, HeaderValue::from_static("\"1\""));
        assert!(matches!(required(&headers), Ok(1)));
        headers.append(header::IF_MATCH, HeaderValue::from_static("\"1\""));
        assert!(
            optional(&headers).is_err(),
            "even identical repeated validators are ambiguous"
        );
    }
}
