//! Additive, browser-safe validation diagnostics in error envelopes.
pub use ruphish_domain::validation::{Code, Issue, MAX_ISSUES};
use serde::{Deserialize, Serialize};

/// Field failures, independent of transport and operational error classification.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReport {
    /// Bounded, value-free issues in evaluation order.
    pub issues: Vec<Issue>,
}
impl From<&ruphish_domain::validation::Report> for ValidationReport {
    fn from(report: &ruphish_domain::validation::Report) -> Self {
        Self {
            issues: report.issues().to_vec(),
        }
    }
}
