//! Typed public interactions, with no representation for submitted field values.
use crate::resources::Error;

/// Public request purpose, independently of delivery status.
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
    /// Resolve an exact origin/RID pair and apply the action transactionally.
    async fn interact(
        &self,
        authority: &str,
        rid: &str,
        action: Action,
        now: i64,
    ) -> Result<Output, Error>;
}
