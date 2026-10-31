//! Complete campaign read contracts shared by HTTP encoders and browser queries.
pub mod draft;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::nullable;

/// Bounded execution metadata page.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Overview {
    /// Campaign headers in the requested page.
    pub items: Vec<Header>,
    /// Next execution cursor, or no further page.
    #[serde(deserialize_with = "nullable")]
    pub next_after: Option<i64>,
    /// Whether this process runs delivery workers.
    pub mailer_enabled: bool,
}

/// Bounded combined draft/execution page.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Directory {
    /// Rows belonging to the requested filter and cursor.
    pub items: Vec<DirectoryRow>,
    /// Continuation only when more rows exist.
    #[serde(deserialize_with = "nullable")]
    pub next_after: Option<String>,
}

/// Campaign navigation without content or service credentials.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DirectoryRow {
    /// Draft identity or decimal execution identity.
    pub id: String,
    /// Conditional draft revision; null for executions.
    #[serde(deserialize_with = "nullable")]
    pub revision: Option<i64>,
    /// Display name; drafts may be untitled.
    pub name: String,
    /// Entry kind selects the corresponding lifecycle.
    #[serde(flatten)]
    pub state: ruphish_domain::campaign::DirectoryState,
    /// Confirmed audience size; null before launch.
    #[serde(deserialize_with = "nullable")]
    pub total: Option<i64>,
    /// Actual schedule; null for drafts.
    #[serde(deserialize_with = "nullable")]
    pub launch_date: Option<String>,
}

/// Saved resource identity, including unavailable references.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Resource {
    /// Saved resource ID.
    pub id: i64,
    /// Current name; null when unavailable.
    #[serde(deserialize_with = "nullable")]
    pub name: Option<String>,
}

/// Confirmed presence-only tracking and redirect policy.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Policy {
    /// Whether email-open observations are enabled.
    pub track_opens: bool,
    /// Post-submission redirect.
    pub redirect_url: String,
    /// Selected owned learning page; custom redirect must be empty when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learning_page_id: Option<i64>,
}

/// Campaign metadata shown beside delivery status.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Header {
    /// Execution identity.
    pub id: i64,
    /// Display name.
    pub name: String,
    /// Lifecycle state.
    pub status: ruphish_domain::campaign::CampaignStatus,
    /// Preserved timestamp.
    #[serde(deserialize_with = "nullable")]
    pub launch_date: Option<String>,
    /// Distribution end; null when absent.
    #[serde(deserialize_with = "nullable")]
    pub send_by_date: Option<String>,
    /// Public campaign URL.
    pub url: String,
    /// Selected email template.
    pub template: Resource,
    /// Selected landing page.
    pub page: Resource,
    /// Selected sending profile.
    pub smtp: Resource,
    /// Current subject; null when unavailable.
    #[serde(deserialize_with = "nullable")]
    pub subject: Option<String>,
    /// Current sender; null when unavailable.
    #[serde(deserialize_with = "nullable")]
    pub profile_sender: Option<String>,
    /// Current override; null when unavailable.
    #[serde(deserialize_with = "nullable")]
    pub template_sender: Option<String>,
    /// Confirmed audience size.
    pub total: i64,
    /// Effective policy; null for unsupported legacy state.
    #[serde(deserialize_with = "nullable")]
    pub policy: Option<Policy>,
    /// Bound domain identity.
    #[serde(deserialize_with = "nullable")]
    pub domain_id: Option<i64>,
    /// Current domain eligibility failure.
    #[serde(deserialize_with = "nullable")]
    pub domain_block: Option<String>,
}

/// One consistent delivery page and campaign-wide snapshot.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Delivery {
    /// Selection-bound continuation for explicit sorted queries.
    #[serde(default)]
    pub next_cursor: Option<String>,
    /// Campaign metadata.
    pub campaign: Header,
    /// Sparse state counts: omitted states have zero rows; the map itself is required.
    pub counts: BTreeMap<ruphish_domain::campaign::DeliveryState, i64>,
    /// Recipients in the requested page.
    pub items: Vec<DeliveryRow>,
    /// Next recipient cursor.
    #[serde(deserialize_with = "nullable")]
    pub next_after: Option<i64>,
    /// Effective presence-only policy.
    #[serde(deserialize_with = "nullable")]
    pub policy: Option<Policy>,
    /// Whether this process runs delivery workers.
    pub mailer_enabled: bool,
    /// Most recent campaign events.
    pub activity: Vec<Activity>,
}

/// Current recipient delivery state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeliveryRow {
    /// Internal recipient identity.
    pub id: i64,
    /// Opaque public recipient identifier.
    pub rid: String,
    /// Administrator-supplied recipient address.
    pub email: String,
    /// Total persisted attempts.
    pub attempt_count: i64,
    /// Transport state.
    pub state: ruphish_domain::campaign::DeliveryState,
    /// Next or last due UTC seconds.
    pub due: i64,
    /// Automatic retries consumed.
    pub retries: i64,
    /// Conditional mutation generation.
    pub generation: i64,
    /// Safe diagnostic code.
    pub code: String,
    /// Bounded inline attempt records.
    pub attempts: Vec<Attempt>,
}

/// Confirmed delivery attempt metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Attempt {
    /// Attempt identity.
    pub generation: i64,
    /// UTC start seconds.
    pub started: i64,
    /// UTC completion seconds if finished.
    #[serde(deserialize_with = "nullable")]
    pub finished: Option<i64>,
    /// Recorded outcome.
    pub state: ruphish_domain::campaign::AttemptState,
    /// Protocol stage if known.
    pub stage: String,
    /// SMTP response if known.
    #[serde(deserialize_with = "nullable")]
    pub smtp_code: Option<i64>,
    /// Delivery correlation identifier.
    pub message_id: String,
}

/// Sanitized campaign activity.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Activity {
    /// Immutable coarse request categories, when observed.
    pub client: Option<ruphish_domain::client::Info>,
    /// Event identity.
    pub id: i64,
    /// Preserved timestamp.
    #[serde(deserialize_with = "nullable")]
    pub time: Option<String>,
    /// Administrator-supplied recipient address.
    pub email: String,
    /// Event kind.
    pub message: String,
    /// Sanitized event metadata, never participant values.
    pub details: String,
}

/// A page of independently accumulated recipient history.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct History {
    /// Recipient identity.
    pub result_id: i64,
    /// Administrator-supplied address.
    pub email: String,
    /// Attempt page.
    pub attempts: Vec<Attempt>,
    /// Event page.
    pub events: Vec<HistoryEvent>,
    /// Older-attempt continuation.
    #[serde(deserialize_with = "nullable")]
    pub next_attempt: Option<i64>,
    /// Older-event continuation.
    #[serde(deserialize_with = "nullable")]
    pub next_event: Option<i64>,
}

/// Sanitized historical event with optional normalized time.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryEvent {
    /// Current event values, origin and edit state.
    pub record: ruphish_domain::event_edit::Record,
    /// Event identity.
    pub id: i64,
    /// Preserved timestamp.
    #[serde(deserialize_with = "nullable")]
    pub time: Option<String>,
    /// UTC seconds if parseable.
    #[serde(deserialize_with = "nullable")]
    pub at: Option<i64>,
    /// Event kind.
    pub message: String,
    /// Sanitized metadata.
    pub details: String,
}

/// Consistent results snapshot for one campaign, filter and page.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Results {
    /// Whether open counts represent an enabled observation channel.
    pub track_opens: bool,
    /// First observed UTC second per metric, before pagination; absent without valid evidence.
    pub first_observed: crate::reporting::ObservationTimes,
    /// Latest observed UTC second per metric, before pagination.
    pub last_observed: crate::reporting::ObservationTimes,

    /// Campaign identity.
    pub id: i64,
    /// Campaign display name.
    pub name: String,
    /// Lifecycle state.
    pub status: ruphish_domain::campaign::CampaignStatus,
    /// Snapshot UTC seconds.
    pub as_of: i64,
    /// Unfiltered audience size.
    pub audience: i64,
    /// Unique recipients per observed event type.
    pub unique: crate::reporting::Counts,
    /// Cumulative recipient progression.
    pub progression: crate::reporting::Counts,
    /// Effective event counts.
    pub totals: crate::reporting::Counts,
    /// Matching recipients before pagination.
    pub recipient_total: i64,
    /// Matching events before pagination.
    pub event_total: i64,
    /// Recipient page.
    pub recipients: Vec<ResultRecipient>,
    /// Event page.
    pub events: Vec<ResultEvent>,
    /// Chart intervals.
    pub buckets: Vec<Bucket>,
    /// Cumulative distinct participants per action, independent of pagination.
    pub participant_buckets: Vec<Bucket>,
    /// Interval width in seconds.
    pub bucket_seconds: i64,
}

/// Observed counts for one recipient.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResultRecipient {
    /// Recipient identity.
    pub id: i64,
    /// Administrator-supplied address.
    pub email: String,
    /// Administrator-supplied name.
    pub name: String,
    /// Transport outcome, or null when no native delivery record exists.
    #[serde(deserialize_with = "nullable")]
    pub delivery: Option<ruphish_domain::campaign::DeliveryState>,
    /// Observed event counts.
    pub counts: crate::reporting::Counts,
    /// Legacy report state without an observed timestamp.
    pub historical_reported: bool,
}

/// One effective observation; no participant-entered values.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResultEvent {
    /// Immutable coarse request categories; absent for historical or non-client observations.
    pub client: Option<ruphish_domain::client::Info>,
    /// Whether an operator has changed the event values since creation.
    pub edited: bool,
    /// Event identity.
    pub id: i64,
    /// Recipient when resolvable.
    #[serde(deserialize_with = "nullable")]
    pub recipient_id: Option<i64>,
    /// Administrator-supplied address.
    pub email: String,
    /// Normalized UTC timestamp.
    pub time: String,
    /// Event kind.
    pub kind: ruphish_domain::events::RecordedKind,
    /// Observation provenance.
    pub source: ruphish_domain::reporting::Source,
    /// Presence only, when recorded.
    #[serde(deserialize_with = "nullable")]
    pub password_present: Option<bool>,
    /// Username presence when explicitly recorded, never inferred from addressing.
    #[serde(deserialize_with = "nullable")]
    pub username_present: Option<bool>,
}

/// One chart interval.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bucket {
    /// UTC bucket start.
    pub time: i64,
    /// Observed event counts.
    pub counts: crate::reporting::Counts,
}

/// Persisted draft snapshot; configuration may deliberately be incomplete.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Draft {
    /// Client-generated draft identity.
    pub id: String,
    /// Conditional-write revision.
    pub revision: i64,
    /// Complete saved structure whose values may still be unfinished; saving never authorizes launch.
    pub configuration: draft::Configuration,
    /// Last editing step.
    pub step: i64,
    /// UTC creation timestamp.
    #[serde(deserialize_with = "nullable")]
    pub created: Option<String>,
    /// UTC modification timestamp.
    #[serde(deserialize_with = "nullable")]
    pub modified: Option<String>,
    /// Launched execution identity, if any.
    #[serde(deserialize_with = "nullable")]
    pub campaign_id: Option<i64>,
    /// Draft or launched.
    pub status: ruphish_domain::campaign_draft::Status,
}

/// Bounded saved-draft page.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Drafts {
    /// Complete saved snapshots, including a legitimate empty array.
    pub items: Vec<Draft>,
    /// Next draft identity, or no further page.
    #[serde(deserialize_with = "nullable")]
    pub next_after: Option<String>,
}

/// Successful removal of a saved draft at its requested revision.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DraftRemoval {
    /// Whether the requested removal completed.
    pub success: bool,
}

#[cfg(test)]
#[path = "../tests/support/campaign_reads.rs"]
mod tests;
