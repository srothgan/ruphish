//! Owner-scoped, transactional observation corrections.
use crate::{auth::Identity, resources::Error};
pub use ruphish_domain::event_edit::*;

/// Storage owns authorization, serialization of writers, retry lookup and atomic audit writes.
#[async_trait::async_trait]
pub trait Repository: Send + Sync {
    /// Read current/original values and a bounded audit page.
    async fn inspect_event(
        &self,
        caller: Identity,
        campaign: i64,
        recipient: i64,
        event: i64,
        before: Option<i64>,
    ) -> Result<Inspection, Error>;
    /// Commit one correction with its audit record, without participant webhooks.
    async fn change_event(
        &self,
        caller: Identity,
        actor: Identity,
        campaign: i64,
        recipient: i64,
        change: Change,
        now: i64,
    ) -> Result<Record, Error>;
}
