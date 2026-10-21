//! Resource use cases and transactional persistence boundaries.
use crate::auth::{AccountError, Identity};
pub use ruphish_domain::resources::*;
use std::sync::Arc;

/// Safe public resource failure.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Safe persistence classification retained for reconciliation and retry policy.
    #[error("Resource operation could not be completed")]
    Storage(#[from] crate::storage::Error),
    #[error(transparent)]
    /// Authentication or authorization failure.
    Policy(#[from] AccountError),
    #[error("{0} not found")]
    /// Missing resource within the caller ownership scope.
    NotFound(&'static str),
    #[error("{0} name already in use")]
    /// An occupied owner-scoped resource name.
    Conflict(&'static str),
    #[error("{0}")]
    /// Invalid input with a safe diagnostic.
    Invalid(String),
    /// A saved schedule must be corrected before launching a draft.
    #[error("Draft schedule is in the past")]
    DraftSchedulePast,
    #[error("{0}")]
    /// A synchronous external delivery failure, including an ambiguous outcome.
    Delivery(String),
    #[error("This resource changed. Reload it before saving; your draft has been kept.")]
    /// A conditional write used a stale revision.
    Changed,
    #[error("This resource is used by an active or scheduled campaign")]
    /// An unfinished campaign still references this resource.
    InUse,
    #[error("Resource operation could not be completed")]
    /// Infrastructure failure with details withheld from clients.
    Internal,
}
impl From<crate::auth::Error> for Error {
    fn from(error: crate::auth::Error) -> Self {
        match error {
            crate::auth::Error::Storage(error) => Self::Storage(error),
            crate::auth::Error::Policy(error) => Self::Policy(error),
            crate::auth::Error::NotFound => Self::Policy(AccountError::Unauthorized),
            _ => Self::Internal,
        }
    }
}
/// Persistence authorizes the live identity inside each read/write transaction.
#[async_trait::async_trait]
pub trait Repository: Send + Sync {
    /// Read the caller-owned resources in stable identifier order.
    async fn list(&self, caller: Identity, kind: Kind) -> Result<Vec<Resource>, Error>;
    /// Read metadata without loading content, attachments or credentials.
    async fn summaries(&self, caller: Identity, kind: Kind) -> Result<Vec<Summary>, Error>;
    /// Read one resource after checking current ownership.
    async fn get(&self, caller: Identity, kind: Kind, id: i64) -> Result<Resource, Error>;
    /// Resolve a legacy resource name and load its content in the same authorized snapshot.
    async fn get_by_name(
        &self,
        caller: Identity,
        kind: Kind,
        name: &str,
    ) -> Result<Resource, Error>;
    /// Validate and atomically persist a complete replacement.
    async fn save(
        &self,
        caller: Identity,
        resource: Resource,
        existing: Option<i64>,
        expected: Option<String>,
    ) -> Result<Resource, Error>;
    /// Delete only after ownership, revision and live-usage checks.
    async fn delete(
        &self,
        caller: Identity,
        kind: Kind,
        id: i64,
        expected: Option<String>,
    ) -> Result<(), Error>;
}
/// Parser/template checks supplied by concrete established libraries.
pub trait Validator: Send + Sync {
    /// Check content rules without persisting or performing delivery.
    fn validate(&self, resource: &Resource) -> Result<(), Error>;
}
/// Coordinates authoritative rules, parsing and guarded atomic persistence.
#[derive(Clone)]
pub struct Service {
    repository: Arc<dyn Repository>,
    validator: Arc<dyn Validator>,
    tools: Arc<dyn ContentTools>,
}

/// Reason a CSV record cannot be imported.
#[derive(Clone, Copy, Debug)]
pub enum RowErrorKind {
    /// Malformed encoding or record syntax.
    Encoding,
    /// Number of fields differs from the header.
    Columns,
    /// Recipient address is not a valid mailbox.
    Email,
}
/// A CSV row that could not be imported, including its original fields.
#[derive(Clone, Debug)]
pub struct RowError {
    /// Machine-readable rejection reason.
    pub kind: RowErrorKind,
    /// One-based input record position including the header.
    pub row: u64,
    /// Safe user-facing explanation.
    pub message: String,
    /// Original fields retained for an actionable import report.
    pub fields: Vec<String>,
}
/// Reviewable import result. Applying recipients is a separate group save.
pub struct CsvImport {
    /// Accepted records; applying them requires a separate save.
    pub recipients: Vec<Recipient>,
    /// Rejected rows, never silently discarded by the review workflow.
    pub errors: Vec<RowError>,
    /// Input column names available for explicit mapping.
    pub columns: Vec<String>,
}
/// Decoded email bodies; attachments are not part of the legacy email import response.
pub struct EmailImport {
    /// Email subject template; empty is supported.
    pub subject: String,
    /// Plain-text alternative.
    pub text: String,
    /// Authoritative authored HTML source.
    pub html: String,
}
/// Format and network boundaries for import and preview operations.
#[async_trait::async_trait]
pub trait ContentTools: Send + Sync {
    /// Parse records into accepted recipients and explicit row errors.
    fn csv(&self, bytes: &[u8], columns: Option<[String; 4]>) -> Result<CsvImport, Error>;
    /// Decode MIME bodies and optionally rewrite links.
    fn email(&self, content: &[u8], convert_links: bool) -> Result<EmailImport, Error>;
    /// Fetch and transform a site through the destination policy.
    async fn site(&self, url: &str) -> Result<String, Error>;
    /// Render a draft using supplied recipient variables.
    fn preview(&self, source: &str, recipient: &Recipient) -> Result<String, Error>;
}
impl Service {
    /// Compose the concrete dependencies for this boundary.
    pub fn new(
        repository: Arc<dyn Repository>,
        validator: Arc<dyn Validator>,
        tools: Arc<dyn ContentTools>,
    ) -> Self {
        Self {
            repository,
            validator,
            tools,
        }
    }
    /// Import and preview adapters; HTTP callers authorize before invoking these operations.
    pub fn tools(&self) -> &dyn ContentTools {
        self.tools.as_ref()
    }
    /// Read the caller-owned resources in stable identifier order.
    pub async fn list(&self, caller: Identity, kind: Kind) -> Result<Vec<Resource>, Error> {
        self.repository.list(caller, kind).await
    }
    /// Read metadata without loading content, attachments or credentials.
    pub async fn summaries(&self, caller: Identity, kind: Kind) -> Result<Vec<Summary>, Error> {
        self.repository.summaries(caller, kind).await
    }
    /// Read one resource after checking current ownership.
    pub async fn get(&self, caller: Identity, kind: Kind, id: i64) -> Result<Resource, Error> {
        self.repository.get(caller, kind, id).await
    }
    /// Resolve a legacy resource name without listing every resource or racing a rename.
    pub async fn get_by_name(
        &self,
        caller: Identity,
        kind: Kind,
        name: &str,
    ) -> Result<Resource, Error> {
        self.repository.get_by_name(caller, kind, name).await
    }
    /// Validate and atomically persist a complete replacement.
    pub async fn save(
        &self,
        caller: Identity,
        mut value: Resource,
        id: Option<i64>,
        expected: Option<String>,
    ) -> Result<Resource, Error> {
        self.validate(&mut value)?;
        self.repository.save(caller, value, id, expected).await
    }
    /// Apply the same authoritative validation to saved resources and inline profiles.
    pub fn validate(&self, value: &mut Resource) -> Result<(), Error> {
        value.validate().map_err(|s| Error::Invalid(s.into()))?;
        self.validator.validate(value)
    }
    /// Delete only after ownership, revision and live-usage checks.
    pub async fn delete(
        &self,
        caller: Identity,
        kind: Kind,
        id: i64,
        expected: Option<String>,
    ) -> Result<(), Error> {
        self.repository.delete(caller, kind, id, expected).await
    }
}
