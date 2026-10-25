//! Error carried from the HTTP client through the read cache to views.
//!
//! A failed read keeps its localized `message` for display and the HTTP `status`
//! and `code` that produced it, so a view can tell a missing entity (404) apart
//! from a transient failure that is worth retrying.

/// A read or mutation failure with its localized message and originating status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiError {
    /// HTTP status, or `0` when no status is available at the failure boundary.
    pub status: u16,
    /// Stable machine-readable error code, empty for client-side failures.
    pub code: String,
    /// Localized, user-facing message.
    pub message: String,
    /// Optional field failures; local validation never constructs an API error.
    pub validation: Option<ruphish_contracts::validation::ValidationReport>,
}

impl ApiError {
    /// A client-side failure without HTTP metadata: transport or an
    /// inactive session.
    pub fn local(message: String) -> Self {
        Self {
            status: 0,
            code: String::new(),
            message,
            validation: None,
        }
    }

    /// A failure associated with a completed HTTP response.
    pub fn http(status: u16, code: String, message: String) -> Self {
        Self {
            status,
            code,
            message,
            validation: None,
        }
    }

    /// The requested entity does not exist.
    pub fn is_not_found(&self) -> bool {
        self.status == 404
    }

    /// Authorization and missing-record failures invalidate previously loaded data.
    pub fn is_terminal(&self) -> bool {
        matches!(self.status, 401 | 403 | 404)
    }
}

/// Interpret the response envelope without trusting server-provided display text.
/// Even a non-JSON rejection must retain its HTTP status for page recovery policy.
pub fn response(
    status: u16,
    decoded: Result<serde_json::Value, String>,
    label: impl FnOnce(&str) -> String,
) -> Result<serde_json::Value, ApiError> {
    let success = (200..300).contains(&status);
    let value = match decoded {
        Ok(value) => value,
        Err(message) if success => {
            return Err(ApiError::http(status, "INVALID_RESPONSE".into(), message));
        }
        Err(_) => serde_json::Value::Null,
    };
    if !success || value["success"] == false {
        let code = value["code"]
            .as_str()
            .filter(|c| !c.is_empty())
            .unwrap_or(match status {
                401 => "AUTH_REQUIRED",
                403 => "AUTH_FORBIDDEN",
                404 => "RESOURCE_NOT_FOUND",
                429 => "RATE_LIMITED",
                503 => "SERVICE_BUSY",
                _ => "REQUEST_FAILED",
            });
        let mut error = ApiError::http(status, code.to_owned(), label(code));
        error.validation = value
            .get("validation")
            .cloned()
            .and_then(|value| {
                serde_json::from_value::<ruphish_contracts::validation::ValidationReport>(value)
                    .ok()
            })
            .filter(|report| {
                report.issues.len() <= ruphish_contracts::validation::MAX_ISSUES
                    && report.issues.iter().all(|issue| {
                        issue.field.len() <= 200
                            && issue
                                .field
                                .bytes()
                                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_'))
                    })
            });
        return Err(error);
    }
    Ok(value)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rejected_responses_keep_status_and_codes_without_exposing_server_messages() {
        for (status, code) in [
            (401, "AUTH_REQUIRED"),
            (403, "AUTH_FORBIDDEN"),
            (404, "RESOURCE_NOT_FOUND"),
            (429, "RATE_LIMITED"),
            (503, "SERVICE_BUSY"),
        ] {
            for body in [
                Err("not JSON".into()),
                Ok(json!({"success":false,"message":"untrusted","code":""})),
            ] {
                let error = response(status, body, |code| format!("localized {code}")).unwrap_err();
                assert_eq!(error.status, status);
                assert_eq!(error.code, code);
                assert_eq!(error.message, format!("localized {code}"));
                assert_eq!(error.is_terminal(), matches!(status, 401 | 403 | 404));
            }
        }
        let error = response(
            409,
            Ok(json!({"success":false,"code":"RESOURCE_CONFLICT","message":"untrusted"})),
            str::to_owned,
        )
        .unwrap_err();
        assert_eq!(error.code, "RESOURCE_CONFLICT");
        assert_eq!(error.status, 409);
        assert!(!error.is_terminal());
    }

    #[test]
    fn malformed_success_and_application_rejections_remain_failures() {
        let malformed = response(200, Err("invalid JSON".into()), str::to_owned).unwrap_err();
        assert_eq!(malformed.status, 200);
        assert!(!malformed.is_terminal());
        let rejected = response(
            200,
            Ok(json!({"success":false,"code":"RESOURCE_NOT_FOUND"})),
            str::to_owned,
        )
        .unwrap_err();
        assert!(
            !rejected.is_not_found(),
            "HTTP status owns missing-record classification"
        );
        assert_eq!(rejected.code, "RESOURCE_NOT_FOUND");
        assert_eq!(
            response(200, Ok(json!({"id":7})), str::to_owned).unwrap(),
            json!({"id":7})
        );
        assert_eq!(
            response(204, Ok(serde_json::Value::Null), str::to_owned).unwrap(),
            serde_json::Value::Null
        );
    }
}
