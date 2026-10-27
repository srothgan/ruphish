//! Campaign transactions and durable execution interfaces.
use crate::{
    auth::Identity,
    delivery::MessageInput,
    resources::{Error, Resource},
};
pub use ruphish_domain::campaign::*;
pub mod directory;
pub mod drafts;
pub mod preparation;

/// Owner-scoped resource identity and optional reviewed revision.
#[derive(Clone, Default)]
pub struct Selection {
    /// Persistent resource identity; zero is an incomplete selection.
    pub id: i64,
    /// Reviewed modification timestamp, checked again in the launch transaction.
    pub revision: Option<String>,
}
/// Redirect overrides resolved once per campaign.
#[derive(Clone, Default)]
pub struct Overrides {
    /// Explicit empty string clears the default redirect.
    pub redirect_url: Option<String>,
    /// Selected owned learning page; custom redirect must be empty when present.
    pub learning_page_id: Option<i64>,
}
/// Normalized campaign input, with no caller-controlled ownership or result IDs.
#[derive(Clone, Default)]
pub struct Create {
    /// Human-readable campaign name; duplicates remain allowed.
    pub name: String,
    /// Ordered group selections establish duplicate precedence.
    pub groups: Vec<Selection>,
    /// Email content selection.
    pub template: Selection,
    /// Landing-page selection.
    pub page: Selection,
    /// Transport selection.
    pub profile: Selection,
    /// Public URL template.
    pub url: String,
    /// UTC launch seconds; omission means transaction creation time.
    pub launch: Option<i64>,
    /// UTC end of initial distribution, not a retry deadline.
    pub send_by: Option<i64>,
    /// Optional campaign capture/redirect overrides.
    pub policy: Overrides,
    /// Owner-scoped launch key and canonical request digest.
    pub idempotency: Option<(String, String)>,
}
/// Resolved, reviewable resource input and unique audience.
pub struct Prepared {
    /// Verified domain resolved in the same resource-selection transaction.
    pub domain_id: i64,
    /// Number of overlapping entries whose recipient attributes conflict.
    pub conflict_count: usize,
    /// First one hundred conflicting pairs, with retained attributes first.
    pub conflicts: Vec<(crate::resources::Recipient, crate::resources::Recipient)>,
    /// Group content in selection order.
    pub groups: Vec<Resource>,
    /// Resolved email template.
    pub template: Resource,
    /// Resolved landing page.
    pub page: Resource,
    /// Resolved sending profile.
    pub profile: Resource,
    /// Actual unique recipients in first-encounter order.
    pub recipients: Vec<crate::resources::Recipient>,
    /// Effective campaign settings, independent of later page changes.
    pub policy: InteractionPolicy,
    /// Effective first due time.
    pub launch: i64,
}
/// Persistent recipient outcome, preserving the legacy result shape.
pub struct RecipientResult {
    /// Stable campaign-recipient identity.
    pub id: i64,
    /// Public opaque recipient identifier.
    pub rid: String,
    /// Immutable campaign recipient attributes.
    pub recipient: crate::resources::Recipient,
    /// Legacy result status, projected from delivery and engagement.
    pub status: String,
    /// Last recorded address.
    pub ip: String,
    /// Last recorded latitude.
    pub latitude: f64,
    /// Last recorded longitude.
    pub longitude: f64,
    /// Reported independently of progression.
    pub reported: bool,
    /// Stored legacy timestamp.
    pub send_date: String,
    /// Stored modification timestamp.
    pub modified_date: String,
}
/// Committed campaign history, with string-encoded legacy detail JSON.
pub struct Event {
    /// Stable recipient identity; unknown for ambiguous historical observations.
    pub recipient_id: Option<i64>,
    /// Stable internal cursor.
    pub id: i64,
    /// Recipient mailbox or empty campaign-wide event.
    pub email: String,
    /// Event time.
    pub time: String,
    /// Legacy event name.
    pub message: String,
    /// Legacy string-encoded JSON.
    pub details: String,
}
/// Owner-scoped campaign with current referenced resources and immutable audience.
pub struct Campaign {
    /// Current owned source groups, if provenance was recorded at creation.
    pub groups: Vec<Resource>,
    /// Campaign identifier.
    pub id: i64,
    /// Display name.
    pub name: String,
    /// Lifecycle; finishing SMTP work does not complete the campaign.
    pub status: String,
    /// Creation timestamp.
    pub created: String,
    /// Launch timestamp.
    pub launch: String,
    /// Initial distribution end or Go zero time.
    pub send_by: String,
    /// Explicit completion timestamp or Go zero time.
    pub completed: String,
    /// Public URL template.
    pub url: String,
    /// Current referenced resources; missing legacy resources remain explicit.
    pub resources: Vec<Resource>,
    /// Recipient snapshots and public progression.
    pub results: Vec<RecipientResult>,
    /// Ordered committed history.
    pub events: Vec<Event>,
    /// Effective immutable capture defaults/overrides.
    pub policy: Option<InteractionPolicy>,
}
impl Campaign {
    /// Derive summary counts from recorded events, never the lossy latest status.
    /// Ambiguous historical addresses remain unassigned, as in analysis.
    pub fn progression(&self) -> [i64; 5] {
        use ruphish_domain::reporting::{Observations, Progression};
        let mut recipients: std::collections::HashMap<_, _> = self
            .results
            .iter()
            .map(|result| (result.id, Observations::default()))
            .collect();
        for event in &self.events {
            if let Some(evidence) = event.recipient_id.and_then(|id| recipients.get_mut(&id)) {
                evidence.record(&event.message, false);
            }
        }
        let mut summary = Progression::default();
        for evidence in recipients.into_values() {
            summary.include(evidence, 1);
        }
        summary.counts
    }
}
/// One current queue owner, invalidated by generation change or cancellation.
pub struct Claim {
    /// Persistent result ID, never the opaque external RID.
    pub result_id: i64,
    /// Campaign being executed.
    pub campaign_id: i64,
    /// Unique generation for conditional writes.
    pub generation: i64,
    /// Resource revisions used for composition, checked before arming submission.
    pub revisions: [String; 2],
    /// Fully resolved coherent message input.
    pub message: MessageInput,
}
/// Bounded delivery rows with operational counts, independent of legacy engagement.
pub struct DeliveryView {
    /// Selection-bound continuation for explicit sorted queries.
    pub next_cursor: Option<String>,
    /// Cursor for another page only when additional owned rows exist.
    pub next_after: Option<i64>,
    /// Most recent fifty sanitized activity records, newest first.
    pub activity: Vec<Event>,
    /// Compact campaign metadata; does not load bodies or legacy result arrays.
    pub campaign: CampaignHeader,
    /// State/count pairs over the whole campaign.
    pub counts: Vec<(String, i64)>,
    /// Requested bounded page in result-ID order.
    pub items: Vec<DeliveryItem>,
}
/// Bounded campaign metadata with an evidence-based continuation cursor.
pub struct CampaignOverview {
    /// Requested page in campaign-ID order.
    pub items: Vec<CampaignHeader>,
    /// Last returned ID, only when another owned row exists.
    pub next_after: Option<i64>,
}
/// Metadata for campaign navigation, separate from the full legacy representation.
pub struct CampaignHeader {
    /// Bound domain resource, absent for an unsupported legacy origin.
    pub domain_id: Option<i64>,
    /// Current delivery eligibility failure; absent when evidence is current.
    pub domain_block: Option<String>,
    /// Persistent identifier.
    pub id: i64,
    /// Display name.
    pub name: String,
    /// Lifecycle state.
    pub status: String,
    /// Initial launch timestamp.
    pub launch: String,
    /// Initial distribution end, or Go zero time when unset.
    pub send_by: String,
    /// Configured public campaign URL.
    pub url: String,
    /// Referenced email template, without content or attachments.
    pub template: CampaignResource,
    /// Referenced landing page, without authored content.
    pub page: CampaignResource,
    /// Referenced sending profile, without connection credentials.
    pub profile: CampaignResource,
    /// Current subject expression, before recipient-specific rendering.
    pub subject: Option<String>,
    /// Configured profile sender, including any display name.
    pub profile_sender: Option<String>,
    /// Optional current template sender override; not a rendered per-recipient address.
    pub template_sender: Option<String>,
    /// Recipient count including migrated historical results.
    pub total: i64,
    /// Effective event-only tracking and redirect policy.
    pub policy: Option<InteractionPolicy>,
}
/// Resource identity preserves missing references without exposing another owner's data.
pub struct CampaignResource {
    /// Saved resource ID, including a missing legacy reference.
    pub id: i64,
    /// Current owned resource name; absent if the resource is unavailable.
    pub name: Option<String>,
}
/// Recipient-specific history, independently paged from campaign-wide activity.
pub struct RecipientHistory {
    /// Persistent result ID within the authorized campaign.
    pub result_id: i64,
    /// Recipient mailbox from the saved campaign snapshot.
    pub email: String,
    /// Most recent attempts, descending by generation.
    pub attempts: Vec<Attempt>,
    /// Most recent sanitized recipient events, descending by event ID.
    pub events: Vec<Event>,
    /// Exclusive upper bound for an older page of attempts.
    pub next_attempt: Option<i64>,
    /// Exclusive upper bound for an older page of events.
    pub next_event: Option<i64>,
}
/// One recipient's delivery state and committed transport history.
pub struct DeliveryItem {
    /// Total persisted attempts, independent of the bounded inline attempt list.
    pub attempt_count: i64,
    /// Internal cursor for pagination and mutation.
    pub id: i64,
    /// Public recipient identifier.
    pub rid: String,
    /// Snapshot mailbox.
    pub email: String,
    /// Delivery state, separate from campaign lifecycle and engagement.
    pub state: String,
    /// Next or last due UTC seconds.
    pub due: i64,
    /// Automatic retries consumed.
    pub retries: i64,
    /// Conditional mutation generation.
    pub generation: i64,
    /// Safe machine-readable reason.
    pub code: String,
    /// Chronological protocol attempts.
    pub attempts: Vec<Attempt>,
}
/// Transport evidence without bodies, credentials or raw peer responses.
pub struct Attempt {
    /// Conditional attempt generation.
    pub generation: i64,
    /// Started UTC seconds.
    pub started: i64,
    /// Ended UTC seconds, if known.
    pub finished: Option<i64>,
    /// Current or terminal attempt state.
    pub state: String,
    /// Last protocol phase.
    pub stage: String,
    /// Explicit peer response code.
    pub smtp_code: Option<i64>,
    /// Message-ID for mail-server reconciliation.
    pub message_id: String,
}
/// Persistence owns atomic claims, cancellation and outcome/event writes.
#[async_trait::async_trait]
pub trait Queue: Send + Sync {
    /// Release only unarmed preparation, without consuming a retry.
    async fn release(&self, claim: &Claim, now: i64) -> Result<bool, Error>;
    /// Recover expired work and claim at most one eligible delivery.
    async fn claim(&self, now: i64) -> Result<Option<Claim>, Error>;
    /// Fence the current claim and persist submission before transmitting content.
    async fn arm(&self, claim: &Claim, now: i64) -> Result<bool, Error>;
    /// Atomically finalize only the matching generation; no SMTP retry on DB error.
    async fn finish(
        &self,
        claim: &Claim,
        outcome: Result<(), DeliveryFailure>,
        now: i64,
    ) -> Result<(), Error>;
}
/// Owner-authorized campaign transaction boundary.
#[async_trait::async_trait]
pub trait Repository: drafts::Repository + Send + Sync {
    /// Bounded metadata for the UI without content, secrets or event payloads.
    async fn overview(
        &self,
        caller: Identity,
        after: i64,
        limit: i64,
    ) -> Result<CampaignOverview, Error> {
        self.overview_filtered(caller, after, limit, None).await
    }
    /// Unified owner-scoped management list, filtered before a single page is selected.
    async fn directory(
        &self,
        caller: Identity,
        filter: directory::Filter,
        after: directory::Cursor,
        limit: i64,
    ) -> Result<directory::Page, Error>;
    /// Execution-only overview with an optional lifecycle filter.
    async fn overview_filtered(
        &self,
        caller: Identity,
        after: i64,
        limit: i64,
        completed: Option<bool>,
    ) -> Result<CampaignOverview, Error>;
    /// Resolve a named-zone local time using iCalendar-compatible rules:
    /// first occurrence in a fold, forward by the transition duration in a gap.
    fn resolve_time(&self, local: &str, zone: &str) -> Result<i64, Error> {
        crate::datetime::resolve_local(local, zone)
            .map(|date| date.as_second())
            .map_err(|error| Error::Invalid(error.to_string()))
    }
    /// Resolve resources and schedule without side effects.
    async fn prepare(&self, caller: Identity, input: &Create, now: i64) -> Result<Prepared, Error>;
    /// Commit the complete campaign or replay its existing idempotent launch.
    async fn create(&self, caller: Identity, input: Create, now: i64) -> Result<Campaign, Error>;
    /// Read current campaign state within ownership.
    async fn get(&self, caller: Identity, id: i64) -> Result<Campaign, Error>;
    /// List owned campaigns in stable order.
    async fn list(&self, caller: Identity) -> Result<Vec<Campaign>, Error>;
    /// Stop future execution and preserve history and first completion time.
    async fn complete(&self, caller: Identity, id: i64, now: i64) -> Result<(), Error>;
    /// Remove owned history and fence in-flight writers in the same transaction.
    async fn delete(&self, caller: Identity, id: i64) -> Result<(), Error>;
    /// Read bounded delivery history after current authorization and ownership checks.
    async fn deliveries(
        &self,
        caller: Identity,
        id: i64,
        after: i64,
        limit: i64,
    ) -> Result<DeliveryView, Error> {
        self.deliveries_sorted(caller, id, after, limit, None).await
    }
    /// Read a globally sorted delivery page without changing legacy ID cursors.
    async fn deliveries_sorted(
        &self,
        caller: Identity,
        id: i64,
        after: i64,
        limit: i64,
        sorting: Option<
            ruphish_domain::pagination::SortedPage<ruphish_domain::pagination::DeliverySort>,
        >,
    ) -> Result<DeliveryView, Error>;
    /// Read recipient history after authorization, bounded to 100 items per stream.
    async fn recipient_history(
        &self,
        caller: Identity,
        campaign: i64,
        result: i64,
        before_attempt: Option<i64>,
        before_event: Option<i64>,
    ) -> Result<RecipientHistory, Error>;
    /// Retry known failure, or explicitly reconcile uncertainty, using current generation.
    async fn resolve(
        &self,
        caller: Identity,
        campaign: i64,
        result: i64,
        generation: i64,
        action: &str,
        now: i64,
    ) -> Result<(), Error>;
}

/// Campaign persistence capabilities supplied together by a database provider.
pub trait Store: Repository + Queue {}
impl<T: Repository + Queue> Store for T {}
