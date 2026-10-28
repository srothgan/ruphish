//! Authorized reporting snapshots and integration boundaries.
use crate::{auth::Identity, resources::Error};
/// Canonical identities shared by reporting boundaries.
pub use ruphish_domain::{
    campaign::{CampaignStatus, DeliveryState},
    events::{Kind, RecordedKind},
    reporting::Source,
};

/// One query owns filter semantics across tables, charts and exports.
#[derive(Clone)]
pub struct Filter {
    /// Exact recipient row identity, never an email supplied as authorization.
    pub recipient: Option<i64>,
    /// Selected recipient identities, intersected with the single-recipient filter.
    pub recipients: Vec<i64>,
    /// Inclusive UTC Unix seconds.
    pub from: Option<i64>,
    /// Exclusive UTC Unix seconds.
    pub until: Option<i64>,
    /// Select recipients with any of these event types in the time/source scope.
    /// Retain all scoped event types for those recipients; empty means no event selection.
    pub event: Vec<ruphish_domain::events::Kind>,
    /// Case-insensitive recipient name/email substring.
    pub search: String,
    /// Report source, or no restriction.
    pub source: Option<Source>,
    /// Event ordering before pagination.
    pub event_sort: ruphish_domain::reporting::EventSort,
    /// Event direction, independent of recipient ordering.
    pub event_order: ruphish_domain::reporting::SortDirection,
    /// Recipient ordering before pagination; events retain chronological ordering.
    pub sort: ruphish_domain::reporting::RecipientSort,
    /// Sort direction; equal values always use ascending recipient identity.
    pub order: ruphish_domain::reporting::SortDirection,
    /// Pagination offset, bounded at the boundary.
    pub offset: usize,
    /// Requested rows (1..200).
    pub limit: usize,
}
impl Default for Filter {
    fn default() -> Self {
        Self {
            recipient: None,
            recipients: Vec::new(),
            from: None,
            until: None,
            event: Vec::new(),
            search: String::new(),
            source: None,
            offset: 0,
            limit: 0,
            sort: Default::default(),
            order: Default::default(),
            event_sort: Default::default(),
            event_order: ruphish_domain::reporting::SortDirection::Desc,
        }
    }
}
/// Safe event data; no arbitrary payload or network identity.
#[derive(Clone)]
pub struct Event {
    /// Whether an operator has changed the event values since creation.
    pub edited: bool,
    /// Durable event identity.
    pub id: i64,
    /// Associated recipient identity if resolvable.
    pub recipient_id: Option<i64>,
    /// Operator-supplied addressing identity.
    pub email: String,
    /// Normalized UTC timestamp.
    pub time: String,
    /// Recognized event name, or historical event name.
    pub kind: RecordedKind,
    /// Provenance of the observation.
    pub source: Source,
    /// Presence only; absence is distinct from false.
    pub password_present: Option<bool>,
    /// Username presence, when explicitly recorded; never inferred.
    pub username_present: Option<bool>,
}
/// Recipient observations, kept separate from transport state.
#[derive(Clone)]
pub struct Recipient {
    /// Persistent result identity.
    pub id: i64,
    /// Address from campaign audience.
    pub email: String,
    /// Display name from campaign audience.
    pub name: String,
    /// Original first name from the campaign audience.
    pub first_name: String,
    /// Original last name from the campaign audience.
    pub last_name: String,
    /// Original position from the campaign audience.
    pub position: String,
    /// Stored legacy send date: scheduled, retry or accepted time, not inferred delivery.
    pub send_date: Option<String>,
    /// Stored result modification time, independent of the selected event window.
    pub modified_date: Option<String>,
    /// Current transport outcome, not inferred from engagement.
    pub delivery: Option<DeliveryState>,
    /// Counts in the selected window: accepted, opened, clicked, submitted, reported.
    pub counts: [i64; 5],
    /// Historical legacy reported state without observed evidence.
    pub historical_reported: bool,
}
/// Time-bucketed observed event counts.
pub struct Bucket {
    /// UTC bucket start.
    pub time: i64,
    /// Accepted, opened, clicked, submitted and reported observations.
    pub counts: [i64; 5],
}
/// Consistent authorized snapshot, with bounded table pages.
pub struct Analysis {
    /// First observed UTC second per metric, before pagination; absent without valid evidence.
    pub first_observed: [Option<i64>; 5],
    /// Latest observed UTC second per metric, before pagination.
    pub last_observed: [Option<i64>; 5],

    /// Campaign identifier.
    pub id: i64,
    /// Campaign display name.
    pub name: String,
    /// Lifecycle state.
    pub status: CampaignStatus,
    /// Snapshot time in UTC Unix seconds.
    pub as_of: i64,
    /// Campaign audience, before filters.
    pub audience: i64,
    /// Distinct recipients for each observed event type in this filter.
    pub unique: [i64; 5],
    /// Cumulative distinct-recipient progression from matching observations.
    /// Separate from raw unique counts, totals, chart buckets and exports.
    pub progression: [i64; 5],
    /// Recorded event totals for each observed event type.
    pub totals: [i64; 5],
    /// Matching recipients, before pagination.
    pub recipient_total: i64,
    /// Matching events, before pagination.
    pub event_total: i64,
    /// Current recipient page.
    pub recipients: Vec<Recipient>,
    /// Current event page.
    pub events: Vec<Event>,
    /// Bounded chart buckets.
    pub buckets: Vec<Bucket>,
    /// Cumulative distinct participants per action in the selected window.
    pub participant_buckets: Vec<Bucket>,
    /// Width of each chart interval in seconds, always a whole number of minutes.
    pub bucket_seconds: i64,
}
/// Generated download with bounded materialization, independent of a live SQL connection.
pub struct Download {
    /// Fixed safe media type.
    pub content_type: &'static str,
    /// Fixed safe file extension.
    pub extension: &'static str,
    /// Generated bytes; enforced export limits bound allocation.
    pub bytes: Vec<u8>,
}
#[async_trait::async_trait]
/// Reporting persistence and export use cases.
pub trait Repository: Send + Sync {
    /// Read observed metrics and bounded rows under one authorized snapshot.
    async fn analysis(&self, caller: Identity, id: i64, filter: Filter) -> Result<Analysis, Error>;
    /// Export a consistent authorized snapshot, never a client-provided dataset.
    async fn export(
        &self,
        caller: Identity,
        id: i64,
        filter: Filter,
        format: &str,
        scope: &str,
    ) -> Result<Download, Error>;
}

/// Provider-owned consistent read; materialization limits apply before returning data.
#[async_trait::async_trait]
pub trait Snapshots: Send + Sync {
    /// Load an authorized analysis, expanding rows only for a bounded export.
    async fn snapshot(
        &self,
        caller: Identity,
        id: i64,
        filter: &Filter,
        export: bool,
    ) -> Result<Analysis, Error>;
}

/// Shared ordering direction exposed to application consumers.
pub use ruphish_domain::reporting::SortDirection;
