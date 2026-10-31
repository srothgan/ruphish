//! Structurally complete saved configuration, distinct from permissive draft input.
use ruphish_domain::campaign_draft as input;
use serde::{Deserialize, Serialize};

/// Saved fields are required even when their values are intentionally incomplete.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    /// Display name; an empty value is a legitimate unfinished draft.
    pub name: String,
    /// Destination; an empty value is a legitimate unfinished draft.
    pub url: String,
    /// Complete selection structure, including unselected identities.
    pub selection: Selections,
    /// Explicit null means the page defaults have not been resolved.
    #[serde(deserialize_with = "crate::nullable")]
    pub policy: Option<Policy>,
    /// Explicit null requests immediate delivery at launch.
    #[serde(deserialize_with = "crate::nullable")]
    pub launch_local: Option<LocalTime>,
    /// Explicit null means no distribution deadline.
    #[serde(deserialize_with = "crate::nullable")]
    pub send_by_local: Option<LocalTime>,
}

/// Saved selection identity; zero is explicitly unselected, not a missing field.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    /// Persistent ID or zero for an unfinished selection.
    pub id: i64,
}

/// Complete saved selection structure; no resource content or service credentials.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selections {
    /// Ordered audience groups, possibly empty.
    pub groups: Vec<Reference>,
    /// Template identity or explicitly unselected.
    pub template: Reference,
    /// Landing-page identity or explicitly unselected.
    pub page: Reference,
    /// Sending-profile identity or explicitly unselected.
    pub smtp: Reference,
}

/// Saved local schedule fields; both may still contain unfinished input.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalTime {
    /// Civil date/time input.
    pub local: String,
    /// IANA time-zone input.
    pub time_zone: String,
}

/// Saved interaction policy; validity for execution remains a domain rule.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    /// Whether email-open observations are enabled.
    pub track_opens: bool,
    /// Explicit null means unresolved defaults; an empty string means no redirect.
    #[serde(deserialize_with = "crate::nullable")]
    pub redirect_url: Option<String>,
    /// Optional owned learning-page identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learning_page_id: Option<i64>,
}

impl From<&input::LocalTime> for LocalTime {
    fn from(value: &input::LocalTime) -> Self {
        Self {
            local: value.local.clone(),
            time_zone: value.time_zone.clone(),
        }
    }
}

impl From<&input::Configuration> for Configuration {
    fn from(value: &input::Configuration) -> Self {
        let reference = |value: &input::Reference| Reference { id: value.id };
        Self {
            name: value.name.clone(),
            url: value.url.clone(),
            selection: Selections {
                groups: value.selection.groups.iter().map(reference).collect(),
                template: reference(&value.selection.template),
                page: reference(&value.selection.page),
                smtp: reference(&value.selection.smtp),
            },
            policy: value.policy.as_ref().map(|policy| Policy {
                track_opens: policy.track_opens,
                redirect_url: policy.redirect_url.clone(),
                learning_page_id: policy.learning_page_id,
            }),
            launch_local: value.launch_local.as_ref().map(LocalTime::from),
            send_by_local: value.send_by_local.as_ref().map(LocalTime::from),
        }
    }
}
