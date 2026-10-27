//! Complete campaign read contracts shared by HTTP encoders and browser queries.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::nullable;

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
    /// Draft or campaign.
    pub kind: String,
    /// Conditional draft revision; null for executions.
    #[serde(deserialize_with = "nullable")]
    pub revision: Option<i64>,
    /// Display name; drafts may be untitled.
    pub name: String,
    /// Lifecycle state.
    pub status: String,
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
    /// Compatibility output, always true; values are never collected.
    pub capture_credentials: bool,
    /// Compatibility output, always true; only presence is observed.
    pub capture_passwords: bool,
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
    pub status: String,
    /// Preserved timestamp.
    pub launch_date: String,
    /// Distribution end; Go zero time when absent.
    pub send_by_date: String,
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
    pub counts: BTreeMap<String, i64>,
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
    pub state: String,
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
    pub state: String,
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
    /// Event identity.
    pub id: i64,
    /// Preserved timestamp.
    pub time: String,
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
    /// Event identity.
    pub id: i64,
    /// Preserved timestamp.
    pub time: String,
    /// UTC seconds if parseable.
    #[serde(deserialize_with = "nullable")]
    pub at: Option<i64>,
    /// Event kind.
    pub message: String,
    /// Sanitized metadata.
    pub details: String,
}

/// Consistent analysis snapshot for one campaign, filter and page.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Analysis {
    /// Campaign identity.
    pub id: i64,
    /// Campaign display name.
    pub name: String,
    /// Lifecycle state.
    pub status: String,
    /// Snapshot UTC seconds.
    pub as_of: i64,
    /// Unfiltered audience size.
    pub audience: i64,
    /// Unique recipients per observed event type.
    pub unique: [i64; 5],
    /// Cumulative recipient progression.
    pub progression: [i64; 5],
    /// Raw event counts.
    pub totals: [i64; 5],
    /// Matching recipients before pagination.
    pub recipient_total: i64,
    /// Matching events before pagination.
    pub event_total: i64,
    /// Recipient page.
    pub recipients: Vec<AnalysisRecipient>,
    /// Event page.
    pub events: Vec<AnalysisEvent>,
    /// Chart intervals.
    pub buckets: Vec<Bucket>,
    /// Interval width in seconds.
    pub bucket_seconds: i64,
}

/// Observed counts for one recipient.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnalysisRecipient {
    /// Recipient identity.
    pub id: i64,
    /// Administrator-supplied address.
    pub email: String,
    /// Administrator-supplied name.
    pub name: String,
    /// Transport outcome.
    pub delivery: String,
    /// Observed event counts.
    pub counts: [i64; 5],
    /// Legacy report state without an observed timestamp.
    pub historical_reported: bool,
}

/// One recorded observation; no participant-entered values.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnalysisEvent {
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
    pub kind: String,
    /// Observation provenance.
    pub source: String,
    /// Presence only, when recorded.
    #[serde(deserialize_with = "nullable")]
    pub password_present: Option<bool>,
}

/// One chart interval.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bucket {
    /// UTC bucket start.
    pub time: i64,
    /// Observed event counts.
    pub counts: [i64; 5],
}

/// Persisted draft snapshot; configuration may deliberately be incomplete.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Draft {
    /// Client-generated draft identity.
    pub id: String,
    /// Conditional-write revision.
    pub revision: i64,
    /// Incomplete input; saving never authorizes launch.
    #[serde(deserialize_with = "draft_configuration")]
    pub configuration: ruphish_domain::campaign_draft::Configuration,
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
    pub status: String,
}

fn draft_configuration<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<ruphish_domain::campaign_draft::Configuration, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    let fields = |value: &serde_json::Value, names: &[&str]| {
        names.iter().all(|name| value.get(*name).is_some())
    };
    let invalid = || serde::de::Error::custom("Incomplete campaign draft configuration");
    if !fields(
        &value,
        &[
            "name",
            "url",
            "selection",
            "policy",
            "launch_local",
            "send_by_local",
        ],
    ) || !fields(&value["selection"], &["groups", "template", "page", "smtp"])
        || ["template", "page", "smtp"]
            .iter()
            .any(|key| value["selection"][key].get("id").is_none())
        || value["selection"]["groups"]
            .as_array()
            .is_none_or(|rows| rows.iter().any(|row| row.get("id").is_none()))
        || (!value["policy"].is_null()
            && !fields(
                &value["policy"],
                &["capture_credentials", "capture_passwords", "redirect_url"],
            ))
        || ["launch_local", "send_by_local"]
            .iter()
            .any(|key| !value[key].is_null() && !fields(&value[key], &["local", "time_zone"]))
    {
        return Err(invalid());
    }
    serde_json::from_value(value).map_err(serde::de::Error::custom)
}

#[cfg(test)]
#[path = "../tests/support/campaign_reads.rs"]
mod tests;
