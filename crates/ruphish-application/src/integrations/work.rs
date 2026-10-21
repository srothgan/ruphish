//! Durable claims and checkpoints used by shared integration orchestration.
//! None of these records contains a submitted credential or retained email body.
use crate::resources::Error;

/// Provider-neutral WebhookClaim data; sensitive records deliberately omit Debug.
pub struct WebhookClaim {
    /// Durable job identity.
    pub id: i64,
    /// Attempt history identity.
    pub run: i64,
    /// Stable event identity for retries.
    pub event: i64,
    /// Claim generation and attempt number.
    pub attempt: i64,
    /// Destination snapshot.
    pub url: String,
    /// Signing credential; never log.
    pub secret: String,
    /// Serialized allowlisted event metadata.
    pub body: Vec<u8>,
}

/// Provider-neutral WebhookCompletion data; sensitive records deliberately omit Debug.
pub struct WebhookCompletion {
    /// Accepted, retry or failed.
    pub state: &'static str,
    /// Closed diagnostic code.
    pub code: &'static str,
    /// HTTP response status if observed.
    pub status: Option<u16>,
    /// Next eligible attempt time.
    pub due: i64,
}

/// Provider-neutral MailboxConfig data; sensitive records deliberately omit Debug.
pub struct MailboxConfig {
    /// Mailbox identity.
    pub id: i64,
    /// Owner identity.
    pub owner: i64,
    /// Configuration generation.
    pub revision: i64,
    /// Server hostname.
    pub host: String,
    /// Server port.
    pub port: u16,
    /// Service account name.
    pub username: String,
    /// Service credential; never log.
    pub password: String,
    /// TLS requirement.
    pub tls: bool,
    /// Selected IMAP folder.
    pub folder: String,
    /// Allowed report sender domain.
    pub restrict: String,
    /// Whether acknowledged messages are removed.
    pub delete: bool,
    /// Saved UIDVALIDITY.
    pub validity: i64,
    /// Saved UID checkpoint.
    pub last: u32,
    /// Whether initial mailbox discovery completed.
    pub initialized: bool,
}

/// Provider-neutral MailboxClaim data; sensitive records deliberately omit Debug.
pub struct MailboxClaim {
    /// Claimed connection and checkpoint snapshot.
    pub config: MailboxConfig,
    /// History identity.
    pub run: i64,
    /// Fence for completion of this claim.
    pub lease_until: i64,
    /// Configured successful polling interval in seconds.
    pub frequency: u32,
    /// Consecutive failures before this attempt.
    pub failures: u32,
}

/// Claims and outcomes are atomic; no external HTTP call runs in their transaction.
#[async_trait::async_trait]
pub trait WebhookQueue: Send + Sync {
    /// Recover expired claims and reserve one eligible attempt.
    async fn claim_webhook(&self, now: i64) -> Result<Option<WebhookClaim>, Error>;
    /// Commit the result only for the current attempt; finalize history atomically.
    async fn finish_webhook(
        &self,
        claim: &WebhookClaim,
        outcome: WebhookCompletion,
        now: i64,
    ) -> Result<(), Error>;
}
/// Mailbox persistence around transient network work; acknowledgement follows commit.
#[async_trait::async_trait]
pub trait MailboxQueue: Send + Sync {
    /// Recover expired claims and reserve one eligible mailbox.
    async fn claim_mailbox(&self, now: i64) -> Result<Option<MailboxClaim>, Error>;
    /// Fence the result by revision and lease, updating history and next check together.
    async fn finish_mailbox(
        &self,
        claim: &MailboxClaim,
        code: &'static str,
        next_check: i64,
        now: i64,
    ) -> Result<(), Error>;
    /// Record a successful login independently of the mailbox metadata response.
    async fn mailbox_login(&self, config: &MailboxConfig) -> Result<(), &'static str>;
    /// Initialize its UID checkpoint after valid metadata is received.
    async fn initialize_mailbox(
        &self,
        config: &MailboxConfig,
        validity: i64,
        next: u32,
    ) -> Result<(), &'static str>;
    /// List committed reports still awaiting external acknowledgement.
    async fn pending_acknowledgements(
        &self,
        config: &MailboxConfig,
        validity: i64,
    ) -> Result<Vec<u32>, &'static str>;
    /// Recheck configuration and owner permission before an external acknowledgement.
    async fn mailbox_current(&self, config: &MailboxConfig) -> Result<(), &'static str>;
    /// Correlate identity, commit event/work and checkpoint atomically. True permits acknowledgement.
    async fn record_report(
        &self,
        config: &MailboxConfig,
        run: i64,
        validity: i64,
        uid: u32,
        identified: Result<(String, String), &'static str>,
    ) -> Result<bool, &'static str>;
    /// Persist acknowledgement after the external operation succeeds.
    async fn acknowledge_report(
        &self,
        config: &MailboxConfig,
        validity: i64,
        uid: u32,
    ) -> Result<(), &'static str>;
}
