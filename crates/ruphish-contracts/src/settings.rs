//! Browser-safe installation policy and danger-action responses.
pub use ruphish_domain::settings::{ClosurePreview, ClosureResult, Features, Policy, Settings};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Complete policy replacement with revision supplied separately.
pub struct Edit {
    /// Complete installation policy.
    pub policy: Policy,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
/// Explicit confirmation of the reviewed installation-wide closure.
pub struct CloseCampaigns {
    /// Opaque actor-bound confirmation identity.
    pub token: String,
    /// Required literal CLOSE ALL confirmation.
    pub confirmation: String,
}
