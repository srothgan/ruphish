//! Typed campaign commands shared by browser and programmable clients.
use crate::legacy::Timestamp;
use serde::{Deserialize, Serialize};

/// Owner-scoped resource identity, optionally bound to a reviewed revision.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewedResource {
    /// Owner-scoped persistent resource ID.
    pub id: i64,
    /// Revision shown during review, if already reviewed.
    pub revision: Option<String>,
}
/// Ordered audience and content selections for preparation and execution.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selections {
    /// Ordered audience collections.
    pub groups: Vec<ReviewedResource>,
    /// Email content.
    pub template: ReviewedResource,
    /// Landing page.
    pub page: ReviewedResource,
    /// Sending profile.
    pub smtp: ReviewedResource,
}
/// Optional redirect defaults.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(default)]
pub struct Policy {
    /// Post-submission redirect override.
    pub redirect_url: Option<String>,
    /// Selected owned learning page; custom redirect must be empty when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learning_page_id: Option<i64>,
}
/// Local time entry resolved authoritatively on the server.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(default)]
pub struct LocalTime {
    /// Civil date/time without an offset.
    pub local: String,
    /// IANA time-zone name.
    pub time_zone: String,
}
/// Campaign input shared by preparation, creation and test sends.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Create {
    /// Campaign display name.
    pub name: String,
    /// Public URL template.
    pub url: String,
    /// Owner-scoped resource IDs and optional reviewed revisions.
    pub selection: Selections,
    /// UTC or offset launch; exclusive with launch_local. Both absent means now.
    pub launch_date: Option<Timestamp>,
    /// UTC or offset distribution end; exclusive with send_by_local.
    pub send_by_date: Option<Timestamp>,
    /// Optional redirect defaults.
    pub policy: Option<Policy>,
    /// Named-zone launch input; exclusive with launch_date.
    pub launch_local: Option<LocalTime>,
    /// Named-zone distribution end; exclusive with send_by_date.
    pub send_by_local: Option<LocalTime>,
}

/// Launch an unchanged saved draft using the same campaign input as preparation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchDraft {
    /// Saved draft revision reviewed by the caller.
    pub revision: i64,
    /// Reviewed campaign command.
    pub campaign: Create,
}

/// Send a reviewed campaign sample to one administrator-selected recipient.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Test {
    /// Reviewed campaign command.
    pub campaign: Create,
    /// Test recipient mailbox.
    pub email: String,
}

impl From<&crate::campaign_workflow::Selection<crate::campaign_workflow::ReviewedResource>>
    for Selections
{
    fn from(
        value: &crate::campaign_workflow::Selection<crate::campaign_workflow::ReviewedResource>,
    ) -> Self {
        let selected = |resource: &crate::campaign_workflow::ReviewedResource| ReviewedResource {
            id: resource.id,
            revision: Some(resource.revision.clone()),
        };
        Self {
            groups: value.groups.iter().map(selected).collect(),
            template: selected(&value.template),
            page: selected(&value.page),
            smtp: selected(&value.smtp),
        }
    }
}
