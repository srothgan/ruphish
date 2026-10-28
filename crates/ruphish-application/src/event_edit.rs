//! Owner-scoped, transactional changes to event rows.
use crate::{auth::Identity, resources::Error};
pub use ruphish_domain::event_edit::*;

/// Storage owns authorization, row conflicts and atomic event writes.
#[async_trait::async_trait]
pub trait Repository: Send + Sync {
    /// Create, update or delete one event without participant-event webhooks.
    async fn change_event(
        &self,
        caller: Identity,
        campaign: i64,
        recipient: i64,
        change: Change,
        now: i64,
    ) -> Result<Outcome, Error>;
}
