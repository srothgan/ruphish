//! MIME composition and one-shot SMTP submission with checked destination sockets.
use crate::{
    outbound::Policy,
    resources::{attachment, invalid, mailbox, render, smtp_address},
    template::Context,
};
use lettre::{
    Message,
    address::Envelope,
    message::{
        Attachment as MailAttachment, MultiPart, SinglePart,
        header::{ContentType, HeaderName, HeaderValue},
    },
    transport::smtp::{
        authentication::{Credentials, Mechanism},
        client::{AsyncSmtpConnection, TlsParameters},
        extension::ClientId,
    },
};
use ruphish_application::{
    delivery::{PreparedMail, Sender, TestMessage},
    resources::*,
};
use std::time::Duration;

/// Bounded delivery worker; credentials and server diagnostics never enter public errors.
pub struct Smtp {
    /// Destination policy shared by checked outbound connections.
    pub policy: Policy,
    /// Configured transparency contact header.
    pub contact: String,
    slots: tokio::sync::Semaphore,
}
impl Smtp {
    /// Compose the concrete dependencies for this boundary.
    pub fn new(policy: Policy, contact: String) -> Self {
        Self {
            policy,
            contact,
            slots: tokio::sync::Semaphore::new(4),
        }
    }
    async fn connection(&self, profile: &Profile) -> Result<AsyncSmtpConnection, Error> {
        let (host, port) = smtp_address(&profile.host)?;
        let addresses = self.policy.resolve(&host, port).await?;
        let tls = TlsParameters::builder(host)
            .dangerous_accept_invalid_certs(profile.ignore_cert_errors)
            .build()
            .map_err(|_| invalid("Cannot configure SMTP TLS"))?;
        let hello = ClientId::Domain("ruphish.local".into());
        let mut connection = AsyncSmtpConnection::connect_tokio1(
            addresses.as_slice(),
            Some(Duration::from_secs(10)),
            &hello,
            if port == 465 { Some(tls.clone()) } else { None },
            None,
        )
        .await
        .map_err(|_| invalid("SMTP connection failed; check host, port and TLS certificate"))?;
        if connection.can_starttls() {
            connection
                .starttls(tls, &hello)
                .await
                .map_err(|_| invalid("SMTP TLS negotiation failed"))?;
        }
        if !profile.username.is_empty() {
            if !connection.is_encrypted() && !addresses.iter().all(|a| a.ip().is_loopback()) {
                return Err(invalid(
                    "SMTP authentication requires TLS except on loopback",
                ));
            }
            connection
                .auth(
                    &[Mechanism::Plain, Mechanism::Login],
                    &Credentials::new(profile.username.clone(), profile.password.clone()),
                )
                .await
                .map_err(|_| invalid("SMTP authentication failed"))?;
        }
        Ok(connection)
    }
}
/// Derive recipient variables and preview links with the same sender distinction as Go.
pub fn context(message: &TestMessage) -> Result<Context, Error> {
    let visible = if message.template.envelope_sender.is_empty() {
        &message.profile.from_address
    } else {
        &message.template.envelope_sender
    };
    let sender = mailbox(visible)?;
    let mut ctx = Context {
        from: sender.name.unwrap_or_else(|| sender.email.to_string()),
        email: message.recipient.email.clone(),
        first_name: message.recipient.first_name.clone(),
        last_name: message.recipient.last_name.clone(),
        position: message.recipient.position.clone(),
        rid: message.rid.clone(),
        ..Context::default()
    };
    let address = render(&message.url, &ctx)?;
    if address.is_empty() {
        ctx.url = format!("?rid={}", message.rid);
        ctx.tracking_url = format!("/track?rid={}", message.rid);
    } else {
        let mut url = url::Url::parse(&address).map_err(|_| invalid("Invalid test URL"))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(invalid("Test URL must use HTTP or HTTPS"));
        }
        let mut pairs: Vec<(String, String)> = url
            .query_pairs()
            .filter(|(k, _)| k != "rid")
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        pairs.push(("rid".into(), message.rid.clone()));
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        url.set_query(None);
        url.query_pairs_mut().extend_pairs(pairs);
        let pathless = address
            .split_once("://")
            .is_some_and(|(_, tail)| !tail.split(['?', '#']).next().unwrap_or("").contains('/'));
        ctx.url = if pathless {
            format!(
                "{}{}",
                &url[..url::Position::BeforePath],
                &url[url::Position::AfterPath..]
            )
        } else {
            url.to_string()
        };
        let mut tracking = url.clone();
        tracking.set_path(&format!("{}/track", url.path().trim_end_matches('/')));
        ctx.tracking_url = tracking.to_string();
        url.set_path("");
        url.set_query(None);
        ctx.base_url = format!(
            "{}{}",
            &url[..url::Position::BeforePath],
            &url[url::Position::AfterPath..]
        );
    }
    ctx.tracker = format!(
        "<img alt='' style='display: none' src='{}'/>",
        ctx.tracking_url
    );
    Ok(ctx)
}
#[async_trait::async_trait]
impl Sender for Smtp {
    fn compose(&self, request: &TestMessage) -> Result<PreparedMail, Error> {
        let ctx = context(request)?;
        let from = mailbox(&request.profile.from_address)?;
        let mut to = mailbox(&request.recipient.email)?;
        if !request.recipient.first_name.is_empty() && !request.recipient.last_name.is_empty() {
            to.name = Some(format!(
                "{} {}",
                request.recipient.first_name, request.recipient.last_name
            ));
        }
        let visible = mailbox(if request.template.envelope_sender.is_empty() {
            &request.profile.from_address
        } else {
            &request.template.envelope_sender
        })?;
        let mut builder =
            Message::builder()
                .from(visible)
                .to(to.clone())
                .raw_header(HeaderValue::new(
                    HeaderName::new_from_ascii_str("X-Mailer"),
                    "gophish".into(),
                ));
        if !self.contact.is_empty() {
            if self.contact.contains(['\r', '\n']) {
                return Err(invalid("Invalid contact address"));
            }
            builder = builder.raw_header(HeaderValue::new(
                HeaderName::new_from_ascii_str("X-Gophish-Contact"),
                self.contact.clone(),
            ));
        }
        for header in &request.profile.headers {
            let key = render(&header.key, &ctx)?;
            // Go sets the recipient header after custom headers; the envelope is independent.
            if key.eq_ignore_ascii_case("To") {
                continue;
            }
            let value = render(&header.value, &ctx)?;
            if value.contains(['\r', '\n', '\0']) {
                return Err(invalid("Invalid rendered header"));
            }
            let name = HeaderName::new_from_ascii(key)
                .map_err(|_| invalid("Invalid rendered header name"))?;
            builder = builder.raw_header(HeaderValue::new(name, value));
        }
        let subject = render(&request.template.subject, &ctx)?;
        if subject.contains(['\r', '\n', '\0']) {
            return Err(invalid("Invalid rendered subject"));
        }
        if !subject.is_empty() {
            builder = builder.subject(subject);
        }
        let text = render(&request.template.text, &ctx)?;
        let html = render(&request.template.html, &ctx)?;
        let mut body = MultiPart::mixed().build();
        if !text.is_empty() && !html.is_empty() {
            body = body.multipart(
                MultiPart::alternative()
                    .singlepart(SinglePart::plain(text))
                    .singlepart(SinglePart::html(html)),
            );
        } else if !html.is_empty() {
            body = body.singlepart(SinglePart::html(html));
        } else {
            body = body.singlepart(SinglePart::plain(text));
        }
        let mut attachment_total = 0usize;
        for a in &request.template.attachments {
            let bytes = attachment(a, &ctx)?;
            attachment_total = attachment_total.saturating_add(bytes.len());
            if attachment_total > crate::resources::MAX_ARCHIVE_EXPANDED {
                return Err(invalid("Rendered attachments exceed 64 MiB"));
            }
            let mime = if a.mime_type.is_empty() {
                "application/octet-stream"
            } else {
                &a.mime_type
            }
            .parse::<ContentType>()
            .map_err(|_| invalid("Invalid attachment MIME type"))?;
            let part = if matches!(
                a.name.rsplit('.').next().unwrap_or(""),
                "png" | "jpg" | "jpeg" | "gif" | "bmp"
            ) {
                MailAttachment::new_inline(a.name.clone()).body(bytes, mime)
            } else {
                MailAttachment::new(a.name.clone()).body(bytes, mime)
            };
            body = body.singlepart(part);
        }
        let message = builder
            .multipart(body)
            .map_err(|_| invalid("Cannot compose test email"))?;
        Ok(PreparedMail {
            from: from.email.to_string(),
            to: to.email.to_string(),
            bytes: message.formatted(),
        })
    }
    async fn connect(&self, profile: &Profile) -> Result<(), Error> {
        let _slot = self
            .slots
            .try_acquire()
            .map_err(|_| invalid("Test-email service is busy; try again shortly"))?;
        tokio::time::timeout(Duration::from_secs(30), async {
            let mut c = self.connection(profile).await?;
            if !c.test_connected().await {
                return Err(invalid("SMTP connection check failed"));
            }
            let _ = c.quit().await;
            Ok(())
        })
        .await
        .map_err(|_| invalid("SMTP connection check timed out"))?
    }
    async fn send(&self, profile: &Profile, mail: PreparedMail) -> Result<(), Error> {
        let _slot = self
            .slots
            .try_acquire()
            .map_err(|_| invalid("Test-email service is busy; try again shortly"))?;
        let mut c = tokio::time::timeout(Duration::from_secs(30), self.connection(profile))
            .await
            .map_err(|_| invalid("SMTP connection timed out before submission"))??;
        let envelope = Envelope::new(
            Some(
                mail.from
                    .parse()
                    .map_err(|_| invalid("Invalid envelope sender"))?,
            ),
            vec![mail.to.parse().map_err(|_| invalid("Invalid recipient"))?],
        )
        .map_err(|_| invalid("Invalid SMTP envelope"))?;
        let result =
            tokio::time::timeout(Duration::from_secs(30), c.send(&envelope, &mail.bytes)).await;
        match result {
            Ok(Ok(_)) => {}
            Ok(Err(e)) if e.is_permanent() || e.is_transient() => {
                return Err(invalid("SMTP server rejected the test email"));
            }
            _ => {
                return Err(invalid(
                    "Delivery outcome is unknown after connection loss or timeout. Check the mail server before sending again.",
                ));
            }
        }
        // QUIT failure cannot undo a confirmed DATA acceptance.
        let _ = tokio::time::timeout(Duration::from_secs(2), c.quit()).await;
        Ok(())
    }
}
