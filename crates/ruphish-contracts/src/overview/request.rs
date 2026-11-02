//! Request contracts shared by HTTP handlers and browser clients.
use serde::{Deserialize, Serialize};

/// Reporting-window query.
#[derive(Serialize, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct WindowQuery {
    /// Requested reporting window.
    pub window: String,
    // Authentication consumes this existing API query parameter before validation.
    #[serde(rename = "api_key")]
    /// Optional query authentication credential.
    pub _api_key: Option<String>,
}
