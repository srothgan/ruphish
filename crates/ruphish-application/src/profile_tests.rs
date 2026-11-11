//! Durable saved-profile diagnostics, separate from template and campaign previews.
use crate::{
    auth::Identity,
    delivery::{MessageInput, Sender, Transport},
    resources::{Error, Profile, Recipient, Template},
};
use ruphish_domain::campaign::{DeliveryFailure, FailureKind, Stage};
pub use ruphish_domain::profile_tests::{Attempt, Kind, Outcome, Summary};
use std::{sync::Arc, time::Duration};

/// Validated operation identity and reviewed resource revision.
#[derive(Clone)]
pub struct Request {
    /// Unique identity reused to reconcile an uncertain request.
    pub id: String,
    /// Reviewed resource revision.
    pub revision: i64,
    /// Connection or fixed-content email.
    pub kind: Kind,
    /// Administrator test recipient, absent for a connection check.
    pub email: Option<String>,
}

/// Start once or return the already persisted operation without repeating its effect.
pub enum Start {
    /// New operation and an authorized immutable transport snapshot.
    Started(Attempt, Profile),
    /// An identical operation was already recorded.
    Existing(Attempt),
}

/// Atomic storage boundary. Network calls never run inside its transactions.
#[async_trait::async_trait]
pub trait Repository: Send + Sync {
    /// Authorize and read current evidence, recovering expired executions.
    async fn read(&self, caller: Identity, profile: i64, now: i64) -> Result<Summary, Error>;
    /// Authorize, compare revision, exclude overlap, and persist before network work.
    async fn begin(
        &self,
        caller: Identity,
        profile: i64,
        request: Request,
        now: i64,
    ) -> Result<Start, Error>;
    /// Persist the external submission boundary before transmitting content.
    async fn submitting(&self, id: &str, now: i64) -> Result<(), Error>;
    /// Complete once; retain an existing terminal result on repeated completion.
    async fn finish(
        &self,
        id: &str,
        outcome: Outcome,
        code: &str,
        smtp_code: Option<u16>,
        now: i64,
    ) -> Result<Attempt, Error>;
}

/// Typed SMTP connection diagnostics, without converting failures to display strings.
#[async_trait::async_trait]
pub trait Connector: Send + Sync {
    /// Validate a single recipient through the same mailbox parser used for submission.
    fn validate_recipient(&self, email: &str) -> Result<(), Error>;
    /// Connect and authenticate without transmitting a message.
    async fn test_connection(&self, profile: &Profile) -> Result<(), DeliveryFailure>;
}

/// Bounded execution continues to record its outcome if the HTTP caller disconnects.
#[derive(Clone)]
pub struct Service {
    repository: Arc<dyn Repository>,
    sender: Arc<dyn Sender>,
    transport: Arc<dyn Transport>,
    connector: Arc<dyn Connector>,
    work: crate::content_work::ContentWork,
}

impl Service {
    /// Compose capabilities at the executable boundary.
    pub fn new(
        repository: Arc<dyn Repository>,
        sender: Arc<dyn Sender>,
        transport: Arc<dyn Transport>,
        connector: Arc<dyn Connector>,
        work: crate::content_work::ContentWork,
    ) -> Self {
        Self {
            repository,
            sender,
            transport,
            connector,
            work,
        }
    }
    /// Read the shared diagnostic history after live authorization.
    pub async fn read(&self, caller: Identity, profile: i64) -> Result<Summary, Error> {
        self.repository
            .read(caller, profile, crate::datetime::now().as_second())
            .await
    }
    /// Start or reconcile one explicit action. Only accepted fixed-content email qualifies.
    pub async fn run(
        &self,
        caller: Identity,
        profile: i64,
        request: Request,
    ) -> Result<Attempt, Error> {
        if request.id.len() < 16
            || request.id.len() > 64
            || !request
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        {
            return Err(Error::Invalid("Use a unique test request identity".into()));
        }
        let mail = match (request.kind, request.email.as_deref()) {
            (Kind::Connection, None) => None,
            (Kind::Email, Some(email))
                if !email.trim().is_empty()
                    && email.len() <= ruphish_domain::profile_tests::MAX_RECIPIENT_BYTES =>
            {
                Some(email.to_owned())
            }
            _ => {
                return Err(Error::Invalid(
                    "Email tests require one recipient; connection tests accept no recipient"
                        .into(),
                ));
            }
        };
        if let Some(email) = &mail {
            self.connector.validate_recipient(email)?;
        }
        let (attempt, profile) = match self
            .repository
            .begin(caller, profile, request, crate::datetime::now().as_second())
            .await?
        {
            Start::Existing(attempt) => return Ok(attempt),
            Start::Started(attempt, profile) => (attempt, profile),
        };
        let service = self.clone();
        // The task owns bounded execution and all outcome persistence. Dropping the
        // request's join handle does not cancel an already authorized SMTP operation.
        tokio::spawn(async move { service.execute(attempt, profile).await })
            .await
            .map_err(|_| Error::Internal)?
    }
    async fn execute(&self, attempt: Attempt, profile: Profile) -> Result<Attempt, Error> {
        let result = if attempt.kind == Kind::Connection {
            tokio::time::timeout(
                Duration::from_secs(30),
                self.connector.test_connection(&profile),
            )
            .await
            .unwrap_or_else(|_| {
                Err(DeliveryFailure::new(
                    FailureKind::Retryable,
                    Stage::Connect,
                    None,
                ))
            })
        } else {
            self.submit(&attempt, profile).await?
        };
        let (outcome, code, smtp_code) = match result {
            Ok(()) => (
                if attempt.kind == Kind::Connection {
                    Outcome::Passed
                } else {
                    Outcome::Accepted
                },
                "",
                None,
            ),
            Err(failure) => (
                if failure.kind == FailureKind::Unknown {
                    Outcome::Unknown
                } else {
                    Outcome::Failed
                },
                failure.diagnostic_code(),
                failure.smtp_code,
            ),
        };
        self.repository
            .finish(
                &attempt.id,
                outcome,
                code,
                smtp_code,
                crate::datetime::now().as_second(),
            )
            .await
    }
    async fn submit(
        &self,
        attempt: &Attempt,
        profile: Profile,
    ) -> Result<Result<(), DeliveryFailure>, Error> {
        let sender = self.sender.clone();
        let input = message(
            profile.clone(),
            attempt.email.clone().ok_or(Error::Internal)?,
        );
        let mail = match self.work.run(move || sender.compose(&input)).await {
            Ok(mail) => mail,
            Err(_) => {
                return Ok(Err(DeliveryFailure::new(
                    FailureKind::Permanent,
                    Stage::Content,
                    None,
                )));
            }
        };
        let pending = match tokio::time::timeout(
            Duration::from_secs(30),
            self.transport.prepare(&profile, mail),
        )
        .await
        {
            Ok(Ok(pending)) => pending,
            Ok(Err(failure)) => return Ok(Err(failure)),
            Err(_) => {
                return Ok(Err(DeliveryFailure::new(
                    FailureKind::Retryable,
                    Stage::Connect,
                    None,
                )));
            }
        };
        self.repository
            .submitting(&attempt.id, crate::datetime::now().as_second())
            .await?;
        Ok(
            tokio::time::timeout(Duration::from_secs(30), pending.submit())
                .await
                .unwrap_or_else(|_| {
                    Err(DeliveryFailure::new(
                        FailureKind::Unknown,
                        Stage::Acceptance,
                        None,
                    ))
                }),
        )
    }
}

fn message(profile: Profile, email: String) -> MessageInput {
    MessageInput {
        track_opens: false, message_id: None,
        recipient: Recipient { email, ..Default::default() },
        template: Template { subject: "Ruphish sending profile test".into(), text: "This is a test email from Ruphish.\n\nYour mail server accepted this message for submission. Finding it in your inbox confirms delivery to this recipient.\n".into(), ..Default::default() },
        template_id: 0, page_id: 0, profile, url: String::new(), rid: String::new(),
    }
}
