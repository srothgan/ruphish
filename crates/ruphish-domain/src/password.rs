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
