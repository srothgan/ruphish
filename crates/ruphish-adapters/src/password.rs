//! Verification and password changes compatible with existing Go bcrypt hashes.

use ruphish_domain::password::{self, PasswordError};

/// Failure to validate or hash a password change.
#[derive(Debug, thiserror::Error)]
pub enum ChangeError {
    /// A domain policy, confirmation, or reuse failure.
    #[error(transparent)]
    Policy(#[from] PasswordError),
    /// Go refuses to generate hashes for passwords longer than 72 bytes.
    #[error("bcrypt: password length exceeds 72 bytes")]
    TooLong,
    /// Randomness or bcrypt encoding failed.
    #[error(transparent)]
    Hash(#[from] bcrypt::BcryptError),
}

/// Verify an existing hash without changing it; malformed hashes are errors.
///
/// Run this CPU-bound operation on a blocking worker when wiring HTTP authentication.
pub fn verify(password: &str, hash: &str) -> Result<bool, bcrypt::BcryptError> {
    bcrypt::verify(password.as_bytes(), hash)
}

/// Validate a change and generate a salted hash at the inherited cost of ten.
///
/// The caller must first authenticate the current password where required.
pub fn change(
    current_hash: &str,
    candidate: &str,
    confirmation: &str,
) -> Result<String, ChangeError> {
    password::validate(candidate)?;
    if candidate != confirmation {
        return Err(PasswordError::Mismatch.into());
    }
    // Go permits replacing an unreadable stored hash after caller authentication.
    if verify(candidate, current_hash).unwrap_or(false) {
        return Err(PasswordError::Reused.into());
    }
    if candidate.len() > 72 {
        return Err(ChangeError::TooLong);
    }
    Ok(bcrypt::hash(candidate.as_bytes(), 10)?)
}
