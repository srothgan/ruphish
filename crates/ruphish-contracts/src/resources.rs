//! Required fields in resource-list responses. Missing fields are not empty data.
use serde::Deserialize;

/// Recipient-group summary response.
#[derive(Clone, Debug, Deserialize)]
pub struct GroupSummaries {
    /// Total saved groups.
    pub total: usize,
    /// Confirmed group summaries, including a legitimate empty array.
    pub groups: Vec<GroupSummary>,
}

/// Group metadata without recipient payloads.
#[derive(Clone, Debug, Deserialize)]
pub struct GroupSummary {
    /// Persistent ID.
    pub id: i64,
    /// Saved name.
    pub name: String,
    /// Modification instant in the preserved wire format.
    pub modified_date: String,
    /// Confirmed recipient count.
    pub num_targets: usize,
}

/// Template, landing-page or sending-profile summary response.
#[derive(Clone, Debug, Deserialize)]
pub struct ResourceSummaries {
    /// Total saved resources.
    pub total: usize,
    /// Confirmed summaries.
    pub items: Vec<ResourceSummary>,
}

/// Resource metadata without bodies, attachments or service credentials.
#[derive(Clone, Debug, Deserialize)]
pub struct ResourceSummary {
    /// Persistent ID.
    pub id: i64,
    /// Saved name.
    pub name: String,
    /// Modification instant in the preserved wire format.
    pub modified_date: String,
    /// Unfinished campaigns referencing this resource.
    pub active_campaigns: usize,
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
