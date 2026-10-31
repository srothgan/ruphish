//! Domain resource and durable verification boundaries.
use crate::{auth::Identity, resources::Error};
pub use ruphish_domain::domains::*;

/// Owner-visible configuration and observations. No network diagnostics are retained.
#[derive(Clone, Debug)]
pub struct Domain {
    /// Current installation policy, projected in the same database snapshot.
    pub verification_required: bool,
    /// Effective deadline for selection; never written back as an observation.
    pub eligible_until: i64,
    /// Effective expiry, absent when verification or expiry is disabled.
    pub expires_at: Option<i64>,
    /// Persistent resource identifier.
    pub id: i64,
    /// Operator-visible display name.
    pub name: String,
    /// Canonical public HTTP(S) origin.
    pub origin: String,
    /// Canonical ASCII DNS name or IP address without IPv6 brackets.
    pub hostname: String,
    /// Explicit operational enablement.
    pub enabled: bool,
    /// Optimistic concurrency revision.
    pub revision: i64,
    /// Current DNS ownership challenge.
    pub challenge: String,
    /// Challenge expiry as UTC Unix seconds.
    pub challenge_expires: i64,
    /// Last completed check as UTC Unix seconds.
    pub checked_at: i64,
    /// Expiry of the last complete successful verification.
    pub verified_until: i64,
    /// Latest DNS control observation.
    pub control: bool,
    /// Latest permitted resolution observation.
    pub dns: bool,
    /// Latest instance routing observation.
    pub routing: bool,
    /// Latest HTTPS observation.
    pub tls: bool,
    /// Closed operational result code.
    pub code: String,
    /// Whether a persisted verification lease is active.
    pub checking: bool,
    /// Current queued or running attempt, independent of domain validity.
    pub active_check: Option<VerificationRun>,
    /// Latest ten completed attempts, newest first; populated on detail responses.
    pub history: Vec<VerificationRun>,
    /// Next automatic or requested check, in UTC Unix seconds.
    pub next_check_at: i64,
    /// Number of referencing campaigns.
    pub campaigns: i64,
}

/// Creation or metadata update; host identity is immutable after creation.
pub struct Edit {
    /// Operator-visible display name.
    pub name: String,
    /// Canonical public HTTP(S) origin.
    pub origin: String,
    /// Explicit operational enablement.
    pub enabled: bool,
    /// Optimistic concurrency revision.
    pub revision: Option<i64>,
}

/// Persisted work claim fenced by revision and lease generation.
pub struct Check {
    /// Logical UTC time at which the worker claimed this check.
    pub started_at: i64,
    /// Domain snapshot covered by this verification claim.
    pub domain: Domain,
    /// Lease generation that fences late results.
    pub generation: i64,
    /// Private per-claim routing expectation, never exposed in the admin resource.
    pub routing_proof: String,
}

/// Closed verification observations. Codes are implementation-owned constants.
#[derive(Default)]
pub struct Observation {
    /// Latest DNS control observation.
    pub control: bool,
    /// Latest permitted resolution observation.
    pub dns: bool,
    /// Latest instance routing observation.
    pub routing: bool,
    /// Latest HTTPS observation.
    pub tls: bool,
    /// Closed operational result code.
    pub code: &'static str,
}

#[async_trait::async_trait]
/// Authorized persistence and durable verification operations.
pub trait Repository: Send + Sync {
    /// List owned resources after checking the live identity.
    async fn list(&self, caller: Identity) -> Result<Vec<Domain>, Error>;
    /// Load one owned resource.
    async fn get(&self, caller: Identity, id: i64) -> Result<Domain, Error>;
    /// Create or conditionally update a resource transactionally.
    async fn save(
        &self,
        caller: Identity,
        id: Option<i64>,
        edit: Edit,
        now: i64,
    ) -> Result<Domain, Error>;
    /// Remove an unused owned resource at the expected revision.
    async fn delete(&self, caller: Identity, id: i64, revision: i64) -> Result<(), Error>;
    /// Coalesce verification work and rotate expired unverified challenges.
    async fn request_check(&self, caller: Identity, id: i64, now: i64) -> Result<Domain, Error>;
    /// Lease one due check, recovering expired work after restart.
    async fn claim(&self, now: i64) -> Result<Option<Check>, Error>;
    /// Commit observations only if the claim still owns the current revision.
    async fn finish(&self, check: &Check, observation: Observation, now: i64) -> Result<(), Error>;
    /// Public content may continue after evidence expiry, but not after a failed check or before
    /// initial verification or after explicit disabling.
    async fn serves(&self, origin_url: &str) -> Result<bool, Error>;
    /// Public routing endpoint returns a bounded proof only for an active probe.
    async fn routing_proof(
        &self,
        origin_url: &str,
        challenge: &str,
        now: i64,
    ) -> Result<Option<String>, Error>;
}

#[async_trait::async_trait]
/// Bounded external observation boundary.
pub trait Verifier: Send + Sync {
    /// Query control proof and check every permitted HTTPS destination.
    async fn verify(&self, check: &Check) -> Observation;
}

/// Durable verification attempt; only closed result codes are exposed.
#[derive(Clone, Debug)]
pub struct VerificationRun {
    /// Persistent attempt identity.
    pub id: i64,
    /// Queued, running, passed, failed or timed_out.
    pub state: String,
    /// Earliest scheduled execution, in UTC Unix seconds.
    pub scheduled_at: i64,
    /// Actual worker start; absent for queued or imported historical checks.
    pub started_at: Option<i64>,
    /// Actual completion; absent while active.
    pub finished_at: Option<i64>,
    /// Safe application-owned result code.
    pub code: String,
}

impl Edit {
    /// Validate the immutable origin and operator label identically for every provider.
    pub fn validate(&self) -> Result<ruphish_domain::domains::Origin, Error> {
        ruphish_domain::domains::validate_edit(&self.name, &self.origin).map_err(Error::Validation)
    }
}
