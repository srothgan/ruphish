//! Request contracts shared by HTTP handlers and browser clients.
use serde::{Deserialize, Serialize};

/// Create an editable campaign draft with an Idempotency-Key header.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftCreate {
    /// Editable draft configuration.
    pub configuration: ruphish_domain::campaign::draft::Configuration,
    /// Current editing step.
    pub step: i64,
}

/// Replace an existing editable campaign draft.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftWrite {
    /// Expected saved revision; missing revisions are rejected.
    pub revision: Option<i64>,
    /// Editable draft configuration.
    pub configuration: ruphish_domain::campaign::draft::Configuration,
    /// Current editing step.
    pub step: i64,
}

/// Draft-list continuation.
#[derive(Serialize, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DraftPage {
    /// Continuation returned by the previous page.
    pub after: i64,
}

/// Combined campaign directory query.
#[derive(Serialize, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DirectoryPage {
    /// Continuation returned by the previous page.
    pub after: String,
    /// Requested campaign category.
    pub filter: String,
    /// Optional sort field.
    pub sort: Option<ruphish_domain::pagination::DirectorySort>,
    /// Optional sort direction.
    pub order: Option<ruphish_domain::reporting::SortDirection>,
}

/// Execution overview query.
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct OverviewPage {
    /// Continuation returned by the previous page.
    pub after: i64,
    /// Requested page size.
    pub limit: Option<i64>,
    /// Optional completion filter.
    pub completed: Option<bool>,
}

/// Delivery-list query.
#[derive(Serialize, Default, Deserialize)]
#[serde(default)]
pub struct DeliveryPage {
    /// Continuation returned by the previous page.
    pub after: i64,
    /// Requested page size.
    pub limit: Option<i64>,
    /// Optional sort field.
    pub sort: Option<ruphish_domain::pagination::DeliverySort>,
    /// Optional sort direction.
    pub order: Option<ruphish_domain::reporting::SortDirection>,
    /// Opaque sorted continuation token.
    pub cursor: String,
}

/// Recipient-history query.
#[derive(Serialize, Deserialize)]
pub struct HistoryPage {
    /// Exclusive attempt-history cursor.
    pub before_attempt: Option<i64>,
    /// Exclusive event-history cursor.
    pub before_event: Option<i64>,
}

/// Resolve an uncertain delivery attempt.
#[derive(Serialize, Deserialize)]
pub struct Resolve {
    /// Delivery generation reviewed by the operator.
    pub generation: i64,
    /// Requested delivery resolution.
    pub action: String,
}
