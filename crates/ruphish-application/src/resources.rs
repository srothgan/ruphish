//! Resource use cases and transactional persistence boundaries.
use crate::auth::{AccountError, Identity};
pub use ruphish_domain::resources::*;
use std::sync::Arc;

/// Resource write intent: absent password keeps the saved credential; an empty value clears it.
pub type Input = Resource<Option<String>>;

/// Safe public resource failure.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A parser or operational failure associated with a known schema field.
    #[error("{source}")]
    Field {
        /// Schema-owned path, never a submitted value.
        field: String,
        /// Original classification and legacy diagnostic.
        source: Box<Error>,
    },
    /// Independent invalid fields, without authored values.
    #[error(transparent)]
    Validation(#[from] ruphish_domain::validation::Report),
    /// Installation policy disables this operation.
    #[error("Feature disabled: {0}")]
    FeatureDisabled(&'static str),
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
impl Error {
    /// Preserve classification while associating a parser failure with a field.
    pub fn at(self, field: impl Into<String>) -> Self {
        Self::Field {
            field: field.into(),
            source: Box::new(self),
        }
    }
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
    /// Read metadata without content or credentials, optionally restricted to one owned identifier.
    async fn summaries(
        &self,
        caller: Identity,
        kind: Kind,
        id: Option<i64>,
    ) -> Result<Vec<Summary>, Error>;
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
        resource: Input,
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
/// Preview context for page content, separate from email rendering.
pub enum PagePreview<'a> {
    /// Expand landing-page expressions using the supplied sample recipient.
    Landing(&'a Recipient),
    /// Keep learning-page content static.
    Learning,
}
/// Parser/template checks supplied by concrete established libraries.
pub trait Validator: Send + Sync {
    /// Render a page through the same document policy used for public serving.
    fn page_preview(
        &self,
        html: &str,
        mode: PagePreview<'_>,
    ) -> Result<(String, Vec<PageWarning>), Error>;
    /// Check content rules without persisting or performing delivery.
    fn validate(&self, resource: &Input) -> Result<(), Error>;
    /// Inspect the processed landing document without storing diagnostic state.
    fn page_warnings(&self, html: &str) -> Result<Vec<PageWarning>, Error>;
}
/// Coordinates authoritative rules, parsing and guarded atomic persistence.
#[derive(Clone)]
pub struct Service {
    repository: Arc<dyn Repository>,
    validator: Arc<dyn Validator>,
    tools: Arc<dyn ContentTools>,
    pub(crate) content_work: crate::content_work::ContentWork,
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
    /// Import a base64 ZIP page bundle into self-contained HTML, without network access.
    fn page_bundle(&self, content: &str) -> Result<String, Error>;
    /// Parse records into accepted recipients and explicit row errors.
    fn csv(&self, bytes: &[u8], columns: Option<[String; 4]>) -> Result<CsvImport, Error>;
    /// Decode MIME bodies and optionally rewrite links.
    fn email(&self, content: &[u8], convert_links: bool) -> Result<EmailImport, Error>;
    /// Fetch and transform a site through the destination policy.
    async fn site(
        &self,
        url: &str,
        work: &crate::content_work::ContentWork,
    ) -> Result<String, Error>;
    /// Render a draft using supplied recipient variables.
    fn preview(&self, source: &str, recipient: &Recipient) -> Result<String, Error>;
}
impl Service {
    /// Same document preparation and diagnostics as the corresponding public page route.
    pub async fn page_preview(
        &self,
        html: &str,
        mode: PagePreview<'_>,
    ) -> Result<(String, Vec<PageWarning>), Error> {
        let html = html.to_owned();
        let recipient = match mode {
            PagePreview::Landing(recipient) => Some(recipient.clone()),
            PagePreview::Learning => None,
        };
        let validator = self.validator.clone();
        self.content_work
            .run(move || {
                let mode = recipient
                    .as_ref()
                    .map_or(PagePreview::Learning, PagePreview::Landing);
                validator.page_preview(&html, mode)
            })
            .await
    }
    /// Compose the concrete dependencies for this boundary.
    pub fn new(
        repository: Arc<dyn Repository>,
        validator: Arc<dyn Validator>,
        tools: Arc<dyn ContentTools>,
        content_work: crate::content_work::ContentWork,
    ) -> Self {
        Self {
            repository,
            validator,
            tools,
            content_work,
        }
    }
    /// Import a page bundle after transport authorization, with bounded parser work.
    pub async fn page_bundle(&self, content: String) -> Result<String, Error> {
        let tools = self.tools.clone();
        self.content_work
            .run(move || tools.page_bundle(&content))
            .await
    }
    /// Parse an owned CSV upload without occupying an async request worker.
    pub async fn csv(
        &self,
        bytes: Vec<u8>,
        columns: Option<[String; 4]>,
    ) -> Result<CsvImport, Error> {
        let tools = self.tools.clone();
        self.content_work
            .run(move || tools.csv(&bytes, columns))
            .await
    }
    /// Decode an owned MIME upload with bounded parser work.
    pub async fn email(&self, content: String, convert_links: bool) -> Result<EmailImport, Error> {
        let tools = self.tools.clone();
        self.content_work
            .run(move || tools.email(content.as_bytes(), convert_links))
            .await
    }
    /// Fetch through the outbound policy and admit the resulting content transformation.
    pub async fn site(&self, url: &str) -> Result<String, Error> {
        self.tools.site(url, &self.content_work).await
    }
    /// Render a standalone message preview with bounded template processing.
    pub async fn preview(&self, source: String, recipient: Recipient) -> Result<String, Error> {
        let tools = self.tools.clone();
        self.content_work
            .run(move || tools.preview(&source, &recipient))
            .await
    }
    /// Read the caller-owned resources in stable identifier order.
    pub async fn list(&self, caller: Identity, kind: Kind) -> Result<Vec<Resource>, Error> {
        self.repository.list(caller, kind).await
    }
    /// Read metadata without content or credentials, optionally restricted to one owned identifier.
    pub async fn summaries(
        &self,
        caller: Identity,
        kind: Kind,
        id: Option<i64>,
    ) -> Result<Vec<Summary>, Error> {
        self.repository.summaries(caller, kind, id).await
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
        value: Input,
        id: Option<i64>,
        expected: Option<String>,
    ) -> Result<Resource, Error> {
        let value = self.validate(value).await?;
        self.repository.save(caller, value, id, expected).await
    }
    /// Apply the same authoritative validation to saved resources and inline profiles.
    pub async fn validate(&self, value: Input) -> Result<Input, Error> {
        let validator = self.validator.clone();
        self.content_work
            .run(move || {
                value.validation().finish()?;
                validator.validate(&value)?;
                Ok(value)
            })
            .await
    }
    /// Reuse serving-time form diagnostics for save, preview and campaign review.
    pub async fn page_warnings(&self, html: &str) -> Result<Vec<PageWarning>, Error> {
        let html = html.to_owned();
        let validator = self.validator.clone();
        self.content_work
            .run(move || validator.page_warnings(&html))
            .await
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
