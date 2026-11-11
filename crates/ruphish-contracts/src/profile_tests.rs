//! Saved-profile operational tests; only the email operation accepts a recipient.
pub use ruphish_domain::profile_tests::{Attempt, Kind, Outcome, Summary};
use serde::{Deserialize, Serialize};

/// Revision-checked request with a reusable identity for uncertain responses.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// Unique operation identity generated once for this user action.
    pub id: String,
    /// Reviewed profile resource revision.
    pub revision: i64,
    /// Operation to perform.
    pub kind: Kind,
    /// Required only for an email test.
    pub email: Option<String>,
}

/// History with live or deleted operator display identities.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct History {
    /// Current policy and diagnostic evidence.
    #[serde(flatten)]
    pub summary: Summary,
    /// Batched display identities for visible attempts.
    pub operators: Vec<crate::attribution::UserReference>,
}
