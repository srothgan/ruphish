//! Configuration, safe observations and commands for outbound integrations.
use crate::{auth::Identity, resources::Error};
pub use ruphish_domain::integrations::{MailboxEdit, WebhookEdit};
/// Safe endpoint details.
pub struct Webhook {
    /// Persistent identifier.
    pub id: i64,
    /// Name.
    pub name: String,
    /// Destination URL.
    pub url: String,
    /// Whether event delivery is enabled.
    pub active: bool,
    /// Configuration revision.
    pub revision: i64,
    /// Whether a signing secret exists, never its value.
    pub has_secret: bool,
    /// Recent delivery attempts.
    pub history: Vec<Run>,
}
/// Public mailbox view, with no service password or raw message content.
pub struct Mailbox {
    /// Persistent resource identity.
    pub id: i64,
    /// Resource name.
    pub name: String,
    /// Enabled state.
    pub enabled: bool,
    /// IMAP hostname.
    pub host: String,
    /// Port.
    pub port: u16,
    /// Configured service username.
    pub username: String,
    /// Secret availability only.
    pub has_password: bool,
    /// Implicit TLS, otherwise required STARTTLS.
    pub tls: bool,
    /// Explicitly skip server certificate chain and hostname verification; TLS is still required.
    pub ignore_cert_errors: bool,
    /// Folder.
    pub folder: String,
    /// Optional domain filter.
    pub restrict_domain: String,
    /// Optional successful-message deletion.
    pub delete_reported: bool,
    /// Poll seconds.
    pub frequency: u32,
    /// Revision.
    pub revision: i64,
    /// Last configuration change, retaining imported timestamp precision.
    pub modified_date: String,
    /// Most recent successful folder login, including imported history.
    pub last_login: String,
    /// Most recent completed check.
    pub last_check: i64,
    /// Most recent successful check.
    pub last_success: i64,
    /// Scheduled check.
    pub next_check: i64,
    /// Current activity.
    pub running: bool,
    /// Closed outcome code.
    pub code: String,
    /// Recent checks.
    pub history: Vec<Run>,
}
/// Sanitized integration operation record.
pub struct Run {
    /// Correlated event for webhook delivery.
    pub event_id: Option<i64>,
    /// Delivery attempt number, absent for mailbox checks.
    pub attempt: Option<i64>,
    /// Record identity.
    pub id: i64,
    /// Start in UTC seconds.
    pub started: i64,
    /// End in UTC seconds; absent while running.
    pub finished: Option<i64>,
    /// HTTP status, when relevant.
    pub status: Option<i64>,
    /// Application-owned outcome classification.
    pub code: String,
    /// Successfully correlated messages.
    pub matched: i64,
    /// Rejected or unmatched messages.
    pub rejected: i64,
}
#[async_trait::async_trait]
/// Live-authorized integration configuration and explicit actions.
pub trait Repository: Send + Sync {
    /// Administrator-only endpoint list.
    async fn webhooks(&self, caller: Identity) -> Result<Vec<Webhook>, Error>;
    /// Endpoint detail and attempts.
    async fn webhook(&self, caller: Identity, id: i64) -> Result<Webhook, Error>;
    /// Atomic endpoint save; jobs for older revisions are cancelled.
    async fn save_webhook(
        &self,
        caller: Identity,
        id: Option<i64>,
        edit: WebhookEdit,
    ) -> Result<Webhook, Error>;
    /// Remove endpoint and cancel jobs.
    async fn delete_webhook(
        &self,
        caller: Identity,
        id: i64,
        revision: Option<i64>,
    ) -> Result<(), Error>;
    /// Send signed validation payload without creating a campaign event.
    async fn validate_webhook(&self, caller: Identity, id: i64) -> Result<(), Error>;
    /// Owner-scoped reporting mailbox list.
    async fn mailboxes(&self, caller: Identity) -> Result<Vec<Mailbox>, Error>;
    /// Mailbox detail and recent runs.
    async fn mailbox(&self, caller: Identity, id: i64) -> Result<Mailbox, Error>;
    /// Save without making network calls.
    async fn save_mailbox(
        &self,
        caller: Identity,
        id: Option<i64>,
        edit: MailboxEdit,
    ) -> Result<Mailbox, Error>;
    /// Remove configuration, checkpoints and run history.
    async fn delete_mailbox(
        &self,
        caller: Identity,
        id: i64,
        revision: Option<i64>,
    ) -> Result<(), Error>;
    /// Validate connection and folder without ingesting messages.
    async fn validate_mailbox(&self, caller: Identity, id: i64) -> Result<(), Error>;
    /// Validate supplied legacy settings without saving or ingesting messages.
    async fn validate_mailbox_settings(
        &self,
        caller: Identity,
        edit: MailboxEdit,
    ) -> Result<(), Error>;
    /// Explicitly resume with new arrivals after the server changes UIDVALIDITY.
    async fn resume_mailbox(&self, caller: Identity, id: i64, revision: i64) -> Result<(), Error>;
    /// Schedule an immediate check; coalesce active work.
    async fn check_mailbox(&self, caller: Identity, id: i64) -> Result<(), Error>;
}

/// Durable background work, independent of the selected database engine.
#[async_trait::async_trait]
pub trait Worker: Send + Sync {
    /// Dispatch a bounded batch of claimed webhooks.
    async fn deliver_webhooks(&self) -> Result<(), Error>;
    /// Poll a bounded batch of claimed mailboxes.
    async fn poll_mailboxes(&self) -> Result<(), Error>;
    /// Run both independent batches; each persists its own outcomes.
    async fn tick(&self) -> Result<(), Error>;
}
/// Integration configuration and execution capabilities of one provider.
pub trait Store: Repository + Worker {}
impl<T: Repository + Worker> Store for T {}

pub mod work;
