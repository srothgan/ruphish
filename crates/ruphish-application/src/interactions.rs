//! Typed public interactions, with no representation for submitted field values.
use crate::resources::Error;

/// Public request purpose, independently of delivery status.
#[derive(Clone)]
pub enum Action {
    /// Render and observe a landing visit.
    View,
    /// Observe a tracking pixel request.
    Open,
    /// Record an independent report flag.
    Report,
    /// Return transparency information without an event.
    Inspect,
    /// Check existence without recording an event.
    Head,
    /// Record only an explicit submission identifier and presence observation.
    Submit {
        /// Random request idempotency key.
        event_id: String,
        /// Whether a designated password control was nonempty.
        password_present: bool,
    },
}
/// Representation chosen by the application, mapped to HTTP by its boundary.
pub enum Output {
    /// Rendered content and its required response policy.
    Html {
        /// Final rendered HTML.
        html: String,
        /// Matching content security policy.
        csp: String,
    },
    /// Transparent tracking pixel.
    Pixel,
    /// Successful response without content.
    Empty,
    /// Public campaign transparency metadata.
    Transparency {
        /// Saved send timestamp.
        send_date: String,
    },
    /// Submission accepted with an optional validated destination.
    Submitted {
        /// Validated absolute HTTPS destination.
        redirect: Option<String>,
    },
}
#[async_trait::async_trait]
/// Public application boundary without transport or submitted-value types.
pub trait Public: Send + Sync {
    /// Serve static educational content without a recipient or interaction event.
    async fn learning_page(&self, origin_url: &str, public_id: &str) -> Result<Output, Error>;
    /// Resolve an exact origin/RID pair and apply the action transactionally.
    async fn interact(
        &self,
        origin_url: &str,
        rid: &str,
        action: Action,
        now: i64,
    ) -> Result<Output, Error>;
}

/// Owned data from a host-bound read, safe to use after releasing the connection.
pub struct Snapshot {
    /// Recipient result identity.
    pub result_id: i64,
    /// Owning campaign identity.
    pub campaign_id: i64,
    /// Recipient address used for event correlation, not submitted form data.
    pub email: String,
    /// Saved send timestamp for transparency.
    pub send_date: String,
    /// Recipient-specific rendering input, present only when needed.
    pub rendering: Option<RenderInput>,
}
/// Complete template input with no dependency on a database connection.
pub struct RenderInput {
    /// Recipient and message context.
    pub message: crate::delivery::MessageInput,
    /// Authored redirect expression.
    pub redirect: String,
    /// Authored HTML for visits; submissions only resolve redirects.
    pub source: Option<String>,
}
/// Provider operations around shared rendering. A snapshot is not permission to commit later.
#[async_trait::async_trait]
pub trait Persistence: Send + Sync {
    /// Resolve an opaque learning-page identity on an eligible owner domain.
    async fn learning_page(&self, origin: &str, public_id: &str) -> Result<String, Error>;
    /// Read one currently eligible origin/identifier pair under a consistent snapshot.
    async fn snapshot(&self, origin: &str, rid: &str, action: &Action) -> Result<Snapshot, Error>;
    /// Recheck eligibility, deduplicate submissions and atomically commit event plus work.
    async fn record(
        &self,
        snapshot: &Snapshot,
        origin: &str,
        action: &Action,
        now: i64,
    ) -> Result<(), Error>;
}

/// Bound event growth without rejecting an idempotent replay of a submission.
pub fn check_event_capacity(count: i64, duplicate: bool) -> Result<(), Error> {
    let limit = 10_000;
    if count >= limit && !duplicate {
        Err(Error::Invalid("interaction_limit_reached".into()))
    } else {
        Ok(())
    }
}

impl Action {
    /// Shared observation classification; persistence never infers an event from a status.
    pub fn kind(&self) -> Option<ruphish_domain::events::Kind> {
        use ruphish_domain::events::Kind;
        match self {
            Self::View => Some(Kind::LinkClicked),
            Self::Open => Some(Kind::EmailOpened),
            Self::Report => Some(Kind::EmailReported),
            Self::Submit { .. } => Some(Kind::FormSubmitted),
            Self::Inspect | Self::Head => None,
        }
    }
    /// Retry identities are opaque random bytes, never participant values.
    pub fn validate(&self) -> Result<(), Error> {
        if let Self::Submit { event_id, .. } = self
            && (event_id.len() != 32 || !event_id.bytes().all(|c| c.is_ascii_hexdigit()))
        {
            return Err(Error::Invalid("submission_invalid_id".into()));
        }
        Ok(())
    }
}
