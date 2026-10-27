//! Complete preparation and duplication responses, separate from permissive writes.
use crate::{campaign_reads::Policy, nullable};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
/// Complete resource selections for a review or editable copy.
pub struct Selection<T> {
    /// Ordered audience selections.
    pub groups: Vec<T>,
    /// Email template selection.
    pub template: T,
    /// Landing-page selection.
    pub page: T,
    /// Sending-profile selection.
    pub smtp: T,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
/// Resource identity retained when duplicating a campaign.
pub struct Reference {
    /// Persistent resource identifier.
    pub id: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
/// Resource identity and revision observed during review.
pub struct ReviewedResource {
    /// Persistent resource identifier.
    pub id: i64,
    /// Display name.
    pub name: String,
    /// Server revision used for conditional launch.
    pub revision: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
/// Recipient attributes used to explain a duplicate conflict.
pub struct RecipientAttributes {
    /// Recipient given name.
    pub first_name: String,
    /// Recipient family name.
    pub last_name: String,
    /// Recipient position.
    pub position: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
/// Conflicting audience records, preserving first-record precedence.
pub struct Conflict {
    /// Recipient mailbox identity.
    pub email: String,
    /// Attributes retained from the first occurrence.
    pub kept: RecipientAttributes,
    /// Conflicting attributes from a later occurrence.
    pub discarded: RecipientAttributes,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
/// Rendered sample of the reviewed email.
pub struct Preview {
    /// Rendered subject.
    pub subject: String,
    /// Rendered HTML body.
    pub html: String,
    /// Rendered plain-text body.
    pub text: String,
    /// Rendered visible From address.
    pub visible_sender: String,
    /// SMTP envelope sender.
    pub transport_sender: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
/// Complete server-confirmed launch review.
pub struct Review {
    /// Advisory findings on the processed landing page.
    #[serde(default)]
    pub warnings: Vec<crate::resource_details::PageWarning>,
    /// Number of unique recipients.
    pub total: usize,
    /// Number of discarded duplicate entries.
    pub duplicate_count: usize,
    /// Resolved launch time in UTC seconds.
    pub launch_date: i64,
    #[serde(deserialize_with = "nullable")]
    /// Optional distribution end in UTC seconds.
    pub send_by_date: Option<i64>,
    /// Resolved resource selections.
    pub selection: Selection<ReviewedResource>,
    /// Effective tracking policy confirmed by review.
    pub policy: Policy,
    /// Whether delivery is enabled by startup configuration.
    pub mailer_enabled: bool,
    /// Total conflicts including those beyond the displayed sample.
    pub conflict_count: usize,
    /// Bounded conflicting recipient sample.
    pub conflicts: Vec<Conflict>,
    /// Rendered sample using the first recipient.
    pub preview: Preview,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
/// Editable campaign copy; null policy means the historical policy is unknown.
pub struct Duplicate {
    /// Display name.
    pub name: String,
    /// Authored campaign URL.
    pub url: String,
    /// Resolved resource selections.
    pub selection: Selection<Reference>,
    #[serde(deserialize_with = "nullable")]
    /// Effective tracking policy, or unknown legacy policy for a copy.
    pub policy: Option<Policy>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn review_requires_recipient_counts_versions_policy_schedule_and_preview() {
        let value = json!({"total":1,"duplicate_count":0,"launch_date":123,"send_by_date":null,
            "selection":{"groups":[{"id":1,"name":"Audience","revision":"v1"}],"template":{"id":2,"name":"Email","revision":"v2"},"page":{"id":3,"name":"Page","revision":"v3"},"smtp":{"id":4,"name":"SMTP","revision":"v4"}},
            "policy":{"redirect_url":""},"mailer_enabled":true,"conflict_count":0,"conflicts":[],
            "preview":{"subject":"Subject","html":"<p>Preview</p>","text":"Preview","visible_sender":"Sender <sender@example.test>","transport_sender":"bounce@example.test"}});
        assert!(serde_json::from_value::<Review>(value.clone()).is_ok());
        for field in value.as_object().expect("fixture").keys() {
            let mut missing = value.clone();
            missing.as_object_mut().map(|v| v.remove(field));
            assert!(
                serde_json::from_value::<Review>(missing.clone()).is_err(),
                "{field}"
            );
        }
        for pointer in [
            "/selection/template",
            "/selection/groups/0",
            "/policy",
            "/preview",
        ] {
            let mut missing = value.clone();
            *missing.pointer_mut(pointer).expect("fixture") = json!({});
            assert!(
                serde_json::from_value::<Review>(missing.clone()).is_err(),
                "{pointer}"
            );
        }
    }

    #[test]
    fn duplication_preserves_explicit_unknown_policy_but_rejects_lost_selections() {
        let value = json!({"name":"Copy","url":"https://example.test","selection":{"groups":[{"id":1}],"template":{"id":2},"page":{"id":0},"smtp":{"id":4}},"policy":null});
        let decoded = serde_json::from_value::<Duplicate>(value.clone()).expect("fixture");
        assert!(decoded.policy.is_none());
        assert_eq!(serde_json::to_value(decoded).expect("wire response"), value);
        let mut known = value.clone();
        known["policy"] = json!({"redirect_url":"https://example.test/after"});
        let decoded = serde_json::from_value::<Duplicate>(known.clone()).expect("known policy");
        assert!(decoded.policy.is_some());
        assert_eq!(serde_json::to_value(decoded).expect("wire response"), known);
        for field in ["name", "url", "selection", "policy"] {
            let mut missing = value.clone();
            missing.as_object_mut().map(|v| v.remove(field));
            assert!(serde_json::from_value::<Duplicate>(missing.clone()).is_err());
        }
        for pointer in ["/selection/template", "/selection/groups/0"] {
            let mut missing = value.clone();
            *missing.pointer_mut(pointer).expect("fixture") = json!({});
            assert!(serde_json::from_value::<Duplicate>(missing.clone()).is_err());
        }
        for policy in [json!({}), json!({"redirect_url":false})] {
            let mut missing = value.clone();
            missing["policy"] = policy;
            assert!(serde_json::from_value::<Duplicate>(missing.clone()).is_err());
        }
    }
}
