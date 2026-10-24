//! Installation settings and the explicitly administrator-scoped closure operation.
use crate::{auth::Identity, resources::Error};
pub use ruphish_domain::settings::{ClosurePreview, ClosureResult, Features, Policy, Settings};

#[async_trait::async_trait]
/// Authorized company policy and campaign closure operations.
pub trait Repository: Send + Sync {
    /// Read policy after checking administrator permission.
    async fn get(&self, caller: Identity) -> Result<Settings, Error>;
    /// Read presentation flags after authenticating the caller.
    async fn features(&self, caller: Identity) -> Result<Features, Error>;
    /// Atomically replace policy at the expected revision.
    async fn save(
        &self,
        caller: Identity,
        revision: i64,
        policy: Policy,
        now: i64,
    ) -> Result<Settings, Error>;
    /// Create an expiring review token for all active campaigns.
    async fn preview_closure(&self, caller: Identity, now: i64) -> Result<ClosurePreview, Error>;
    /// Complete exactly the reviewed set, or replay its saved outcome.
    async fn close_campaigns(
        &self,
        caller: Identity,
        token: &str,
        now: i64,
    ) -> Result<ClosureResult, Error>;
}
