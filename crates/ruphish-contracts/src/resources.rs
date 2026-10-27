//! Resource metadata responses and explicit test-delivery inputs.
use serde::{Deserialize, Serialize};

/// Recipient-group summary response.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GroupSummaries {
    /// Total saved groups.
    pub total: usize,
    /// Confirmed group summaries, including a legitimate empty array.
    pub groups: Vec<GroupSummary>,
}

/// Group metadata without recipient payloads.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GroupSummary {
    /// Persistent ID.
    pub id: i64,
    /// Saved name.
    pub name: String,
    /// Modification instant in the preserved wire format.
    #[serde(deserialize_with = "crate::nullable")]
    pub modified_date: Option<String>,
    /// Confirmed recipient count.
    pub num_targets: usize,
}

/// Template, landing-page or sending-profile summary response.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ResourceSummaries {
    /// Total saved resources.
    pub total: usize,
    /// Confirmed summaries.
    pub items: Vec<ResourceSummary>,
}

/// Resource metadata without bodies, attachments or service credentials.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ResourceSummary {
    /// Persistent ID.
    pub id: i64,
    /// Saved name.
    pub name: String,
    /// Modification instant in the preserved wire format.
    #[serde(deserialize_with = "crate::nullable")]
    pub modified_date: Option<String>,
    /// Unfinished campaigns referencing this resource.
    pub active_campaigns: usize,
}

/// Imported authored HTML, without a saved resource identity.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct HtmlImport {
    /// Imported source.
    pub html: String,
}

/// Imported email content, before it is saved as a template.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EmailImport {
    /// Subject expression.
    pub subject: String,
    /// Plain-text source.
    pub text: String,
    /// HTML source.
    pub html: String,
}

/// Prepared page preview and advisory diagnostics.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PagePreview {
    /// Prepared HTML.
    pub html: String,
    /// Findings, including a required empty list when no issue is found.
    pub warnings: Vec<crate::resource_details::PageWarning>,
}

/// Advisory validation without a prepared document.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PageDiagnostics {
    /// Findings, including a required empty list when no issue is found.
    pub warnings: Vec<crate::resource_details::PageWarning>,
}

/// CSV import review before a group is persisted.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CsvReview {
    /// Valid administrator-supplied recipient rows.
    pub targets: Vec<crate::resource_details::Recipient>,
    /// Available source columns.
    pub columns: Vec<String>,
    /// Rejected source rows.
    pub errors: Vec<CsvError>,
}

/// One rejected CSV source row.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CsvError {
    /// Source row number.
    pub row: u64,
    /// Stable rejection code.
    pub code: crate::errors::ErrorCode,
    /// Human-readable diagnostic.
    pub message: String,
    /// Source values supplied by the administrator.
    pub fields: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_valid_empty_collections_are_empty_lists() {
        for invalid in [
            json!({}),
            json!({"total":0}),
            json!({"total":0,"groups":null}),
            json!({"total":0,"groups":{}}),
        ] {
            assert!(serde_json::from_value::<GroupSummaries>(invalid).is_err());
        }
        assert!(serde_json::from_value::<GroupSummaries>(json!({"total":0,"groups":[]})).is_ok());
        assert!(serde_json::from_value::<ResourceSummaries>(json!({"total":0,"items":[]})).is_ok());
        assert!(
            serde_json::from_value::<ResourceSummaries>(json!({"total":0,"groups":[]})).is_err()
        );
    }

    #[test]
    fn missing_counts_and_identity_are_not_displayed_as_zero() {
        let group =
            json!({"id":4,"name":"Example","modified_date":"2026-10-21T12:00:00Z","num_targets":0});
        let resource = json!({"id":4,"name":"Example","modified_date":"2026-10-21T12:00:00Z","active_campaigns":0});
        for field in ["id", "name", "modified_date", "num_targets"] {
            let mut row = group.clone();
            row.as_object_mut().map(|o| o.remove(field));
            assert!(serde_json::from_value::<GroupSummary>(row).is_err());
        }
        let mut row = resource;
        row.as_object_mut().map(|o| o.remove("active_campaigns"));
        assert!(serde_json::from_value::<ResourceSummary>(row).is_err());
        assert!(serde_json::from_value::<GroupSummary>(group).is_ok());
    }
}

/// Authored static learning-page input; public identity is always server owned.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct LearningPageInput {
    /// Existing resource ID; ignored when creating.
    #[serde(default)]
    pub id: i64,
    /// Owner-scoped resource name.
    pub name: String,
    /// Static authored HTML.
    pub html: String,
}

/// Explicit SMTP selection for a test message; never inferred from failed validation.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "source", rename_all = "snake_case", deny_unknown_fields)]
pub enum TestProfile {
    /// Use the current owner-scoped saved configuration and credential.
    Saved {
        /// Saved sending-profile identifier.
        id: i64,
    },
    /// Test supplied settings. A nonzero profile ID resolves an omitted password from that owned profile.
    Draft {
        /// Editable SMTP settings and optional replacement credential.
        profile: crate::legacy::SendingProfile,
    },
}
