//! Browser-safe reporting requests. No connection credentials or participant payloads.
use serde::{Deserialize, Serialize};
/// URL filters shared by analysis and download links.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct Filter {
    /// Recipient row identity.
    pub recipient: Option<i64>,
    /// Comma-separated recipient identities for multi-selection; empty means all.
    pub recipients: String,
    /// Inclusive UTC seconds.
    pub from: Option<i64>,
    /// Exclusive UTC seconds.
    pub until: Option<i64>,
    /// Event name.
    pub event: String,
    /// Recipient search.
    pub search: String,
    /// Observation source.
    pub source: String,
    /// Row offset.
    pub offset: usize,
    /// Page length.
    pub limit: usize,
    /// Download format.
    pub format: String,
    /// Download scope.
    pub scope: String,
}
impl Default for Filter {
    fn default() -> Self {
        Self {
            recipient: None,
            recipients: String::new(),
            from: None,
            until: None,
            event: String::new(),
            search: String::new(),
            source: String::new(),
            offset: 0,
            limit: 50,
            format: "csv".into(),
            scope: "events".into(),
        }
    }
}
