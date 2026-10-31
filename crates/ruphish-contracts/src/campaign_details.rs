//! Complete campaign, result and summary response contracts.
use crate::{nullable, resource_details as resource};
use serde::{Deserialize, Serialize};

/// Campaign identity, lifecycle and preserved schedule metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Metadata {
    /// Whether this campaign collects email-open observations.
    pub track_opens: bool,
    /// Execution identity.
    pub id: i64,
    /// Display name.
    pub name: String,
    /// Campaign lifecycle.
    pub status: ruphish_domain::campaign::CampaignStatus,
    /// Creation timestamp, or explicitly unknown.
    #[serde(deserialize_with = "nullable")]
    pub created_date: Option<String>,
    /// Launch timestamp, or explicitly unknown.
    #[serde(deserialize_with = "nullable")]
    pub launch_date: Option<String>,
    /// Distribution end, or no deadline.
    #[serde(deserialize_with = "nullable")]
    pub send_by_date: Option<String>,
    /// Completion timestamp, or not completed.
    #[serde(deserialize_with = "nullable")]
    pub completed_date: Option<String>,
}

/// Current referenced resources and the immutable campaign audience.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Campaign {
    /// Shared execution metadata.
    #[serde(flatten)]
    pub metadata: Metadata,
    /// Public campaign URL.
    pub url: String,
    /// Current template; unavailable resources have explicit empty records.
    pub template: resource::Resource<resource::Template>,
    /// Current landing page.
    pub page: resource::Resource<resource::Page>,
    /// Current SMTP settings, never the stored password.
    pub smtp: resource::Resource<resource::Profile>,
    /// Audience snapshots; an empty audience is omitted in this projection.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub results: Vec<Recipient>,
    /// Committed history; omitted when empty and in creation acknowledgements.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timeline: Vec<Event>,
    /// Source groups supplied only in creation acknowledgements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groups: Option<Vec<resource::Resource<resource::Group>>>,
}

/// One audience snapshot and its projected engagement status.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Recipient {
    /// Opaque public recipient identifier.
    pub id: String,
    /// Administrator-supplied recipient attributes.
    #[serde(flatten)]
    pub recipient: resource::Recipient,
    /// Current engagement projection, distinct from delivery state.
    pub status: String,
    /// Sanitized historical address.
    pub ip: String,
    /// Sanitized historical latitude.
    pub latitude: f64,
    /// Sanitized historical longitude.
    pub longitude: f64,
    /// Independent reported flag.
    pub reported: bool,
    /// Scheduled send timestamp, or explicitly unknown.
    #[serde(deserialize_with = "nullable")]
    pub send_date: Option<String>,
    /// Last change, or explicitly unknown.
    #[serde(deserialize_with = "nullable")]
    pub modified_date: Option<String>,
}

/// Sanitized event history, retaining meaningful imported event names.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    /// Owning execution.
    pub campaign_id: i64,
    /// Administrator-supplied recipient address, empty for campaign-wide events.
    pub email: String,
    /// Recorded time, or explicitly unknown.
    #[serde(deserialize_with = "nullable")]
    pub time: Option<String>,
    /// Recorded event name.
    pub message: String,
    /// Sanitized string-encoded metadata; never participant-entered values.
    pub details: String,
}

/// Campaign-wide counts derived from the authoritative progression rules.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Counts {
    /// Audience size.
    pub total: usize,
    /// Recorded SMTP acceptances.
    pub sent: i64,
    /// Cumulative opens.
    pub opened: i64,
    /// Cumulative clicks.
    pub clicked: i64,
    /// Recorded submissions.
    pub submitted_data: i64,
    /// Independent reports.
    pub email_reported: i64,
    /// Failed recipients.
    pub error: usize,
}

/// One campaign summary without content or credential settings.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Summary {
    /// Execution metadata.
    #[serde(flatten)]
    pub metadata: Metadata,
    /// Audience and progression counts.
    pub stats: Counts,
}

/// Owner-scoped campaign summaries.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Summaries {
    /// Number of returned campaigns.
    pub total: usize,
    /// Confirmed summaries, including a legitimate empty array.
    pub campaigns: Vec<Summary>,
}
