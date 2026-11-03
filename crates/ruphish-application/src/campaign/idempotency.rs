//! Idempotent creation rules shared by campaign executions and saved drafts.
use crate::resources::Error;

/// Separate creation namespaces even when callers reuse a key across endpoints.
#[derive(Clone, Copy)]
pub enum Operation {
    /// Direct execution creation.
    Campaign,
    /// Incomplete draft creation.
    Draft,
}
impl Operation {
    /// Stable database discriminator.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Campaign => "campaign",
            Self::Draft => "draft",
        }
    }
}
/// Bound keys before database lookup on every provider.
pub fn validate(key: &str) -> Result<(), Error> {
    if key.is_empty() || key.len() > 128 {
        return Err(Error::Invalid("Invalid idempotency key".into()));
    }
    Ok(())
}
/// A key identifies one immutable creation request.
pub fn reconcile(expected: &str, stored: &str) -> Result<(), Error> {
    if expected != stored {
        return Err(Error::Conflict("Idempotency key"));
    }
    Ok(())
}
