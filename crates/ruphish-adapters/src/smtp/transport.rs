//! Lettre owns protocol encoding; this boundary owns phases and safe diagnostics.
use super::Smtp;
use crate::resources::{invalid, smtp_address};
use lettre::{
    address::Envelope,
    transport::smtp::{
        Error as SmtpError,
        client::AsyncSmtpConnection,
        commands::{Data, Mail, Rcpt},
        extension::{ClientId, Extension, MailBodyParameter, MailParameter},
    },
};
use ruphish_application::{
    delivery::{PreparedMail, Submission, Transport},
    resources::{Error, Profile},
};
use ruphish_domain::campaign::{DeliveryFailure, FailureKind, Stage};
use std::time::Duration;
use tokio::time::timeout;

pub(super) fn failure(error: SmtpError, stage: Stage) -> DeliveryFailure {
    // Lettre's async implicit-TLS handshake wraps rustls failures in connection
    // errors. Classify their typed causes rather than relying on the outer kind.
    let stage = if stage == Stage::Connect && tls_error(&error) {
        Stage::Tls
    } else {
        stage
    };
    let code = error.status().and_then(|c| c.to_string().parse().ok());
    let kind = if error.is_transient() {
        FailureKind::Retryable
    } else if error.is_tls() || matches!(stage, Stage::Tls | Stage::Authentication) {
        FailureKind::Blocked
    } else if error.is_permanent() {
        FailureKind::Permanent
    } else if stage == Stage::Acceptance {
        FailureKind::Unknown
    } else {
        FailureKind::Retryable
    };
    DeliveryFailure::new(kind, stage, code)
}

fn tls_error(error: &SmtpError) -> bool {
    let mut source: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(error) = source {
        if error.is::<tokio_rustls::rustls::Error>()
            || error
                .downcast_ref::<SmtpError>()
                .is_some_and(SmtpError::is_tls)
        {
            return true;
        }
        // io::Error::source can skip the wrapped error itself. Inspect its
        // payload so certificate errors remain visible through nested wrappers.
        source = match error
            .downcast_ref::<std::io::Error>()
            .and_then(std::io::Error::get_ref)
        {
            Some(inner) => Some(inner),
            None => error.source(),
        };
    }
    false
}
pub(super) fn timed_out(stage: Stage) -> DeliveryFailure {
    DeliveryFailure::new(
        if stage == Stage::Acceptance {
            FailureKind::Unknown
        } else {
            FailureKind::Retryable
        },
        stage,
        None,
    )
}
pub(super) fn public_failure(error: DeliveryFailure) -> Error {
    let message = match error.kind {
        FailureKind::Unknown => {
            "Delivery outcome is unknown after connection loss or timeout. Check the mail server before sending again."
        }
        FailureKind::Blocked => "SMTP configuration, authentication or certificate needs attention",
        FailureKind::Permanent => "SMTP server rejected the test email",
        FailureKind::Retryable => {
            "SMTP server temporarily unavailable or rejected the test email; try again later"
        }
    };
    let stage = match error.stage {
        Stage::Capacity => "local sending capacity",
        Stage::Configuration => "SMTP configuration",
        Stage::Dns => "DNS lookup",
        Stage::DestinationPolicy => "destination policy",
        Stage::Connect => "connection or SMTP greeting",
        Stage::Tls => "TLS negotiation/certificate validation",
        Stage::Authentication => "authentication",
        Stage::Envelope => "sender/recipient acceptance",
        Stage::Data => "DATA command",
        Stage::Acceptance => "message acceptance",
        Stage::Content => "message composition",
    };
    let code = error
        .smtp_code
        .map(|code| format!("; SMTP {code}"))
        .unwrap_or_default();
    invalid(format!("{message} (stage: {stage}{code})"))
}
impl Smtp {
    pub(super) async fn connection(
        &self,
        profile: &Profile,
    ) -> Result<AsyncSmtpConnection, DeliveryFailure> {
        let blocked = |stage| DeliveryFailure::new(FailureKind::Blocked, stage, None);
        let (host, port) =
            smtp_address(&profile.host).map_err(|_| blocked(Stage::Configuration))?;
        let addresses =
            self.policy
                .resolve_checked(&host, port)
                .await
                .map_err(|error| match error {
                    crate::outbound::ResolveFailure::Denied => blocked(Stage::DestinationPolicy),
                    crate::outbound::ResolveFailure::Unavailable => timed_out(Stage::Dns),
                    crate::outbound::ResolveFailure::TooMany => blocked(Stage::Configuration),
                })?;
        let hello = ClientId::Domain("ruphish.local".into());
        let mut connection = super::connection::connect(addresses, host, port, profile).await?;
        if !profile.username.is_empty() {
            timeout(
                Duration::from_secs(300),
                super::authentication::authenticate(&mut connection, &hello, profile),
            )
            .await
            .map_err(|_| timed_out(Stage::Authentication))??;
        }
        Ok(connection)
    }
}
struct Pending {
    connection: AsyncSmtpConnection,
    mail: PreparedMail,
    _slot: tokio::sync::OwnedSemaphorePermit,
}
#[async_trait::async_trait]
impl Submission for Pending {
    async fn submit(mut self: Box<Self>) -> Result<(), DeliveryFailure> {
        let response = timeout(
            Duration::from_secs(600),
            self.connection.message(&self.mail.bytes),
        )
        .await
        .map_err(|_| timed_out(Stage::Acceptance))?
        .map_err(|e| failure(e, Stage::Acceptance))?;
        if !response.code().to_string().starts_with('2') {
            return Err(DeliveryFailure::new(
                FailureKind::Unknown,
                Stage::Acceptance,
                None,
            ));
        }
        // Failure closing an accepted transaction never changes its outcome.
        let _ = timeout(Duration::from_secs(2), self.connection.quit()).await;
        Ok(())
    }
}
#[async_trait::async_trait]
impl Transport for Smtp {
    async fn prepare(
        &self,
        profile: &Profile,
        mail: PreparedMail,
    ) -> Result<Box<dyn Submission>, DeliveryFailure> {
        let slot = self
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| timed_out(Stage::Capacity))?;
        let bad_envelope = || DeliveryFailure::new(FailureKind::Permanent, Stage::Envelope, None);
        let envelope = Envelope::new(
            Some(mail.from.parse().map_err(|_| bad_envelope())?),
            vec![mail.to.parse().map_err(|_| bad_envelope())?],
        )
        .map_err(|_| bad_envelope())?;
        let mut connection = self.connection(profile).await?;
        let mut parameters = Vec::new();
        if !mail.from.is_ascii() || !mail.to.is_ascii() {
            if !connection
                .server_info()
                .supports_feature(Extension::SmtpUtfEight)
            {
                return Err(bad_envelope());
            }
            parameters.push(MailParameter::SmtpUtfEight);
        }
        if !mail.bytes.is_ascii() {
            if !connection
                .server_info()
                .supports_feature(Extension::EightBitMime)
            {
                return Err(DeliveryFailure::new(
                    FailureKind::Permanent,
                    Stage::Content,
                    None,
                ));
            }
            parameters.push(MailParameter::Body(MailBodyParameter::EightBitMime));
        }
        let response = timeout(
            Duration::from_secs(300),
            connection.command(Mail::new(envelope.from().cloned(), parameters)),
        )
        .await
        .map_err(|_| timed_out(Stage::Envelope))?
        .map_err(|e| failure(e, Stage::Envelope))?;
        if !response.code().to_string().starts_with('2') {
            return Err(bad_envelope());
        }
        for recipient in envelope.to() {
            let response = timeout(
                Duration::from_secs(300),
                connection.command(Rcpt::new(recipient.clone(), vec![])),
            )
            .await
            .map_err(|_| timed_out(Stage::Envelope))?
            .map_err(|e| failure(e, Stage::Envelope))?;
            if !response.code().to_string().starts_with('2') {
                return Err(bad_envelope());
            }
        }
        let response = timeout(Duration::from_secs(120), connection.command(Data))
            .await
            .map_err(|_| timed_out(Stage::Data))?
            .map_err(|e| failure(e, Stage::Data))?;
        if response.code().to_string() != "354" {
            return Err(DeliveryFailure::new(
                FailureKind::Permanent,
                Stage::Data,
                None,
            ));
        }
        Ok(Box::new(Pending {
            connection,
            mail,
            _slot: slot,
        }))
    }
}
