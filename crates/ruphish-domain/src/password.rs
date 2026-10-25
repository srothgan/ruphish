//! Existing password policy, including its UTF-8 byte-length semantics.

/// A password-change validation failure with the existing public message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PasswordError {
    /// No candidate was supplied.
    #[error("No password provided")]
    Empty,
    /// The candidate contains fewer than eight UTF-8 bytes.
    #[error("Password must be at least 8 characters")]
    TooShort,
    /// The confirmation differs from the new password.
    #[error("Passwords do not match")]
    Mismatch,
    /// The candidate verifies against the current password hash.
    #[error("Cannot reuse existing password")]
    Reused,
}

/// Validate the inherited policy; bcrypt's separate input limit is not a policy rule.
pub fn validate(password: &str) -> Result<(), PasswordError> {
    if password.is_empty() {
        Err(PasswordError::Empty)
    } else if password.len() < 8 {
        Err(PasswordError::TooShort)
    } else {
        Ok(())
    }
}

/// Supported credential input bound, also enforced before cryptographic work.
pub const MAX_BYTES: usize = 1024;

/// Candidate policy shared by setup, replacement and administrator resets.
pub fn validation(password: &str, field: &str) -> crate::validation::Report {
    use crate::validation::{Code, Report};
    let mut report = Report::default();
    if let Err(error) = validate(password) {
        let (code, message) = match error {
            PasswordError::Empty => (Code::Required, "No password provided"),
            _ => (
                Code::PasswordShort,
                "Password must be at least 8 characters",
            ),
        };
        report.reject(field, code, message);
    }
    if password.len() > MAX_BYTES {
        report.reject(
            field,
            Code::PasswordLong,
            "Password exceeds the supported length",
        );
    }
    report
}

/// Confirmation is a separate field; no secret values enter diagnostics.
pub fn replacement(candidate: &str, confirmation: &str) -> crate::validation::Report {
    let mut report = validation(candidate, "password");
    if candidate != confirmation {
        report.reject(
            "confirmation",
            crate::validation::Code::PasswordMismatch,
            "Passwords do not match",
        );
    }
    report
}
